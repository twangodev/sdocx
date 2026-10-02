use super::super::widget_text_constructor;
use super::cell_host::{install_host, reset_host};
use super::*;

#[path = "text_cell_emission.rs"]
mod emission;
#[path = "cell_source_inputs.rs"]
mod source_inputs;

pub(crate) use super::super::widget_text_constructor::Paths;

pub(crate) fn capture(machine: &mut Machine, paths: Paths<'_>) {
    capture_mode(machine, paths, CaptureMode::Measurement);
}

pub(crate) fn capture_emission(machine: &mut Machine, paths: Paths<'_>) {
    capture_mode(machine, paths, CaptureMode::CachedEmission);
}

pub(crate) fn capture_source_inputs(machine: &mut Machine, paths: Paths<'_>) {
    capture_mode(machine, paths, CaptureMode::SourceInputs);
}

#[derive(Clone, Copy)]
enum CaptureMode {
    Measurement,
    CachedEmission,
    SourceInputs,
}

type SourceObserver = fn(&Machine, u64) -> String;

impl CaptureMode {
    fn controls(self) -> Vec<Case> {
        match self {
            Self::Measurement => cases(),
            Self::CachedEmission => {
                let mut controls = cases();
                controls.extend(emission::newline_cases());
                controls
            }
            Self::SourceInputs => source_inputs::cases(),
        }
    }

    fn captures_emission_input(self) -> bool {
        !matches!(self, Self::Measurement)
    }

    fn source_observer(self) -> Option<SourceObserver> {
        matches!(self, Self::SourceInputs).then_some(source_inputs::snapshot)
    }

    fn memory_fills(self) -> &'static [u8] {
        match self {
            Self::SourceInputs => &[0, 85, 165, 255, 0],
            Self::Measurement | Self::CachedEmission => &[0, 165, 255, 0],
        }
    }

    fn memory_fill_header(self) -> &'static str {
        match self {
            Self::SourceInputs => "[0,85,165,255]",
            Self::Measurement | Self::CachedEmission => "[0,165,255]",
        }
    }

    fn header(self) -> String {
        const CACHED_INPUT: &str = "\"cached_emission_input_capture\":true,\"span_getter\":\"0x61f3c\",\"paragraph_index_getter\":\"0x62344\",\"gravity_state_offset\":212,\"paragraph_flag_offset\":65,";
        match self {
            Self::Measurement => String::new(),
            Self::CachedEmission => CACHED_INPUT.into(),
            Self::SourceInputs => format!(
                "\"actual_source_input_capture\":true,\"source_model_getter_observation_after_cached_emission\":true,{CACHED_INPUT}"
            ),
        }
    }
}

