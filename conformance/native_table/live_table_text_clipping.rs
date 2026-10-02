use super::*;

const COMPOSER: u64 = 0x1200_0000;
const COMPOSER_SHA256: &str = "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f";
const CLONE_LAYOUT: u64 = MODEL + 0x79000;
const WRITER: u64 = MODEL + 0x76000;
const PDF: u64 = MODEL + 0x76200;
const PDF_VTABLE: u64 = MODEL + 0x76800;
const PAINT: u64 = MODEL + 0x76400;
const PAINT_VTABLE: u64 = MODEL + 0x77000;
const DATA: u64 = MODEL + 0x77a00;
const HOST: u64 = MODEL + 0x78000;
const PAGE: u64 = HOST;
const CREATE_PAINT: u64 = HOST + 32;
const RELEASE_PAINT: u64 = HOST + 64;
const PAINT_SETTER: u64 = HOST + 96;
const TRANSLATE: u64 = HOST + 128;
const DRAW_TEXT: u64 = HOST + 160;
const SKIP_EXPORT: u64 = HOST + 192;

fn rectangle(engine: Engine, address: u64) -> [u32; 4] {
    std::array::from_fn(|axis| read_u32(engine, address + axis as u64 * 4))
}
fn returned(engine: Engine) -> [u32; 4] {
    std::array::from_fn(|axis| read_register(engine, 136 + axis as i32) as u32)
}

