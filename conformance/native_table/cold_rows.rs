use super::*;
use frames::{BASE, BASE_SHA256, LAYOUT};
use geometry::WIDGET_SHA256;

const WIDGET: u64 = 0x0400_0000;
const COLD_ROWS: u64 = DRAWING_BASE + 0xaaf2c;
const FIND_LAYOUT: u64 = DRAWING_BASE + 0xb3788;
const KEY: u64 = MODEL + 0xe000;
const PADDING: u64 = 0x0300_0200;
const MEASURE: u64 = 0x0300_0220;
const RETURN_ZERO: u64 = 0x0300_0240;

struct Call {
    slot: usize,
    dimensions: [f32; 2],
    padding: u64,
}

#[derive(Default)]
struct Observation {
    layouts: Vec<u64>,
    pending: Option<(u64, u64)>,
    calls: Vec<Call>,
}

unsafe extern "C" fn observe(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observed = unsafe { &mut *data.cast::<Observation>() };
    let layout = read_register(engine, REGISTER_X0);
    match address {
        PADDING => {
            assert!(observed.pending.is_none());
            observed.pending = Some((layout, read_register(engine, REGISTER_X0 + 1)));
        }
        MEASURE => {
            let (expected, padding) = observed.pending.take().unwrap();
            assert_eq!(expected, layout);
            let slot = observed
                .layouts
                .iter()
                .position(|value| *value == layout)
                .unwrap();
            assert!(observed.calls.len() < observed.layouts.len());
            observed.calls.push(Call {
                slot,
                dimensions: [
                    read_float(engine, layout + 524),
                    read_float(engine, layout + 528),
                ],
                padding,
            });
        }
        _ => unreachable!(),
    }
}

struct Recorder {
    engine: Engine,
    hook: usize,
    observed: Box<Observation>,
}

impl Recorder {
    fn new(machine: &Machine) -> Self {
        for address in [PADDING, MEASURE] {
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
        }
        write(machine.engine, RETURN_ZERO, &0x52800000_u32.to_le_bytes());
        write(
            machine.engine,
            RETURN_ZERO + 4,
            &0xd65f03c0_u32.to_le_bytes(),
        );
        for (plt, target) in [
            (DRAWING_BASE + 0xbd0e0, DRAWING_BASE + 0xae914),
            (DRAWING_BASE + 0xbd0f0, DRAWING_BASE + 0xaea60),
            (DRAWING_BASE + 0xbd4a0, PADDING),
            (DRAWING_BASE + 0xbd4b0, WIDGET + 0xd398c),
            (DRAWING_BASE + 0xbd4c0, WIDGET + 0xd399c),
            (DRAWING_BASE + 0xbd4d0, RETURN_ZERO),
            (DRAWING_BASE + 0xbd3f0, RETURN_ZERO),
            (DRAWING_BASE + 0xbd4f0, MEASURE),
            (DRAWING_BASE + 0xbd420, WIDGET + 0xd3b78),
            (DRAWING_BASE + 0xb8b10, BASE + 0xb108c),
            (DRAWING_BASE + 0xb8b20, BASE + 0xb109c),
            (DRAWING_BASE + 0xbd1b0, DRAWING_BASE + 0xaff74),
        ] {
            bind_native(machine.engine, plt, target);
        }
        let mut result = Self {
            engine: machine.engine,
            hook: 0,
            observed: Box::default(),
        };
        check(unsafe {
            uc_hook_add(
                machine.engine,
                &mut result.hook,
                4,
                observe as *mut c_void,
                ptr::from_mut(result.observed.as_mut()).cast(),
                PADDING,
                MEASURE,
            )
        });
        result
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        check(unsafe { uc_hook_del(self.engine, self.hook) });
    }
}

struct Run {
    start: usize,
    rectangles: Vec<[f32; 4]>,
    measured_heights: Vec<f32>,
}

struct Case {
    name: String,
    grid: BorderCase,
    minima: Vec<f32>,
    maxima: Vec<f32>,
    pending_gaps: Vec<f32>,
    runs: Vec<Run>,
}