fn capture_mode(machine: &mut Machine, paths: Paths<'_>, mode: CaptureMode) {
    use widget_text_constructor::{CELL_DRAWING, CELL_LAYOUT, CONTENT, CONTENT_SHA256, WIDGET};
    let (mut environment, fonts) = setup_with_preloaded_libraries(
        machine,
        paths.base,
        paths.text,
        paths.skia,
        paths.font,
        paths.xml,
        paths.cpp,
        &[
            (paths.widget, WIDGET, geometry::WIDGET_SHA256),
            (paths.content, CONTENT, CONTENT_SHA256),
            (paths.drawing, CELL_DRAWING, DRAWING_SHA256),
        ],
        &[(paths.model, 0, LIBRARY_SHA256)],
    );
    let mut host = install_host(machine, &mut environment, fonts);
    let mut recorder = Recorder::new(machine);
    let mut outputs = Vec::new();
    for case in mode.controls() {
        let mut canonical = None;
        for &fill in mode.memory_fills() {
            reset_host(machine, &mut environment, &mut host, fill);
            *recorder.trace = Trace::default();
            let prepared = prepare_cell(machine, case);
            let (wrapper, rich) = (prepared.wrapper, prepared.rich);
            recorder.trace.rich = rich;
            let implementation = read_u64(machine.engine, rich);
            let flags = bytes(machine.engine, implementation + 112, 4);
            assert_eq!(flags[1..3], [0, 1]);
            let before_measure = snapshot(machine, rich);
            register(machine.engine, 136, u64::from(case.width.to_bits()));
            machine.call(WIDGET + 0xd398c, &[CELL_LAYOUT]);
            register(machine.engine, 136, u64::from(1000_f32.to_bits()));
            machine.call(WIDGET + 0xd399c, &[CELL_LAYOUT]);
            machine.call(
                WIDGET + 0xd73b0,
                &[CELL_LAYOUT, 0, u32::MAX as u64, u32::MAX as u64],
            );
            let measured = snapshot(machine, rich);
            let measured_bounds = text_bounds(machine, wrapper, rich);
            machine.call(
                CELL_DRAWING + 0x8c05c,
                &[CELL_LAYOUT, 0, u32::MAX as u64, u32::MAX as u64],
            );
            let metadata = mode
                .captures_emission_input()
                .then(|| emission::Input::capture(machine, rich, &recorder.trace.paragraphs));
            let placed = metadata.as_ref().map_or_else(
                || snapshot(machine, rich),
                |input| snapshot_with_metadata(machine, rich, &input.entries),
            );
            let placed_bounds = text_bounds(machine, wrapper, rich);
            let emitted = emit(machine, rich);
            let source_input = mode
                .source_observer()
                .map(|observe| observe(machine, prepared.source_object));
            assert!(machine.heap.cursor < MODEL + 0x50000);
            let trace = &recorder.trace;
            let observed_text =
                read_spen_string(machine.engine, machine.call(TEXT + 0x8b970, &[wrapper]));
            assert_eq!(observed_text.as_deref().unwrap_or(""), case.text);
            let mut output = format!(
                "{{\"name\":{},\"text_utf8\":{},\"native_text_utf8\":{},\"source_utf16\":{:?},\"model_shape_type\":4,\"requested_font_size_bits\":{},\"requested_width_bits\":{},\"caller_layout_direction\":{},\"supplied_margins_bits\":{},\"native_flags\":{flags:?},\"before_measure\":{before_measure},\"constructor_before_first_span\":{},\"measured_entries\":{measured},\"placed_entries\":{placed},\"measured_text_bounds_bits\":{measured_bounds:?},\"placed_text_bounds_bits\":{placed_bounds:?},\"emitted_runs\":{emitted},\"measure_widths\":{:?},\"layout_arguments\":{:?},\"do_paragraph_w2_w3\":{:?},\"effective_widths\":{:?},\"paragraph_calculate_laytextout_blockinfo_setlayout_calls\":{:?},\"icu_calls\":{:?}}}",
                json_string(case.name),
                json_string(case.text),
                optional_json(observed_text.as_deref()),
                case.text.encode_utf16().collect::<Vec<_>>(),
                case.font_size
                    .map_or_else(|| "null".into(), |s| s.to_bits().to_string()),
                case.width.to_bits(),
                case.direction,
                case.margins
                    .map_or_else(|| "null".into(), |m| format!("{:?}", m.map(f32::to_bits))),
                trace.before_first_span.as_deref().unwrap_or("null"),
                trace.measure_widths,
                trace.layout_arguments,
                trace.paragraph_arguments,
                trace.effective_widths,
                trace.calls,
                host.paragraphs.calls
            );
            if let Some(input) = source_input {
                append_object_field(&mut output, "source_input", &input);
            }
            if let Some(input) = metadata {
                append_object_field(&mut output, "emission_input", &input.context);
            }
            if let Some(expected) = &canonical {
                assert_eq!(&output, expected, "case {} fill {fill}", case.name);
            } else {
                canonical = Some(output);
            }
        }
        outputs.push(canonical.unwrap());
    }
    println!(
        "{{{}\"dependencies\":{},\"memory_fills\":{},\"repeat_zero_fill\":true,\"model_library_sha256\":{},\"base_library_sha256\":{},\"text_library_sha256\":{},\"skia_library_sha256\":{},\"widget_library_sha256\":{},\"content_library_sha256\":{},\"drawing_library_sha256\":{},\"font_config_sha256\":{},\"capture_boundary\":{},\"cases\":[{}]}}",
        mode.header(),
        dependency_metadata(),
        mode.memory_fill_header(),
        json_string(LIBRARY_SHA256),
        json_string(frames::BASE_SHA256),
        json_string(geometry::TEXT_SHA256),
        json_string(text_font_source::SKIA_SHA256),
        json_string(geometry::WIDGET_SHA256),
        json_string(CONTENT_SHA256),
        json_string(DRAWING_SHA256),
        json_string("9864ad4db5012ad4b63f82fcd0375b4f6a02a92d762147805ebdc02de0f4ed22"),
        json_string(
            "Actual Model TableCellContentObject constructor and ComponentText.SetText/optional font-size setter execute; actual Drawing cell/Widget/Content/Text constructors, SetObject, Widget Update including native span/paragraph conversion, actual measureImpl, NAME font selection, bundled Minikin/HarfBuzz/Skia/FreeType, Drawing cell Layout and actual native paragraph calculation/placement execute. No measured entries, character classifications, glyph records, native widths or font getters are supplied. Real measured caches then feed complete native getDrawnTextRun; emitted run fields and TextLayout.GetTextBound are observed. Caller device interfaces use explicit default density/direction and null text manager. Caller IContext color service returns its input ARGB unchanged; UUID service supplies deterministic opaque IDs. Android ICU loader is substituted with pinned host ICU76.1/Unicode16 data for bidi/break/property/locale APIs; bundled HB Unicode tables remain native. Host allocation/file/libc/single-thread services follow NAME/shaping captures. TLS storage is relocated away from deep native stack without changing native instructions. This capture does not execute complete TableLayout allocation, table rectangle producers, document pagination, source parsing/cloning, device ICU, final SVG/PDF writer consumption, pixels or arbitrary system font configuration."
        ),
        outputs.join(",")
    );
}