#[derive(Default)]
struct Run {
    range: [u32; 2],
    local: [u32; 4],
    source: [u32; 4],
    origin: [u32; 2],
    object: bool,
    source_is_selected_cell: bool,
    selected: bool,
    intersected: Option<bool>,
    world: Option<[u32; 4]>,
    translation: Option<[u32; 2]>,
    clip: Option<[u32; 4]>,
    backend_rect: Option<[u32; 4]>,
    positions: Vec<u32>,
}
impl Run {
    fn json(&self) -> String {
        format!(
            "{{\"range_inclusive\":{:?},\"local_rect_bits\":{:?},\"actual_source_rect_bits\":{:?},\"caller_origin_bits\":{:?},\"is_object\":{},\"source_is_selected_cell\":{},\"clip_selected\":{},\"intersect_succeeded\":{},\"world_clip_bits\":{},\"paint_translation_bits\":{},\"passed_pdf_clip_bits\":{},\"backend_rect_bits\":{},\"scaled_position_bits\":{:?}}}",
            self.range,
            self.local,
            self.source,
            self.origin,
            self.object,
            self.source_is_selected_cell,
            self.selected,
            optional(self.intersected),
            optional(self.world),
            optional(self.translation),
            optional(self.clip),
            optional(self.backend_rect),
            self.positions
        )
    }
}
fn optional<T: std::fmt::Debug>(value: Option<T>) -> String {
    value.map_or_else(|| "null".into(), |v| format!("{v:?}"))
}
#[derive(Default)]
struct Observation {
    runs: Vec<Run>,
    paints: usize,
    releases: usize,
    skipped_exports: usize,
    expected_source: u64,
    native_calls: BTreeMap<&'static str, u32>,
}
unsafe extern "C" fn observe(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let state = unsafe { &mut *data.cast::<Observation>() };
    let x0 = read_register(engine, REGISTER_X0);
    if let Some((name, _)) = ENTRIES.iter().find(|(_, target)| *target == address) {
        *state.native_calls.entry(name).or_default() += 1;
        return;
    }
    match address {
        PAGE => {
            assert_eq!(x0, PDF);
            for (i, v) in [0_f32, 0., 600., 800.].into_iter().enumerate() {
                register(engine, 136 + i as i32, u64::from(v.to_bits()));
            }
        }
        CREATE_PAINT => {
            assert_eq!(x0, PDF);
            state.paints += 1;
            register(engine, REGISTER_X0, PAINT);
        }
        RELEASE_PAINT => {
            assert_eq!(x0, PDF);
            assert_eq!(read_register(engine, REGISTER_X0 + 1), PAINT);
            state.releases += 1;
        }
        PAINT_SETTER => assert_eq!(x0, PAINT),
        TRANSLATE => {
            assert_eq!(x0, PAINT);
            state.runs.last_mut().unwrap().translation = Some(std::array::from_fn(|i| {
                read_register(engine, 136 + i as i32) as u32
            }));
        }
        DRAW_TEXT => {
            assert_eq!(x0, PDF);
            let run = state.runs.last_mut().unwrap();
            run.clip = Some(rectangle(engine, read_register(engine, REGISTER_X0 + 4)));
            run.backend_rect = Some(std::array::from_fn(|i| {
                read_register(engine, 138 + i as i32) as u32
            }));
            let vector = read_register(engine, REGISTER_X0 + 2);
            let begin = read_u64(engine, vector);
            let end = read_u64(engine, vector + 8);
            assert!(end >= begin && end - begin <= 1024 && (end - begin).is_multiple_of(4));
            run.positions = (begin..end)
                .step_by(4)
                .map(|p| read_u32(engine, p))
                .collect();
        }
        SKIP_EXPORT => state.skipped_exports += 1,
        a if a == COMPOSER + 0x37f508 => {
            let pointer = read_register(engine, REGISTER_X0 + 2);
            state.runs.push(Run {
                range: [read_u32(engine, pointer), read_u32(engine, pointer + 4)],
                local: rectangle(engine, pointer + 104),
                origin: std::array::from_fn(|i| read_register(engine, 136 + i as i32) as u32),
                object: bytes(engine, pointer + 120, 1)[0] != 0,
                source_is_selected_cell: read_register(engine, REGISTER_X0 + 1)
                    == state.expected_source,
                ..Default::default()
            });
        }
        a if a == COMPOSER + 0x37f6d0 => state.runs.last_mut().unwrap().source = returned(engine),
        a if a == COMPOSER + 0x37f6e8 => state.runs.last_mut().unwrap().selected = true,
        a if a == COMPOSER + 0x37f724 => {
            let run = state.runs.last_mut().unwrap();
            run.intersected = Some(read_register(engine, REGISTER_X0) & 1 != 0);
            run.world = Some(rectangle(engine, read_register(engine, REGISTER_SP) + 32));
        }
        _ => unreachable!("interface {address:x}"),
    }
}
struct Recorder {
    engine: Engine,
    hooks: Vec<usize>,
    observation: Box<Observation>,
}
impl Recorder {
    fn new(machine: &Machine) -> Self {
        let mut result = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            observation: Box::default(),
        };
        for address in [
            PAGE,
            CREATE_PAINT,
            RELEASE_PAINT,
            PAINT_SETTER,
            TRANSLATE,
            DRAW_TEXT,
            SKIP_EXPORT,
        ] {
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
        }
        for (object, vtable) in [(PDF, PDF_VTABLE), (PAINT, PAINT_VTABLE)] {
            write(machine.engine, object, &vtable.to_le_bytes());
        }
        for offset in (0..=320).step_by(8) {
            write(
                machine.engine,
                PAINT_VTABLE + offset,
                &PAINT_SETTER.to_le_bytes(),
            );
        }
        write(machine.engine, PAINT_VTABLE + 272, &TRANSLATE.to_le_bytes());
        for (offset, target) in [
            (48, PAGE),
            (88, CREATE_PAINT),
            (96, RELEASE_PAINT),
            (152, DRAW_TEXT),
        ] {
            write(machine.engine, PDF_VTABLE + offset, &target.to_le_bytes());
        }
        for plt in [0x552eb0, 0x5534d0, 0x5534f0, 0x553500] {
            absolute(machine.engine, COMPOSER + plt, SKIP_EXPORT);
        }
        let mut targets = ENTRIES.iter().map(|(_, a)| *a).collect::<Vec<_>>();
        targets.extend([
            PAGE,
            CREATE_PAINT,
            RELEASE_PAINT,
            PAINT_SETTER,
            TRANSLATE,
            DRAW_TEXT,
            SKIP_EXPORT,
            COMPOSER + 0x37f508,
            COMPOSER + 0x37f6d0,
            COMPOSER + 0x37f6e8,
            COMPOSER + 0x37f724,
        ]);
        for address in targets {
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    observe as *mut c_void,
                    ptr::from_mut(result.observation.as_mut()).cast(),
                    address,
                    address,
                )
            });
            result.hooks.push(hook);
        }
        result
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        for &hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, hook) });
        }
    }
}
fn absolute(engine: Engine, address: u64, target: u64) {
    write(engine, address, &0x58000050_u32.to_le_bytes());
    write(engine, address + 4, &0xd61f0200_u32.to_le_bytes());
    write(engine, address + 8, &target.to_le_bytes());
}

#[derive(Clone, Copy, Debug)]
enum SourceControl {
    None,
    BottomBelow,
    BottomEqual,
    BottomAbove,
    Disjoint,
    WideOnly,
}

