use super::*;

const EVENT: u64 = MODEL + 0x76000;
const NATIVE_ENTRYPOINTS: &str = r#"{"cell_width_producer":"Drawing:0x8c13c","cell_measured_width":"Drawing:0x8c070","cell_optional_minimum":"Drawing:0x8c078","cell_measure":"Drawing:0xaea60","source_changed":"Drawing:0xb1848","native_column_setter_observer":"Drawing:0xb1984","native_resize_column":"Drawing:0xb0088","source_set_text":"Model:0x39c9d4","set_spannable_over_pages":"Model:0x2d21f4","model_column_setter":"Model:0x3d3b64","model_column_getter":"Model:0x3d3f24","model_valid_column_width":"Model:0x3c8420"}"#;
const HOOKS: [u64; 10] = [
    0x8c13c, 0x8c1f4, 0x8c200, 0x8c228, 0x8c238, 0x8c264, 0xb1848, 0xb1984, 0xb0088, 0xaea60,
];

#[derive(Default)]
struct ProducerCall {
    child: u64,
    configured: u32,
    before_optional: Option<u32>,
    lines: Vec<([u32; 4], u32, u32, u32)>,
    margins: Option<[u32; 2]>,
    returned: Option<u32>,
    after_optional: Option<u32>,
}

#[derive(Default)]
struct ColumnTrace {
    pending: Option<ProducerCall>,
    producers: Vec<ProducerCall>,
    calls: BTreeMap<u64, u32>,
    setter_inputs: Vec<[u32; 2]>,
}

fn optional_width(engine: Engine, child: u64) -> Option<u32> {
    (bytes(engine, child + 624, 1)[0] != 0).then(|| read_u32(engine, child + 620))
}

unsafe extern "C" fn observe(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let trace = unsafe { &mut *data.cast::<ColumnTrace>() };
    let offset = address - CELL_DRAWING;
    *trace.calls.entry(offset).or_default() += 1;
    match offset {
        0x8c13c => {
            assert!(trace.pending.is_none());
            let child = read_register(engine, REGISTER_X0);
            trace.pending = Some(ProducerCall {
                child,
                configured: read_u32(engine, child + 524),
                before_optional: optional_width(engine, child),
                ..Default::default()
            });
        }
        0x8c1f4 => {
            let stack = read_register(engine, REGISTER_SP);
            let paragraph =
                std::array::from_fn(|axis| read_u32(engine, stack + 48 + axis as u64 * 4));
            trace.pending.as_mut().unwrap().lines.push((
                paragraph,
                read_register(engine, 145) as u32,
                read_register(engine, 136) as u32,
                read_u32(engine, stack + 72),
            ));
        }
        0x8c200 => {
            let (_, _, _, indent) = trace.pending.as_mut().unwrap().lines.last_mut().unwrap();
            assert_eq!(*indent, read_register(engine, 137) as u32);
        }
        0x8c228 => {
            trace.pending.as_mut().unwrap().margins = Some([read_register(engine, 136) as u32, 0]);
        }
        0x8c238 => {
            trace.pending.as_mut().unwrap().margins.as_mut().unwrap()[1] =
                read_register(engine, 136) as u32;
        }
        0x8c264 => {
            let mut producer = trace.pending.take().unwrap();
            producer.returned = Some(read_register(engine, 144) as u32);
            producer.after_optional = optional_width(engine, producer.child);
            trace.producers.push(producer);
        }
        0xb1984 => trace.setter_inputs.push([
            read_register(engine, REGISTER_X0 + 1) as u32,
            read_register(engine, 136) as u32,
        ]),
        0xb1848 | 0xb0088 | 0xaea60 => {}
        _ => unreachable!(),
    }
}

fn width_state(machine: &Machine, stage: &str) -> String {
    let cells = (0..4).map(|slot| {
        let cell = machine.call(0x3d2be0, &[TABLE, slot / 2, slot % 2]);
        let child = machine.call(CELL_DRAWING + 0xacb84, &[LAYOUT, cell]);
        let frame = returned_rect(machine, CELL_DRAWING + 0xab4bc, &[LAYOUT, cell]);
        let source = machine.call(0x3c22bc, &[cell]);
        let text = read_spen_string(machine.engine,machine.call(0x39c9e8,&[source+40]));
        let minimum = optional_width(machine.engine, child).map_or_else(||"null".to_owned(),|bits|bits.to_string());
        format!("{{\"slot\":{slot},\"text_utf8\":{},\"frame_bits\":{frame:?},\"configured_width_bits\":{},\"measured_width_bits\":{},\"optional_minimum_bits\":{minimum}}}",optional_json(text.as_deref()),read_u32(machine.engine,child+524),read_u32(machine.engine,child+400))
    }).collect::<Vec<_>>();
    let columns = (0..2)
        .map(|column| {
            machine.call(0x3d3f24, &[TABLE, column]);
            read_register(machine.engine, 136) as u32
        })
        .collect::<Vec<_>>();
    format!(
        "{{\"stage\":{},\"model_column_width_bits\":{columns:?},\"cells\":[{}]}}",
        json_string(stage),
        cells.join(",")
    )
}

