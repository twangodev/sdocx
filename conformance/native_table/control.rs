use super::*;
use frames::{BASE_SHA256, LAYOUT};
use geometry::{TEXT_SHA256, TextMetrics, WIDGET_SHA256};

const LAYOUT_ROW: u64 = DRAWING_BASE + 0xb0420;
const FIRST_LINE: u64 = DRAWING_BASE + 0xb2360;
const LAYOUT_CELL: u64 = 0x0300_0200;

struct SelectedCells {
    cells: usize,
    slots: Vec<usize>,
}

unsafe extern "C" fn record_cell(engine: Engine, _: u64, _: u32, data: *mut c_void) {
    let selected = unsafe { &mut *data.cast::<SelectedCells>() };
    assert_eq!(read_register(engine, REGISTER_X0), LAYOUT);
    let cell = read_register(engine, REGISTER_X0 + 1);
    let offset = cell.checked_sub(CELL_BASE).unwrap();
    assert_eq!(offset % CELL_STRIDE, 0);
    let slot = usize::try_from(offset / CELL_STRIDE).unwrap();
    assert!(slot < selected.cells && selected.slots.len() < selected.cells);
    selected.slots.push(slot);
}

struct LayoutRecorder {
    engine: Engine,
    hook: usize,
    selected: Box<SelectedCells>,
}

impl LayoutRecorder {
    fn new(machine: &Machine) -> Self {
        write(machine.engine, LAYOUT_CELL, &0xd65f03c0_u32.to_le_bytes());
        bind_native(machine.engine, DRAWING_BASE + 0xbd1f0, LAYOUT_CELL);
        let mut result = Self {
            engine: machine.engine,
            hook: 0,
            selected: Box::new(SelectedCells {
                cells: 0,
                slots: Vec::new(),
            }),
        };
        check(unsafe {
            uc_hook_add(
                machine.engine,
                &mut result.hook,
                4,
                record_cell as *mut c_void,
                ptr::from_mut(result.selected.as_mut()).cast(),
                LAYOUT_CELL,
                LAYOUT_CELL,
            )
        });
        result
    }
}

impl Drop for LayoutRecorder {
    fn drop(&mut self) {
        check(unsafe { uc_hook_del(self.engine, self.hook) });
    }
}

#[derive(Clone, Copy)]
enum Kind {
    FirstLine,
    LayoutRow,
    WarmRow,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::FirstLine => "first_line",
            Self::LayoutRow => "layout_row",
            Self::WarmRow => "warm_row",
        }
    }
}

struct Update {
    row: usize,
    rectangles: Vec<[f32; 4]>,
}

impl Update {
    fn json(&self) -> String {
        format!(
            "{{\"row\":{},\"rectangles\":{:?}}}",
            self.row, self.rectangles
        )
    }
}

struct Action {
    kind: Kind,
    update: Update,
}

struct Case {
    name: String,
    grid: BorderCase,
    metrics: Vec<TextMetrics>,
    last_bottoms: Vec<f32>,
    minima: Vec<f32>,
    pending_gaps: Vec<f32>,
    caches: Vec<Update>,
    actions: Vec<Action>,
}

fn bands(offset: f32) -> Vec<[f32; 4]> {
    vec![
        [0.0, 110.5 + offset, 1080.0, 130.5 + offset],
        [0.0, 230.5 + offset, 1080.0, 250.5 + offset],
    ]
}

