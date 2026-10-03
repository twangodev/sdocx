use super::*;
use std::cell::RefCell;

const OBJECT: u64 = MODEL + 0x76000;
const SPAN: u64 = MODEL + 0x76200;
const RECT: u64 = MODEL + 0x76300;

const OBJECT_ENTRIES: [(&str, u64); 6] = [
    ("image_construct", 0x420450),
    ("image_set_rect", 0x397708),
    ("cell_append_object_span", 0x3c26e8),
    ("widget_convert_object_span", WIDGET + 0xd53e4),
    ("widget_object_span_size_changed", WIDGET + 0xd4c90),
    ("table_object_span_size_changed", CELL_DRAWING + 0xb0d6c),
];

unsafe extern "C" fn observe_object(_: Engine, address: u64, _: u32, data: *mut c_void) {
    let trace = unsafe { &*data.cast::<RefCell<Trace>>() };
    let name = OBJECT_ENTRIES
        .iter()
        .find(|(_, target)| *target == address)
        .unwrap()
        .0;
    *trace.borrow_mut().calls.entry(name).or_default() += 1;
}

#[derive(Clone, Copy)]
struct Input {
    name: &'static str,
    object_type: u32,
    bounds: [f32; 4],
    option: u32,
    constraint: u32,
    resize: Option<[f32; 4]>,
}

fn object_rect(machine: &Machine) -> [u32; 4] {
    let getter = read_u64(machine.engine, read_u64(machine.engine, OBJECT) + 160);
    returned_rect(machine, getter, &[OBJECT])
}

fn insert(machine: &Machine, input: Input) -> String {
    write(machine.engine, OBJECT, &[0; 256]);
    match input.object_type {
        3 => {
            machine.call(0x420328, &[OBJECT]);
            assert_eq!(machine.call(0x420450, &[OBJECT, 1]) & 1, 1);
            float_arguments(machine, input.bounds);
            assert_eq!(machine.call(0x397708, &[OBJECT, 0, 0]) & 1, 1);
        }
        22 => {
            machine.call(0x3d2690, &[OBJECT]);
            for (axis, value) in input.bounds.into_iter().enumerate() {
                write(machine.engine, RECT + axis as u64 * 4, &value.to_le_bytes());
            }
            assert_eq!(machine.call(0x3d27d8, &[OBJECT, RECT, 2, 2]) & 1, 1);
        }
        23 => {
            machine.call(0x4741b8, &[OBJECT]);
            assert_eq!(machine.call(0x474258, &[OBJECT]) & 1, 1);
        }
        _ => unreachable!(),
    }
    assert_eq!(
        machine.call(0x2c9fec, &[OBJECT]),
        u64::from(input.object_type)
    );
    write(machine.engine, SPAN, &[0; 64]);
    machine.call(0x417440, &[SPAN]);
    assert_eq!(machine.call(0x417fc4, &[SPAN, OBJECT]) & 1, 1);
    machine.call(0x417598, &[SPAN, 2]);
    assert_eq!(
        machine.call(0x417afc, &[SPAN, u64::from(input.option)]) & 1,
        1
    );
    assert_eq!(
        machine.call(0x417b38, &[SPAN, u64::from(input.constraint)]) & 1,
        1
    );
    let cell = machine.call(0x3d2be0, &[TABLE, 0, 0]);
    let source = machine.call(0x3c22bc, &[cell]);
    let count_before = machine.call(0x39d630, &[source + 40]);
    let text_before = read_spen_string(machine.engine, machine.call(0x39c9e8, &[source + 40]));
    let accepted = machine.call(0x3c26e8, &[cell, SPAN]) & 1;
    assert_eq!(accepted, u64::from(input.object_type == 3));
    let count_after = machine.call(0x39d630, &[source + 40]);
    let text_after = read_spen_string(machine.engine, machine.call(0x39c9e8, &[source + 40]));
    assert_eq!(count_after, count_before + accepted);
    if accepted == 0 {
        assert_eq!(text_before, text_after);
    } else {
        assert_eq!(text_after.as_deref(), Some("AV\u{fffc}"));
    }
    format!(
        "{{\"object_type\":{},\"source_bounds_bits\":{},\"layout_option\":{},\"layout_constraint\":{},\"source_resize_bounds_bits\":{},\"public_cell_append_accepted\":{},\"object_count_before\":{count_before},\"object_count_after\":{count_after},\"text_before\":{},\"text_after\":{},\"native_drawn_rect_bits\":{:?}}}",
        input.object_type,
        if input.object_type == 23 {
            "null".to_owned()
        } else {
            format!("{:?}", input.bounds.map(f32::to_bits))
        },
        input.option,
        input.constraint,
        input.resize.map_or_else(
            || "null".to_owned(),
            |rect| format!("{:?}", rect.map(f32::to_bits))
        ),
        accepted != 0,
        optional_json(text_before.as_deref()),
        optional_json(text_after.as_deref()),
        object_rect(machine),
    )
}