#[derive(Clone, Copy)]
struct SourceEdit {
    slot: u64,
    text: &'static str,
}

fn edit_source(machine: &Machine, input: SourceEdit) -> String {
    let cell = machine.call(0x3d2be0, &[TABLE, input.slot / 2, input.slot % 2]);
    let source = machine.call(0x3c22bc, &[cell]);
    let old = read_spen_string(machine.engine, machine.call(0x39c9e8, &[source + 40])).unwrap();
    let string = spen_string(machine, INPUT + 0x100, Some(input.text));
    assert_eq!(machine.call(0x39c9d4, &[source + 40, string]) & 1, 1);
    write(machine.engine, EVENT, &[0; 160]);
    write(machine.engine, EVENT + 8, &source.to_le_bytes());
    write(machine.engine, EVENT + 120, &1_u32.to_le_bytes());
    write(machine.engine, EVENT + 132, &0_u32.to_le_bytes());
    write(
        machine.engine,
        EVENT + 136,
        &(old.encode_utf16().count() as u32).to_le_bytes(),
    );
    write(
        machine.engine,
        EVENT + 140,
        &(input.text.encode_utf16().count() as u32).to_le_bytes(),
    );
    machine.call(0x2d21f4, &[TABLE, 1]);
    assert_eq!(machine.call(0x2d2208, &[TABLE]) & 1, 1);
    machine.call(CELL_DRAWING + 0xb1848, &[LAYOUT, EVENT, 0]);
    format!(
        "{{\"slot\":{},\"old_text_utf8\":{},\"new_text_utf8\":{},\"caller_event_type\":1,\"caller_start_utf16\":0,\"caller_removed_utf16\":{},\"caller_added_utf16\":{},\"native_after_callback\":{}}}",
        input.slot,
        json_string(&old),
        json_string(input.text),
        old.encode_utf16().count(),
        input.text.encode_utf16().count(),
        width_state(machine, "source-edit-callback")
    )
}

