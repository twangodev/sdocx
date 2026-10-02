use super::super::widget_text_constructor::{self, CELL_DRAWING, CONTENT, CONTENT_SHA256, WIDGET};
use super::cell_host::{install_host, reset_host};
use super::*;

#[path = "live_table_text_clipping.rs"]
mod text_clipping;

#[path = "live_table_padding.rs"]
mod padding;

pub(crate) fn capture_padding(machine: &mut Machine, paths: widget_text_constructor::Paths<'_>) {
    padding::capture(machine, paths);
}

pub(crate) fn capture_text_clipping(
    machine: &mut Machine,
    paths: widget_text_constructor::Paths<'_>,
    composer: &Path,
) {
    text_clipping::capture(machine, paths, composer);
}

const TABLE: u64 = MODEL + 0x9000;
const LAYOUT: u64 = MODEL + 0x74000;
const CONTEXT: u64 = MODEL + 0x50000;
const INPUT: u64 = MODEL + 0x72000;
const COLOR: u64 = MODEL + 0x60800;
const DOCUMENT_WIDTH: u64 = MODEL + 0x60820;
const BANDS: u64 = MODEL + 0x75000;
const BAND_DATA: u64 = MODEL + 0x75100;

struct Host {
    cell: Box<cell_host::Host>,
    math_calls: BTreeMap<String, u32>,
}

fn host_import(engine: Engine, name: &str, args: [u64; 8], data: *mut c_void) -> Option<u64> {
    let host = unsafe { &mut *data.cast::<Host>() };
    if matches!(
        name,
        "ATrace_isEnabled" | "ATrace_beginSection" | "ATrace_endSection"
    ) {
        return Some(0);
    }
    if name == "pow" {
        let inputs = [read_register(engine, 40), read_register(engine, 41)];
        let result = f64::from_bits(inputs[0])
            .powf(f64::from_bits(inputs[1]))
            .to_bits();
        register(engine, 40, result);
        let call =
            format!("{{\"name\":\"pow\",\"input_bits\":{inputs:?},\"result_bits\":{result}}}");
        *host.math_calls.entry(call).or_default() += 1;
        return Some(0);
    }
    if name == "acosf" {
        let input = read_register(engine, 136) as u32;
        let result = f32::from_bits(input).acos().to_bits();
        register(engine, 136, u64::from(result));
        let call =
            format!("{{\"name\":\"acosf\",\"input_bits\":{input},\"result_bits\":{result}}}");
        *host.math_calls.entry(call).or_default() += 1;
        return Some(0);
    }
    cell_host::host_import(engine, name, args, ptr::from_mut(host.cell.as_mut()).cast())
}

const ENTRIES: [(&str, u64); 13] = [
    ("table_constructor", CELL_DRAWING + 0xa9d58),
    ("table_measure", CELL_DRAWING + 0xaa530),
    ("table_init", CELL_DRAWING + 0xaa6b4),
    ("table_layout", CELL_DRAWING + 0xaa3d4),
    ("table_update_cell", CELL_DRAWING + 0xae914),
    ("table_measure_cell", CELL_DRAWING + 0xaea60),
    ("table_layout_cell", CELL_DRAWING + 0xb06d4),
    ("widget_constructor", WIDGET + 0xd2fb4),
    ("widget_update", WIDGET + 0xd3e3c),
    ("widget_measure_impl", WIDGET + 0xd73b0),
    ("widget_lowercase_layout", WIDGET + 0xd3b88),
    ("text_layout", TEXT + 0x8ac10),
    ("cell_layout", CELL_DRAWING + 0x8c05c),
];

#[derive(Default)]
struct Trace {
    calls: BTreeMap<&'static str, u32>,
}
unsafe extern "C" fn trace(_: Engine, address: u64, _: u32, data: *mut c_void) {
    let observation = unsafe { &mut *data.cast::<Trace>() };
    let name = ENTRIES
        .iter()
        .find(|(_, target)| *target == address)
        .unwrap()
        .0;
    *observation.calls.entry(name).or_default() += 1;
}
impl Trace {
    fn take(&mut self) -> String {
        let calls = std::mem::take(&mut self.calls)
            .into_iter()
            .map(|(name, count)| format!("{}:{count}", json_string(name)))
            .collect::<Vec<_>>();
        format!("{{{}}}", calls.join(","))
    }
}