impl Case {
    fn new(name: &str, rows: usize, columns: usize) -> Self {
        let mut grid = BorderCase::new("warm-control-grid", rows, columns);
        grid.heights.fill(100.0);
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
            metrics: vec![
                TextMetrics {
                    has_text_layout: true,
                    has_text: true,
                    first_line_height: 20.0,
                    top_margin: 0.0,
                    measured_height: 70.0,
                };
                rows * columns
            ],
            last_bottoms: vec![20.0; rows * columns],
            minima: vec![0.0; rows],
            pending_gaps: vec![0.0; rows],
            caches: (0..rows)
                .map(|row| Update {
                    row,
                    rectangles: bands(0.0),
                })
                .collect(),
            actions: Vec::new(),
        }
    }

    fn action(&mut self, kind: Kind, row: usize, offset: f32) {
        self.actions.push(Action {
            kind,
            update: Update {
                row,
                rectangles: bands(offset),
            },
        });
    }

    fn fixture(&self, machine: &mut Machine, recorder: &mut LayoutRecorder) -> String {
        frames::initialize(machine, &self.grid);
        splits::initialize_cache(machine);
        write(
            machine.engine,
            LAYOUT + 904,
            &(MODEL + 0xd800).to_le_bytes(),
        );
        write(machine.engine, LAYOUT + 912, &64_u64.to_le_bytes());
        write(machine.engine, LAYOUT + 936, &1_f32.to_le_bytes());
        let rows = self.grid.heights.len();
        let columns = self.grid.widths.len();
        recorder.selected.cells = rows * columns;
        let gaps = read_u64(machine.engine, LAYOUT + 856);
        for row in 0..rows {
            write(
                machine.engine,
                ROW_BASE + row as u64 * ROW_STRIDE + 128,
                &self.minima[row].to_le_bytes(),
            );
            write(
                machine.engine,
                gaps + row as u64 * 4,
                &self.pending_gaps[row].to_le_bytes(),
            );
        }
        for (slot, metrics) in self.metrics.iter().enumerate() {
            metrics.supply_last_line(machine, slot, self.last_bottoms[slot]);
        }
        for update in &self.caches {
            splits::supply_bands(machine, &update.rectangles);
            machine.call(splits::UPDATE_SPLIT, &[LAYOUT, update.row as u64]);
        }
        let initial = frames::snapshot(machine, rows, columns);
        let initial_splits = splits::snapshot_json(machine, rows * columns);
        let mut actions = Vec::new();
        for action in &self.actions {
            recorder.selected.slots.clear();
            let row = action.update.row;
            assert!(row < rows);
            splits::supply_bands(machine, &action.update.rectangles);
            let changed = match action.kind {
                Kind::FirstLine => {
                    machine.call(FIRST_LINE, &[LAYOUT, row as u64]);
                    "null".to_owned()
                }
                Kind::LayoutRow | Kind::WarmRow => {
                    let changed = machine.call(LAYOUT_ROW, &[LAYOUT, row as u64]);
                    assert!(changed <= 1);
                    if matches!(action.kind, Kind::WarmRow) {
                        machine.call(DRAWING_BASE + 0xaecf4, &[LAYOUT, row as u64]);
                        machine.call(DRAWING_BASE + 0xb28ec, &[LAYOUT, row as u64]);
                    }
                    (changed == 1).to_string()
                }
            };
            let state = frames::snapshot(machine, rows, columns);
            let splits = splits::snapshot_json(machine, rows * columns);
            actions.push(format!(
                "{{\"kind\":{:?},\"row\":{row},\"rectangles\":{:?},\"changed\":{changed},\"relayout_cells\":{:?},\"frames\":{:?},\"pending_gaps\":{:?},\"cell_splits\":[{splits}]}}",
                action.kind.name(), action.update.rectangles, recorder.selected.slots,
                state.grid, state.pending_gaps,
            ));
        }
        format!(
            "{{\"name\":{:?},\"heights\":{:?},\"widths\":{:?},\"spans\":{:?},\"metrics\":[{}],\"last_line_bottoms\":{:?},\"minima\":{:?},\"cache_updates\":[{}],\"pending_gaps\":{:?},\"initial_frames\":{:?},\"initial_splits\":[{initial_splits}],\"actions\":[{}]}}",
            self.name,
            self.grid.heights,
            self.grid.widths,
            self.grid.spans,
            self.metrics
                .iter()
                .map(|metrics| metrics.json())
                .collect::<Vec<_>>()
                .join(","),
            self.last_bottoms,
            self.minima,
            self.caches
                .iter()
                .map(Update::json)
                .collect::<Vec<_>>()
                .join(","),
            self.pending_gaps,
            initial.grid,
            actions.join(","),
        )
    }
}