fn append_object_field(object: &mut String, key: &str, value: &str) {
    assert_eq!(object.pop(), Some('}'));
    object.push_str(&format!(",{}:{value}}}", json_string(key)));
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    text: &'static str,
    width: f32,
    font_size: Option<f32>,
    direction: u32,
    margins: Option<[f32; 4]>,
}
fn cases() -> Vec<Case> {
    let mut cases: Vec<_> = [
        ("default-auto", "AV abc", 0.0, None, 0),
        ("default-narrow", "AV abc", 40.0, None, 0),
        ("default-wide", "AV abc", 400.0, None, 0),
        ("subpixel-auto", "AV", 0.75, Some(17.0), 0),
        ("fractional-width", "AV abc", 80.9, Some(17.0), 0),
        ("combining-auto", "A\u{301}B", 0.0, Some(17.0), 0),
        ("supplementary-wrap", "A😀e\u{301}B", 25.0, Some(17.0), 0),
        ("tab-newline", "AV\tabc\nTo", 60.0, Some(17.0), 0),
        ("mixed-hebrew-direction1", "AV אב", 90.0, Some(17.0), 1),
        ("trailing-newline", "AV\n", 80.0, Some(17.0), 0),
        ("empty-auto", "", 0.0, None, 0),
    ]
    .into_iter()
    .map(|(name, text, width, font_size, direction)| Case {
        name,
        text,
        width,
        font_size,
        direction,
        margins: None,
    })
    .collect();
    cases.push(Case {
        name: "multi-paragraph-auto",
        text: "AV\nToTo",
        width: 0.0,
        font_size: Some(17.0),
        direction: 0,
        margins: None,
    });
    cases.push(Case {
        name: "margin-auto",
        text: "AV abc",
        width: 0.0,
        font_size: Some(17.0),
        direction: 0,
        margins: Some([1.25, 2.5, 3.75, 4.5]),
    });
    cases.push(Case {
        name: "zero-advance-cross-line",
        text: "\u{200b}\n\u{200b}",
        width: 0.0,
        font_size: Some(17.0),
        direction: 0,
        margins: None,
    });
    cases
}
struct PreparedCell {
    source_object: u64,
    wrapper: u64,
    rich: u64,
}

