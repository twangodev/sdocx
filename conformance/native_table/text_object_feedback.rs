use super::*;

const FUNCTION: u64 = MODEL + 0x17000;
const FUNCTION_VTABLE: u64 = FUNCTION + 0x80;
const CALLBACK: u64 = 0x0300_0500;
const MINIMUM: u64 = MODEL + 0x17100;
const OBSTACLES: u64 = MODEL + 0x17200;
const CHANGED: u64 = MODEL + 0x17400;
const PREVIOUS: u64 = MODEL + 0x17500;
const TREE: u64 = MODEL + 0x17700;
const TREE_VECTORS: u64 = MODEL + 0x17800;
const TREE_RECT: u64 = MODEL + 0x17900;
const CAPTURE_BOUNDARY: &str = "Complete native GetBlockInfo and m_CheckObjectChanged execute on supplied dense 80-byte MeasureData entries, kind 5, object type 1 (block) or 2 (inline), over-page flag, source start, break annotations, paragraph leading, object margins and available rectangle. The native callback thunk and virtual invocation execute; the host callback supplies only rectangle and minimum and records the source anchor, candidate and prior entry. Register snapshots, entry mutations, selected ranges and the changed-anchor vector are observed. Warm calls retain entries and the changed-anchor vector. Native rectangle, margin, leading, 0.001 comparisons, width and height mutation and vector algorithms execute. Four obstacle controls construct a native single-leaf RectFTree from one supplied rectangle and execute native intersection and minimum rejection. Tree record padding bytes 17 through 19 are excluded. Host allocation, deletion, bounded copy and supplied callback dimensions are boundaries. Native source, shaping, font production, classification, upstream object dispatch, source callbacks, general document or page obstacle trees, the full paragraph loop, placement, cached glyph emission and SVG or PDF do not execute. Large and decimal advances are supplied numeric controls, not font-produced measurements.";

#[derive(Clone)]
struct Case {
    name: &'static str,
    block: bool,
    advances: Vec<f32>,
    kinds: Vec<u32>,
    breaks: Vec<u32>,
    object: usize,
    object_type: u32,
    callback: bool,
    callback_rect: [f32; 4],
    callback_minimum: f32,
    top_of_page: bool,
    candidate_y: f32,
    leading_pixels: f32,
    leading_multiplier: f32,
    object_margins: [f32; 2],
    budget: f32,
    full_width: f32,
    obstacle: Option<[f32; 4]>,
    over_pages: bool,
    source_start: u32,
}
impl Case {
    fn direct(name: &'static str) -> Self {
        Self {
            name,
            block: false,
            advances: vec![10.0],
            kinds: vec![5],
            breaks: vec![1],
            object: 0,
            object_type: 2,
            callback: true,
            callback_rect: [2.0, 3.0, 10.0, 20.125],
            callback_minimum: 0.0,
            top_of_page: false,
            candidate_y: 11.25,
            leading_pixels: 0.0,
            leading_multiplier: 1.35,
            object_margins: [0.0; 2],
            budget: 30.0,
            full_width: 30.0,
            obstacle: None,
            over_pages: true,
            source_start: 100,
        }
    }
    fn block(
        name: &'static str,
        advances: &[f32],
        breaks: &[u32],
        object: usize,
        budget: f32,
    ) -> Self {
        let mut case = Self::direct(name);
        case.block = true;
        case.advances = advances.to_vec();
        case.kinds = vec![0; advances.len()];
        case.kinds[object] = 5;
        case.breaks = breaks.to_vec();
        case.object = object;
        case.budget = budget;
        case.full_width = budget;
        case.candidate_y = 0.0;
        case
    }
}

