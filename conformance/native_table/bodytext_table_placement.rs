use super::*;
const BODY: u64 = 0x0f00_0000;
const COMPOSER: u64 = 0x1200_0000;
const COMPOSER_HASH: &str = "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f";
const BODY_HASH: &str = "27324ca3807f07e0c1d0647b23eb9af1296762a8c9d892ee486f37b1eb9543f0";
const NATIVE_ENTRYPOINTS: &str = r#"{"document_constructor":"Model:0x3e1378","document_construct":"Model:0x3e1408","append_object_span":"Model:0x39d510","span_constructor":"Model:0x417440","span_set_object":"Model:0x417fc4","span_set_index":"Model:0x417598","span_set_constraint":"Model:0x417b38","document_update":"Widget:0xd3e3c","document_measure":"Widget:0xd73b0","document_layout":"Widget:0xd3b88","document_get_text_bound":"Text:0x8afd4","object_size_registration_window":["Bodytext:0xb01bc","Bodytext:0xb02dc"],"on_update_object_size":"Bodytext:0xb0b9c","object_size_output_observer":"Bodytext:0xb0f34","object_update_info":"Bodytext:0xf80e8","page_local_target_observer":"Bodytext:0xf82a4","native_parent_observer":"Bodytext:0xf82a8","placement_by_span_list":"Bodytext:0xd9618","placement_list":"Bodytext:0xd9320","model_cell_bridge":"Bodytext:0xd78ec","cell_rect_setter":"Model:0x3c20cc","table_raw_rect":"Model:0x2caa60","table_drawn_rect":"Model:0x3d48c4","cell_frame":"Drawing:0xab4bc","listener_registration_window":["Bodytext:0xb3228","Bodytext:0xb3314"],"view_callback_window":["Bodytext:0xd080c","Bodytext:0xd085c"],"notification":"Drawing:0xb1bf0","size_gate":"Bodytext:0xd76e0"}"#;
const DOC: u64 = MODEL + 0xd0000;
const DOC_WRAPPER: u64 = MODEL + 0xd1000;
const OWNER: u64 = MODEL + 0xd2000;
const VIEW: u64 = MODEL + 0xd3000;
const GROUP: u64 = MODEL + 0xd4000;
const SPAN: u64 = MODEL + 0xd5000;
const STORAGE: u64 = MODEL + 0xd6000;
const SETUP_STACK: u64 = MODEL + 0xd9000;
fn put(machine: &Machine, address: u64, value: u64) {
    write(machine.engine, address, &value.to_le_bytes());
}

