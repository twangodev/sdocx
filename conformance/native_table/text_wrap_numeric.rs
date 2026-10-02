use super::*;

const OBSTACLES: u64 = MODEL + 0x14000;
const CHANGED_OBJECTS: u64 = OBSTACLES + 24;
const PREVIOUS_BLOCK: u64 = CHANGED_OBJECTS + 24;
const GET_BLOCK_INFO: u64 = TEXT + 0x6ab9c;

#[derive(Default)]
struct NumericTrace {
    pending_candidate: Option<Candidate>,
    candidates: Vec<Candidate>,
    pending_commit: Option<Commit>,
    commits: Vec<Commit>,
}

struct Candidate {
    entry_utf16: u32,
    committed_bits: u32,
    pending_bits: u32,
    advance_bits: u32,
    base_bits: u32,
    candidate_bits: u32,
    available_bits: u32,
}

struct Commit {
    entry_utf16: u32,
    reason: &'static str,
    committed_before_bits: u32,
    pending_bits: u32,
    committed_after_bits: u32,
}

fn entry_index(engine: Engine) -> u32 {
    let offset = read_register(engine, REGISTER_X0 + 24)
        .checked_sub(ENTRIES)
        .unwrap();
    assert_eq!(offset % 80, 0);
    let index = offset / 80;
    assert!(index < 8);
    index as u32
}

fn scalar_bits(engine: Engine, index: i32) -> u32 {
    read_register(engine, 136 + index) as u32
}

unsafe extern "C" fn record_numeric(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let trace = unsafe { &mut *data.cast::<NumericTrace>() };
    match address - TEXT {
        0x6ae04 => {
            assert!(trace.pending_candidate.is_none() && trace.candidates.len() < 8);
            trace.pending_candidate = Some(Candidate {
                entry_utf16: entry_index(engine),
                committed_bits: scalar_bits(engine, 10),
                pending_bits: scalar_bits(engine, 8),
                advance_bits: scalar_bits(engine, 12),
                base_bits: 0,
                candidate_bits: 0,
                available_bits: scalar_bits(engine, 9),
            });
        }
        0x6ae1c => {
            let mut candidate = trace.pending_candidate.take().unwrap();
            assert_eq!(candidate.entry_utf16, entry_index(engine));
            candidate.base_bits = scalar_bits(engine, 11);
            candidate.candidate_bits = scalar_bits(engine, 12);
            trace.candidates.push(candidate);
        }
        0x6af34 | 0x6af64 | 0x6af80 => {
            assert!(trace.pending_commit.is_none() && trace.commits.len() < 16);
            trace.pending_commit = Some(Commit {
                entry_utf16: entry_index(engine),
                reason: match address - TEXT {
                    0x6af34 => "break_end",
                    0x6af64 => "space",
                    0x6af80 => "tab",
                    _ => unreachable!(),
                },
                committed_before_bits: scalar_bits(engine, 10),
                pending_bits: scalar_bits(engine, 8),
                committed_after_bits: 0,
            });
        }
        0x6af3c | 0x6af6c | 0x6af88 => {
            let mut commit = trace.pending_commit.take().unwrap();
            commit.committed_after_bits = scalar_bits(engine, 10);
            trace.commits.push(commit);
        }
        _ => unreachable!(),
    }
}

struct TraceRecorder {
    engine: Engine,
    hooks: Vec<usize>,
    state: Box<NumericTrace>,
}

impl TraceRecorder {
    fn new(machine: &Machine) -> Self {
        let mut recorder = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            state: Box::default(),
        };
        for offset in [
            0x6ae04, 0x6ae1c, 0x6af34, 0x6af3c, 0x6af64, 0x6af6c, 0x6af80, 0x6af88,
        ] {
            let address = TEXT + offset;
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    record_numeric as *mut c_void,
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

impl Drop for TraceRecorder {
    fn drop(&mut self) {
        for hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, *hook) });
        }
    }
}

struct NumericCase {
    name: &'static str,
    advances: &'static [f32],
    kinds: &'static [u32],
    break_ends: &'static [u32],
    budget: f32,
    old_point: [f32; 2],
    ink: [f32; 4],
    alignment: u32,
    reverse_visual_map: bool,
}

impl NumericCase {
    fn ordinary(
        name: &'static str,
        advances: &'static [f32],
        break_ends: &'static [u32],
        budget: f32,
    ) -> Self {
        Self {
            name,
            advances,
            kinds: &[0; 8],
            break_ends,
            budget,
            old_point: [2.0, 3.0],
            ink: [3.0, -12.0, 11.0, 6.0],
            alignment: 0,
            reverse_visual_map: false,
        }
    }