fn float_arguments(machine: &Machine, values: [f32; 4]) {
    for (index, value) in values.into_iter().enumerate() {
        register(
            machine.engine,
            136 + index as i32,
            u64::from(value.to_bits()),
        );
    }
}
fn returned_rect(machine: &Machine, address: u64, arguments: &[u64]) -> [u32; 4] {
    machine.call(address, arguments);
    std::array::from_fn(|index| read_register(machine.engine, 136 + index as i32) as u32)
}
fn float_getter(machine: &Machine, address: u64, object: u64) -> u32 {
    machine.call(address, &[object]);
    read_register(machine.engine, 136) as u32
}

fn snapshot(machine: &Machine, stage: &str, calls: String) -> String {
    let table_impl = read_u64(machine.engine, TABLE + 104);
    let cells = (0..4).map(|slot| {
        let cell = machine.call(0x3d2be0, &[TABLE, slot / 2, slot % 2]);
        let source = machine.call(0x3c22bc, &[cell]);
        let owner = machine.call(GET_FRAME_CELL, &[table_impl, slot / 2, slot % 2]);
        let owner_slot = (0..4).position(|index| machine.call(0x3d2be0, &[TABLE, index / 2, index % 2]) == owner).unwrap();
        let layout = machine.call(CELL_DRAWING + 0xacb84, &[LAYOUT, cell]);
        assert_ne!(layout, 0);
        assert_eq!(read_u64(machine.engine, read_u64(machine.engine, layout) + 24), CELL_DRAWING + 0x8c328);
        assert_eq!(read_u64(machine.engine, layout + 416), source);
        let wrapper = widget_text_constructor::text_wrapper(machine, layout);
        let rich = read_u64(machine.engine, wrapper + 64);
        let native_text = read_spen_string(machine.engine, machine.call(TEXT + 0x8b970, &[wrapper]));
        let source_text = read_spen_string(machine.engine, machine.call(0x39c9e8, &[source + 40]));
        let source_rect = returned_rect(machine, 0x2caa60, &[source]);
        let frame = returned_rect(machine, CELL_DRAWING + 0xab4bc, &[LAYOUT, cell]);
        let bounds = cell_measurement::text_bounds(machine, wrapper, rich);
        let entries = cell_measurement::snapshot(machine, rich);
        let emitted = cell_measurement::emit(machine, rich);
        let margins = [0x8b8f0,0x8b90c,0x8b928,0x8b944].map(|getter| float_getter(machine,TEXT+getter,wrapper));
        let font_size = float_getter(machine,TEXT+0x8b860,wrapper);
        let direction = machine.call(TEXT+0x8bc38,&[wrapper]);
        format!("{{\"slot\":{slot},\"owner_slot\":{owner_slot},\"span\":{:?},\"native_text_utf8\":{},\"source_text_utf8\":{},\"source_rect_bits\":{source_rect:?},\"frame_bits\":{frame:?},\"layout_width_bits\":{},\"layout_height_bits\":{},\"text_scale_bits\":{},\"font_size_bits\":{font_size},\"margin_bits\":{margins:?},\"direction\":{direction},\"update_size_callback_present\":{},\"editing_callback_present\":{},\"entries\":{entries},\"text_bound_bits\":{bounds:?},\"emitted\":{emitted}}}",[read_u32(machine.engine,cell+52),read_u32(machine.engine,cell+56)],optional_json(native_text.as_deref()),optional_json(source_text.as_deref()),read_u32(machine.engine,layout+524),read_u32(machine.engine,layout+528),read_u32(machine.engine,layout+600),read_u64(machine.engine,layout+288)!=0,read_u64(machine.engine,layout+96)!=0)
    }).collect::<Vec<_>>();
    format!(
        "{{\"stage\":{},\"native_calls\":{calls},\"cold_measure_required\":{},\"scale_bits\":{},\"content_rect_bits\":{:?},\"measured_rect_bits\":{:?},\"cells\":[{}]}}",
        json_string(stage),
        bytes(machine.engine, LAYOUT + 649, 1)[0] != 0,
        read_u32(machine.engine, LAYOUT + 1112),
        returned_rect(machine, CELL_DRAWING + 0xab4a8, &[LAYOUT]),
        returned_rect(machine, CELL_DRAWING + 0xab494, &[LAYOUT]),
        cells.join(",")
    )
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    bounds: [f32; 4],
    texts: [&'static str; 4],
    merge: Option<[u32; 4]>,
    font_size: Option<f32>,
    margins: Option<[f32; 4]>,
    scale: f32,
    density: f32,
    direction: u32,
    bands: &'static [[f32; 4]],
    resize: Option<f32>,
}
impl Case {
    fn sample(
        self,
        machine: &mut Machine,
        environment: &mut NativeFontEnvironment,
        host: &mut Host,
        observation: &mut Trace,
        fill: u8,
    ) -> String {
        self.sample_observed(machine, environment, host, observation, fill, None)
    }

    fn sample_observed(
        self,
        machine: &mut Machine,
        environment: &mut NativeFontEnvironment,
        host: &mut Host,
        observation: &mut Trace,
        fill: u8,
        after_warm: Option<&dyn Fn(&Machine) -> String>,
    ) -> String {
        reset_host(machine, environment, &mut host.cell, fill);
        machine.heap.cursor = MODEL + 0x80000;
        machine.heap.limit = MODEL + 0xc0000;
        host.math_calls.clear();
        observation.take();
        widget_text_constructor::configure_context(
            machine,
            widget_text_constructor::DeviceProfile {
                density: self.density,
                scaled_density: self.density,
                layout_direction: self.direction,
                ..Default::default()
            },
        );
        for (address, instructions) in [
            (COLOR, [0x2a0103e0_u32, 0xd65f03c0]),
            (DOCUMENT_WIDTH, [0x52800000, 0xd65f03c0]),
        ] {
            for (index, instruction) in instructions.into_iter().enumerate() {
                write(
                    machine.engine,
                    address + index as u64 * 4,
                    &instruction.to_le_bytes(),
                );
            }
        }
        write(machine.engine, CONTEXT + 8 + 80, &COLOR.to_le_bytes());
        write(
            machine.engine,
            CONTEXT + 8 + 144,
            &DOCUMENT_WIDTH.to_le_bytes(),
        );
        write(machine.engine, TABLE, &[0; 256]);
        for (index, value) in self.bounds.into_iter().enumerate() {
            write(
                machine.engine,
                INPUT + index as u64 * 4,
                &value.to_le_bytes(),
            );
        }
        machine.call(0x3d2690, &[TABLE]);
        assert_eq!(machine.call(0x3d27d8, &[TABLE, INPUT, 2, 2]), 1);
        if let Some([row, column, last_row, last_column]) = self.merge {
            assert_eq!(
                machine.call(
                    0x3d6038,
                    &[
                        TABLE,
                        u64::from(row),
                        u64::from(column),
                        u64::from(last_row),
                        u64::from(last_column)
                    ]
                ),
                1
            );
        }
        for (slot, text) in self.texts.into_iter().enumerate() {
            let cell = machine.call(0x3d2be0, &[TABLE, slot as u64 / 2, slot as u64 % 2]);
            let source = machine.call(0x3c22bc, &[cell]);
            let string = spen_string(machine, INPUT + 0x100, Some(text));
            assert_eq!(machine.call(0x39c9d4, &[source + 40, string]) & 1, 1);
            if let Some(size) = self.font_size {
                float_arguments(machine, [size, 0., 0., 0.]);
                machine.call(0x3b06fc, &[read_u64(machine.engine, source + 56)]);
            }
            if let Some(margins) = self.margins {
                float_arguments(machine, margins);
                machine.call(0x3c2554, &[cell]);
            }
        }
        write(machine.engine, LAYOUT, &[0; 1120]);
        float_arguments(machine, [self.density, 0., 0., 0.]);
        machine.call(CELL_DRAWING + 0xa9d58, &[LAYOUT, CONTEXT, TABLE, 1000]);
        let manager = read_u64(machine.engine, LAYOUT + 592);
        assert_eq!(machine.call(WIDGET + 0xaff8c, &[manager]), 1000);
        assert_eq!(
            float_getter(machine, WIDGET + 0xaff9c, manager),
            self.density.to_bits()
        );
        float_arguments(machine, [self.scale, 0., 0., 0.]);
        machine.call(CELL_DRAWING + 0xacfe8, &[LAYOUT]);
        for (index, band) in self.bands.iter().enumerate() {
            for (component, value) in band.iter().enumerate() {
                write(
                    machine.engine,
                    BAND_DATA + index as u64 * 16 + component as u64 * 4,
                    &value.to_le_bytes(),
                );
            }
        }
        for (offset, value) in [
            (0, BAND_DATA),
            (8, BAND_DATA + self.bands.len() as u64 * 16),
            (16, BAND_DATA + self.bands.len() as u64 * 16),
        ] {
            write(machine.engine, BANDS + offset, &value.to_le_bytes());
        }
        machine.call(CELL_DRAWING + 0xad040, &[LAYOUT, BANDS]);
        machine.call(CELL_DRAWING + 0xaa530, &[LAYOUT]);
        let measured = snapshot(machine, "cold-measure", observation.take());
        machine.call(CELL_DRAWING + 0xaa3d4, &[LAYOUT]);
        let placed = snapshot(machine, "warm-layout", observation.take());
        let mut states = vec![measured, placed];
        if let Some(width) = self.resize {
            float_arguments(machine, [width, 0., 0., 0.]);
            assert_eq!(machine.call(0x3d3b64, &[TABLE, 0]) & 1, 1);
            machine.call(CELL_DRAWING + 0xb0088, &[LAYOUT, 0]);
            machine.call(CELL_DRAWING + 0xaa3d4, &[LAYOUT]);
            states.push(snapshot(
                machine,
                "warm-column-resize-and-layout",
                observation.take(),
            ));
        }
        let writer = after_warm.map(|observe| observe(machine));
        assert!(machine.heap.cursor < MODEL + 0xc0000);
        let mut result = format!(
            "{{\"name\":{},\"bounds_bits\":{:?},\"texts_utf8\":[{}],\"texts_assigned_after_merge\":true,\"merge\":{},\"font_size_bits\":{},\"margin_bits\":{},\"text_scale_bits\":{},\"document_width\":1000,\"document_density_bits\":{},\"display_direction\":{},\"supplied_local_split_band_bits\":{:?},\"warm_first_column_width_bits\":{},\"math_calls\":[{}],\"icu_calls\":{:?},\"states\":[{}]}}",
            json_string(self.name),
            self.bounds.map(f32::to_bits),
            self.texts.map(json_string).join(","),
            self.merge
                .map_or_else(|| "null".to_owned(), |m| format!("{m:?}")),
            self.font_size
                .map_or_else(|| "null".to_owned(), |v| v.to_bits().to_string()),
            self.margins.map_or_else(
                || "null".to_owned(),
                |m| format!("{:?}", m.map(f32::to_bits))
            ),
            self.scale.to_bits(),
            self.density.to_bits(),
            self.direction,
            self.bands
                .iter()
                .map(|band| band.map(f32::to_bits))
                .collect::<Vec<_>>(),
            self.resize
                .map_or_else(|| "null".to_owned(), |v| v.to_bits().to_string()),
            host.math_calls
                .iter()
                .map(|(call, count)| format!("{{\"call\":{call},\"count\":{count}}}"))
                .collect::<Vec<_>>()
                .join(","),
            host.cell.paragraphs.calls,
            states.join(",")
        );
        if let Some(writer) = writer {
            assert_eq!(result.pop(), Some('}'));
            result.push_str(&format!(",\"writer\":{writer}}}"));
        }
        result
    }
}

pub(crate) fn capture(machine: &mut Machine, paths: widget_text_constructor::Paths<'_>) {
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
    let mut host = Box::new(Host {
        cell: install_host(machine, &mut environment, fonts),
        math_calls: BTreeMap::new(),
    });
    environment.set_host_import_handler(host_import, ptr::from_mut(host.as_mut()).cast());
    let mut observation = Box::<Trace>::default();
    for (_, address) in ENTRIES {
        let mut hook = 0;
        check(unsafe {
            uc_hook_add(
                machine.engine,
                &mut hook,
                4,
                trace as *mut c_void,
                ptr::from_mut(observation.as_mut()).cast(),
                address,
                address,
            )
        });
    }
    let base = Case {
        name: "ordinary-native-defaults",
        bounds: [13.25, -19.5, 173.25, 180.5],
        texts: ["AV abc", "To", "A😀B", "last"],
        merge: None,
        font_size: None,
        margins: None,
        scale: 1.,
        density: 1.,
        direction: 0,
        bands: &[],
        resize: None,
    };
    let bands: &[[f32; 4]] = &[[0., 60., 180., 120.], [0., 140., 180., 250.]];
    let cases = [
        base,
        Case {
            name: "zero-absolute-origin",
            bounds: [0., 0., 160., 200.],
            ..base
        },
        Case {
            name: "narrow-native-defaults",
            bounds: [13.25, -19.5, 93.25, 100.5],
            ..base
        },
        Case {
            name: "fractional-grid",
            bounds: [13.25, -19.5, 175.05, 181.0],
            ..base
        },
        Case {
            name: "native-source-font17",
            font_size: Some(17.),
            ..base
        },
        Case {
            name: "asymmetric-source-margins",
            font_size: Some(17.),
            margins: Some([1.25, 2.5, 3.75, 4.5]),
            ..base
        },
        Case {
            name: "text-scale075",
            scale: 0.75,
            ..base
        },
        Case {
            name: "text-scale15",
            scale: 1.5,
            ..base
        },
        Case {
            name: "document-density2",
            density: 2.,
            ..base
        },
        Case {
            name: "direction1-mixed-text",
            direction: 1,
            texts: ["AV אב", "To", "ÁB", "last"],
            font_size: Some(17.),
            ..base
        },
        Case {
            name: "ordinary-local-split-bands",
            bands,
            ..base
        },
        Case {
            name: "warm-width-grow",
            resize: Some(120.),
            ..base
        },
        Case {
            name: "warm-width-shrink",
            resize: Some(40.),
            ..base
        },
        Case {
            name: "merged-columns-covered-text",
            merge: Some([0, 0, 0, 1]),
            ..base
        },
        Case {
            name: "merged-rows-covered-text",
            merge: Some([0, 0, 1, 0]),
            ..base
        },
        Case {
            name: "merged-two-by-two-covered-text",
            merge: Some([0, 0, 1, 1]),
            ..base
        },
        Case {
            name: "merged-two-by-two-split-bands",
            merge: Some([0, 0, 1, 1]),
            bands,
            ..base
        },
        Case {
            name: "merged-empty-owner-covered-text",
            merge: Some([0, 0, 1, 1]),
            texts: ["", "To", "A😀B", "last"],
            ..base
        },
    ];
    let output = cases
        .into_iter()
        .map(|case| {
            let expected = case.sample(machine, &mut environment, &mut host, &mut observation, 0);
            for fill in [85, 165, 255, 0] {
                let actual =
                    case.sample(machine, &mut environment, &mut host, &mut observation, fill);
                assert!(
                    actual == expected,
                    "case {} fill {fill} disagrees",
                    case.name
                );
            }
            expected
        })
        .collect::<Vec<_>>();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"dependencies\":{},\"memory_fills\":[0,85,165,255],\"repeat_zero_fill\":true,\"model_library_sha256\":{},\"base_library_sha256\":{},\"text_library_sha256\":{},\"skia_library_sha256\":{},\"widget_library_sha256\":{},\"content_library_sha256\":{},\"drawing_library_sha256\":{},\"native_entrypoints\":{{\"table_constructor\":\"Drawing:0xa9d58\",\"cold_measure\":\"Drawing:0xaa530\",\"init\":\"Drawing:0xaa6b4\",\"warm_layout\":\"Drawing:0xaa3d4\",\"set_text_scale\":\"Drawing:0xacfe8\",\"set_split_bands\":\"Drawing:0xad040\",\"model_column_width\":\"Model:0x3d3b64\",\"warm_resize_column\":\"Drawing:0xb0088\",\"editing_callback_installation\":\"Drawing:0xaada4\"}},\"capture_boundary\":{},\"cases\":[{}]}}",
        dependency_metadata(),
        json_string(LIBRARY_SHA256),
        json_string(frames::BASE_SHA256),
        json_string(geometry::TEXT_SHA256),
        json_string(text_font_source::SKIA_SHA256),
        json_string(geometry::WIDGET_SHA256),
        json_string(CONTENT_SHA256),
        json_string(DRAWING_SHA256),
        json_string(
            "Complete native Model table/row/cell/content construction, native merge and physical-cell text/optional font-size/margin setters; complete native TableLayout constructor and TextManager, cold Measure/init/extendRowBySplit/updateMeasuredRect, genuine child constructors/SetObject/SetTextScale/Widget Update, NAME font selection and bundled Minikin/HarfBuzz/Skia/FreeType measurement, complete warm Layout and optional Model SetColumnWidth/native resizeColumn execute. Native defaults establish margins and text policies. Actual measured entries, TextLayout.GetTextBound and cached emitted runs are observed; no child widths, measured entries, glyphs, classifications, font getters, cache frames or geometry callback values are supplied. Caller supplies fixed 2x2 source bounds, per-physical-cell text after merge, optional source style controls, independent table text scale, explicit default device/profile interfaces, document width1000/density and local split bands via actual setter. Caller IContext color service returns input ARGB, preferred document-width getter returns0 so constructor argument1000 applies; context/display storage is supplied while actual table TextManager executes. Native editing callback at child std::function+64 is installed by init; update-size callback target+288 and table forwarding callbacks remain constructor-null. Pinned host ICU76.1/Unicode16 replaces Android ICU; host allocation/files/libc/single-thread services, deterministic UUIDs, pow/acosf primitives and disabled ATrace are explicit boundaries. Model registry and runtime allocators reset between captures; table/input storage is zeroed before construction and allocation fills vary owned native memory. No saved parser/clone, complete Bodytext callback/document text rectangle production, document pagination/split-band production, native device ICU, final writer consumption or pixels execute."
        ),
        output.join(",")
    );
}
