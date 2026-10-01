use super::*;

pub(super) const BASE: u64 = 0x0200_0000;
pub(super) const BASE_SHA256: &str =
    "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb";
pub(super) const LAYOUT: u64 = MODEL + 0xc000;
const KEY: u64 = MODEL + 0xe000;
const INITIALIZE: u64 = DRAWING_BASE + 0xaa6b4;
const FIND_FRAME: u64 = DRAWING_BASE + 0xb4d70;
const TEXT_CONSTRUCTOR: u64 = 0x0300_0000;
const REGISTER_S0: i32 = 136;

#[derive(Clone, Copy)]
enum Change {
    OffsetRows { row: usize, amount: f32 },
    ExtendRow { row: usize, amount: f32 },
}

impl Change {
    fn apply(&self, machine: &Machine, rows: usize) {
        let (function, row, amount) = match *self {
            Self::OffsetRows { row, amount } => (DRAWING_BASE + 0xade0c, row, amount),
            Self::ExtendRow { row, amount } => (DRAWING_BASE + 0xaff74, row, amount),
        };
        assert!(row < rows && amount.is_finite());
        register(machine.engine, REGISTER_S0, u64::from(amount.to_bits()));
        machine.call(function, &[LAYOUT, row as u64]);
    }

    fn json(&self) -> String {
        let (kind, row, amount) = match *self {
            Self::OffsetRows { row, amount } => ("offset_rows", row, amount),
            Self::ExtendRow { row, amount } => ("extend_row", row, amount),
        };
        format!("{{\"kind\":{kind:?},\"row\":{row},\"amount\":{amount:?}}}")
    }
}

struct Case {
    grid: BorderCase,
    pending_gaps: Vec<f32>,
    changes: Vec<Change>,
}

impl Case {
    fn new(mut grid: BorderCase) -> Self {
        if grid.outer_border.is_none() {
            grid.native_defaults = true;
            grid.outer_border = Some(
                [BorderStyle {
                    color: 0xff000000,
                    width: 1.0,
                    start_radius: 0.0,
                    end_radius: 0.0,
                }; 4],
            );
        }
        Self {
            pending_gaps: vec![0.0; grid.heights.len()],
            grid,
            changes: Vec::new(),
        }
    }
}

pub(super) struct Snapshot {
    pub grid: Vec<[f32; 4]>,
    pub cached: Vec<[f32; 4]>,
    pub pending_gaps: Vec<f32>,
}

pub(super) fn snapshot(machine: &Machine, rows: usize, columns: usize) -> Snapshot {
    let grid = read_u64(machine.engine, LAYOUT + 832);
    let end = read_u64(machine.engine, LAYOUT + 840);
    assert_eq!(end - grid, rows as u64 * 24);
    let mut frames = Vec::new();
    let mut cached = Vec::new();
    for row in 0..rows {
        let cells = read_u64(machine.engine, grid + row as u64 * 24);
        let end = read_u64(machine.engine, grid + row as u64 * 24 + 8);
        assert_eq!(end - cells, columns as u64 * 16);
        for column in 0..columns {
            let rectangle: [f32; 4] = std::array::from_fn(|axis| {
                read_float(machine.engine, cells + column as u64 * 16 + axis as u64 * 4)
            });
            frames.push(rectangle);
            let cell = CELL_BASE + (row * columns + column) as u64 * CELL_STRIDE;
            write(machine.engine, KEY, &cell.to_le_bytes());
            let node = machine.call(FIND_FRAME, &[LAYOUT + 752, KEY]);
            assert!((HEAP..machine.heap.cursor).contains(&node));
            cached.push(std::array::from_fn::<_, 4, _>(|axis| {
                read_float(machine.engine, node + 24 + axis as u64 * 4)
            }));
        }
    }
    assert_eq!(frames, cached);
    let gaps = read_u64(machine.engine, LAYOUT + 856);
    let pending_gaps = (0..rows)
        .map(|row| read_float(machine.engine, gaps + row as u64 * 4))
        .collect();
    Snapshot {
        grid: frames,
        cached,
        pending_gaps,
    }
}

