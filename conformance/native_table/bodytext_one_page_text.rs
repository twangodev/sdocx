use super::*;

const SOURCE_VECTOR: u64 = STORAGE + 0x800;
const CALLS: [(&str, u64); 14] = [
    ("model_document_constructor", 0x3e1378),
    ("model_document_construct", 0x3e1408),
    ("widget_constructor", WIDGET + 0xd2fb4),
    ("widget_update", WIDGET + 0xd3e3c),
    ("widget_measure", WIDGET + 0xd73b0),
    ("widget_layout", WIDGET + 0xd3b88),
    ("body_layout_constructor", BODY + 0xaf948),
    ("body_document_constructor", BODY + 0xa8c3c),
    ("body_document_max_width", BODY + 0xa98c8),
    ("body_document_total_height", BODY + 0xa98e0),
    ("body_measure_text", BODY + 0xb2e54),
    ("text_bound", TEXT + 0x8afd4),
    ("line_count", TEXT + 0x8adb0),
    ("cached_run_emission", TEXT + 0x66c98),
];

struct Recorder {
    engine: Engine,
    hooks: Vec<usize>,
    calls: Box<NativeCalls>,
}

impl Recorder {
    fn new(machine: &Machine) -> Self {
        let mut result = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            calls: Box::default(),
        };
        for (_, address) in CALLS {
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    native_call as *mut c_void,
                    ptr::from_mut(result.calls.as_mut()).cast(),
                    address,
                    address,
                )
            });
            result.hooks.push(hook);
        }
        result
    }

    fn snapshot(&self) -> String {
        let calls = CALLS
            .iter()
            .filter_map(|(name, address)| {
                self.calls
                    .counts
                    .get(address)
                    .map(|count| format!("{}:{count}", json_string(name)))
            })
            .collect::<Vec<_>>();
        format!("{{{}}}", calls.join(","))
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        for &hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, hook) });
        }
    }
}

fn style_spans(machine: &Machine, common: u64, kind: u64, mask: u64) -> String {
    if common == 0 {
        return "[]".into();
    }
    write(machine.engine, SOURCE_VECTOR, &[0; 24]);
    machine.call(0x3e32f8, &[common, mask, SOURCE_VECTOR]);
    let begin = read_u64(machine.engine, SOURCE_VECTOR);
    let end = read_u64(machine.engine, SOURCE_VECTOR + 8);
    let capacity = read_u64(machine.engine, SOURCE_VECTOR + 16);
    assert!(end >= begin && capacity >= end && (end - begin).is_multiple_of(8));
    assert!(end - begin <= 2048);
    let spans = (begin..end)
        .step_by(8)
        .map(|pointer| {
            let span = read_u64(machine.engine, pointer);
            assert_eq!(machine.call(0x40c698, &[span]), kind);
            let start = machine.call(0x40c754, &[span]) as u32 as i32;
            let end = machine.call(0x40c810, &[span]) as u32 as i32;
            let interval = machine.call(0x40c8cc, &[span]);
            let property = match kind {
                3 => format!(
                    "\"font_size_bits\":{}",
                    float_getter(machine, 0x409cf8, span)
                ),
                1 => format!(
                    "\"color_argb\":{},\"color_type\":{}",
                    machine.call(0x40a42c, &[span]),
                    machine.call(0x40a484, &[span])
                ),
                _ => unreachable!(),
            };
            format!(
                "{{\"kind\":{kind},\"utf16_range\":[{start},{end}],\"interval\":{interval},{property}}}"
            )
        })
        .collect::<Vec<_>>();
    if begin != 0 {
        machine.call(0x47ac00, &[begin]);
    }
    write(machine.engine, SOURCE_VECTOR, &[0; 24]);
    format!("[{}]", spans.join(","))
}

pub(super) fn source(machine: &Machine, length: u64) -> String {
    let component = DOC + 40;
    let common = read_u64(machine.engine, read_u64(machine.engine, DOC + 56) + 8);
    let text = read_spen_string(machine.engine, machine.call(0x39c9e8, &[component])).unwrap();
    let fonts = (0..=length)
        .map(|index| {
            machine.call(0x39f504, &[component, index]);
            read_register(machine.engine, 136) as u32
        })
        .collect::<Vec<_>>();
    let margins = [0x39e680, 0x39e6c4, 0x39e708, 0x39e74c]
        .map(|getter| float_getter(machine, getter, component));
    format!(
        "{{\"text_utf8\":{},\"common_present\":{},\"font_size_at_utf16_including_end_bits\":{fonts:?},\"margin_bits\":{margins:?},\"font_size_spans\":{},\"foreground_spans\":{},\"observed_after_measurement\":true}}",
        json_string(&text),
        common != 0,
        style_spans(machine, common, 3, 8),
        style_spans(machine, common, 1, 2)
    )
}

