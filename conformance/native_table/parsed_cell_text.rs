use super::*;
use text_font_source::{TEXT, bytes, json_string};
use text_span_font_name::cell_host::{install_host, reset_host};
use widget_text_constructor::{CELL_DRAWING, CELL_LAYOUT, CONTENT, CONTENT_SHA256, WIDGET};

pub(crate) use super::widget_text_constructor::Paths;

const OBJECT: u64 = MODEL + 0x71000;
const INPUT: u64 = MODEL + 0x72000;
const INPUT_POINTER: u64 = MODEL + 0x73000;
const REMAINING: u64 = MODEL + 0x73008;
const VECTOR: u64 = MODEL + 0x74000;
const FORMAT_VERSION: u64 = 5500;

#[derive(Clone, Copy)]
struct Span {
    kind: u32,
    start: u32,
    end: u32,
    interval: u32,
    payload: u32,
}

struct Case {
    name: &'static str,
    text: &'static str,
    common_present: bool,
    spans: Vec<Span>,
}

fn cases() -> Vec<Case> {
    let font = |end| Span {
        kind: 3,
        start: 0,
        end,
        interval: 1,
        payload: 50_f32.to_bits(),
    };
    vec![
        Case {
            name: "absent-common",
            text: "",
            common_present: false,
            spans: vec![],
        },
        Case {
            name: "empty-common-no-spans",
            text: "",
            common_present: true,
            spans: vec![],
        },
        Case {
            name: "nonempty-common-no-spans",
            text: "AV",
            common_present: true,
            spans: vec![],
        },
        Case {
            name: "nonempty-foreground-only",
            text: "AV",
            common_present: true,
            spans: vec![Span {
                kind: 1,
                start: 0,
                end: 2,
                interval: 1,
                payload: 0xff123456,
            }],
        },
        Case {
            name: "nonempty-font50",
            text: "AV",
            common_present: true,
            spans: vec![font(2)],
        },
        Case {
            name: "empty-caret-font50",
            text: "",
            common_present: true,
            spans: vec![font(0)],
        },
        Case {
            name: "nonempty-caret-font50",
            text: "AV",
            common_present: true,
            spans: vec![font(0)],
        },
    ]
}

fn payload(case: &Case) -> Vec<u8> {
    if !case.common_present {
        return Vec::new();
    }
    let mut common = Vec::new();
    let units = case.text.encode_utf16().collect::<Vec<_>>();
    common.extend_from_slice(&(units.len() as u32).to_le_bytes());
    for unit in units {
        common.extend_from_slice(&unit.to_le_bytes());
    }
    common.extend_from_slice(&(case.spans.len() as u32).to_le_bytes());
    for span in &case.spans {
        common.extend_from_slice(&24_u16.to_le_bytes());
        for word in [span.kind, span.start, span.end, span.interval, span.payload] {
            common.extend_from_slice(&word.to_le_bytes());
        }
        common.extend_from_slice(&0_u32.to_le_bytes());
    }
    common.extend_from_slice(&0_u32.to_le_bytes());
    common.extend_from_slice(&[0; 16]);
    common.push(0);
    common.extend_from_slice(&0_u16.to_le_bytes());
    common.extend_from_slice(&[0; 8]);
    let mut data = (common.len() as u32).to_le_bytes().to_vec();
    data.extend(common);
    data
}

fn string(engine: Engine, address: u64) -> Option<String> {
    if address == 0 {
        return None;
    }
    let implementation = read_u64(engine, address + 8);
    assert_ne!(implementation, 0);
    let length = read_u32(engine, implementation + 12) as usize;
    assert!(length <= 64);
    let source = read_u64(engine, implementation + 16);
    let units = bytes(engine, source, length * 2)
        .chunks_exact(2)
        .map(|unit| u16::from_le_bytes(unit.try_into().unwrap()))
        .collect::<Vec<_>>();
    Some(String::from_utf16(&units).unwrap())
}

