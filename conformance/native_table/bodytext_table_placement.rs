use super::*;
#[path = "bodytext_one_page_text.rs"]
mod one_page_text;
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
const BODY_DOCUMENT: u64 = STORAGE + 0x5000;
const PAGE_RECORD: u64 = STORAGE + 0x5100;
const PAGE_LIST: u64 = STORAGE + 0x5200;
const GROUP_RANGES: u64 = STORAGE + 0x5300;
const PAGE_MANAGER: u64 = STORAGE + 0x5400;
const PAGE_OBSTACLE: u64 = STORAGE + 0x5700;
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
    model_state_transitions: Vec<String>,
    split_band_inputs: Vec<Vec<[u32; 4]>>,
}

fn stored_table_state(engine: Engine, implementation: u64, stage: &str, input: &[u32]) -> String {
    let data = read_u64(engine, read_u64(engine, TABLE + 16) + 24);
    let raw: [u32; 4] = std::array::from_fn(|i| read_u32(engine, data + 8 + i as u64 * 4));
    let content: [u32; 4] =
        std::array::from_fn(|i| read_u32(engine, implementation + 16 + i as u64 * 4));
    let column_begin = read_u64(engine, implementation + 32);
    let column_end = read_u64(engine, implementation + 40);
    assert!(column_end >= column_begin && column_end - column_begin <= 64);
    let columns = (column_begin..column_end)
        .step_by(4)
        .map(|p| read_u32(engine, p))
        .collect::<Vec<_>>();
    let row_begin = read_u64(engine, implementation + 104);
    let row_end = read_u64(engine, implementation + 112);
    assert!(row_end >= row_begin && row_end - row_begin <= 128);
    let rows = (row_begin..row_end)
        .step_by(8)
        .map(|p| read_u32(engine, read_u64(engine, p) + 72))
        .collect::<Vec<_>>();
    let maxima = (row_begin..row_end)
        .step_by(8)
        .map(|p| read_u32(engine, read_u64(engine, p) + 124))
        .collect::<Vec<_>>();
    format!(
        "{{\"stage\":{},\"input_bits\":{input:?},\"raw_rect_bits\":{raw:?},\"content_rect_bits\":{content:?},\"stored_column_width_bits\":{columns:?},\"stored_row_height_bits\":{rows:?},\"maximum_row_height_bits\":{maxima:?}}}",
        json_string(stage)
    )
}