pub(super) fn bodytext_writer(machine: &Machine) -> String {
    writer(machine, 1., SourceControl::None)
}

fn writer(machine: &Machine, density: f32, control: SourceControl) -> String {
    let mut recorder = Recorder::new(machine);
    let clone = machine.call(0x36d6cc, &[22, 0]);
    assert_ne!(clone, 0);
    let vt = read_u64(machine.engine, clone);
    assert_eq!(
        machine.call(read_u64(machine.engine, vt + 184), &[clone, TABLE]),
        1
    );
    write(machine.engine, CLONE_LAYOUT, &[0; 1120]);
    float_arguments(machine, [density, 0., 0., 0.]);
    machine.call(
        CELL_DRAWING + 0xa9d58,
        &[CLONE_LAYOUT, CONTEXT, clone, 1000],
    );
    machine.call(CELL_DRAWING + 0xaa530, &[CLONE_LAYOUT]);
    machine.call(CELL_DRAWING + 0xaa3d4, &[CLONE_LAYOUT]);
    machine.call(CELL_DRAWING + 0xacc88, &[CLONE_LAYOUT]);
    write(machine.engine, WRITER, &[0; 128]);
    for (offset, value) in [(32, PDF), (40, clone), (48, CLONE_LAYOUT)] {
        write(machine.engine, WRITER + offset, &value.to_le_bytes());
    }
    write(machine.engine, WRITER + 8, &density.to_le_bytes());
    write(machine.engine, WRITER + 112, &1_f32.to_le_bytes());
    register(machine.engine, REGISTER_X0 + 19, WRITER);
    register(machine.engine, REGISTER_X0 + 20, clone);
    register(machine.engine, REGISTER_SP, STACK);
    register(machine.engine, REGISTER_X30, STOP);
    let error = unsafe {
        uc_emu_start(
            machine.engine,
            COMPOSER + 0x37e4ac,
            COMPOSER + 0x37e4f8,
            1_000_000,
            10000,
        )
    };
    assert_eq!(
        error,
        0,
        "measured writer PC {:x}",
        read_register(machine.engine, 260)
    );
    assert_eq!(read_register(machine.engine, 260), COMPOSER + 0x37e4f8);
    let mut cells = Vec::new();
    for slot in 0..4 {
        let cell = machine.call(0x3d2be0, &[clone, slot / 2, slot % 2]);
        let source = machine.call(0x3c22bc, &[cell]);
        let layout = machine.call(CELL_DRAWING + 0xacb84, &[CLONE_LAYOUT, cell]);
        let wrapper = widget_text_constructor::text_wrapper(machine, layout);
        let rich = read_u64(machine.engine, wrapper + 64);
        let before = returned_rect(machine, 0x2caa60, &[source]);
        let bound = cell_measurement::text_bounds(machine, wrapper, rich);
        let mut supplied_source = None;
        if slot == 0 && !matches!(control, SourceControl::None) {
            write(machine.engine, DATA, &[0; 120]);
            write(machine.engine, DATA + 104, &1_f32.to_le_bytes());
            let text = machine.call(0x39c9e8, &[source + 40]);
            let length = read_spen_string(machine.engine, text)
                .unwrap_or_default()
                .encode_utf16()
                .count();
            float_arguments(machine, [0.; 4]);
            machine.call(CELL_DRAWING + 0x8c084, &[layout, 0, length as u64, DATA]);
            let begin = read_u64(machine.engine, DATA + 24);
            let end = read_u64(machine.engine, DATA + 32);
            assert!(end > begin);
            let run = read_u64(machine.engine, begin);
            let bottom = read_float(machine.engine, run + 116);
            assert!(bottom > 0.);
            let height = match control {
                SourceControl::BottomBelow => f32::from_bits(bottom.to_bits() + 1),
                SourceControl::BottomEqual => bottom,
                SourceControl::BottomAbove => f32::from_bits(bottom.to_bits() - 1),
                SourceControl::Disjoint => 1.,
                SourceControl::WideOnly => bottom + 100.,
                SourceControl::None => unreachable!(),
            };
            let origin = if matches!(control, SourceControl::Disjoint) {
                1000.
            } else {
                0.
            };
            let width = if matches!(control, SourceControl::WideOnly) {
                1.
            } else {
                100.
            };
            let input = [origin, origin, origin + width, origin + height];
            float_arguments(machine, input);
            let vtable = read_u64(machine.engine, source);
            machine.call(read_u64(machine.engine, vtable + 40), &[source, 0, 0]);
            supplied_source = Some(input.map(f32::to_bits));
        }
        let source_rect = returned_rect(machine, 0x2caa60, &[source]);
        let frame = returned_rect(machine, CELL_DRAWING + 0xab4bc, &[CLONE_LAYOUT, cell]);
        recorder.observation.expected_source = source;
        let offset = recorder.observation.runs.len();
        machine.call(COMPOSER + 0x37ec88, &[WRITER, slot / 2, slot % 2]);
        let emitted = recorder.observation.runs[offset..]
            .iter()
            .map(Run::json)
            .collect::<Vec<_>>();
        cells.push(format!("{{\"slot\":{slot},\"source_rect_before_control_bits\":{before:?},\"source_rect_bits\":{source_rect:?},\"supplied_source_setter_bits\":{},\"frame_bits\":{frame:?},\"text_bound_bits\":{bound:?},\"update_size_callback_present\":{},\"editing_callback_present\":{},\"runs\":[{}]}}",optional(supplied_source),read_u64(machine.engine,layout+288)!=0,read_u64(machine.engine,layout+96)!=0,emitted.join(",")));
    }
    let o = &recorder.observation;
    assert_eq!(o.paints, o.releases);
    format!(
        "{{\"source_control\":{},\"native_factory_copy\":true,\"rounded_measured_world_bits\":{:?},\"paint_count\":{},\"native_calls\":{{{}}},\"skipped_sibling_export_calls\":{},\"cells\":[{}]}}",
        json_string(&format!("{control:?}")),
        rectangle(machine.engine, WRITER + 96),
        o.paints,
        o.native_calls
            .iter()
            .map(|(name, count)| format!("{}:{count}", json_string(name)))
            .collect::<Vec<_>>()
            .join(","),
        o.skipped_exports,
        cells.join(",")
    )
}