fn initialize_adapter(machine: &Machine) {
    for (plt, target) in [
        (DRAWING_BASE + 0xbd300, DRAWING_BASE + 0xb325c),
        (DRAWING_BASE + 0xbcd90, 0x3d2b20),
        (DRAWING_BASE + 0xbcda0, 0x3d2b80),
        (DRAWING_BASE + 0xbcdb0, 0x3d2be0),
        (DRAWING_BASE + 0xbd370, 0x3d3f24),
        (DRAWING_BASE + 0xbd380, 0x3d32b0),
        (DRAWING_BASE + 0xbce30, 0x3dc420),
        (DRAWING_BASE + 0xbce60, 0x3c22bc),
        (DRAWING_BASE + 0xb9280, BASE + 0xb11a4),
        (0x4872e0, 0x3d2b20),
        (0x4872f0, 0x3d2b80),
        (DRAWING_BASE + 0xbb470, TEXT_CONSTRUCTOR),
        (DRAWING_BASE + 0xbd0a0, DRAWING_BASE + 0xade0c),
    ] {
        bind_native(machine.engine, plt, target);
    }
    for plt in [0xbd410, 0xbd360] {
        write(
            machine.engine,
            DRAWING_BASE + plt,
            &0xd65f03c0_u32.to_le_bytes(),
        );
    }
}

pub(super) fn initialize(machine: &mut Machine, case: &BorderCase) {
    initialize_with_height_limit(machine, case, None);
}

#[derive(Clone, Copy)]
pub(super) struct HeightLimit {
    pub enabled: bool,
    pub maximum: f32,
}

impl HeightLimit {
    pub fn json_field(self) -> String {
        format!(
            "\"height_limit\":{{\"enabled\":{},\"maximum\":{:?}}},",
            self.enabled, self.maximum
        )
    }
}

pub(super) fn initialize_with_height_limit(
    machine: &mut Machine,
    case: &BorderCase,
    limit: Option<HeightLimit>,
) {
    machine.initialize_borders(case);
    if let Some(limit) = limit {
        write(machine.engine, MODEL + 186, &[u8::from(limit.enabled)]);
        write(machine.engine, MODEL + 176, &limit.maximum.to_le_bytes());
        write(machine.engine, TABLE_OBJECT + 104, &MODEL.to_le_bytes());
        assert_eq!(
            machine.call(0x3db38c, &[TABLE_OBJECT]),
            u64::from(limit.enabled)
        );
        machine.call(0x3dabcc, &[TABLE_OBJECT]);
        assert_eq!(
            read_register(machine.engine, 136) as u32,
            limit.maximum.to_bits()
        );
    }
    initialize_layout(machine);
}

pub(super) fn initialize_layout(machine: &Machine) {
    configure_layout(machine);
    machine.call(INITIALIZE, &[LAYOUT]);
}

pub(super) fn configure_layout(machine: &Machine) {
    initialize_adapter(machine);
    for (index, instruction) in [0xaa1f03e1_u32, 0xd2805002, 0x14000000].iter().enumerate() {
        write(
            machine.engine,
            TEXT_CONSTRUCTOR + index as u64 * 4,
            &instruction.to_le_bytes(),
        );
    }
    bind_native(machine.engine, TEXT_CONSTRUCTOR + 8, MEMSET);
    write(machine.engine, TABLE_OBJECT + 104, &MODEL.to_le_bytes());
    write(machine.engine, LAYOUT + 584, &TABLE_OBJECT.to_le_bytes());
    for (map, buckets) in [
        (LAYOUT + 712, MODEL + 0xd000),
        (LAYOUT + 752, MODEL + 0xd200),
        (LAYOUT + 984, MODEL + 0xd400),
    ] {
        write(machine.engine, map, &buckets.to_le_bytes());
        write(machine.engine, map + 8, &64_u64.to_le_bytes());
        write(machine.engine, map + 32, &1_f32.to_le_bytes());
    }
}