#[derive(Default)]
struct Trace {
    supplied_rect: [f32; 4],
    supplied_minimum: f32,
    active_entry: u64,
    events: Vec<String>,
    callback_calls: u32,
}
fn entry_snapshot(engine: Engine, entry: u64) -> String {
    format!(
        "{{\"advance_bits\":{},\"height_bits\":{},\"ink_bits\":{:?},\"kind\":{},\"object_type\":{},\"over_pages\":{},\"margin_bits\":{:?}}}",
        read_u32(engine, entry),
        read_u32(engine, entry + 4),
        rectangle(engine, entry + 32).map(f32::to_bits),
        read_u32(engine, entry + 48),
        read_u32(engine, entry + 64),
        bytes(engine, entry + 68, 1)[0] != 0,
        [72, 76].map(|offset| read_u32(engine, entry + offset)),
    )
}
fn bytes(engine: Engine, address: u64, length: usize) -> Vec<u8> {
    let mut out = vec![0; length];
    check(unsafe { uc_mem_read(engine, address, out.as_mut_ptr().cast(), length) });
    out
}
unsafe extern "C" fn observe(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let trace = unsafe { &mut *data.cast::<Trace>() };
    if address == CALLBACK {
        let anchor = read_u32(engine, read_register(engine, REGISTER_X0 + 1));
        let candidate = read_u32(engine, read_register(engine, REGISTER_X0 + 2));
        trace.events.push(format!(
            "{{\"stage\":\"supplied_measure_callback\",\"source_utf16_anchor\":{anchor},\"candidate_y_bits\":{candidate},\"entry_before\":{}}}",
            entry_snapshot(engine, trace.active_entry),
        ));
        set_rectangle(
            engine,
            read_register(engine, REGISTER_X0 + 3),
            trace.supplied_rect,
        );
        float(
            engine,
            read_register(engine, REGISTER_X0 + 4),
            trace.supplied_minimum,
        );
        trace.callback_calls += 1;
        return;
    }
    if address == TEXT + 0x6c2dc {
        trace.active_entry = read_register(engine, REGISTER_X0 + 1);
        trace.events.push(format!(
            "{{\"stage\":\"native_check_object_changed\",\"relative_utf16\":{},\"candidate_y_bits\":{},\"top_of_page\":{},\"entry_before\":{}}}",
            read_register(engine, REGISTER_X0 + 2) as u32,
            read_register(engine, 136) as u32,
            read_register(engine, REGISTER_X0 + 3) != 0,
            entry_snapshot(engine, trace.active_entry),
        ));
        return;
    }
    if address == TEXT + 0x6e71c {
        trace.events.push(format!(
            "{{\"stage\":\"native_obstacle_tree_lookup\",\"candidate_rectangle_bits\":{:?},\"paragraph_limits_bits\":{:?},\"native_flag\":{},\"tolerance_bits\":{}}}",
            rectangle(engine, read_register(engine, REGISTER_X0 + 1)).map(f32::to_bits),
            [read_register(engine, 136) as u32, read_register(engine, 137) as u32],
            read_register(engine, REGISTER_X0 + 3) as u32,
            read_register(engine, 138) as u32,
        ));
        return;
    }
    let entry = read_register(engine, REGISTER_X0 + 24);
    assert!(entry >= ENTRIES && entry < ENTRIES + 8 * 80);
    trace.events.push(format!(
        "{{\"stage\":\"Text+0x{:x}\",\"relative_utf16\":{},\"committed_bits\":{},\"pending_bits\":{},\"candidate_register_bits\":{},\"available_bits\":{},\"entry\":{}}}",
        address - TEXT, (entry - ENTRIES) / 80,
        read_register(engine, 146) as u32, read_register(engine, 144) as u32,
        read_register(engine, 148) as u32, read_register(engine, 145) as u32,
        entry_snapshot(engine, entry),
    ));
}