unsafe extern "C" fn native_call(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let calls = unsafe { &mut *data.cast::<NativeCalls>() };
    *calls.counts.entry(address).or_default() += 1;
    if address == CELL_DRAWING + 0xad040 {
        let vector = read_register(engine, REGISTER_X0 + 1);
        let begin = read_u64(engine, vector);
        let end = read_u64(engine, vector + 8);
        assert!(end >= begin && end - begin <= 512);
        calls.split_band_inputs.push(
            (begin..end)
                .step_by(16)
                .map(|rect| std::array::from_fn(|i| read_u32(engine, rect + i as u64 * 4)))
                .collect(),
        );
    }
    if let Some((stage, entry, components)) = match address {
        0x3c6f78 => Some(("SetRect.entry", true, 4)),
        0x3c7058 => Some(("SetRect.complete", false, 0)),
        0x3c6ae4 => Some(("SetContentSize.entry", true, 2)),
        0x3c6b50 => Some(("SetContentSize.complete", false, 0)),
        _ => None,
    } {
        let implementation = read_register(engine, REGISTER_X0 + if entry { 0 } else { 19 });
        if implementation == read_u64(engine, TABLE + 104) {
            let input = (0..components)
                .map(|i| read_register(engine, 136 + i) as u32)
                .collect::<Vec<_>>();
            calls.model_state_transitions.push(stored_table_state(
                engine,
                implementation,
                stage,
                &input,
            ));
        }
        return;
    }
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
    native_page_measurement: bool,
    native_obstacles: bool,
    plain_text: Option<(&'static str, Option<f32>)>,
}

fn native_rect_list(machine: &Machine, list: u64) -> Vec<[u32; 4]> {
    let count = machine.call(BODY + 0x104740, &[list]);
    assert!(count <= 32);
    let token = machine.call(BODY + 0x104810, &[list]);
    let mut rectangles = Vec::new();
    for _ in 0..count {
        let data = machine.call(BODY + 0x104820, &[list, token]);
        assert_ne!(data, 0);
        rectangles.push(std::array::from_fn(|i| {
            read_u32(machine.engine, data + i as u64 * 4)
        }));
        assert_eq!(machine.call(BODY + 0x104830, &[list, token]) & 1, 1);
    }
    machine.call(BODY + 0x104840, &[list, token]);
    rectangles
}

fn initialize_native_obstacles(machine: &Machine) -> String {
    write(machine.engine, PAGE_MANAGER, &[0; 640]);
    machine.call(WIDGET + 0xaeea0, &[PAGE_MANAGER]);
    machine.call(WIDGET + 0xaff84, &[PAGE_MANAGER, 360]);
    float_arguments(machine, [1., 0., 0., 0.]);
    machine.call(WIDGET + 0xaff94, &[PAGE_MANAGER]);
    write(machine.engine, PAGE_OBSTACLE, &[0; 120]);
    machine.call(
        BODY + 0xb869c,
        &[PAGE_OBSTACLE, CONTEXT, BODY_DOCUMENT, PAGE_MANAGER],
    );
    put(machine, OWNER + 472, PAGE_MANAGER);
    put(machine, OWNER + 480, PAGE_OBSTACLE);
    machine.call(BODY + 0xb2cfc, &[OWNER]);
    let obstacles = native_rect_list(machine, PAGE_OBSTACLE + 48);
    let padding = native_rect_list(machine, PAGE_OBSTACLE + 80);
    format!(
        "{{\"supplied_document_width\":360,\"supplied_document_density_bits\":{},\"obstacle_rect_bits\":{obstacles:?},\"padding_rect_bits\":{padding:?},\"native_page_wpage_is_null\":true,\"complete_update_obstacle_executed\":true}}",
        1f32.to_bits()
    )
}

fn initialize_one_page_document(machine: &Machine, size: [f32; 2]) {
    write(machine.engine, BODY_DOCUMENT, &[0; 152]);
    machine.call(BODY + 0xa8c3c, &[BODY_DOCUMENT, 0]);
    put(machine, BODY_DOCUMENT + 16, DOC);
    for (address, value) in [
        (BODY_DOCUMENT + 24, PAGE_LIST),
        (BODY_DOCUMENT + 32, PAGE_LIST + 8),
        (BODY_DOCUMENT + 40, PAGE_LIST + 8),
        (PAGE_LIST, PAGE_RECORD),
        (OWNER + 456, BODY_DOCUMENT),
    ] {
        put(machine, address, value);
    }
    write(machine.engine, PAGE_RECORD, &[0; 32]);
    for (index, value) in [0_i32, 0, 0, size[0] as i32, size[1] as i32]
        .into_iter()
        .enumerate()
    {
        write(
            machine.engine,
            PAGE_RECORD + index as u64 * 4,
            &value.to_le_bytes(),
        );
    }
    write(
        machine.engine,
        BODY_DOCUMENT + 148,
        &(size[0] as i32).to_le_bytes(),
    );
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
    let mut observed_addresses = vec![
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
    ];
    if input.native_page_measurement {
        observed_addresses.extend([
            BODY + 0xa8c3c,
            BODY + 0xa98a8,
            BODY + 0xa98c8,
            BODY + 0xa98e0,
            BODY + 0xb2e54,
            BODY + 0xf2480,
            0x3c6f78,
            0x3c7058,
            0x3c6ae4,
            0x3c6b50,
        ]);
    }
    if input.native_obstacles {
        observed_addresses.extend([
            WIDGET + 0xaeea0,
            BODY + 0xb869c,
            BODY + 0xb8964,
            BODY + 0xb8dfc,
            BODY + 0xb2cfc,
            BODY + 0xb3504,
            CELL_DRAWING + 0xad040,
            WIDGET + 0xd7390,
            WIDGET + 0xd73a0,
        ]);
    }
    for address in observed_addresses {
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
    if input.native_page_measurement {
        initialize_one_page_document(machine, input.document_size);
    }
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
    if !input.native_page_measurement {
        float_arguments(machine, [input.document_size[0], 0., 0., 0.]);
        machine.call(WIDGET + 0xd398c, &[DOC_WRAPPER]);
        float_arguments(machine, [input.document_size[1], 0., 0., 0.]);
        machine.call(WIDGET + 0xd399c, &[DOC_WRAPPER]);
    }
    assert_eq!(
        read_spen_string(machine.engine, machine.call(0x39c9e8, &[DOC + 40])).as_deref(),
        Some("\u{fffc}")
    );
    let obstacle_source = input
        .native_obstacles
        .then(|| initialize_native_obstacles(machine));
    if input.native_page_measurement {
        machine.call(BODY + 0xb2e54, &[OWNER]);
    } else {
        machine.call(
            WIDGET + 0xd73b0,
            &[DOC_WRAPPER, 0, u32::MAX as u64, u32::MAX as u64],
        );
    }
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
    if input.native_page_measurement {
        put(machine, GROUP + 1792, BODY_DOCUMENT);
        for (address, value) in [
            (GROUP + 1872, GROUP_RANGES),
            (GROUP + 1880, GROUP_RANGES + 8),
            (GROUP + 1888, GROUP_RANGES + 8),
        ] {
            put(machine, address, value);
        }
        write(machine.engine, GROUP_RANGES, &0_i32.to_le_bytes());
        let length = machine.call(TEXT + 0x8b104, &[wrapper]) as u32;
        write(machine.engine, GROUP_RANGES + 4, &length.to_le_bytes());
        write(machine.engine, GROUP + 2012, &0_i32.to_le_bytes());
    }
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
    if input.native_page_measurement {
        put(machine, VIEW + 1000, BODY_DOCUMENT);
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
    let expected_updates = if input.native_obstacles { 2 } else { 1 };
    assert_eq!(calls.counts.get(&(BODY + 0xb0b9c)), Some(&expected_updates));
    assert_eq!(calls.object_size_updates.len(), expected_updates as usize);
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

    let mut result = format!(
        "{{\"supplied_layout_constraint\":{},\"supplied_document_size_bits\":{:?},\"supplied_page_origin_bits\":{:?},\"supplied_moved_page_origin_bits\":{:?},\"native_calls\":{{{native_calls}}},\"object_size_updates\":[{}],\"document_bound_bits\":{bound:?},\"page_local_drawn_target_bits\":{:?},\"native_parent_rects_bits\":{:?},\"table_raw_before_bits\":{table_raw_before:?},\"table_drawn_before_bits\":{table_drawn_before:?},\"table_raw_before_move_bits\":{table_raw_before_move:?},\"table_drawn_before_move_bits\":{table_drawn_before_move:?},\"native_parent_rect_bits\":{info_rect:?},\"source_before\":{before},\"source_after\":{after},\"source_after_page_origin_notification\":{after_origin_notification},\"source_after_page_origin_placement\":{after_origin_placement},\"writer_before\":{writer_before},\"writer_after\":{writer_after},\"writer_after_page_origin_placement\":{writer_after_origin_placement},\"entries\":{}}}",
        input.constraint,
        input.document_size.map(f32::to_bits),
        input.page_origin.map(f32::to_bits),
        input.moved_page_origin.map(f32::to_bits),
        calls.object_size_updates.join(","),
        calls.page_local_drawn_targets,
        calls.native_parent_rects,
        cell_measurement::snapshot(machine, rich)
    );
    if input.native_page_measurement {
        let source = one_page_text::source(machine, machine.call(TEXT + 0x8b104, &[wrapper]));
        let without_last_page = machine.call(BODY + 0xa98e0, &[BODY_DOCUMENT, 0]) as u32 as i32;
        let including_last_page = machine.call(BODY + 0xa98e0, &[BODY_DOCUMENT, 1]) as u32 as i32;
        let infinite = machine.call(BODY + 0xa98a8, &[BODY_DOCUMENT]) & 1 != 0;
        let document_rect = returned_rect(machine, 0x2caa60, &[DOC]);
        assert!(!infinite);
        assert_eq!(without_last_page, 0);
        assert_eq!(read_u32(machine.engine, DOC_WRAPPER + 528), 0);
        assert_eq!(result.pop(), Some('}'));
        result.push_str(&format!(",\"document_model_source\":{source},\"model_state_transitions\":[{}],\"native_page_measurement\":{{\"is_infinite_scroll\":{infinite},\"height_without_last_page\":{without_last_page},\"height_including_last_page\":{including_last_page},\"widget_layout_height_bits\":{},\"document_model_rect_bits\":{document_rect:?},\"supplied_page_record\":[0,0,0,{},{}],\"supplied_group_range_from_native_text_length\":true}}}}",calls.model_state_transitions.join(","),read_u32(machine.engine,DOC_WRAPPER+528),input.document_size[0] as i32,input.document_size[1] as i32));
    }
    if let Some(obstacle_source) = obstacle_source {
        assert_eq!(result.pop(), Some('}'));
        result.push_str(&format!(",\"native_obstacle_source\":{obstacle_source},\"native_callback_split_band_inputs_bits\":{:?}}}",calls.split_band_inputs));
    }
    result
}
pub(super) fn capture(
    machine: &mut Machine,
    paths: widget_text_constructor::Paths<'_>,
    body: &Path,
    composer: &Path,
) {
    capture_profile(machine, paths, body, composer, false, false);
}

pub(super) fn capture_one_page(
    machine: &mut Machine,
    paths: widget_text_constructor::Paths<'_>,
    body: &Path,
    composer: &Path,
) {
    capture_profile(machine, paths, body, composer, true, false);
}

pub(super) fn capture_one_page_obstacles(
    machine: &mut Machine,
    paths: widget_text_constructor::Paths<'_>,
    body: &Path,
    composer: &Path,
) {
    capture_profile(machine, paths, body, composer, true, true);
}

fn capture_profile(
    machine: &mut Machine,
    paths: widget_text_constructor::Paths<'_>,
    body: &Path,
    composer: &Path,
    one_page: bool,
    native_obstacles: bool,
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
    let mut inputs = if one_page {
        vec![
            PlacementInput {
                name: "ordinary-one-page-zero-widget-height",
                constraint: 2,
                page_origin: [0., 0.],
                moved_page_origin: [-19.5, 31.125],
                document_size: [240., 600.],
                native_page_measurement: true,
                native_obstacles,
                plain_text: None,
            },
            PlacementInput {
                name: "ordinary-default-font-one-page-text",
                constraint: 2,
                page_origin: [0., 0.],
                moved_page_origin: [0., 0.],
                document_size: [240., 600.],
                native_page_measurement: true,
                native_obstacles: false,
                plain_text: Some(("AV abc", None)),
            },
            PlacementInput {
                name: "ordinary-font50-wrapped-one-page-text",
                constraint: 2,
                page_origin: [0., 0.],
                moved_page_origin: [0., 0.],
                document_size: [40., 600.],
                native_page_measurement: true,
                native_obstacles: false,
                plain_text: Some(("AV abc AV", Some(50.))),
            },
        ]
    } else {
        vec![
            PlacementInput {
                name: "overpages-nonzero-origin",
                constraint: 2,
                page_origin: [11.25, -7.5],
                moved_page_origin: [24.25, 9.5],
                document_size: [400., 1000.],
                native_page_measurement: false,
                native_obstacles: false,
                plain_text: None,
            },
            PlacementInput {
                name: "overpages-zero-origin",
                constraint: 2,
                page_origin: [0., 0.],
                moved_page_origin: [-19.5, 31.125],
                document_size: [240., 600.],
                native_page_measurement: false,
                native_obstacles: false,
                plain_text: None,
            },
            PlacementInput {
                name: "overpages-overlap-padding",
                constraint: 1,
                page_origin: [-5.25, 17.125],
                moved_page_origin: [8.75, -9.5],
                document_size: [320., 800.],
                native_page_measurement: false,
                native_obstacles: false,
                plain_text: None,
            },
        ]
    };
    if native_obstacles {
        inputs.truncate(1);
        inputs[0].name = "ordinary-one-page-native-page-padding";
    }
    for input in inputs {
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
            let observer = |m: &Machine| {
                if let Some((text, font_size)) = input.plain_text {
                    one_page_text::observe(
                        m,
                        text,
                        font_size,
                        input.document_size[0] as i32,
                        input.document_size[1] as i32,
                    )
                } else {
                    observe_parent(m, input)
                }
            };
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
    let mut entrypoints = if one_page {
        format!(
            "{},\"bodytext_document_constructor\":\"Bodytext:0xa8c3c\",\"native_infinite_scroll_getter\":\"Bodytext:0xa98a8\",\"native_page_width_getter\":\"Bodytext:0xa98c8\",\"native_total_page_height_getter\":\"Bodytext:0xa98e0\",\"complete_bodytext_measure\":\"Bodytext:0xb2e54\",\"ordinary_page_range_by_text_index\":\"Bodytext:0xf2480\"}}",
            NATIVE_ENTRYPOINTS.strip_suffix('}').unwrap()
        )
    } else {
        NATIVE_ENTRYPOINTS.to_owned()
    };
    if native_obstacles {
        entrypoints.pop();
        entrypoints.push_str(",\"text_manager_constructor\":\"Widget:0xaeea0\",\"obstacle_constructor\":\"Bodytext:0xb869c\",\"complete_update_obstacle\":\"Bodytext:0xb2cfc\",\"make_obstacle\":\"Bodytext:0xb8964\",\"page_padding_producer\":\"Bodytext:0xb8dfc\"}");
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
        entrypoints,
        json_string(if native_obstacles {
            "Complete genuine ordinary BodyTextDocument constructor and complete BodyTextLayout.updateObstacle run with actual TextManager(width360,density1) and actual BodyTextPageObstacle constructors. Complete MakeObstacle and updatePagePaddingRect execute native Constant IDs63/64/65/66 through the configured display, actual document page-list/max-width/total-height getters, then install actual obstacle/padding lists and margins into real Widget. The supplied one-page record has no WPage object, so native nontext obstacle selection is empty; page padding is produced by native code, never seeded. Actual list getters observe the produced rectangles; complete native document measurement, object-size callback, ordinary page-range lookup, source Model cell feedback, placement, Drawing notifications and all three clone Composer text writer stages then execute. Actual default source style lists/font and Model SetRect/SetContentSize transitions are observed. Caller supplies source table/text/document dimensions, one-page integer record/selected max width, actual owner associations/page-view origins and group range from native text length. Parsing, full SetDocument/convertPageList, page indexer/group-range production, nonnull WPage selection, device ICU, aggregate visible-cell selection and PDF backend/pixels remain excluded. Shared recording writer skips sibling bitmap/background/decoration exports. Five allocation-fill/repeat samples must match exactly."
        } else if one_page {
            "The object case reuses the complete native Model document ObjectSpan, real Widget/Text conversion/measure/layout, Bodytext object-size callback, source Model cell bridge, actual ordinary page-range lookup, Drawing notification and three actual clone Composer writer stages. The plain-text controls construct genuine no-object Model sources, observe effective font and raw style spans, and use real Widget/Text measurement/placement/getters. Actual BodyTextDocument(false) constructor/vtable GetPageMaxWidth/GetPageTotalHeight and complete BodyTextLayout.measureText execute: GetPageTotalHeight(false) obtains last page offset0, independently of the positive page/model document height600, and feeds native Widget layout height0. Caller supplies explicitly named one-page integer record, selected max width, document/body-owner associations, page-view origin, and one-page group text range derived from actual native string length. Native ordinary IsInfiniteScrollDocument=false and GetObjectUpdateInfo false branch/getPageRangeByTextIndex execute; no infinite-scroll return is substituted. Expected geometry is never supplied. Five allocation-fill/repeat samples reset caller/native owned state. Full SetDocument/convertPageList, full page indexer/group-range production, parsing, complete newObjectLayout/initBodyTextView, device ICU, aggregate table visible-cell selection and final PDF backend/pixels remain excluded. Shared runtime and writer interfaces match bodytext_table_placement and live_table_text_clipping; PDF/Paint record actual foreground clip transport while bitmap/background/decoration sibling exports are skipped."
        } else {
            "Actual Model document ObjectTextBox+ObjectSpan AppendObjectSpan inserts U+FFFC; actual Widget conversion, measurement and placement produce real document entries and GetTextBound; bounded Bodytext callback registration [0xb01bc,0xb02dc) installs genuine onUpdateObjectSize. Cold native table Measure precedes actual document feedback TableLayout.Layout. Native GetObjectUpdateInfo reads document bound, subtracts supplied page origin and applies actual GetRectByDrawnRect; complete updateObjectSpanRectBySpanList/model bridge derives saved cell/content rectangles from actual table cell frames. Native listener [0xb3228,0xb3314) and view callback [0xd080c,0xd085c) setup windows precede complete Drawing notifications with actual document GetTextBound. Page-origin-only controls preserve cells on size notification, then complete placement updates them. Actual native clone/copy, second TableLayout and cached writer execute before/after source callbacks, without supplied table/cell source setters or host object dimensions/entries. Caller supplies explicit document shape and Widget dimensions, source table input, context/device, zeroed caller owner/view associations and one genuine table layout in owner map; finite one-page infinite-scroll interface and page origins are supplied, obstacle/history/cache services null. Owned allocations vary fills; supplied caller storage zeroed every sample. Parser, full SetBodyTextDocument, full newObjectLayout/initBodyTextView, actual page-bound production, device ICU and final PDF backend excluded. Writer boundary matches live_table_text_clipping: caller explicitly selects all four raw physical slots; aggregate visible-cell selection is excluded. Actual clone writer measured-rounding setup is bounded to [0x37e4ac,0x37e4f8), followed by complete per-cell writeTextContent/writeTextBlock. PDF/Paint methods are recording interfaces; sibling bitmap/background/decoration exports are skipped."
        }),
        output.join(",")
    );
}