pub(super) fn capture(
    machine: &mut Machine,
    base_path: &Path,
    widget_path: &Path,
    text_path: &Path,
) {
    frames::load_base(machine, base_path);
    splits::load_lists(machine);
    geometry::load_measurements(machine, widget_path, text_path);
    bottom::load_compression(machine);
    for (plt, target) in [
        (DRAWING_BASE + 0xbd2a0, splits::UPDATE_SPLIT),
        (DRAWING_BASE + 0xbd290, FIRST_LINE),
        (DRAWING_BASE + 0xbd430, 0x3d3a70),
    ] {
        bind_native(machine.engine, plt, target);
    }
    let mut recorder = LayoutRecorder::new(machine);
    let mut cases = Vec::new();
    for (name, spans) in [
        ("unmerged-warm-control", vec![]),
        ("first-column-row-owner", vec![(0, [3, 1])]),
        ("other-column-row-owner", vec![(1, [3, 1])]),
        ("whole-grid-owner", vec![(0, [3, 2])]),
        ("covered-owner-chain", vec![(0, [2, 1]), (2, [2, 1])]),
    ] {
        let mut case = Case::new(name, 3, 2);
        for (slot, span) in spans {
            case.grid.spans[slot] = span;
        }
        for row in 0..3 {
            case.action(Kind::LayoutRow, row, 0.0);
            case.action(Kind::FirstLine, row, 10.0);
            case.action(Kind::WarmRow, row, 20.0);
        }
        cases.push(case);
    }
    for minimum in [
        f32::from_bits(20_f32.to_bits() - 1),
        20.0,
        f32::from_bits(20_f32.to_bits() + 1),
    ] {
        let mut case = Case::new("unchanged-gap-removal-boundary", 2, 1);
        case.pending_gaps[1] = 40.0;
        case.metrics[1].first_line_height = minimum;
        case.action(Kind::LayoutRow, 1, 0.0);
        cases.push(case);
    }
    for minimum in [
        f32::from_bits(f32::EPSILON.to_bits() - 1),
        f32::EPSILON,
        f32::from_bits(f32::EPSILON.to_bits() + 1),
    ] {
        let mut case = Case::new("first-line-float-epsilon-boundary", 2, 1);
        for update in &mut case.caches {
            update.rectangles = vec![[0.0, 100.5, 1080.0, 120.5]];
        }
        case.metrics[1].first_line_height = minimum;
        for kind in [Kind::LayoutRow, Kind::FirstLine] {
            case.actions.push(Action {
                kind,
                update: Update {
                    row: 1,
                    rectangles: vec![[0.0, 100.5, 1080.0, 120.5]],
                },
            });
        }
        cases.push(case);
    }
    for gap in [
        f32::from_bits(0.001_f32.to_bits() - 1),
        0.001,
        f32::from_bits(0.001_f32.to_bits() + 1),
        40.0,
    ] {
        let mut case = Case::new("first-line-pending-gap-boundary", 3, 1);
        case.pending_gaps[1] = gap;
        case.pending_gaps[2] = 15.0;
        case.action(Kind::FirstLine, 1, 0.0);
        case.action(Kind::LayoutRow, 1, 0.0);
        cases.push(case);
    }
    let mut empty_previous = Case::new("unchanged-gap-with-empty-previous-cache", 2, 1);
    empty_previous.caches[0].rectangles.clear();
    empty_previous.pending_gaps[1] = 40.0;
    empty_previous.action(Kind::LayoutRow, 1, 0.0);
    cases.push(empty_previous);
    let mut empty = Case::new("empty-caches-always-relayout", 2, 2);
    for update in &mut empty.caches {
        update.rectangles.clear();
    }
    for row in [0, 1, 1, 0] {
        empty.actions.push(Action {
            kind: Kind::LayoutRow,
            update: Update {
                row,
                rectangles: Vec::new(),
            },
        });
    }
    cases.push(empty);
    let mut missing = Case::new("first-line-with-missing-cache", 2, 2);
    missing.caches.clear();
    missing.action(Kind::FirstLine, 1, 0.0);
    cases.push(missing);
    let mut changed = Case::new("changed-split-with-pending-gap", 3, 2);
    changed.pending_gaps = vec![0.0, 40.0, 15.0];
    changed.action(Kind::LayoutRow, 1, 25.0);
    changed.action(Kind::LayoutRow, 1, 25.0);
    cases.push(changed);
    let mut no_owner = Case::new("changed-row-with-no-self-owners", 3, 2);
    no_owner.grid.spans[0] = [3, 2];
    no_owner.action(Kind::LayoutRow, 1, 25.0);
    cases.push(no_owner);
    let mut random = 0x7761_726d_6374_726c_u64;
    for index in 0..128 {
        let rows = 1 + index % 5;
        let columns = 1 + index / 5 % 5;
        let mut case = Case::new(&format!("mixed-warm-control-{index}"), rows, columns);
        for row in 0..rows {
            case.grid.heights[row] = [0.0, 10.0, 100.0, 400.25][(index + row) % 4];
            case.minima[row] = [0.0, 12.0, 40.0][(index / 7 + row) % 3];
            case.pending_gaps[row] = [0.0, 0.0005, 20.0, 100.0][(index / 3 + row) % 4];
            for column in 0..columns {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                let slot = row * columns + column;
                case.grid.spans[slot] = [
                    1 + (random as usize % (rows - row)) as u32,
                    1 + (random as usize / rows % (columns - column)) as u32,
                ];
                case.metrics[slot] = TextMetrics {
                    has_text_layout: random & 3 != 0,
                    has_text: random & 4 != 0,
                    first_line_height: [0.0, 0.1, 10.25, 100.0][random as usize / 7 % 4],
                    top_margin: [0.0, 0.3, 12.5][random as usize / 13 % 3],
                    measured_height: [0.0, 0.0005, 12.0, 250.0][random as usize / 17 % 4],
                };
                case.last_bottoms[slot] = [0.0, 20.0, 1000.0][random as usize / 19 % 3];
            }
        }
        for step in 0..2 * rows + 2 {
            let row = step % rows;
            let kind = [Kind::LayoutRow, Kind::FirstLine, Kind::WarmRow][step % 3];
            case.action(kind, row, (step / rows) as f32 * 25.0);
        }
        cases.push(case);
    }
    let mut captures = Vec::new();
    for case in cases {
        machine.heap.allocation_fill = 0;
        let expected = case.fixture(machine, &mut recorder);
        for fill in [0xa5, 0xff] {
            machine.heap.allocation_fill = fill;
            assert_eq!(
                case.fixture(machine, &mut recorder),
                expected,
                "{}",
                case.name
            );
        }
        captures.push(expected);
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"widget_library_sha256\":\"{WIDGET_SHA256}\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"layout_row_address\":\"0xb0420\",\"first_line_address\":\"0xb2360\",\"capture_boundary\":\"native warm-row decisions with fixed supplied two-line caches; layoutCell recorded without shaping; text initialization, single-thread mutex operations and diagnostic interfaces isolated\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}
