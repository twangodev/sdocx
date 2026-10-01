use super::*;
use frames::{BASE, BASE_SHA256, LAYOUT};

pub(super) const UPDATE_SPLIT: u64 = DRAWING_BASE + 0xb24cc;
const FIND_SPLIT: u64 = DRAWING_BASE + 0xb4194;
const KEY: u64 = MODEL + 0xe000;
const RECTANGLES: u64 = MODEL + 0xe100;
const DIAGNOSTIC_TLS: u64 = 0x0300_0100;

pub(super) fn initialize_cache(machine: &Machine) {
    write(
        machine.engine,
        LAYOUT + 792,
        &(MODEL + 0xd600).to_le_bytes(),
    );
    write(machine.engine, LAYOUT + 800, &64_u64.to_le_bytes());
    write(machine.engine, LAYOUT + 824, &1_f32.to_le_bytes());
}

pub(super) fn supply_bands(machine: &Machine, rectangles: &[[f32; 4]]) {
    assert!(rectangles.len() <= 8);
    write(machine.engine, LAYOUT + 688, &RECTANGLES.to_le_bytes());
    write(
        machine.engine,
        LAYOUT + 696,
        &(RECTANGLES + rectangles.len() as u64 * 16).to_le_bytes(),
    );
    for (index, rectangle) in rectangles.iter().enumerate() {
        for (axis, coordinate) in rectangle.iter().enumerate() {
            assert!(coordinate.is_finite());
            write(
                machine.engine,
                RECTANGLES + index as u64 * 16 + axis as u64 * 4,
                &coordinate.to_le_bytes(),
            );
        }
    }
}

pub(super) fn snapshot(machine: &Machine, cells: usize) -> Vec<Option<Vec<[f32; 4]>>> {
    (0..cells)
        .map(|slot| {
            let cell = CELL_BASE + slot as u64 * CELL_STRIDE;
            write(machine.engine, KEY, &cell.to_le_bytes());
            let node = machine.call(FIND_SPLIT, &[LAYOUT + 792, KEY]);
            if node == 0 {
                return None;
            }
            let list = read_u64(machine.engine, node + 24);
            assert!((HEAP..machine.heap.cursor).contains(&list));
            let count = machine.call(BASE + 0x9d120, &[list]);
            assert!(count <= 8);
            Some(
                (0..count)
                    .map(|index| {
                        let pointer = machine.call(BASE + 0x9d608, &[list, index]);
                        assert!((HEAP..machine.heap.cursor).contains(&pointer));
                        std::array::from_fn(|axis| {
                            read_float(machine.engine, pointer + axis as u64 * 4)
                        })
                    })
                    .collect(),
            )
        })
        .collect()
}

pub(super) fn snapshot_json(machine: &Machine, cells: usize) -> String {
    snapshot(machine, cells)
        .iter()
        .map(|rectangles| {
            rectangles
                .as_ref()
                .map_or("null".to_owned(), |r| format!("{r:?}"))
        })
        .collect::<Vec<_>>()
        .join(",")
}

struct Update {
    row: usize,
    rectangles: Vec<[f32; 4]>,
}

struct Case {
    name: String,
    grid: BorderCase,
    updates: Vec<Update>,
}

fn bands(offset: f32) -> Vec<[f32; 4]> {
    vec![
        [0.0, 40.0 + offset, 1080.0, 60.0 + offset],
        [0.0, 150.0 + offset, 1080.0, 180.0 + offset],
    ]
}

impl Case {
    fn new(name: &str, rows: usize, columns: usize) -> Self {
        let mut grid = BorderCase::new("row-split-grid", rows, columns);
        grid.borders.fill(None);
        grid.native_defaults = true;
        grid.outer_border = Some(
            [BorderStyle {
                color: 0xff000000,
                width: 1.0,
                start_radius: 0.0,
                end_radius: 0.0,
            }; 4],
        );
        Self {
            name: name.to_owned(),
            grid,
            updates: Vec::new(),
        }
    }