fn fixture(machine: &mut Machine, input: &Case) -> String {
    let case = &input.grid;
    initialize(machine, case);
    let rows = case.heights.len();
    let columns = case.widths.len();
    let initial = snapshot(machine, rows, columns);
    assert!(initial.pending_gaps.iter().all(|gap| *gap == 0.0));
    let gaps = read_u64(machine.engine, LAYOUT + 856);
    for (row, gap) in input.pending_gaps.iter().enumerate() {
        write(machine.engine, gaps + row as u64 * 4, &gap.to_le_bytes());
    }
    for change in &input.changes {
        change.apply(machine, rows);
    }
    let final_state = snapshot(machine, rows, columns);
    format!(
        "{{\"name\":{:?},\"origin\":{:?},\"heights\":{:?},\"widths\":{:?},\"spans\":{:?},\"native_defaults\":{},\"outer_border\":{},\"initial_frames\":{:?},\"pending_gaps\":{:?},\"changes\":[{}],\"frames\":{:?},\"cached_frames\":{:?},\"final_pending_gaps\":{:?}}}",
        case.name,
        case.origin,
        case.heights,
        case.widths,
        case.spans,
        case.native_defaults,
        case.outer_border.map_or("null".to_owned(), |styles| {
            format!(
                "[{}]",
                styles
                    .iter()
                    .map(|style| style.json())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }),
        initial.grid,
        input.pending_gaps,
        input
            .changes
            .iter()
            .map(Change::json)
            .collect::<Vec<_>>()
            .join(","),
        final_state.grid,
        final_state.cached,
        final_state.pending_gaps,
    )
}

pub(super) fn load_base(machine: &Machine, base_path: &Path) {
    map_library(machine.engine, base_path, BASE, BASE_SHA256);
    check(unsafe { uc_mem_map(machine.engine, TEXT_CONSTRUCTOR, 0x1000, 7) });
}

pub(super) fn capture(machine: &mut Machine, base_path: &Path) {
    load_base(machine, base_path);
    let mut cases = Vec::new();
    for (name, rows, columns, changes) in [
        ("unit", 3, 4, vec![]),
        ("column-span", 3, 4, vec![(0, [1, 3])]),
        ("row-span", 3, 4, vec![(1, [3, 1])]),
        ("rectangle-span", 3, 4, vec![(0, [2, 3])]),
        ("latent-column-chain", 1, 4, vec![(0, [1, 2]), (1, [1, 3])]),
        ("latent-row-chain", 4, 1, vec![(0, [2, 1]), (1, [3, 1])]),
    ] {
        let mut case = BorderCase::new(name, rows, columns);
        case.borders.fill(None);
        for (slot, span) in changes {
            case.spans[slot] = span;
        }
        cases.push(Case::new(case));
    }
    for (name, heights, widths, origin) in [
        (
            "fractional-grid",
            vec![0.1, 0.2, 0.3],
            vec![0.3, 0.7, 1.1],
            [13.25, -19.5],
        ),
        (
            "large-grid",
            vec![16_777_216.0, 0.75, 3.25],
            vec![16_777_216.0, 2.5, 7.25],
            [13.25, -19.5],
        ),
        (
            "zero-row-height",
            vec![0.0, 0.0, 10.0],
            vec![10.0, 20.0],
            [13.25, -19.5],
        ),
        (
            "origin-independent",
            vec![31.25, 67.75, 104.25],
            vec![41.125, 83.875],
            [-1_000_000.0, 1_000_000.0],
        ),
    ] {
        let mut case = BorderCase::new(name, heights.len(), widths.len());
        case.borders.fill(None);
        case.heights = heights;
        case.widths = widths;
        case.origin = origin;
        cases.push(Case::new(case));
    }
    for (name, colors, widths) in [
        ("fractional-border", [0xff123456; 4], [0.1, 0.3, 0.2, 0.05]),
        ("unequal-border", [0xff123456; 4], [2.0, 5.0, 8.0, 1.0]),
        (
            "inactive-wide-border",
            [0, 0xff123456, 0, 0xff123456],
            [100.0, 2.0, 200.0, 1.0],
        ),
    ] {
        let mut case = BorderCase::new(name, 3, 4);
        case.borders.fill(None);
        case.outer_border = Some(std::array::from_fn(|edge| BorderStyle {
            color: colors[edge],
            width: widths[edge],
            start_radius: 0.0,
            end_radius: 0.0,
        }));
        cases.push(Case::new(case));
    }
    use Change::{ExtendRow, OffsetRows};
    for (name, pending, changes) in [
        (
            "positive-consumes-gap",
            [60.0, 30.0, 0.0],
            vec![OffsetRows {
                row: 0,
                amount: 20.0,
            }],
        ),
        (
            "positive-crosses-gaps",
            [60.0, 30.0, 0.0],
            vec![OffsetRows {
                row: 0,
                amount: 100.0,
            }],
        ),
        (
            "negative-removes-gaps",
            [60.0, 30.0, 10.0],
            vec![OffsetRows {
                row: 0,
                amount: -10.0,
            }],
        ),
        (
            "negative-from-middle",
            [60.0, 30.0, 10.0],
            vec![OffsetRows {
                row: 1,
                amount: -10.0,
            }],
        ),
        (
            "exact-gap-boundary",
            [60.0, 0.0, 0.0],
            vec![OffsetRows {
                row: 0,
                amount: 60.0,
            }],
        ),
        (
            "epsilon-gap-boundary",
            [f32::EPSILON, 0.0, 0.0],
            vec![OffsetRows {
                row: 0,
                amount: 0.0,
            }],
        ),
        (
            "grow-row",
            [0.0; 3],
            vec![
                ExtendRow {
                    row: 1,
                    amount: 50.0,
                },
                OffsetRows {
                    row: 2,
                    amount: 50.0,
                },
            ],
        ),
        (
            "shrink-row-through-gaps",
            [0.0, 20.0, 0.0],
            vec![
                ExtendRow {
                    row: 0,
                    amount: -10.0,
                },
                OffsetRows {
                    row: 1,
                    amount: -10.0,
                },
            ],
        ),
    ] {
        for merged in [false, true] {
            let mut input = Case::new(BorderCase::new(name, 3, 4));
            input.grid.borders.fill(None);
            if merged {
                input.grid.spans[0] = [2, 3];
            }
            input.pending_gaps = pending.to_vec();
            input.changes = changes.clone();
            cases.push(input);
        }
    }
    let mut random = 0x6672_616d_6573_u64;
    for index in 0..64 {
        let mut input = Case::new(BorderCase::new("mixed-row-changes", 3, 4));
        input.grid.borders.fill(None);
        if index % 2 == 1 {
            input.grid.spans[0] = [2, 3];
        }
        input.pending_gaps = vec![0.0, 5.0, 20.0];
        for _ in 0..8 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let row = random as usize % 3;
            let amount = [0.0, -0.0, 0.125, -0.125, 50.0, -10.0][random as usize / 3 % 6];
            input.changes.push(if random & 64 == 0 {
                ExtendRow { row, amount: 2.5 }
            } else {
                OffsetRows { row, amount }
            });
        }
        cases.push(input);
    }
    let mut captures = Vec::new();
    for case in &cases {
        machine.heap.allocation_fill = 0;
        let expected = fixture(machine, case);
        for fill in [0xa5, 0xff] {
            machine.heap.allocation_fill = fill;
            assert_eq!(fixture(machine, case), expected, "{}", case.grid.name);
        }
        captures.push(expected);
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"initializer_address\":\"0xaa6b4\",\"text_adapter\":\"zeroed text-layout storage; SetObject and SetTextScale omitted; native shaping not executed\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}