fn setup_range(machine: &Machine, start: u64, end: u64) {
    register(machine.engine, REGISTER_SP, SETUP_STACK);
    check(unsafe {
        uc_emu_start(
            machine.engine,
            BODY + start,
            BODY + end,
            machine.call_timeout_micros,
            machine.call_instruction_limit,
        )
    });
    assert_eq!(read_register(machine.engine, 260), BODY + end);
}
#[derive(Default)]
struct NativeCalls {
    counts: BTreeMap<u64, u32>,
    page_local_drawn_targets: Vec<[u32; 4]>,
    native_parent_rects: Vec<[u32; 4]>,
    object_size_updates: Vec<String>,
    object_size_input: Option<(u32, u32, [u32; 4])>,
}
unsafe extern "C" fn native_call(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let calls = unsafe { &mut *data.cast::<NativeCalls>() };
    *calls.counts.entry(address).or_default() += 1;
    if address == BODY + 0xb0b9c {
        let rect = read_register(engine, REGISTER_X0 + 2);
        calls.object_size_input = Some((
            read_register(engine, REGISTER_X0 + 1) as u32,
            read_register(engine, 136) as u32,
            std::array::from_fn(|i| read_u32(engine, rect + i as u64 * 4)),
        ));
    }
    if address == BODY + 0xb0f34 {
        let (anchor, candidate_y, input) = calls
            .object_size_input
            .take()
            .expect("object callback input");
        let rect = read_register(engine, REGISTER_X0 + 20);
        let minimum = read_register(engine, REGISTER_X0 + 19);
        let output: [u32; 4] = std::array::from_fn(|i| read_u32(engine, rect + i as u64 * 4));
        calls.object_size_updates.push(format!("{{\"utf16_anchor\":{anchor},\"candidate_y_bits\":{candidate_y},\"input_rect_bits\":{input:?},\"output_rect_bits\":{output:?},\"minimum_height_bits\":{}}}",read_u32(engine,minimum)));
    }
    if address == BODY + 0xf82a4 {
        let target = read_register(engine, REGISTER_X0 + 1);
        calls
            .page_local_drawn_targets
            .push(std::array::from_fn(|i| {
                read_u32(engine, target + i as u64 * 4)
            }));
    }
    if address == BODY + 0xf82a8 {
        calls.native_parent_rects.push(std::array::from_fn(|i| {
            read_register(engine, 136 + i as i32) as u32
        }));
    }
}
#[derive(Clone, Copy)]
struct PlacementInput {
    name: &'static str,
    constraint: u32,
    page_origin: [f32; 2],
    moved_page_origin: [f32; 2],
    document_size: [f32; 2],
}
fn set_page_origin(machine: &Machine, origin: [f32; 2]) {
    for (i, value) in origin.into_iter().enumerate() {
        write(
            machine.engine,
            STORAGE + 0x2400 + 1696 + i as u64 * 4,
            &value.to_le_bytes(),
        );
    }
}
fn source_cells(machine: &Machine) -> String {
    let cells = (0..4)
        .map(|slot| {
            let cell = machine.call(0x3d2be0, &[TABLE, slot / 2, slot % 2]);
            let source = machine.call(0x3c22bc, &[cell]);
            let saved: [u32; 4] =
                std::array::from_fn(|i| read_u32(machine.engine, cell + 64 + i as u64 * 4));
            let content = returned_rect(machine, 0x2caa60, &[source]);
            let frame = returned_rect(machine, CELL_DRAWING + 0xab4bc, &[LAYOUT, cell]);
            format!(
                "{{\"slot\":{slot},\"saved_cell_rect_bits\":{saved:?},\"content_model_rect_bits\":{content:?},\"drawing_frame_bits\":{frame:?}}}"
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", cells.join(","))
}
fn observe_parent(machine: &Machine, input: PlacementInput) -> String {
    write(machine.engine, DOC, &vec![0; 0xb000]);
    let mut calls = Box::<NativeCalls>::default();
    let mut hooks = Vec::new();
    for address in [
        BODY + 0xb0b9c,
        BODY + 0xb0f34,
        CELL_DRAWING + 0xaa530,
        CELL_DRAWING + 0xaa3d4,
        TEXT + 0x779d0,
        BODY + 0xf80e8,
        BODY + 0xf82a4,
        BODY + 0xf82a8,
        BODY + 0xd9320,
        BODY + 0xd78ec,
        BODY + 0xb59d0,
        BODY + 0xdce8c,
        BODY + 0xd76e0,
        TEXT + 0x8afd4,
    ] {
        let mut h = 0;
        check(unsafe {
            uc_hook_add(
                machine.engine,
                &mut h,
                4,
                native_call as *mut c_void,
                ptr::from_mut(calls.as_mut()).cast(),
                address,
                address,
            )
        });
        hooks.push(h);
    }

    write(machine.engine, DOC, &[0; 256]);
    machine.call(0x3e1378, &[DOC]);
    let string = spen_string(machine, STORAGE, Some(""));
    assert_eq!(machine.call(0x3e1408, &[DOC, string, 0]) & 1, 1);
    float_arguments(
        machine,
        [0., 0., input.document_size[0], input.document_size[1]],
    );
    machine.call(0x397708, &[DOC, 0, 0]);
    machine.call(0x417440, &[SPAN]);
    assert_eq!(machine.call(0x417fc4, &[SPAN, TABLE]) & 1, 1);
    machine.call(0x417598, &[SPAN, 0]);
    assert_eq!(
        machine.call(0x417b38, &[SPAN, u64::from(input.constraint)]) & 1,
        1
    );
    assert_eq!(machine.call(0x417654, &[SPAN]), TABLE);
    assert_eq!(machine.call(0x39d510, &[DOC + 40, SPAN]) & 1, 1);

    write(machine.engine, DOC_WRAPPER, &[0; 624]);
    machine.call(WIDGET + 0xd2fb4, &[DOC_WRAPPER, CONTEXT, 0]);
    machine.call(WIDGET + 0xd3974, &[DOC_WRAPPER, DOC]);

    write(machine.engine, OWNER, &[0; 568]);
    machine.call(BODY + 0xaf948, &[OWNER, CONTEXT, 0]);
    put(machine, OWNER + 488, DOC_WRAPPER);
    let handle = machine.call(0x2cb0d0, &[TABLE]);
    let map = OWNER + 496;
    let buckets = STORAGE + 0x1000;
    let node = buckets + 0x100;
    for (address, value) in [
        (map, buckets),
        (map + 8, 1),
        (map + 16, node),
        (map + 24, 1),
        (buckets, map + 16),
        (node + 8, handle),
        (node + 24, LAYOUT),
    ] {
        put(machine, address, value);
    }
    write(machine.engine, node + 16, &(handle as u32).to_le_bytes());
    write(machine.engine, map + 32, &1f32.to_le_bytes());
    register(machine.engine, REGISTER_X0 + 19, OWNER);
    setup_range(machine, 0xb01bc, 0xb02dc);
    assert_ne!(read_u64(machine.engine, DOC_WRAPPER + 288), 0);
    machine.call(WIDGET + 0xd3e3c, &[DOC_WRAPPER]);
    float_arguments(machine, [input.document_size[0], 0., 0., 0.]);
    machine.call(WIDGET + 0xd398c, &[DOC_WRAPPER]);
    float_arguments(machine, [input.document_size[1], 0., 0., 0.]);
    machine.call(WIDGET + 0xd399c, &[DOC_WRAPPER]);
    assert_eq!(
        read_spen_string(machine.engine, machine.call(0x39c9e8, &[DOC + 40])).as_deref(),
        Some("\u{fffc}")
    );
    machine.call(
        WIDGET + 0xd73b0,
        &[DOC_WRAPPER, 0, u32::MAX as u64, u32::MAX as u64],
    );
    let wrapper = machine.call(WIDGET + 0xd39ac, &[DOC_WRAPPER]);
    assert_eq!(machine.call(TEXT + 0x8b104, &[wrapper]), 1);
    let rich = read_u64(machine.engine, wrapper + 64);

    machine.call(
        WIDGET + 0xd3b88,
        &[DOC_WRAPPER, 0, u32::MAX as u64, u32::MAX as u64, 0, 0],
    );
    let bound = returned_rect(machine, TEXT + 0x8afd4, &[wrapper, 0]);

    for (address, value) in [
        (GROUP + 1784, OWNER),
        (GROUP + 1792, STORAGE + 0x2000),
        (STORAGE + 0x2000, STORAGE + 0x2100),
        (STORAGE + 0x2100 + 32, STORAGE + 0x2200),
        (GROUP + 1848, STORAGE + 0x2300),
        (GROUP + 1856, STORAGE + 0x2310),
        (GROUP + 1864, STORAGE + 0x2310),
        (STORAGE + 0x2300, STORAGE + 0x2400),
    ] {
        put(machine, address, value);
    }
    write(
        machine.engine,
        STORAGE + 0x2200,
        &0x52800020u32.to_le_bytes(),
    );
    write(
        machine.engine,
        STORAGE + 0x2204,
        &0xd65f03c0u32.to_le_bytes(),
    );
    set_page_origin(machine, input.page_origin);
    let table_raw_before = returned_rect(machine, 0x2caa60, &[TABLE]);
    let table_drawn_before = returned_rect(machine, 0x3d48c4, &[TABLE]);
    register(machine.engine, REGISTER_X0 + 8, STORAGE + 0x3000);
    machine.call(BODY + 0xf80e8, &[GROUP, SPAN]);
    let info = read_u64(machine.engine, STORAGE + 0x3000);
    assert_ne!(info, 0);
    let info_rect: [u32; 4] =
        std::array::from_fn(|i| read_u32(machine.engine, info + i as u64 * 4));

    let writer_before = super::text_clipping::bodytext_writer(machine);
    let before = source_cells(machine);
    for (address, value) in [
        (VIEW + 1000, STORAGE + 0x2000),
        (VIEW + 1008, DOC),
        (VIEW + 1048, OWNER),
        (VIEW + 1056, GROUP),
        (STORAGE + 0x4000, STORAGE + 0x4100),
        (STORAGE + 0x4008, STORAGE + 0x4108),
        (STORAGE + 0x4010, STORAGE + 0x4108),
        (STORAGE + 0x4100, SPAN),
    ] {
        put(machine, address, value);
    }
    machine.call(BODY + 0xd9618, &[VIEW, STORAGE + 0x4000, 0]);
    let after = source_cells(machine);
    assert_ne!(before, after);

    assert_eq!(read_u64(machine.engine, LAYOUT + 496), LAYOUT + 504);
    assert_eq!(read_u64(machine.engine, LAYOUT + 512), 0);
    put(machine, SETUP_STACK, LAYOUT);
    register(machine.engine, REGISTER_X0 + 20, OWNER);
    setup_range(machine, 0xb3228, 0xb3314);
    assert_eq!(read_u64(machine.engine, LAYOUT + 512), 1);
    register(machine.engine, REGISTER_X0 + 19, VIEW);
    setup_range(machine, 0xd080c, 0xd085c);
    assert_ne!(read_u64(machine.engine, OWNER + 272), 0);
    machine.call(CELL_DRAWING + 0xb1bf0, &[LAYOUT]);
    assert_eq!(calls.counts.get(&(BODY + 0xb59d0)), Some(&1));
    assert_eq!(calls.counts.get(&(BODY + 0xdce8c)), Some(&1));
    assert_eq!(calls.counts.get(&(BODY + 0xd76e0)), Some(&1));
    let writer_after = super::text_clipping::bodytext_writer(machine);
    set_page_origin(machine, input.moved_page_origin);
    machine.call(CELL_DRAWING + 0xb1bf0, &[LAYOUT]);
    let after_origin_notification = source_cells(machine);
    assert_eq!(after, after_origin_notification);
    let bridge_count_before_move = calls.counts.get(&(BODY + 0xd78ec)).copied().unwrap_or(0);
    assert_eq!(bridge_count_before_move, 1);
    let table_raw_before_move = returned_rect(machine, 0x2caa60, &[TABLE]);
    let table_drawn_before_move = returned_rect(machine, 0x3d48c4, &[TABLE]);
    machine.call(BODY + 0xd9618, &[VIEW, STORAGE + 0x4000, 0]);
    let after_origin_placement = source_cells(machine);
    assert_ne!(after, after_origin_placement);
    let writer_after_origin_placement = super::text_clipping::bodytext_writer(machine);
    assert_eq!(calls.counts.get(&(BODY + 0xb0b9c)), Some(&1));
    assert_eq!(calls.object_size_updates.len(), 1);
    assert!(calls.object_size_input.is_none());
    assert_eq!(calls.counts.get(&(BODY + 0xd78ec)), Some(&2));
    assert_eq!(calls.counts.get(&(BODY + 0xd9320)), Some(&2));
    let native_calls = calls
        .counts
        .iter()
        .map(|(a, count)| format!("\"0x{a:x}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    for h in hooks {
        check(unsafe { uc_hook_del(machine.engine, h) });
    }

    format!(
        "{{\"supplied_layout_constraint\":{},\"supplied_document_size_bits\":{:?},\"supplied_page_origin_bits\":{:?},\"supplied_moved_page_origin_bits\":{:?},\"native_calls\":{{{native_calls}}},\"object_size_updates\":[{}],\"document_bound_bits\":{bound:?},\"page_local_drawn_target_bits\":{:?},\"native_parent_rects_bits\":{:?},\"table_raw_before_bits\":{table_raw_before:?},\"table_drawn_before_bits\":{table_drawn_before:?},\"table_raw_before_move_bits\":{table_raw_before_move:?},\"table_drawn_before_move_bits\":{table_drawn_before_move:?},\"native_parent_rect_bits\":{info_rect:?},\"source_before\":{before},\"source_after\":{after},\"source_after_page_origin_notification\":{after_origin_notification},\"source_after_page_origin_placement\":{after_origin_placement},\"writer_before\":{writer_before},\"writer_after\":{writer_after},\"writer_after_page_origin_placement\":{writer_after_origin_placement},\"entries\":{}}}",
        input.constraint,
        input.document_size.map(f32::to_bits),
        input.page_origin.map(f32::to_bits),
        input.moved_page_origin.map(f32::to_bits),
        calls.object_size_updates.join(","),
        calls.page_local_drawn_targets,
        calls.native_parent_rects,
        cell_measurement::snapshot(machine, rich)
    )
}
pub(super) fn capture(
    machine: &mut Machine,
    paths: widget_text_constructor::Paths<'_>,
    body: &Path,
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
            (body, BODY, BODY_HASH),
            (composer, COMPOSER, COMPOSER_HASH),
        ],
        &[(paths.model, 0, LIBRARY_SHA256)],
    );
    let mut host = Box::new(Host {
        cell: install_host(machine, &mut environment, fonts),
        math_calls: BTreeMap::new(),
    });
    environment.set_host_import_handler(host_import, ptr::from_mut(host.as_mut()).cast());
    let mut observation = Box::<Trace>::default();
    let mut initial_state_hooks = Vec::new();
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
        initial_state_hooks.push(hook);
    }
    let mut output = Vec::new();
    for input in [
        PlacementInput {
            name: "overpages-nonzero-origin",
            constraint: 2,
            page_origin: [11.25, -7.5],
            moved_page_origin: [24.25, 9.5],
            document_size: [400., 1000.],
        },
        PlacementInput {
            name: "overpages-zero-origin",
            constraint: 2,
            page_origin: [0., 0.],
            moved_page_origin: [-19.5, 31.125],
            document_size: [240., 600.],
        },
        PlacementInput {
            name: "overpages-overlap-padding",
            constraint: 1,
            page_origin: [-5.25, 17.125],
            moved_page_origin: [8.75, -9.5],
            document_size: [320., 800.],
        },
    ] {
        let case = Case {
            name: input.name,
            bounds: [0., 0., 160., 8.],
            texts: ["AV\nTo\nlast", "To", "ÁB", "last"],
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
            eprintln!("case {} allocation fill {fill}", input.name);
            let observer = |m: &Machine| observe_parent(m, input);
            let result = case.sample_observed(
                machine,
                &mut environment,
                &mut host,
                &mut observation,
                fill,
                Some(&observer),
            );
            if let Some(expected) = &canonical {
                assert_eq!(&result, expected, "native fill {fill} diverged");
            } else {
                canonical = Some(result);
            }
        }
        output.push(canonical.unwrap());
    }
    for hook in initial_state_hooks {
        check(unsafe { uc_hook_del(machine.engine, hook) });
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"dependencies\":{},\"memory_fills\":[0,85,165,255],\"repeat_zero_fill\":true,\"model_library_sha256\":{},\"base_library_sha256\":{},\"text_library_sha256\":{},\"skia_library_sha256\":{},\"widget_library_sha256\":{},\"content_library_sha256\":{},\"drawing_library_sha256\":{},\"bodytext_library_sha256\":{},\"composer_library_sha256\":{},\"native_entrypoints\":{},\"capture_boundary\":{},\"cases\":[{}]}}",
        dependency_metadata(),
        json_string(LIBRARY_SHA256),
        json_string(frames::BASE_SHA256),
        json_string(geometry::TEXT_SHA256),
        json_string(text_font_source::SKIA_SHA256),
        json_string(geometry::WIDGET_SHA256),
        json_string(CONTENT_SHA256),
        json_string(DRAWING_SHA256),
        json_string(BODY_HASH),
        json_string(COMPOSER_HASH),
        NATIVE_ENTRYPOINTS,
        json_string(
            "Actual Model document ObjectTextBox+ObjectSpan AppendObjectSpan inserts U+FFFC; actual Widget conversion, measurement and placement produce real document entries and GetTextBound; bounded Bodytext callback registration [0xb01bc,0xb02dc) installs genuine onUpdateObjectSize. Cold native table Measure precedes actual document feedback TableLayout.Layout. Native GetObjectUpdateInfo reads document bound, subtracts supplied page origin and applies actual GetRectByDrawnRect; complete updateObjectSpanRectBySpanList/model bridge derives saved cell/content rectangles from actual table cell frames. Native listener [0xb3228,0xb3314) and view callback [0xd080c,0xd085c) setup windows precede complete Drawing notifications with actual document GetTextBound. Page-origin-only controls preserve cells on size notification, then complete placement updates them. Actual native clone/copy, second TableLayout and cached writer execute before/after source callbacks, without supplied table/cell source setters or host object dimensions/entries. Caller supplies explicit document shape and Widget dimensions, source table input, context/device, zeroed caller owner/view associations and one genuine table layout in owner map; finite one-page infinite-scroll interface and page origins are supplied, obstacle/history/cache services null. Owned allocations vary fills; supplied caller storage zeroed every sample. Parser, full SetBodyTextDocument, full newObjectLayout/initBodyTextView, actual page-bound production, device ICU and final PDF backend excluded. Writer boundary matches live_table_text_clipping: caller explicitly selects all four raw physical slots; aggregate visible-cell selection is excluded. Actual clone writer measured-rounding setup is bounded to [0x37e4ac,0x37e4f8), followed by complete per-cell writeTextContent/writeTextBlock. PDF/Paint methods are recording interfaces; sibling bitmap/background/decoration exports are skipped."
        ),
        output.join(",")
    );
}