impl Case {
    fn new(name: &str, rows: usize, columns: usize) -> Self {
        let mut grid = BorderCase::new("cold-row-grid", rows, columns);
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
            minima: vec![0.0; rows],
            maxima: vec![f32::MAX; rows],
            pending_gaps: vec![0.0; rows],
            runs: vec![Run {
                start: 0,
                rectangles: vec![[0.0, 120.5, 1080.0, 150.5], [0.0, 230.5, 1080.0, 250.5]],
                measured_heights: (0..rows * columns)
                    .map(|slot| 50.0 + slot as f32 * 30.0)
                    .collect(),
            }],
        }
    }

    fn fixture(&self, machine: &mut Machine, recorder: &mut Recorder) -> String {
        frames::initialize(machine, &self.grid);
        splits::initialize_cache(machine);
        let rows = self.grid.heights.len();
        let columns = self.grid.widths.len();
        let initial = frames::snapshot(machine, rows, columns);
        let gaps = read_u64(machine.engine, LAYOUT + 856);
        for row in 0..rows {
            let pointer = ROW_BASE + row as u64 * ROW_STRIDE;
            write(
                machine.engine,
                pointer + 128,
                &self.minima[row].to_le_bytes(),
            );
            write(
                machine.engine,
                pointer + 124,
                &self.maxima[row].to_le_bytes(),
            );
            write(
                machine.engine,
                gaps + row as u64 * 4,
                &self.pending_gaps[row].to_le_bytes(),
            );
        }
        recorder.observed.layouts.clear();
        for slot in 0..rows * columns {
            let cell = CELL_BASE + slot as u64 * CELL_STRIDE;
            write(machine.engine, KEY, &cell.to_le_bytes());
            let node = machine.call(FIND_LAYOUT, &[LAYOUT + 712, KEY]);
            assert!((HEAP..machine.heap.cursor).contains(&node));
            let layout = read_u64(machine.engine, node + 24);
            assert!((HEAP..machine.heap.cursor).contains(&layout));
            recorder.observed.layouts.push(layout);
        }
        let mut runs = Vec::new();
        for run in &self.runs {
            assert!(run.start <= rows);
            recorder.observed.calls.clear();
            assert!(recorder.observed.pending.is_none());
            for (layout, height) in recorder.observed.layouts.iter().zip(&run.measured_heights) {
                write(machine.engine, layout + 404, &height.to_le_bytes());
            }
            splits::supply_bands(machine, &run.rectangles);
            machine.call(COLD_ROWS, &[LAYOUT, run.start as u64]);
            assert!(recorder.observed.pending.is_none());
            assert_eq!(
                recorder
                    .observed
                    .calls
                    .iter()
                    .map(|call| call.slot)
                    .collect::<Vec<_>>(),
                (run.start * columns..rows * columns).collect::<Vec<_>>()
            );
            let mut calls = Vec::new();
            for call in &recorder.observed.calls {
                assert!((HEAP..machine.heap.cursor).contains(&call.padding));
                let count = machine.call(BASE + 0x9d120, &[call.padding]);
                assert!(count <= 8);
                let rectangles: Vec<[f32; 4]> = (0..count)
                    .map(|index| {
                        let pointer = machine.call(BASE + 0x9d608, &[call.padding, index]);
                        assert!((HEAP..machine.heap.cursor).contains(&pointer));
                        std::array::from_fn(|axis| {
                            read_float(machine.engine, pointer + axis as u64 * 4)
                        })
                    })
                    .collect();
                calls.push(format!(
                    "{{\"slot\":{},\"dimensions\":{:?},\"rectangles\":{rectangles:?}}}",
                    call.slot, call.dimensions
                ));
            }
            let state = frames::snapshot(machine, rows, columns);
            let splits = splits::snapshot_json(machine, rows * columns);
            runs.push(format!("{{\"start\":{},\"rectangles\":{:?},\"measured_heights\":{:?},\"calls\":[{}],\"frames\":{:?},\"pending_gaps\":{:?},\"cell_splits\":[{splits}]}}", run.start, run.rectangles, run.measured_heights, calls.join(","), state.grid, state.pending_gaps));
        }
        format!(
            "{{\"name\":{:?},\"heights\":{:?},\"widths\":{:?},\"spans\":{:?},\"minima\":{:?},\"maxima\":{:?},\"pending_gaps\":{:?},\"initial_frames\":{:?},\"runs\":[{}]}}",
            self.name,
            self.grid.heights,
            self.grid.widths,
            self.grid.spans,
            self.minima,
            self.maxima,
            self.pending_gaps,
            initial.grid,
            runs.join(",")
        )
    }
}