pub(crate) fn capture(
    machine: &mut Machine,
    paths: widget_text_constructor::Paths<'_>,
    composer: &Path,
) {
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
            (composer, COMPOSER, COMPOSER_SHA256),
        ],
        &[(paths.model, 0, LIBRARY_SHA256)],
    );
    let mut host = Box::new(Host {
        cell: install_host(machine, &mut environment, fonts),
        math_calls: BTreeMap::new(),
    });
    environment.set_host_import_handler(host_import, ptr::from_mut(host.as_mut()).cast());
    let mut observation = Trace::default();
    let base = Case {
        name: "native-source-default",
        bounds: [0., 0., 160., 200.],
        texts: ["AV abc", "To", "ÁB", "last"],
        merge: None,
        font_size: Some(17.),
        margins: None,
        scale: 1.,
        density: 1.,
        direction: 0,
        bands: &[],
        resize: None,
    };
    let cases = [
        (base, SourceControl::None),
        (
            Case {
                name: "translated-native-source",
                bounds: [13.25, -19.5, 173.25, 180.5],
                ..base
            },
            SourceControl::None,
        ),
        (
            Case {
                name: "short-native-source-multiline",
                bounds: [0., 0., 160., 8.],
                texts: ["AV\nTo\nlast", "To", "ÁB", "last"],
                ..base
            },
            SourceControl::None,
        ),
        (
            Case {
                name: "bottom-below-source-height",
                ..base
            },
            SourceControl::BottomBelow,
        ),
        (
            Case {
                name: "bottom-equals-source-height",
                ..base
            },
            SourceControl::BottomEqual,
        ),
        (
            Case {
                name: "bottom-above-source-height",
                ..base
            },
            SourceControl::BottomAbove,
        ),
        (
            Case {
                name: "disjoint-source-control",
                ..base
            },
            SourceControl::Disjoint,
        ),
        (
            Case {
                name: "wide-only-source-control",
                ..base
            },
            SourceControl::WideOnly,
        ),
        (
            Case {
                name: "merged-native-source",
                merge: Some([0, 0, 1, 1]),
                ..base
            },
            SourceControl::None,
        ),
        (
            Case {
                name: "density-two-native-source",
                density: 2.,
                ..base
            },
            SourceControl::None,
        ),
        (
            Case {
                name: "asymmetric-native-source-margins",
                margins: Some([1.25, 2.5, 3.75, 4.5]),
                ..base
            },
            SourceControl::None,
        ),
    ];
    let output = cases
        .into_iter()
        .map(|(case, control)| {
            let observer = |m: &Machine| writer(m, case.density, control);
            let expected = case.sample_observed(
                machine,
                &mut environment,
                &mut host,
                &mut observation,
                0,
                Some(&observer),
            );
            for fill in [85, 165, 255, 0] {
                assert_eq!(
                    expected,
                    case.sample_observed(
                        machine,
                        &mut environment,
                        &mut host,
                        &mut observation,
                        fill,
                        Some(&observer)
                    ),
                    "{} fill{fill}",
                    case.name
                );
            }
            expected
        })
        .collect::<Vec<_>>();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"dependencies\":{},\"memory_fills\":[0,85,165,255],\"repeat_zero_fill\":true,\"model_library_sha256\":{},\"base_library_sha256\":{},\"text_library_sha256\":{},\"skia_library_sha256\":{},\"widget_library_sha256\":{},\"content_library_sha256\":{},\"drawing_library_sha256\":{},\"composer_library_sha256\":{},\"native_entrypoints\":{},\"capture_boundary\":{},\"cases\":[{}]}}",
        dependency_metadata(),
        json_string(LIBRARY_SHA256),
        json_string(frames::BASE_SHA256),
        json_string(geometry::TEXT_SHA256),
        json_string(text_font_source::SKIA_SHA256),
        json_string(geometry::WIDGET_SHA256),
        json_string(CONTENT_SHA256),
        json_string(DRAWING_SHA256),
        json_string(COMPOSER_SHA256),
        "{\"Model.ObjectFactoryCreate\":\"0x36d6cc\",\"Model.ObjectTableGetCell\":\"0x3d2be0\",\"Model.ObjectTableCellGetContentObject\":\"0x3c22bc\",\"Model.ObjectShapeGetRect\":\"0x2caa60\",\"Model.ObjectCopyVirtualSlot\":184,\"Model.ObjectSetRectVirtualSlot\":40,\"Drawing.TableLayoutConstructor\":\"0xa9d58\",\"Drawing.TableLayoutMeasure\":\"0xaa530\",\"Drawing.TableLayoutLayout\":\"0xaa3d4\",\"Drawing.TableLayoutUpdateTextDrawingPosition\":\"0xacc88\",\"Drawing.TableLayoutGetTableCellLayout\":\"0xacb84\",\"Drawing.TableLayoutGetFrameCellRect\":\"0xab4bc\",\"Drawing.CellLayoutGetDrawnTextData\":\"0x8c084\",\"Text.TextLayoutGetTextBound\":\"0x8afd4\",\"Text.RichTextGetTextBound\":\"0x7a4ac\",\"Text.RichTextDrawingGetDrawnText\":\"0x68418\",\"Composer.WriterMeasuredWorldWindow\":[\"0x37e4ac\",\"0x37e4f8\"],\"Composer.TableWriterWriteTextContent\":\"0x37ec88\",\"Composer.TableWriterWriteTextBlock\":\"0x37f508\",\"Composer.ContentGetRectReturned\":\"0x37f6d0\",\"Composer.ConditionalClipSelected\":\"0x37f6e8\",\"Composer.ForegroundIntersectReturned\":\"0x37f724\",\"Composer.SetPaintInfo\":\"0x3835fc\",\"Composer.PdfDrawTextCallsite\":\"0x37f8cc\"}",
        json_string(
            "Caller selects all four raw physical slots, including merged covered cells; aggregate writer visible-cell selection is excluded. Complete native Model table/content/text construction, full TableLayout/Widget/Text measure and placement, native factory clone/copy and second actual clone TableLayout construction/measurement/placement/UpdateTextDrawingPosition, actual writer measured world rounding, complete writeTextContent and writeTextBlock with native cached DrawnTextData/GetDrawnText emission and actual content Model GetRect execute. Default source rectangles derive from native constructor/copy; named SourceControl cases additionally call actual source ObjectShape.SetRect with explicitly supplied controls after warm layout, not a callback-derived source. Real TextLayout.GetTextBound results are observed. PDF factory/Paint methods and final DrawText are recording interfaces; sibling bitmap-font/background/decoration exports are skipped. Native geometry/intersection/scaling/guards and font-data getters execute. Context/device/color, allocator/UUID/libc/files/single-thread/pow/acosf/ATrace and pinned host ICU76.1 boundaries match live_table_layout. Upstream parent document object-entry bound generation and Bodytext callback activation, device ICU, parser, final PDF backend clipping and pixels are excluded."
        ),
        output.join(",")
    );
}