fn measured_lines(machine: &Machine, wrapper: u64) -> String {
    let count = machine.call(TEXT + 0x8adb0, &[wrapper]);
    assert!(count <= 64);
    let lines = (0..count)
        .map(|index| {
            let start = machine.call(TEXT + 0x8ae34, &[wrapper, index]) as u32 as i32;
            let end = machine.call(TEXT + 0x8ae80, &[wrapper, index]) as u32 as i32;
            let background = returned_rect(machine, TEXT + 0x8b42c, &[wrapper, index]);
            machine.call(TEXT + 0x8b4c0, &[wrapper, index]);
            let top = read_register(machine.engine, 136) as u32;
            format!(
                "{{\"line\":{index},\"source_inclusive_utf16\":[{start},{end}],\"line_top_bits\":{top},\"background_rect_bits\":{background:?}}}"
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", lines.join(","))
}

pub(super) fn observe(
    machine: &Machine,
    text: &str,
    font_size: Option<f32>,
    width: i32,
    height: i32,
) -> String {
    assert!(width > 0 && height > 0);
    write(machine.engine, DOC, &vec![0; 0xb000]);
    let recorder = Recorder::new(machine);
    machine.call(0x3e1378, &[DOC]);
    let string = spen_string(machine, STORAGE, Some(text));
    assert_eq!(machine.call(0x3e1408, &[DOC, string, 0]) & 1, 1);
    if let Some(size) = font_size {
        float_arguments(machine, [size, 0., 0., 0.]);
        machine.call(0x3b06fc, &[read_u64(machine.engine, DOC + 56)]);
    }
    float_arguments(machine, [0., 0., width as f32, height as f32]);
    machine.call(0x397708, &[DOC, 0, 0]);
    machine.call(WIDGET + 0xd2fb4, &[DOC_WRAPPER, CONTEXT, 0]);
    machine.call(WIDGET + 0xd3974, &[DOC_WRAPPER, DOC]);
    machine.call(WIDGET + 0xd3e3c, &[DOC_WRAPPER]);
    machine.call(BODY + 0xaf948, &[OWNER, CONTEXT, 0]);
    put(machine, OWNER + 488, DOC_WRAPPER);
    initialize_one_page_document(machine, [width as f32, height as f32]);
    machine.call(BODY + 0xb2e54, &[OWNER]);
    machine.call(
        WIDGET + 0xd3b88,
        &[DOC_WRAPPER, 0, u32::MAX as u64, u32::MAX as u64, 0, 0],
    );
    let wrapper = machine.call(WIDGET + 0xd39ac, &[DOC_WRAPPER]);
    assert_eq!(wrapper, read_u64(machine.engine, DOC_WRAPPER + 368));
    let rich = read_u64(machine.engine, wrapper + 64);
    let length = machine.call(TEXT + 0x8b104, &[wrapper]);
    assert_eq!(length as usize, text.encode_utf16().count());
    let bound = returned_rect(machine, TEXT + 0x8afd4, &[wrapper, 0]);
    let entries = cell_measurement::snapshot(machine, rich);
    let bounds = cell_measurement::text_bounds(machine, wrapper, rich);
    let lines = measured_lines(machine, wrapper);
    let emitted = cell_measurement::emit(machine, rich);
    let source = source(machine, length);
    let document_rect = returned_rect(machine, 0x2caa60, &[DOC]);
    let infinite = machine.call(BODY + 0xa98a8, &[BODY_DOCUMENT]) & 1 != 0;
    let page_width = machine.call(BODY + 0xa98c8, &[BODY_DOCUMENT]) as u32 as i32;
    let without_last_page = machine.call(BODY + 0xa98e0, &[BODY_DOCUMENT, 0]) as u32 as i32;
    let including_last_page = machine.call(BODY + 0xa98e0, &[BODY_DOCUMENT, 1]) as u32 as i32;
    let widget_height = read_u32(machine.engine, DOC_WRAPPER + 528);
    assert!(!infinite);
    assert_eq!(without_last_page, 0);
    assert_eq!(widget_height, 0);
    format!(
        "{{\"supplied_text_utf8\":{},\"supplied_font_size_setter_bits\":{},\"supplied_page_record\":[0,0,0,{width},{height}],\"is_infinite_scroll\":{infinite},\"native_max_width\":{page_width},\"height_without_last_page\":{without_last_page},\"height_including_last_page\":{including_last_page},\"widget_layout_width_bits\":{},\"widget_layout_height_bits\":{widget_height},\"document_model_rect_bits\":{document_rect:?},\"native_calls\":{},\"source\":{source},\"text_bound_at_zero_bits\":{bound:?},\"text_bound_first_index\":-1,\"text_bound_by_index_bits\":{bounds:?},\"measured_lines\":{lines},\"entries\":{entries},\"emitted\":{emitted},\"capture_boundary\":{}}}",
        json_string(text),
        font_size.map_or_else(|| "null".into(), |size| size.to_bits().to_string()),
        read_u32(machine.engine, DOC_WRAPPER + 524),
        recorder.snapshot(),
        json_string(
            "Actual Model document constructor/Construct and optional ObjectShapeText.SetFontSize setter provide ordinary source without object spans. Positive Model Shape.SetRect, finite BodyTextDocument(false) construction and actual BodyTextLayout construction execute. Caller associates native owner/document/Widget and supplies one page record plus maximum width; complete BodyTextLayout.measureText obtains width and GetPageTotalHeight(false) through native document virtual methods and invokes actual Widget Measure. Widget height is observed, never supplied. Native Widget Update/Layout, Text bounds, dense measured entries, line getters and cached run emission execute. Native Model effective font and raw style-span getters are observed after producer output. Native page bounds/max-width production, complete SetBodyTextDocument, source parsing, object callbacks, device ICU, Composer/PDF backend and pixels are excluded; host/runtime boundaries match the parent capture."
        )
    )
}