fn prepare_cell(machine: &Machine, case: Case) -> PreparedCell {
    use widget_text_constructor::{CELL_LAYOUT, WIDGET};
    let object = MODEL + 0x71000;
    write(machine.engine, object, &[0; 240]);
    machine.call(0x3c17f0, &[object, 0, 0]);
    assert_eq!(machine.call(0x396e14, &[object]), 4);
    let string = spen_string(machine, MODEL + 0x72000, Some(case.text));
    assert_eq!(machine.call(0x39c9d4, &[object + 40, string]) & 1, 1);
    if let Some(size) = case.font_size {
        register(machine.engine, 136, u64::from(size.to_bits()));
        machine.call(0x3b06fc, &[read_u64(machine.engine, object + 56)]);
    }
    if let Some(margins) = case.margins {
        for (index, value) in margins.into_iter().enumerate() {
            register(
                machine.engine,
                136 + index as i32,
                u64::from(value.to_bits()),
            );
        }
        machine.call(0x39e678, &[object + 40]);
    }
    widget_text_constructor::construct_cell(
        machine,
        CELL_LAYOUT,
        widget_text_constructor::DeviceProfile {
            layout_direction: case.direction,
            ..Default::default()
        },
    );
    write(
        machine.engine,
        MODEL + 0x60500,
        &0x2a0103e0_u32.to_le_bytes(),
    );
    write(
        machine.engine,
        MODEL + 0x60504,
        &0xd65f03c0_u32.to_le_bytes(),
    );
    write(
        machine.engine,
        MODEL + 0x50000 + 8 + 80,
        &(MODEL + 0x60500).to_le_bytes(),
    );
    machine.call(WIDGET + 0xd3974, &[CELL_LAYOUT, object]);
    machine.call(WIDGET + 0xd3e3c, &[CELL_LAYOUT]);
    let wrapper = widget_text_constructor::text_wrapper(machine, CELL_LAYOUT);
    PreparedCell {
        source_object: object,
        wrapper,
        rich: read_u64(machine.engine, wrapper + 64),
    }
}

#[derive(Default)]
struct Trace {
    rich: u64,
    before_first_span: Option<String>,
    measure_widths: Vec<i32>,
    layout_arguments: Vec<[i32; 5]>,
    paragraph_arguments: Vec<[i32; 2]>,
    effective_widths: Vec<i32>,
    calls: [u32; 4],
    paragraphs: Vec<u64>,
}

unsafe extern "C" fn trace_native(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let trace = unsafe { &mut *data.cast::<Trace>() };
    match address - TEXT {
        0x7710c => {
            if trace.before_first_span.is_none() {
                trace.before_first_span = Some(slots(engine, trace.rich, &BTreeMap::new()));
            }
        }
        0x8aadc => trace
            .measure_widths
            .push(read_register(engine, REGISTER_X0 + 1) as i32),
        0x8ac10 => trace.layout_arguments.push(std::array::from_fn(|i| {
            read_register(engine, REGISTER_X0 + 1 + i as i32) as i32
        })),
        0x7278c => {
            trace
                .paragraph_arguments
                .push([2, 3].map(|i| read_register(engine, REGISTER_X0 + i) as i32));
            trace
                .paragraphs
                .push(read_register(engine, REGISTER_X0 + 1));
        }
        0x73cd0 => {
            trace.calls[0] += 1;
            trace
                .effective_widths
                .push(read_register(engine, REGISTER_X0 + 3) as i32);
        }
        0x6a5c0 => trace.calls[1] += 1,
        0x6ab9c => trace.calls[2] += 1,
        0x6b4a4 => trace.calls[3] += 1,
        _ => unreachable!(),
    }
}
struct Recorder {
    trace: Box<Trace>,
    engine: Engine,
    hooks: Vec<usize>,
}
impl Recorder {
    fn new(machine: &Machine) -> Self {
        let mut recorder = Self {
            trace: Box::default(),
            engine: machine.engine,
            hooks: Vec::new(),
        };
        for offset in [
            0x7710c, 0x8aadc, 0x8ac10, 0x7278c, 0x73cd0, 0x6a5c0, 0x6ab9c, 0x6b4a4,
        ] {
            let address = TEXT + offset;
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    trace_native as *mut c_void,
                    ptr::from_mut(recorder.trace.as_mut()).cast(),
                    address,
                    address,
                )
            });
            recorder.hooks.push(hook);
        }
        recorder
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        for hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, *hook) });
        }
    }
}