fn style_spans(machine: &Machine, kind: u64, mask: u64) -> String {
    let component = read_u64(machine.engine, OBJECT + 56);
    let common = read_u64(machine.engine, component + 8);
    if common == 0 {
        return "[]".into();
    }
    write(machine.engine, VECTOR, &[0; 24]);
    machine.call(0x3e32f8, &[common, mask, VECTOR]);
    let begin = read_u64(machine.engine, VECTOR);
    let end = read_u64(machine.engine, VECTOR + 8);
    let capacity = read_u64(machine.engine, VECTOR + 16);
    assert!(end >= begin && capacity >= end && (end - begin) % 8 == 0 && end - begin <= 128);
    let spans = (begin..end)
        .step_by(8)
        .map(|pointer| {
            let span = read_u64(machine.engine, pointer);
            assert_eq!(machine.call(0x40c698, &[span]), kind);
            let start = machine.call(0x40c754, &[span]);
            let end = machine.call(0x40c810, &[span]);
            let interval = machine.call(0x40c8cc, &[span]);
            let value = match kind {
                3 => {
                    machine.call(0x409cf8, &[span]);
                    let size = read_register(machine.engine, 136) as u32;
                    format!("\"font_size_bits\":{size}")
                }
                1 => {
                    let color = machine.call(0x40a42c, &[span]);
                    let color_type = machine.call(0x40a484, &[span]);
                    format!("\"color_argb\":{color},\"color_type\":{color_type}")
                }
                _ => panic!("unsupported native style snapshot kind {kind}"),
            };
            format!("{{\"start\":{start},\"end\":{end},\"interval\":{interval},{value}}}")
        })
        .collect::<Vec<_>>();
    if begin != 0 {
        machine.call(0x47ac00, &[begin]);
    }
    write(machine.engine, VECTOR, &[0; 24]);
    format!("[{}]", spans.join(","))
}

fn source_snapshot(machine: &Machine) -> String {
    let source = string(machine.engine, machine.call(0x39c9e8, &[OBJECT + 40]));
    let common = read_u64(machine.engine, read_u64(machine.engine, OBJECT + 56) + 8);
    let format = if common == 0 {
        "null".into()
    } else {
        let version = machine.call(0x3e6410, &[common]);
        assert_eq!(version, 4000);
        version.to_string()
    };
    let color = machine.call(0x39f4ec, &[OBJECT + 40, 0]);
    let alignment = machine.call(0x39f548, &[OBJECT + 40]);
    let gravity = machine.call(0x39f824, &[OBJECT + 40]);
    let fonts = [0_u32, 1, 2].map(|position| {
        machine.call(0x39f504, &[OBJECT + 40, u64::from(position)]);
        read_register(machine.engine, 136) as u32
    });
    format!(
        "{{\"text\":{},\"common_present\":{},\"native_format_version\":{format},\"color_at_0_argb\":{color},\"alignment\":{alignment},\"gravity\":{gravity},\"font_size_at_0_1_2_bits\":{fonts:?},\"font_size_spans\":{},\"foreground_spans\":{}}}",
        source.as_deref().map_or_else(|| "null".into(), json_string),
        common != 0,
        style_spans(machine, 3, 8),
        style_spans(machine, 1, 2)
    )
}

fn sample(machine: &Machine, case: &Case) -> String {
    write(machine.engine, OBJECT, &[0; 240]);
    machine.call(0x3c17f0, &[OBJECT, 0, 0]);
    assert_eq!(machine.call(0x396e14, &[OBJECT]), 4);
    let constructed = source_snapshot(machine);
    let data = payload(case);
    write(machine.engine, INPUT, &data);
    write(machine.engine, INPUT_POINTER, &INPUT.to_le_bytes());
    write(
        machine.engine,
        REMAINING,
        &(data.len() as u32).to_le_bytes(),
    );
    register(machine.engine, 136, u64::from(1_f32.to_bits()));
    assert_eq!(
        machine.call(
            0x3b21d4,
            &[
                read_u64(machine.engine, OBJECT + 56),
                INPUT_POINTER,
                REMAINING,
                u64::from(case.common_present),
                FORMAT_VERSION,
                2,
                0,
                0,
            ]
        ) & 1,
        1
    );
    assert_eq!(
        read_u64(machine.engine, INPUT_POINTER),
        INPUT + data.len() as u64
    );
    assert_eq!(read_u32(machine.engine, REMAINING), 0);
    let parsed = source_snapshot(machine);
    widget_text_constructor::construct_cell(machine, CELL_LAYOUT, Default::default());
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
    machine.call(WIDGET + 0xd3974, &[CELL_LAYOUT, OBJECT]);
    machine.call(WIDGET + 0xd3e3c, &[CELL_LAYOUT]);
    let wrapper = widget_text_constructor::text_wrapper(machine, CELL_LAYOUT);
    machine.call(TEXT + 0x8b860, &[wrapper]);
    let default = read_register(machine.engine, 136) as u32;
    let caret = [0_u32, 1, 2].map(|position| {
        machine.call(TEXT + 0x8b354, &[wrapper, u64::from(position)]);
        read_register(machine.engine, 136) as u32
    });
    let copied = string(machine.engine, machine.call(TEXT + 0x8b970, &[wrapper]));
    register(machine.engine, 136, u64::from(400_f32.to_bits()));
    machine.call(WIDGET + 0xd398c, &[CELL_LAYOUT]);
    register(machine.engine, 136, u64::from(1000_f32.to_bits()));
    machine.call(WIDGET + 0xd399c, &[CELL_LAYOUT]);
    machine.call(
        WIDGET + 0xd73b0,
        &[CELL_LAYOUT, 0, u32::MAX as u64, u32::MAX as u64],
    );
    let rich = read_u64(machine.engine, wrapper + 64);
    let count = read_u32(machine.engine, rich + 104) as usize;
    assert!(count <= 64);
    let entries = read_u64(machine.engine, rich + 112);
    let sizes = (0..count)
        .map(|index| read_u32(machine.engine, entries + index as u64 * 80 + 60))
        .collect::<Vec<_>>();
    let native_text = copied.as_deref().map_or_else(|| "null".into(), json_string);
    format!(
        "{{\"name\":{},\"serialized_common_present\":{},\"serialized_payload_bytes\":{data:?},\"constructed\":{constructed},\"parsed\":{parsed},\"native_wrapper_text\":{native_text},\"wrapper_default_size_bits\":{default},\"wrapper_caret_0_1_2_bits\":{caret:?},\"measured_entry_font_size_bits\":{sizes:?}}}",
        json_string(case.name),
        case.common_present
    )
}