    fn fixture(&self, machine: &mut Machine) -> String {
        frames::initialize(machine, &self.grid);
        let rows = self.grid.heights.len();
        let columns = self.grid.widths.len();
        initialize_cache(machine);
        let frames = frames::snapshot(machine, rows, columns).grid;
        let mut updates = Vec::new();
        for update in &self.updates {
            assert!(update.row < rows);
            supply_bands(machine, &update.rectangles);
            let changed = machine.call(UPDATE_SPLIT, &[LAYOUT, update.row as u64]);
            assert!(changed <= 1);
            let splits = snapshot_json(machine, rows * columns);
            updates.push(format!(
                "{{\"row\":{},\"rectangles\":{:?},\"changed\":{},\"cell_splits\":[{}]}}",
                update.row,
                update.rectangles,
                changed == 1,
                splits
            ));
        }
        format!(
            "{{\"name\":{:?},\"heights\":{:?},\"widths\":{:?},\"spans\":{:?},\"frames\":{frames:?},\"updates\":[{}]}}",
            self.name,
            self.grid.heights,
            self.grid.widths,
            self.grid.spans,
            updates.join(",")
        )
    }
}

pub(super) fn load_lists(machine: &Machine) {
    for (plt, target) in [
        (DRAWING_BASE + 0xbd340, 0x3d2d3c),
        (DRAWING_BASE + 0xbd2d0, DRAWING_BASE + 0xb2cac),
        (DRAWING_BASE + 0xbd2e0, DRAWING_BASE + 0xb2e18),
        (DRAWING_BASE + 0xbd450, BASE + 0xb14b0),
        (DRAWING_BASE + 0xb9380, BASE + 0x9c868),
        (DRAWING_BASE + 0xb9390, BASE + 0x9c928),
        (DRAWING_BASE + 0xb93a0, BASE + 0x9d000),
        (DRAWING_BASE + 0xb9400, BASE + 0x9d1c8),
        (DRAWING_BASE + 0xb9410, BASE + 0x9d120),
        (DRAWING_BASE + 0xb9420, BASE + 0x9e894),
        (DRAWING_BASE + 0xb9430, BASE + 0x9d4f0),
        (DRAWING_BASE + 0xb9440, BASE + 0x9d73c),
        (DRAWING_BASE + 0xbc950, BASE + 0x9e2ec),
        (DRAWING_BASE + 0xba6c0, BASE + 0x9d608),
        (BASE + 0xe55d0, DELETE),
        (BASE + 0xe5650, DELETE),
        (BASE + 0xe55f0, NEW),
        (BASE + 0xe56f0, NEW),
        (BASE + 0xe5740, BASE + 0x9d000),
        (BASE + 0xe5e50, BASE + 0x9d120),
        (BASE + 0xe5e60, BASE + 0x9d1c8),
        (BASE + 0xe5e70, BASE + 0x9d40c),
        (BASE + 0xe5e80, BASE + 0x9d4f0),
        (BASE + 0xe5e90, BASE + 0x9d608),
        (BASE + 0xe5ea0, BASE + 0x9d73c),
        (BASE + 0xe5ef0, BASE + 0x9e894),
        (BASE + 0xe5f10, BASE + 0x9c87c),
    ] {
        bind_native(machine.engine, plt, target);
    }
    for plt in [0xe5f20, 0xe5f30, 0xe5f40, 0xe5f50, 0xe5620, 0xe6ef0] {
        write(machine.engine, BASE + plt, &0xd65f03c0_u32.to_le_bytes());
    }
    for (index, instruction) in [
        0xd2800000_u32 | (((TLS + 0x100) as u32 & 0xffff) << 5),
        0xf2a00000_u32 | ((((TLS + 0x100) >> 16) as u32) << 5),
        0xd65f03c0,
    ]
    .iter()
    .enumerate()
    {
        write(
            machine.engine,
            DIAGNOSTIC_TLS + index as u64 * 4,
            &instruction.to_le_bytes(),
        );
    }
    bind_native(machine.engine, BASE + 0xe7bb0, DIAGNOSTIC_TLS);
}