impl ColumnTrace {
    fn json(&mut self, machine: &Machine) -> String {
        assert!(self.pending.is_none());
        let children = (0..4)
            .map(|slot| {
                let cell = machine.call(0x3d2be0, &[TABLE, slot / 2, slot % 2]);
                machine.call(CELL_DRAWING + 0xacb84, &[LAYOUT, cell])
            })
            .collect::<Vec<_>>();
        let producers = self.producers.iter().map(|producer| {
            let slot = children.iter().position(|child|*child==producer.child).unwrap();
            let lines = producer.lines.iter().map(|(paragraph,width,paragraph_width,indent)|format!("{{\"line_width_bits\":{width},\"paragraph_rect_bits\":{paragraph:?},\"paragraph_width_bits\":{paragraph_width},\"indent_bits\":{indent}}}")).collect::<Vec<_>>();
            let optional = |width:Option<u32>|width.map_or_else(||"null".to_owned(),|bits|bits.to_string());
            let margins = producer.margins.map_or_else(||"null".to_owned(),|bits|format!("{bits:?}"));
            format!("{{\"slot\":{slot},\"configured_width_bits\":{},\"optional_before_bits\":{},\"lines\":[{}],\"margin_left_right_bits\":{margins},\"returned_width_bits\":{},\"optional_after_bits\":{}}}",producer.configured,optional(producer.before_optional),lines.join(","),producer.returned.unwrap(),optional(producer.after_optional))
        }).collect::<Vec<_>>();
        let counts = self
            .calls
            .iter()
            .map(|(offset, count)| format!("\"Drawing:0x{offset:x}\":{count}"))
            .collect::<Vec<_>>();
        format!(
            "{{\"calls\":{{{}}},\"set_column_width_inputs\":{:?},\"producer_calls\":[{}]}}",
            counts.join(","),
            self.setter_inputs,
            producers.join(",")
        )
    }
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
    let mut observation = Box::<Trace>::default();
    let mut hooks = Vec::new();
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
        hooks.push(hook);
    }
    let mut columns = Box::<ColumnTrace>::default();
    for offset in HOOKS {
        let mut hook = 0;
        check(unsafe {
            uc_hook_add(
                machine.engine,
                &mut hook,
                4,
                observe as *mut c_void,
                ptr::from_mut(columns.as_mut()).cast(),
                CELL_DRAWING + offset,
                CELL_DRAWING + offset,
            )
        });
        hooks.push(hook);
    }
    let base = Case {
        name: "ordinary",
        bounds: [13.25, -19.5, 173.25, 180.5],
        texts: ["AV abc", "To", "AB", "last"],
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
        (base, None),
        (
            Case {
                name: "empty-source",
                texts: [""; 4],
                ..base
            },
            None,
        ),
        (
            Case {
                name: "fractional-width",
                bounds: [13.25, -19.5, 175.05, 180.5],
                ..base
            },
            None,
        ),
        (
            Case {
                name: "asymmetric-margins",
                margins: Some([1.25, 2.5, 3.75, 4.5]),
                ..base
            },
            None,
        ),
        (
            Case {
                name: "font50",
                font_size: None,
                ..base
            },
            None,
        ),
        (
            Case {
                name: "merged-covered-width",
                merge: Some([0, 0, 1, 0]),
                ..base
            },
            None,
        ),
        (
            Case {
                name: "warm-column-growth",
                resize: Some(120.),
                ..base
            },
            None,
        ),
        (
            Case {
                name: "warm-column-shrink",
                resize: Some(40.),
                ..base
            },
            None,
        ),
        (
            Case {
                name: "edit-empty-to-wide-glyph",
                bounds: [0., 0., 20., 200.],
                texts: [""; 4],
                ..base
            },
            Some(SourceEdit { slot: 0, text: "W" }),
        ),
        (
            Case {
                name: "edit-wide-to-narrow",
                texts: ["WWWW", "", "", ""],
                ..base
            },
            Some(SourceEdit { slot: 0, text: "i" }),
        ),
    ];
    let mut output = Vec::new();
    for (case, edit) in cases {
        let mut canonical = None;
        for fill in [0, 85, 165, 255, 0] {
            *columns = ColumnTrace::default();
            let observer = |machine: &Machine| {
                let before = width_state(machine, "before-source-edit");
                let event =
                    edit.map_or_else(|| "null".to_owned(), |input| edit_source(machine, input));
                format!("{{\"before\":{before},\"source_edit\":{event}}}")
            };
            let mut sample = case.sample_observed(
                machine,
                &mut environment,
                &mut host,
                &mut observation,
                fill,
                Some(&observer),
            );
            let producer = columns.json(machine);
            assert_eq!(sample.pop(), Some('}'));
            sample.push_str(&format!(",\"width_production\":{producer}}}"));
            if let Some(expected) = &canonical {
                assert_eq!(&sample, expected, "{} fill {fill}", case.name)
            } else {
                canonical = Some(sample)
            }
        }
        output.push(canonical.unwrap());
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"dependencies\":{},\"memory_fills\":[0,85,165,255],\"repeat_zero_fill\":true,\"model_library_sha256\":{},\"base_library_sha256\":{},\"text_library_sha256\":{},\"skia_library_sha256\":{},\"widget_library_sha256\":{},\"content_library_sha256\":{},\"drawing_library_sha256\":{},\"native_entrypoints\":{NATIVE_ENTRYPOINTS},\"capture_boundary\":{},\"cases\":[{}]}}",
        dependency_metadata(),
        json_string(LIBRARY_SHA256),
        json_string(frames::BASE_SHA256),
        json_string(geometry::TEXT_SHA256),
        json_string(text_font_source::SKIA_SHA256),
        json_string(geometry::WIDGET_SHA256),
        json_string(CONTENT_SHA256),
        json_string(DRAWING_SHA256),
        json_string(
            "Complete genuine native cold/warm cell measurement and optional width production execute on caller-controlled table source and pinned fonts through the shared live-table harness. Passive instruction hooks record getCellLayoutWidth configured width, per-line paragraph rectangle/width/indent operands, margin operands, return width and optional cache before/after; hooks do not write guest state or invoke nested native calls. Source edit controls use native SetText and SetSpannableOverPages, then caller-supplied ObjectChangedTextInfo full-replacement fields execute complete native OnCellChanged, Widget source update/Measure and any native column setter/resize. Native event construction, document history/listener dispatch and editor drag control do not execute. Caller supplies only source bounds/text/font/margin controls, optional requested resize width and source-edit event range/type; no measured widths, cached optional values, shaped entries, frame geometry or callback dimensions are supplied. Host ICU76.1/Unicode16, allocation/files/libc/single-thread services, deterministic UUIDs, pow/acosf and disabled ATrace remain explicit boundaries. Saved parser/cloning, Bodytext placement, pagination-band production, native device ICU, final vector transport and pixels do not execute."
        ),
        output.join(",")
    );
    for hook in hooks {
        check(unsafe { uc_hook_del(machine.engine, hook) });
    }
}