struct Recorder {
    engine: Engine,
    hooks: Vec<usize>,
    state: Box<Trace>,
}
impl Recorder {
    fn new(machine: &Machine) -> Self {
        let mut recorder = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            state: Box::default(),
        };
        for address in [
            CALLBACK,
            TEXT + 0x6c2dc,
            TEXT + 0x6ae04,
            TEXT + 0x6ae1c,
            TEXT + 0x6ae80,
            TEXT + 0x6af08,
            TEXT + 0x6e71c,
        ] {
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    observe as *mut c_void,
                    ptr::from_mut(recorder.state.as_mut()).cast(),
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

impl Case {
    fn execute(&self, machine: &mut Machine, recorder: &mut Recorder, fill: u8) -> String {
        initialize_layout_shell(machine, fill);
        machine.heap.cursor = HEAP;
        machine.heap.allocation_fill = fill;
        machine.heap.allocations = 0;
        machine.heap.deletes = 0;
        machine.heap.fills = 0;
        *recorder.state = Trace {
            supplied_rect: self.callback_rect,
            supplied_minimum: self.callback_minimum,
            ..Trace::default()
        };
        for (address, size) in [
            (FUNCTION, 256),
            (OBSTACLES, 48),
            (CHANGED, 24),
            (PREVIOUS, 80),
        ] {
            write(machine.engine, address, &vec![0; size]);
        }
        write(machine.engine, CALLBACK, &0xd65f03c0_u32.to_le_bytes());
        for (address, value) in [
            (FUNCTION, FUNCTION_VTABLE),
            (FUNCTION_VTABLE + 48, CALLBACK),
            (PARAGRAPH + 32, if self.callback { FUNCTION } else { 0 }),
        ] {
            write(machine.engine, address, &value.to_le_bytes());
        }
        float(machine.engine, PARAGRAPH + 168, 0.0);
        float(machine.engine, PARAGRAPH + 172, self.full_width);
        if let Some(obstacle) = self.obstacle {
            write(machine.engine, TREE_VECTORS, &[0; 72]);
            set_rectangle(machine.engine, TREE_RECT, obstacle);
            for (address, value) in [
                (TREE_VECTORS, TREE_RECT),
                (TREE_VECTORS + 8, TREE_RECT + 16),
                (TREE_VECTORS + 16, TREE_RECT + 16),
            ] {
                write(machine.engine, address, &value.to_le_bytes());
            }
            machine.call(
                TEXT + 0x6d980,
                &[TREE, TREE_VECTORS, TREE_VECTORS + 24, TREE_VECTORS + 48],
            );
            assert_ne!(read_u64(machine.engine, TREE + 8), 0);
            write(machine.engine, MEASURE + 16, &TREE.to_le_bytes());
        }
        float(machine.engine, RICH_PARAGRAPH + 32, self.leading_pixels);
        float(machine.engine, RICH_PARAGRAPH + 36, self.leading_multiplier);
        write(
            machine.engine,
            RICH_PARAGRAPH + 4,
            &self.source_start.to_le_bytes(),
        );
        for (index, &advance) in self.advances.iter().enumerate() {
            let entry = ENTRIES + index as u64 * 80;
            write(machine.engine, entry, &[0; 80]);
            float(machine.engine, entry, advance);
            float(machine.engine, entry + 4, 17.125);
            float(machine.engine, entry + 60, 17.125);
            set_rectangle(machine.engine, entry + 32, [3.0, -12.0, 11.0, 6.0]);
            write(machine.engine, entry + 48, &self.kinds[index].to_le_bytes());
            write(
                machine.engine,
                entry + 56,
                &self.breaks[index].to_le_bytes(),
            );
            if index == self.object {
                write(machine.engine, entry + 64, &self.object_type.to_le_bytes());
                write(machine.engine, entry + 68, &[u8::from(self.over_pages)]);
                for (offset, margin) in [72, 76].into_iter().zip(self.object_margins) {
                    float(machine.engine, entry + offset, margin);
                }
            }
        }
        let entry = ENTRIES + self.object as u64 * 80;
        let tree_observation = if self.obstacle.is_some() {
            let root = read_u64(machine.engine, TREE + 8);
            let records = read_u64(machine.engine, root + 8);
            assert_eq!(read_u64(machine.engine, root + 16) - records, 20);
            format!(
                "{{\"native_leaf\":{},\"record_count\":1,\"record_rectangle_bits\":{:?},\"record_flag\":{},\"node_bounds_bits\":{:?}}}",
                bytes(machine.engine, root + 48, 1)[0] != 0,
                rectangle(machine.engine, records).map(f32::to_bits),
                bytes(machine.engine, records + 16, 1)[0],
                rectangle(machine.engine, root + 32).map(f32::to_bits)
            )
        } else {
            "null".into()
        };
        let before = entry_snapshot(machine.engine, entry);
        let mut stages = Vec::new();
        for stage in ["cold", "warm", "warm_repeat"] {
            recorder.state.events.clear();
            recorder.state.callback_calls = 0;
            float(machine.engine, MINIMUM, -123.0);
            let selection = if self.block {
                write(machine.engine, STACK, &CHANGED.to_le_bytes());
                register(machine.engine, REGISTER_X0 + 8, BLOCKS);
                for (index, value) in [0.0, self.candidate_y, self.budget, 100.0]
                    .into_iter()
                    .enumerate()
                {
                    scalar(machine.engine, index as i32, value);
                }
                machine.call(
                    TEXT + 0x6ab9c,
                    &[
                        PARAGRAPH,
                        RICH_PARAGRAPH,
                        ENTRIES,
                        PREVIOUS,
                        0,
                        self.advances.len() as u64,
                        0,
                        OBSTACLES,
                    ],
                );
                format!(
                    "{{\"range_utf16_inclusive\":{:?},\"layout_bits\":{:?},\"available_bits\":{:?},\"flags\":{:?},\"metric_bits\":{:?}}}",
                    [
                        read_u32(machine.engine, BLOCKS) as i32,
                        read_u32(machine.engine, BLOCKS + 4) as i32
                    ],
                    rectangle(machine.engine, BLOCKS + 8).map(f32::to_bits),
                    rectangle(machine.engine, BLOCKS + 24).map(f32::to_bits),
                    bytes(machine.engine, BLOCKS + 40, 3),
                    [52, 56, 60, 64].map(|offset| read_u32(machine.engine, BLOCKS + offset))
                )
            } else {
                scalar(machine.engine, 0, self.candidate_y);
                machine.call(
                    TEXT + 0x6c2dc,
                    &[
                        PARAGRAPH,
                        entry,
                        self.object as u64,
                        u64::from(self.top_of_page),
                        CHANGED,
                        MINIMUM,
                    ],
                );
                "null".into()
            };
            let begin = read_u64(machine.engine, CHANGED);
            let end = read_u64(machine.engine, CHANGED + 8);
            assert!(end >= begin && end - begin <= 64);
            let changed: Vec<_> = (0..(end - begin) / 4)
                .map(|i| read_u32(machine.engine, begin + i * 4))
                .collect();
            stages.push(format!("{{\"stage\":{stage:?},\"selection\":{selection},\"minimum_bits\":{},\"entry_after\":{},\"changed_source_utf16_anchors\":{changed:?},\"callback_calls\":{},\"events\":[{}]}}",
                if self.block { "null".into() } else { read_u32(machine.engine, MINIMUM).to_string() },
                entry_snapshot(machine.engine, entry), recorder.state.callback_calls, recorder.state.events.join(",")));
        }
        format!(
            "{{\"name\":{:?},\"route\":{:?},\"supplied_advances_bits\":{:?},\"supplied_kinds\":{:?},\"supplied_break_ends_utf16\":{:?},\"object_utf16\":{},\"callback_present\":{},\"supplied_callback_rectangle_bits\":{:?},\"supplied_callback_minimum_bits\":{},\"supplied_candidate_y_bits\":{},\"supplied_top_of_page\":{},\"supplied_leading_pixels_bits\":{},\"supplied_leading_multiplier_bits\":{},\"source_start_utf16\":{},\"budget_bits\":{},\"paragraph_full_width_bits\":{},\"supplied_obstacle_bits\":{},\"native_obstacle_tree\":{tree_observation},\"entry_before\":{before},\"stages\":[{}]}}",
            self.name,
            if self.block {
                "GetBlockInfo"
            } else {
                "m_CheckObjectChanged"
            },
            self.advances
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            self.kinds,
            self.breaks,
            self.object,
            self.callback,
            self.callback_rect.map(f32::to_bits),
            self.callback_minimum.to_bits(),
            self.candidate_y.to_bits(),
            self.top_of_page,
            self.leading_pixels.to_bits(),
            self.leading_multiplier.to_bits(),
            self.source_start,
            self.budget.to_bits(),
            self.full_width.to_bits(),
            self.obstacle
                .map_or_else(|| "null".into(), |r| format!("{:?}", r.map(f32::to_bits))),
            stages.join(",")
        )
    }
}

fn cases() -> Vec<Case> {
    let mut cases = vec![Case::direct(
        "inline_unchanged_visible_width_differs_from_advance",
    )];
    for (name, rect) in [
        ("inline_width_changes", [2.0, 3.0, 11.0, 20.125]),
        ("inline_height_changes", [2.0, 3.0, 10.0, 23.0]),
        ("inline_both_change", [2.0, 3.0, 11.0, 23.0]),
        ("callback_empty_rectangle", [0.0; 4]),
        ("callback_inverted_rectangle", [10.0, 10.0, 0.0, 0.0]),
    ] {
        let mut case = Case::direct(name);
        case.callback_rect = rect;
        cases.push(case);
    }
    let mut absent = Case::direct("absent_callback");
    absent.callback = false;
    cases.push(absent);
    for (name, top) in [
        ("minimum_retained", false),
        ("minimum_cleared_at_page_top", true),
    ] {
        let mut case = Case::direct(name);
        case.callback_minimum = 5.25;
        case.top_of_page = top;
        cases.push(case);
    }
    for (name, margins, pixels, multiplier) in [
        ("block_positive_margins", [2.25, 3.5], 0.0, 1.35),
        ("block_zero_margins_percent_leading", [0.0, 0.0], 0.0, 1.35),
        ("block_zero_margins_pixel_leading", [0.0, 0.0], 4.5, 1.35),
        ("block_one_margin_zero", [2.25, 0.0], 0.0, 1.35),
    ] {
        let mut case = Case::direct(name);
        case.object_type = 1;
        case.object_margins = margins;
        case.leading_pixels = pixels;
        case.leading_multiplier = multiplier;
        cases.push(case);
    }
    let height = (17.125_f32 + 0.001).to_bits();
    for (name, bits) in [
        ("height_epsilon_lower_ulp", height - 1),
        ("height_epsilon_upper_ulp", height + 1),
    ] {
        let mut case = Case::direct(name);
        case.callback_rect = [0.0, 0.0, 8.0, f32::from_bits(bits)];
        cases.push(case);
    }
    let width = (8.0_f32 + 0.001).to_bits();
    for (name, bits) in [
        ("width_epsilon_lower_ulp", width - 1),
        ("width_epsilon_upper_ulp", width),
    ] {
        let mut case = Case::direct(name);
        case.callback_rect = [0.0, 0.0, f32::from_bits(bits), 17.125];
        cases.push(case);
    }
    for (name, advances, breaks, object, budget) in [
        ("first_object_overflow", vec![10.0], vec![1], 0, 5.0),
        (
            "continuation_object_overflow",
            vec![6.0, 10.0],
            vec![2, 2],
            1,
            10.0,
        ),
        (
            "prior_committed_break_rejects_object",
            vec![6.0, 1.0, 10.0],
            vec![2, 2, 3],
            2,
            8.0,
        ),
        (
            "no_break_rejects_object_before_callback",
            vec![6.0, 1.0, 10.0],
            vec![3, 3, 3],
            2,
            8.0,
        ),
        (
            "object_changes_after_fitting_old_width",
            vec![3.0, 4.0],
            vec![2, 2],
            1,
            8.0,
        ),
        (
            "native_f32_object_fold",
            vec![16_777_216.0, 1.0, 1.0],
            vec![3, 3, 3],
            2,
            16_777_216.0,
        ),
        (
            "native_f32_object_fold_committed",
            vec![16_777_216.0, 1.0, 1.0],
            vec![1, 3, 3],
            2,
            16_777_216.0,
        ),
        (
            "decimal_object_fold",
            vec![6.52, 0.7, 1.83],
            vec![3, 3, 3],
            2,
            9.05,
        ),
    ] {
        let mut case = Case::block(name, &advances, &breaks, object, budget);
        case.callback_rect = [0.0, 0.0, 20.0, 23.0];
        cases.push(case);
    }
    let mut leading = Case::block(
        "kind4_prefix_object_exception",
        &[0.0, 10.0],
        &[2, 2],
        1,
        5.0,
    );
    leading.kinds[0] = 4;
    cases.push(leading);
    let mut narrowed = Case::block(
        "narrowed_region_suppresses_first_object_callback",
        &[10.0],
        &[1],
        0,
        5.0,
    );
    narrowed.full_width = 30.0;
    cases.push(narrowed);
    for (name, object_type, callback) in [
        ("inline_over_pages_false", 2, true),
        ("block_over_pages_false", 1, true),
        ("inline_over_pages_false_absent_callback", 2, false),
    ] {
        let mut case = Case::block(name, &[3.0, 4.0], &[2, 2], 1, 8.0);
        case.object_type = object_type;
        case.over_pages = false;
        case.callback = callback;
        cases.push(case);
    }
    for (name, obstacle, minimum, top) in [
        (
            "minimum_obstacle_rejects_update",
            [0.0, 12.0, 30.0, 15.0],
            5.25,
            false,
        ),
        (
            "minimum_obstacle_disjoint_allows_update",
            [0.0, 30.0, 30.0, 35.0],
            5.25,
            false,
        ),
        (
            "top_clears_minimum_bypasses_obstacle",
            [0.0, 12.0, 30.0, 15.0],
            5.25,
            true,
        ),
        (
            "zero_minimum_bypasses_obstacle",
            [0.0, 12.0, 30.0, 15.0],
            0.0,
            false,
        ),
    ] {
        let mut case = Case::direct(name);
        case.obstacle = Some(obstacle);
        case.callback_minimum = minimum;
        case.top_of_page = top;
        case.callback_rect = [0.0, 0.0, 20.0, 23.0];
        cases.push(case);
    }
    cases
}

pub(super) fn capture(machine: &mut Machine) {
    for (plt, target) in [
        (0xef7a0, TEXT + 0x6bf40),
        (0xef7b0, TEXT + 0x6c24c),
        (0xef7c0, TEXT + 0x6c2dc),
        (0xef7d0, TEXT + 0x6c5b0),
        (0xef870, TEXT + 0x6cc64),
        (0xef820, TEXT + 0x6c8a0),
        (0xef830, TEXT + 0x6c9d4),
        (0xef880, TEXT + 0x6cdc4),
        (0xef990, TEXT + 0x6deb0),
        (0xef910, TEXT + 0x6e71c),
        (0xef9b0, TEXT + 0x6e68c),
        (0xef9f0, BASE + 0xb127c),
        (0xeee20, BASE + 0xb109c),
        (0xef950, BASE + 0xb11bc),
        (0xeec30, NEW),
        (0xeec40, DELETE),
        (0xf0910, MEMSET),
    ] {
        bind_native(machine.engine, TEXT + plt, target);
    }
    let _copy = CopyHook::new(machine);
    let mut recorder = Recorder::new(machine);
    let mut output = Vec::new();
    for case in cases() {
        let canonical = case.execute(machine, &mut recorder, 0);
        for fill in [165, 255, 0] {
            assert_eq!(
                case.execute(machine, &mut recorder, fill),
                canonical,
                "{} fill{fill}",
                case.name
            );
        }
        output.push(canonical);
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"native_call_instruction_limit\":{},\"memory_fills\":[0,165,255],\"repeat_zero_fill\":true,\"get_block_info\":\"Text+0x6ab9c\",\"check_object_changed\":\"Text+0x6c2dc\",\"actual_callback_thunk\":\"Text+0x6879c\",\"obstacle_tree_constructor\":\"Text+0x6d980\",\"capture_boundary\":{CAPTURE_BOUNDARY:?},\"cases\":[{}]}}",
        machine.call_instruction_limit,
        output.join(",")
    );
}