pub(crate) fn capture(machine: &mut Machine, paths: Paths<'_>) {
    let (mut environment, fonts) = text_span_font_name::setup_with_preloaded_libraries(
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
    let mut records = Vec::new();
    for case in cases() {
        let mut canonical = None;
        for fill in [0, 85, 165, 255, 0] {
            reset_host(machine, &mut environment, &mut host, fill);
            let record = sample(machine, &case);
            assert!(machine.heap.cursor < MODEL + 0x50000);
            if let Some(expected) = &canonical {
                assert_eq!(&record, expected, "case {} fill {fill}", case.name);
            } else {
                canonical = Some(record);
            }
        }
        records.push(canonical.unwrap());
    }
    println!(
        "{{\"dependencies\":{},\"allocation_fills\":[0,85,165,255],\"repeat_fill\":0,\"model_library_sha256\":{},\"base_library_sha256\":{},\"widget_library_sha256\":{},\"content_library_sha256\":{},\"drawing_library_sha256\":{},\"native_entrypoints\":{{\"cell_content_constructor\":\"Model:0x3c17f0\",\"apply_binary_text_data\":\"Model:0x3b21d4\",\"style_span_list\":\"Model:0x3e32f8\",\"component_font_size_at\":\"Model:0x39f504\",\"set_object\":\"Widget:0xd3974\",\"update\":\"Widget:0xd3e3c\",\"measure\":\"Widget:0xd73b0\"}},\"binary_inputs\":{{\"format_version\":{FORMAT_VERSION},\"document_type\":2,\"orientation\":0,\"caller_integer\":0,\"scale_bits\":1065353216}},\"capture_boundary\":{},\"cases\":[{}]}}",
        text_span_font_name::dependency_metadata(),
        json_string(LIBRARY_SHA256),
        json_string(frames::BASE_SHA256),
        json_string(geometry::WIDGET_SHA256),
        json_string(CONTENT_SHA256),
        json_string(DRAWING_SHA256),
        json_string(
            "Actual native type4 Model cell-content construction, ApplyBinary_TextData, TextCommon WDoc parser, native span factory/serialization readers, source span getters, Widget SetObject/Update and bounded true native font measurement execute. Serialized payloads and explicit parser ABI parameters are caller inputs. Native font-size and foreground spans are observed through Model getters; no source style or measured font size is supplied. Native GetFormatVersion reports the required format of the retained source, independently of the supplied parser version. The default device profile and identity context color service are supplied host boundaries; device theme resolution is not captured. Shared pinned NAME/HarfBuzz/Skia/FreeType and host ICU76 services execute under their existing capture boundaries. Actual parent table binary caller, complete document parsing/layout, object callback timing, clone drawing and final export do not execute."
        ),
        records.join(",")
    );
}