fn slots(engine: Engine, rich: u64, fonts: &BTreeMap<u64, String>) -> String {
    slots_with_metadata(engine, rich, fonts, None)
}

fn slots_with_metadata(
    engine: Engine,
    rich: u64,
    fonts: &BTreeMap<u64, String>,
    metadata: Option<&[String]>,
) -> String {
    let count = read_u32(engine, rich + 104) as usize;
    assert!(count <= 64);
    let entries = read_u64(engine, rich + 112);
    let caches = read_u64(engine, rich + 136);
    let mut result = Vec::new();
    for index in 0..count {
        let entry = entries + index as u64 * 80;
        let cache = caches + index as u64 * 40;
        let begin = read_u64(engine, cache);
        let end = read_u64(engine, cache + 8);
        assert!(end >= begin && (end - begin) % 12 == 0 && end - begin <= 768);
        let glyphs: Vec<[u32; 3]> = (begin..end)
            .step_by(12)
            .map(|p| [0, 4, 8].map(|o| read_u32(engine, p + o)))
            .collect();
        let font = read_u64(engine, cache + 24);
        let font = if font == 0 {
            "null"
        } else {
            fonts.get(&font).map(String::as_str).unwrap_or("null")
        };
        result.push(format!("{{\"utf16_slot\":{index},\"advance_bits\":{},\"font_height_bits\":{},\"position_bits\":{:?},\"layout_rect_bits\":{:?},\"ink_rect_bits\":{:?},\"kind\":{},\"raw_direction\":{},\"break_index\":{},\"font_size_bits\":{},\"placement_mode\":{},\"inline_overpages\":{},\"object_margin_bits\":{:?},\"glyph_records\":{glyphs:?},\"font\":{font},\"fake_bold\":{},\"fake_italic\":{},\"drawable\":{}}}",
            read_u32(engine,entry),read_u32(engine,entry+4),[8,12].map(|o|read_u32(engine,entry+o)),[16,20,24,28].map(|o|read_u32(engine,entry+o)),[32,36,40,44].map(|o|read_u32(engine,entry+o)),read_u32(engine,entry+48),read_u32(engine,entry+52),read_u32(engine,entry+56) as i32,read_u32(engine,entry+60),read_u32(engine,entry+64),bytes(engine,entry+68,1)[0]!=0,[72,76].map(|o|read_u32(engine,entry+o)),bytes(engine,cache+32,1)[0]!=0,bytes(engine,cache+33,1)[0]!=0,bytes(engine,cache+34,1)[0]!=0));
    }
    if let Some(metadata) = metadata {
        assert_eq!(metadata.len(), result.len());
        for (entry, fields) in result.iter_mut().zip(metadata) {
            assert_eq!(entry.pop(), Some('}'));
            entry.push_str(fields);
            entry.push('}');
        }
    }
    format!("[{}]", result.join(","))
}

pub(super) fn snapshot(machine: &Machine, rich: u64) -> String {
    snapshot_metadata(machine, rich, None)
}