pub(super) fn capture(machine: &mut Machine, base_path: &Path, widget_path: &Path) {
    frames::load_base(machine, base_path);
    splits::load_lists(machine);
    map_library(machine.engine, widget_path, WIDGET, WIDGET_SHA256);
    let mut recorder = Recorder::new(machine);
    let mut cases = Vec::new();
    for (name, spans) in [
        ("unmerged-cold-rows", vec![]),
        ("column-span-raw-measurements", vec![(0, [1, 3])]),
        ("row-span-raw-measurements", vec![(0, [3, 1])]),
        ("whole-grid-raw-measurements", vec![(0, [3, 3])]),
        (
            "covered-owner-chain-measurements",
            vec![(0, [2, 1]), (3, [2, 1])],
        ),
    ] {
        let mut case = Case::new(name, 3, 3);
        for (slot, span) in spans {
            case.grid.spans[slot] = span;
        }
        cases.push(case);
    }
    for measured in [
        0.0,
        0.0005,
        0.001,
        10.0,
        100.0,
        f32::from_bits(100_f32.to_bits() + 1),
    ] {
        let mut case = Case::new("positive-growth-boundary", 3, 2);
        case.runs[0].measured_heights.fill(measured);
        case.minima.fill(300.0);
        case.maxima.fill(1.0);
        cases.push(case);
    }
    for start in 0..=3 {
        let mut case = Case::new("cold-start-row", 3, 2);
        case.runs[0].start = start;
        case.pending_gaps = vec![30.0, 20.0, 10.0];
        cases.push(case);
    }
    let mut repeated = Case::new("repeated-cold-measurement", 3, 2);
    for heights in [
        [150.0, 200.0, 300.0, 100.0, 400.0, 250.0],
        [20.0; 6],
        [250.0; 6],
    ] {
        repeated.runs.push(Run {
            start: 0,
            rectangles: Vec::new(),
            measured_heights: heights.to_vec(),
        });
    }
    cases.push(repeated);
    let mut random = 0x636f_6c64_726f_7773_u64;
    for index in 0..128 {
        let rows = 1 + index % 5;
        let columns = 1 + index / 5 % 5;
        let mut case = Case::new(&format!("mixed-cold-rows-{index}"), rows, columns);
        case.runs[0].start = index % (rows + 1);
        case.runs[0].rectangles = match index % 4 {
            0 => Vec::new(),
            1 => vec![[0.0, 0.5, 1080.0, 10.5]],
            2 => vec![[0.0, 100.5, 1080.0, 120.5], [0.0, 300.5, 1080.0, 340.5]],
            _ => vec![[0.0, 400.5, 1080.0, 440.5], [0.0, 50.5, 1080.0, 60.5]],
        };
        for row in 0..rows {
            case.grid.heights[row] = [0.0, 0.0005, 20.125, 100.25][(index + row) % 4];
            case.minima[row] = [0.0, 150.0][(index + row) % 2];
            case.maxima[row] = [1.0, 50.0, f32::MAX][(index + row) % 3];
            case.pending_gaps[row] = [0.0, 10.125, 50.25][(index / 3 + row) % 3];
            for column in 0..columns {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                let slot = row * columns + column;
                case.grid.spans[slot] = [
                    1 + (random as usize % (rows - row)) as u32,
                    1 + (random as usize / rows % (columns - column)) as u32,
                ];
                case.runs[0].measured_heights[slot] =
                    [0.0, 0.0005, 0.001, 19.875, 100.0, 150.5, 1000.25][random as usize % 7];
            }
        }
        cases.push(case);
    }
    let mut captured = Vec::new();
    for case in &cases {
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
        captured.push(expected);
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"widget_library_sha256\":\"{WIDGET_SHA256}\",\"cold_rows_address\":\"0xaaf2c\",\"update_cell_address\":\"0xae914\",\"measure_cell_address\":\"0xaea60\",\"measurement_inputs\":\"supplied cached measured heights; native cold driver, cell frame inputs, measurement differences and row updates; text update, measurement, padding assignment and font selection intercepted\",\"cases\":[\n{}\n]}}",
        captured.join(",\n")
    );
}