pub(super) fn capture(machine: &mut Machine, base_path: &Path) {
    frames::load_base(machine, base_path);
    load_lists(machine);
    let mut cases = Vec::new();
    for (name, rows, columns, spans) in [
        ("unmerged", 3, 2, vec![]),
        ("column-span", 3, 3, vec![(0, [1, 3])]),
        ("row-span-first-column", 3, 2, vec![(0, [3, 1])]),
        ("row-span-other-column", 3, 2, vec![(1, [3, 1])]),
        ("whole-grid-owner", 3, 3, vec![(0, [3, 3])]),
        ("covered-row-chain", 4, 1, vec![(0, [2, 1]), (1, [3, 1])]),
    ] {
        let mut case = Case::new(name, rows, columns);
        for (slot, span) in spans {
            case.grid.spans[slot] = span;
        }
        for row in 0..rows {
            case.updates.push(Update {
                row,
                rectangles: bands(0.0),
            });
        }
        for row in (0..rows).rev() {
            case.updates.push(Update {
                row,
                rectangles: bands(25.0),
            });
        }
        cases.push(case);
    }
    let mut empty = Case::new("distinct-empty-lists-change", 2, 2);
    empty.updates = (0..4)
        .map(|_| Update {
            row: 0,
            rectangles: Vec::new(),
        })
        .collect();
    cases.push(empty);
    let mut later = Case::new("unchanged-first-retains-later", 2, 3);
    later.updates.push(Update {
        row: 0,
        rectangles: bands(0.0),
    });
    let mut rectangles = bands(0.0);
    rectangles[1][1] += 25.0;
    later.updates.push(Update { row: 0, rectangles });
    cases.push(later);
    for difference in [
        f32::from_bits(0.001_f32.to_bits() - 1),
        0.001,
        f32::from_bits(0.001_f32.to_bits() + 1),
    ] {
        let mut case = Case::new("first-rectangle-threshold", 1, 2);
        case.updates.push(Update {
            row: 0,
            rectangles: bands(0.0),
        });
        let mut rectangles = bands(0.0);
        rectangles[0][0] = difference;
        case.updates.push(Update { row: 0, rectangles });
        cases.push(case);
    }
    let mut random = 0x726f_772d_7370_6c69_u64;
    for index in 0..128 {
        let rows = 1 + index % 5;
        let columns = 1 + index / 5 % 5;
        let mut case = Case::new(&format!("mixed-split-updates-{index}"), rows, columns);
        for row in 0..rows {
            for column in 0..columns {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                case.grid.spans[row * columns + column] = [
                    1 + (random as usize % (rows - row)) as u32,
                    1 + (random as usize / rows % (columns - column)) as u32,
                ];
            }
        }
        for step in 0..2 * rows + 2 {
            let mut rectangles = bands((step / 2) as f32 * 25.0);
            if step % 2 == 1 {
                rectangles[1][2] = 900.0;
            }
            if step % 5 == 4 {
                rectangles.clear();
            }
            if step % 7 == 6 {
                rectangles.reverse();
            }
            case.updates.push(Update {
                row: step % rows,
                rectangles,
            });
        }
        cases.push(case);
    }
    let mut captures = Vec::new();
    for case in cases {
        machine.heap.allocation_fill = 0;
        let expected = case.fixture(machine);
        for fill in [0xa5, 0xff] {
            machine.heap.allocation_fill = fill;
            assert_eq!(case.fixture(machine), expected, "{}", case.name);
        }
        captures.push(expected);
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"update_split_address\":\"0xb24cc\",\"cache_comparison_address\":\"0xb2e18\",\"capture_boundary\":\"native frame and list selection with supplied bands; text initialization, single-thread mutex operations and diagnostic interfaces isolated\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}