fn snapshot_with_metadata(machine: &Machine, rich: u64, metadata: &[String]) -> String {
    snapshot_metadata(machine, rich, Some(metadata))
}

fn snapshot_metadata(machine: &Machine, rich: u64, metadata: Option<&[String]>) -> String {
    let count = read_u32(machine.engine, rich + 104) as usize;
    assert!(count <= 64);
    let caches = read_u64(machine.engine, rich + 136);
    let mut fonts = BTreeMap::new();
    for index in 0..count {
        let font = read_u64(machine.engine, caches + index as u64 * 40 + 24);
        if font != 0 && !fonts.contains_key(&font) {
            let id = machine.call(TEXT + 0x85d7c, &[font]);
            let bitmap = machine.call(TEXT + 0x85d98, &[font]) != 0;
            let language = cxx_string(machine.engine, machine.call(TEXT + 0x85db0, &[font]));
            fonts.insert(
                font,
                format!(
                    "{{\"source_id\":{id},\"bitmap\":{bitmap},\"language\":{}}}",
                    json_string(&language)
                ),
            );
        }
    }
    slots_with_metadata(machine.engine, rich, &fonts, metadata)
}

pub(super) fn text_bounds(machine: &Machine, wrapper: u64, rich: u64) -> Vec<[u32; 4]> {
    let count = read_u32(machine.engine, rich + 104) as i32;
    assert!((0..=64).contains(&count));
    (-1..=count)
        .map(|index| {
            machine.call(TEXT + 0x8afd4, &[wrapper, index as u32 as u64]);
            std::array::from_fn(|i| read_register(machine.engine, 136 + i as i32) as u32)
        })
        .collect()
}

fn words(engine: Engine, address: u64) -> Vec<u32> {
    let begin = read_u64(engine, address);
    let end = read_u64(engine, address + 8);
    assert!(end >= begin && (end - begin) % 4 == 0 && end - begin <= 1024);
    (begin..end)
        .step_by(4)
        .map(|p| read_u32(engine, p))
        .collect()
}
pub(super) fn emit(machine: &Machine, rich: u64) -> String {
    let count = read_u32(machine.engine, rich + 104);
    let output = MODEL + 0x73000;
    write(machine.engine, output, &[0; 24]);
    register(machine.engine, 136, 0);
    register(machine.engine, 137, 0);
    let accepted = machine.call(
        TEXT + 0x66c98,
        &[rich + 16, 0, count.wrapping_sub(1) as u64, output, 0],
    ) & 1
        != 0;
    let begin = read_u64(machine.engine, output);
    let end = read_u64(machine.engine, output + 8);
    assert!(end >= begin && (end - begin) % 8 == 0 && end - begin <= 512);
    let result:Vec<_>=(begin..end).step_by(8).map(|p|{
        let run=read_u64(machine.engine,p);
        let codes=words(machine.engine,run+8);let positions=words(machine.engine,run+32);
        assert_eq!(codes.len(),positions.len());
        format!("{{\"range_inclusive\":{:?},\"codewords\":{codes:?},\"position_bits\":{positions:?},\"origin_bits\":{:?},\"ink_rect_bits\":{:?},\"layout_rect_bits\":{:?},\"is_object\":{},\"source_id\":{},\"font_size_bits\":{},\"foreground\":{},\"style_byte\":{},\"background\":{}}}",
            [0,4].map(|o|read_u32(machine.engine,run+o)),[80,84].map(|o|read_u32(machine.engine,run+o)),[88,92,96,100].map(|o|read_u32(machine.engine,run+o)),[104,108,112,116].map(|o|read_u32(machine.engine,run+o)),bytes(machine.engine,run+120,1)[0]!=0,read_u32(machine.engine,run+124),read_u32(machine.engine,run+132),read_u32(machine.engine,run+136),bytes(machine.engine,run+140,1)[0],read_u32(machine.engine,run+144))
    }).collect();
    format!(
        "{{\"accepted\":{accepted},\"runs\":[{}]}}",
        result.join(",")
    )
}