    fn execute(&self, machine: &Machine, trace: &mut TraceRecorder, fill: u8) -> String {
        assert!(!self.advances.is_empty() && self.advances.len() <= 8);
        assert!(self.kinds.len() >= self.advances.len());
        assert_eq!(self.break_ends.len(), self.advances.len());
        *trace.state = NumericTrace::default();
        initialize_layout_shell(machine, fill);
        write(machine.engine, OBSTACLES, &[0; 48 + 80]);
        float(machine.engine, PARAGRAPH + 168, 0.0);
        float(machine.engine, PARAGRAPH + 172, self.budget);
        float(machine.engine, RICH_PARAGRAPH + 32, 0.0);
        float(machine.engine, RICH_PARAGRAPH + 36, 1.35);
        write(
            machine.engine,
            RICH_PARAGRAPH + 28,
            &self.alignment.to_le_bytes(),
        );
        let mut supplied = Vec::new();
        let mut visual_to_logical = Vec::new();
        for (index, advance) in self.advances.iter().copied().enumerate() {
            let entry = ENTRIES + index as u64 * 80;
            write(machine.engine, entry, &[0; 80]);
            float(machine.engine, entry, advance);
            float(machine.engine, entry + 4, 17.125);
            float(machine.engine, entry + 60, 17.125);
            float(machine.engine, entry + 8, self.old_point[0]);
            float(machine.engine, entry + 12, self.old_point[1]);
            set_rectangle(machine.engine, entry + 32, self.ink);
            write(machine.engine, entry + 48, &self.kinds[index].to_le_bytes());
            write(
                machine.engine,
                entry + 56,
                &self.break_ends[index].to_le_bytes(),
            );
            let logical = if self.reverse_visual_map {
                self.advances.len() - index - 1
            } else {
                index
            } as u32;
            write(
                machine.engine,
                LOGICAL_MAP + index as u64 * 4,
                &logical.to_le_bytes(),
            );
            visual_to_logical.push(logical);
            supplied.push(format!(
                "{{\"advance_bits\":{},\"kind\":{},\"break_end_utf16\":{},\"object_type\":0}}",
                advance.to_bits(),
                self.kinds[index],
                self.break_ends[index]
            ));
        }
        write(machine.engine, STACK, &CHANGED_OBJECTS.to_le_bytes());
        register(machine.engine, REGISTER_X0 + 8, BLOCKS);
        for (index, value) in [0.0, 0.0, self.budget, 100.0].into_iter().enumerate() {
            scalar(machine.engine, index as i32, value);
        }
        machine.call(
            GET_BLOCK_INFO,
            &[
                PARAGRAPH,
                RICH_PARAGRAPH,
                ENTRIES,
                PREVIOUS_BLOCK,
                0,
                self.advances.len() as u64,
                u64::from(self.alignment),
                OBSTACLES,
            ],
        );
        assert!(trace.state.pending_candidate.is_none() && trace.state.pending_commit.is_none());
        let selected = [
            read_u32(machine.engine, BLOCKS),
            read_u32(machine.engine, BLOCKS + 4),
        ];
        assert!(selected[0] <= selected[1] && (selected[1] as usize) < self.advances.len());
        let block_layout = rectangle(machine.engine, BLOCKS + 8).map(f32::to_bits);
        let block_available = rectangle(machine.engine, BLOCKS + 24).map(f32::to_bits);
        let block_metrics =
            [52, 56, 60, 64].map(|offset| read_u32(machine.engine, BLOCKS + offset));
        let space_count = read_u32(machine.engine, BLOCKS + 48);
        write(machine.engine, BLOCK_POINTERS, &BLOCKS.to_le_bytes());
        write(machine.engine, LINES, &BLOCK_POINTERS.to_le_bytes());
        write(
            machine.engine,
            LINES + 8,
            &(BLOCK_POINTERS + 8).to_le_bytes(),
        );
        for (destination, source) in [(24, 52), (28, 56), (32, 60), (36, 64)] {
            write(
                machine.engine,
                LINES + destination,
                &read_u32(machine.engine, BLOCKS + source).to_le_bytes(),
            );
        }
        scalar(machine.engine, 0, 0.0);
        machine.call(SET_LAYOUT, &[PARAGRAPH, LINES]);
        let cursor_bits = scalar_bits(machine.engine, 0);
        assert_eq!(read_u64(machine.engine, PARAGRAPH + 112), PLACED_LINES + 56);
        let entries = (0..self.advances.len())
            .map(|index| {
                let entry = ENTRIES + index as u64 * 80;
                format!(
                    "{{\"position_bits\":{:?},\"layout_rect_bits\":{:?},\"ink_rect_bits\":{:?}}}",
                    [
                        read_u32(machine.engine, entry + 8),
                        read_u32(machine.engine, entry + 12)
                    ],
                    rectangle(machine.engine, entry + 16).map(f32::to_bits),
                    rectangle(machine.engine, entry + 32).map(f32::to_bits)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let candidates = trace.state.candidates.iter().map(|candidate| format!(
            "{{\"entry_utf16\":{},\"committed_bits\":{},\"pending_bits\":{},\"advance_bits\":{},\"base_bits\":{},\"candidate_bits\":{},\"available_bits\":{}}}",
            candidate.entry_utf16, candidate.committed_bits, candidate.pending_bits, candidate.advance_bits,
            candidate.base_bits, candidate.candidate_bits, candidate.available_bits)).collect::<Vec<_>>().join(",");
        let commits = trace.state.commits.iter().map(|commit| format!(
            "{{\"entry_utf16\":{},\"reason\":{:?},\"committed_before_bits\":{},\"pending_bits\":{},\"committed_after_bits\":{}}}",
            commit.entry_utf16,commit.reason,commit.committed_before_bits,commit.pending_bits,commit.committed_after_bits)).collect::<Vec<_>>().join(",");
        format!(
            "{{\"name\":{:?},\"supplied_entries\":[{}],\"requested_range_utf16\":[0,{}],\"supplied_available_rect_bits\":{:?},\"supplied_size_bits\":{},\"supplied_spacing_bits\":{},\"supplied_cursor_bits\":0,\"supplied_layout_options\":0,\"visual_to_logical_utf16\":{:?},\"budget_bits\":{},\"old_point_bits\":{:?},\"supplied_ink_bits\":{:?},\"alignment\":{},\"reverse_visual_map\":{},\"selected_range_utf16_inclusive\":{:?},\"block_layout_rect_bits\":{:?},\"block_available_rect_bits\":{:?},\"block_metric_bits\":{:?},\"space_count\":{},\"candidate_operations\":[{}],\"commit_operations\":[{}],\"post_cursor_bits\":{},\"entries\":[{}],\"retained_line\":{{\"point_bits\":{:?},\"layout_rect_bits\":{:?},\"ink_rect_bits\":{:?},\"range_utf16_inclusive\":{:?}}}}}",
            self.name,
            supplied.join(","),
            self.advances.len(),
            [0.0, 0.0, self.budget, 100.0].map(f32::to_bits),
            17.125_f32.to_bits(),
            1.35_f32.to_bits(),
            visual_to_logical,
            self.budget.to_bits(),
            self.old_point.map(f32::to_bits),
            self.ink.map(f32::to_bits),
            self.alignment,
            self.reverse_visual_map,
            selected,
            block_layout,
            block_available,
            block_metrics,
            space_count,
            candidates,
            commits,
            cursor_bits,
            entries,
            [
                read_u32(machine.engine, PLACED_LINES),
                read_u32(machine.engine, PLACED_LINES + 4)
            ],
            rectangle(machine.engine, PLACED_LINES + 8).map(f32::to_bits),
            rectangle(machine.engine, PLACED_LINES + 24).map(f32::to_bits),
            [
                read_u32(machine.engine, PLACED_LINES + 40),
                read_u32(machine.engine, PLACED_LINES + 44)
            ]
        )
    }
}

fn cases() -> Vec<NumericCase> {
    let mut cases = vec![
        NumericCase::ordinary("exact_budget", &[10.0, 12.0, 8.0], &[3, 3, 3], 30.0),
        NumericCase::ordinary(
            "below_budget",
            &[10.0, 12.0, 8.0],
            &[3, 3, 3],
            f32::from_bits(30_f32.to_bits() - 1),
        ),
        NumericCase::ordinary(
            "above_budget",
            &[10.0, 12.0, 8.0],
            &[3, 3, 3],
            f32::from_bits(30_f32.to_bits() + 1),
        ),
        NumericCase::ordinary(
            "grouped_f32",
            &[16_777_216.0, 1.0, 1.0],
            &[1, 3, 3],
            16_777_220.0,
        ),
        NumericCase::ordinary(
            "continuous_f32",
            &[16_777_216.0, 1.0, 1.0],
            &[3, 3, 3],
            16_777_220.0,
        ),
        NumericCase::ordinary(
            "committed_break",
            &[10.0, 2.0, 9.0, 9.0],
            &[2, 2, 4, 4],
            23.0,
        ),
        NumericCase::ordinary("pending_word", &[10.0, 2.0, 9.0, 9.0], &[4, 4, 4, 4], 23.0),
        NumericCase::ordinary(
            "break_at_index_zero",
            &[10.0, 2.0, 9.0, 9.0],
            &[1, 4, 4, 4],
            13.0,
        ),
        NumericCase::ordinary(
            "zero_continuations",
            &[10.0, 0.0, 0.0, 12.0],
            &[4, 4, 4, 4],
            10.0,
        ),
        NumericCase::ordinary(
            "oversized_first_slot",
            &[10.0, 0.0, 0.0, 12.0],
            &[4, 4, 4, 4],
            5.0,
        ),
        NumericCase::ordinary("negative_advance", &[10.0, -3.0, 8.0], &[3, 3, 3], 15.0),
    ];
    for (name, kinds) in [
        ("space_commit", &[0, 1, 0, 0][..]),
        ("tab_commit", &[0, 2, 0, 0][..]),
    ] {
        let mut case = NumericCase::ordinary(name, &[10.0, 2.0, 9.0, 9.0], &[4, 4, 4, 4], 23.0);
        case.kinds = kinds;
        cases.push(case);
    }
    let mut justification = NumericCase::ordinary(
        "space_tab_justification",
        &[10.0, 2.0, 3.0, 4.0],
        &[4, 4, 4, 4],
        40.0,
    );
    justification.kinds = &[0, 1, 2, 0];
    justification.alignment = 3;
    cases.push(justification);
    let mut changed_ink =
        NumericCase::ordinary("ink_control", &[10.0, 12.0, 8.0], &[3, 3, 3], 30.0);
    changed_ink.ink = [1003.0, -1012.0, 2011.0, 1006.0];
    cases.push(changed_ink);
    let mut old_point =
        NumericCase::ordinary("old_point_control", &[10.0, 12.0, 8.0], &[3, 3, 3], 30.0);
    old_point.old_point = [5.25, -7.125];
    cases.push(old_point);
    let mut reverse =
        NumericCase::ordinary("reverse_visual_map", &[10.0, 12.0, 8.0], &[3, 3, 3], 30.0);
    reverse.reverse_visual_map = true;
    cases.push(reverse);
    cases
}

pub(super) fn capture(machine: &mut Machine) {
    bind_native(machine.engine, TEXT + 0xef7d0, TEXT + 0x6c5b0);
    bind_native(machine.engine, TEXT + 0xef870, TEXT + 0x6cc64);
    let _copy = CopyHook::new(machine);
    let mut trace = TraceRecorder::new(machine);
    let cases = cases()
        .into_iter()
        .map(|case| {
            let expected = case.execute(machine, &mut trace, 0);
            for fill in [0xa5, 0xff, 0] {
                assert_eq!(
                    case.execute(machine, &mut trace, fill),
                    expected,
                    "{} memory fill {fill}",
                    case.name
                );
            }
            expected
        })
        .collect::<Vec<_>>()
        .join(",\n");
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"memory_fills\":[0,165,255],\"repeat_zero_fill\":true,\"get_block_info\":\"0x6ab9c\",\"set_layout\":\"0x6b4a4\",\"numeric_hook_addresses\":[\"0x6ae04\",\"0x6ae1c\",\"0x6af34\",\"0x6af3c\",\"0x6af64\",\"0x6af6c\",\"0x6af80\",\"0x6af88\"],\"capture_boundary\":\"Actual native GetBlockInfo selects a single block from supplied ordinary 80-byte MeasureData UTF16 slots, advances, kinds, break ends and available f32 rectangle. Actual candidate/commit register operands/results and selected range/block metrics are read. Actual SetLayout/GetBaseline/alignment/justification/rectangle helpers then place selected slots and retain line logical/ink rectangles. Line maxima are copied from the actual block result into the supplied single-line shell; one-line baseline spacing1.35, size17.125, cursor0, empty obstacles, no objects/bullets, identity or explicitly reversed visual-to-logical map and layout options0 are caller inputs. Cached entry ink and old points are supplied controls. Existing common layout shell initialization is reused. Host memory copy is bounded; actual Base rectangle helpers execute. No text source, native shaping/font selection, source-to-owner choice, ICU break/bidi production, automatic paragraph/wrapping loop, natural paragraph-width ceiling, draw clip gates, full composition or raster/vector output executes. Synthetic large and negative advances expose numeric stages; they are not asserted font-produced metrics.\",\"cases\":[\n{cases}\n]}}"
    );
}