fn after_warm(machine: &Machine, input: Input, trace: &RefCell<Trace>) -> String {
    let before = object_rect(machine);
    let setup_calls = trace.borrow_mut().take();
    let resized = input.resize.map(|rect| {
        float_arguments(machine, rect);
        assert_eq!(machine.call(0x397708, &[OBJECT, 0, 0]) & 1, 1);
        let cell = machine.call(0x3d2be0, &[TABLE, 0, 0]);
        machine.call(CELL_DRAWING + 0xb0d6c, &[LAYOUT, cell, SPAN]);
        snapshot(
            machine,
            "image-size-notification",
            trace.borrow_mut().take(),
        )
    });
    format!(
        "{{\"object_producer_calls\":{setup_calls},\"native_drawn_rect_before_bits\":{before:?},\"native_drawn_rect_after_bits\":{:?},\"after_actual_size_notification\":{}}}",
        object_rect(machine),
        resized.unwrap_or_else(|| "null".to_owned()),
    )
}

pub(super) fn capture(machine: &mut Machine, paths: widget_text_constructor::Paths<'_>) {
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
    CodeObserver::with(machine, Trace::default(), |machine, observation| {
        observation.observe(trace, ENTRIES.map(|(_, address)| address));
        CodeObserver::with(
            machine,
            RefCell::new(Trace::default()),
            |machine, object_trace| {
                object_trace.observe(observe_object, OBJECT_ENTRIES.map(|(_, address)| address));
                let base = Input {
                    name: "inline-image-normal",
                    object_type: 3,
                    bounds: [7.25, -3.5, 37.75, 38.75],
                    option: 1,
                    constraint: 0,
                    resize: None,
                };
                let cases = [
                    base,
                    Input {
                        name: "inline-image-over-pages",
                        constraint: 1,
                        ..base
                    },
                    Input {
                        name: "block-image-small-margin",
                        option: 2,
                        constraint: 1,
                        ..base
                    },
                    Input {
                        name: "block-image-medium-margin",
                        option: 3,
                        constraint: 2,
                        ..base
                    },
                    Input {
                        name: "inline-wide-image",
                        bounds: [0., 0., 160., 25.],
                        ..base
                    },
                    Input {
                        name: "inline-tall-image-resize",
                        bounds: [0., 0., 30., 150.25],
                        resize: Some([0., 0., 55.5, 205.75]),
                        ..base
                    },
                    Input {
                        name: "public-cell-rejects-table",
                        object_type: 22,
                        ..base
                    },
                    Input {
                        name: "public-cell-rejects-code",
                        object_type: 23,
                        ..base
                    },
                ];
                let output = cases
                    .into_iter()
                    .map(|input| {
                        let case = Case {
                            name: input.name,
                            bounds: [13.25, -19.5, 173.25, 180.5],
                            texts: ["AV", "To", "last", "ÁB"],
                            merge: None,
                            font_size: Some(17.),
                            margins: None,
                            scale: 1.,
                            density: 1.,
                            direction: 0,
                            bands: &[],
                            resize: None,
                        };
                        let mut canonical = None;
                        for fill in [0, 85, 165, 255, 0] {
                            object_trace.borrow_mut().take();
                            let sample = case.sample_customized(
                                machine,
                                &mut environment,
                                &mut host,
                                observation,
                                fill,
                                Some(&|machine| insert(machine, input)),
                                Some(&|machine| after_warm(machine, input, &object_trace)),
                            );
                            if let Some(expected) = &canonical {
                                assert_eq!(&sample, expected, "{} fill {fill}", input.name);
                            } else {
                                canonical = Some(sample);
                            }
                        }
                        canonical.unwrap()
                    })
                    .collect::<Vec<_>>();
                println!(
                    "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"dependencies\":{},\"memory_fills\":[0,85,165,255],\"repeat_zero_fill\":true,\"model_library_sha256\":{},\"base_library_sha256\":{},\"text_library_sha256\":{},\"skia_library_sha256\":{},\"widget_library_sha256\":{},\"content_library_sha256\":{},\"drawing_library_sha256\":{},\"native_entrypoints\":{{\"image_constructor\":\"Model:0x420328\",\"image_construct\":\"Model:0x420450\",\"image_source_rect_setter\":\"Model:0x397708\",\"cell_append_object_span\":\"Model:0x3c26e8\",\"size_notification\":\"Drawing:0xb0d6c\"}},\"capture_boundary\":{},\"cases\":[{}]}}",
                    dependency_metadata(),
                    json_string(LIBRARY_SHA256),
                    json_string(frames::BASE_SHA256),
                    json_string(geometry::TEXT_SHA256),
                    json_string(text_font_source::SKIA_SHA256),
                    json_string(geometry::WIDGET_SHA256),
                    json_string(CONTENT_SHA256),
                    json_string(DRAWING_SHA256),
                    json_string(
                        "Complete native Model table/cell/source construction and genuine Image constructor/Construct/SetRect, ObjectSpan constructor/setters/public TableCell AppendObjectSpan, actual TableLayout cold Measure/warm Layout and Widget source conversion/shaping/placement execute. Source image bounds and layout options/constraints are caller inputs; no measured object dimensions or callbacks are supplied. One resize invokes actual image source SetRect then native TableLayout.onObjectSpanChanged, retaining actual warm cache. Genuine public cell insertion rejects constructed Table22/Code23 without source mutation; this does not establish serialized parser rejection. Images have nil media/bitmap data; capture proves source geometry, not decoded pixels or media loading. Existing host font/ICU, allocation, libc/device interfaces and default width1000/density1 boundaries apply; child update-size callbacks remain actual constructor state. No Bodytext/document placement, native raster rendering or final writer executes."
                    ),
                    output.join(","),
                );
            },
        );
    });
}
