use super::*;
use frames::{BASE, BASE_SHA256, LAYOUT};
use geometry::{TEXT_SHA256, TextMetrics, WIDGET_SHA256};

const WIDGET: u64 = 0x0400_0000;
const UPDATE_CELL: u64 = DRAWING_BASE + 0xae914;
const MEASURE_TEXT: u64 = 0x0300_0220;
const LAYOUT_CELL: u64 = 0x0300_0240;
const RETURN_ZERO: u64 = 0x0300_0260;

#[derive(Default)]
struct Observation {
    heights: Vec<f32>,
    active: Option<usize>,
    cold: Vec<usize>,
    warm: Vec<usize>,
}

fn slot(cell: u64, cells: usize) -> usize {
    let offset = cell.checked_sub(CELL_BASE).unwrap();
    assert_eq!(offset % CELL_STRIDE, 0);
    let slot = usize::try_from(offset / CELL_STRIDE).unwrap();
    assert!(slot < cells);
    slot
}

unsafe extern "C" fn observe(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observation = unsafe { &mut *data.cast::<Observation>() };
    match address {
        UPDATE_CELL => {
            assert_eq!(read_register(engine, REGISTER_X0), LAYOUT);
            observation.active = Some(slot(
                read_register(engine, REGISTER_X0 + 1),
                observation.heights.len(),
            ));
        }
        MEASURE_TEXT => {
            let slot = observation.active.take().unwrap();
            let wrapper = read_register(engine, REGISTER_X0);
            write(
                engine,
                wrapper + 404,
                &observation.heights[slot].to_le_bytes(),
            );
            observation.cold.push(slot);
        }
        LAYOUT_CELL => {
            assert_eq!(read_register(engine, REGISTER_X0), LAYOUT);
            observation.warm.push(slot(
                read_register(engine, REGISTER_X0 + 1),
                observation.heights.len(),
            ));
        }
        _ => unreachable!(),
    }
}

struct Recorder {
    engine: Engine,
    hooks: Vec<usize>,
    observation: Box<Observation>,
}

impl Recorder {
    fn new(machine: &Machine) -> Self {
        for address in [MEASURE_TEXT, LAYOUT_CELL] {
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
        }
        write(machine.engine, RETURN_ZERO, &0x52800000_u32.to_le_bytes());
        write(
            machine.engine,
            RETURN_ZERO + 4,
            &0xd65f03c0_u32.to_le_bytes(),
        );
        for (plt, target) in [
            (DRAWING_BASE + 0xbcfa0, DRAWING_BASE + 0xaa234),
            (DRAWING_BASE + 0xbce50, 0x3d2e84),
            (DRAWING_BASE + 0xbcd70, 0x2d2208),
            (DRAWING_BASE + 0xbcfd0, DRAWING_BASE + 0xaa6b4),
            (DRAWING_BASE + 0xbcfe0, DRAWING_BASE + 0xaaf2c),
            (DRAWING_BASE + 0xbcff0, DRAWING_BASE + 0xab168),
            (DRAWING_BASE + 0xbcfc0, DRAWING_BASE + 0xaa448),
            (DRAWING_BASE + 0xbcfb0, DRAWING_BASE + 0xaa370),
            (DRAWING_BASE + 0xbd0e0, UPDATE_CELL),
            (DRAWING_BASE + 0xbd0f0, DRAWING_BASE + 0xaea60),
            (DRAWING_BASE + 0xbd4b0, WIDGET + 0xd398c),
            (DRAWING_BASE + 0xbd4c0, WIDGET + 0xd399c),
            (DRAWING_BASE + 0xbd4f0, MEASURE_TEXT),
            (DRAWING_BASE + 0xb8b10, BASE + 0xb108c),
            (DRAWING_BASE + 0xbd1e0, DRAWING_BASE + 0xb0420),
            (DRAWING_BASE + 0xbd1f0, LAYOUT_CELL),
            (DRAWING_BASE + 0xbd110, DRAWING_BASE + 0xaecf4),
            (DRAWING_BASE + 0xbd2b0, DRAWING_BASE + 0xb28ec),
            (DRAWING_BASE + 0xbd2a0, splits::UPDATE_SPLIT),
            (DRAWING_BASE + 0xbd290, DRAWING_BASE + 0xb2360),
            (DRAWING_BASE + 0xbd430, 0x3d3a70),
        ] {
            bind_native(machine.engine, plt, target);
        }
        for plt in [
            0xb8a30, 0xbd510, 0xbd520, 0xbd4a0, 0xbd4d0, 0xbd3f0, 0xbd120, 0xbd190, 0xbd210,
            0xbd130, 0xbd140,
        ] {
            bind_native(machine.engine, DRAWING_BASE + plt, RETURN_ZERO);
        }
        let mut recorder = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            observation: Box::default(),
        };
        for address in [UPDATE_CELL, MEASURE_TEXT, LAYOUT_CELL] {
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    observe as *mut c_void,
                    ptr::from_mut(recorder.observation.as_mut()).cast(),
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

struct Case {
    name: String,
    cold_via_layout: bool,
    grid: BorderCase,
    measured: Vec<f32>,
    first_heights: Vec<f32>,
    last_bottoms: Vec<f32>,
    bands: Vec<[f32; 4]>,
    warm_bands: Vec<Vec<[f32; 4]>>,
}

impl Case {
    fn new(name: &str, rows: usize, columns: usize) -> Self {
        let mut grid = BorderCase::new("lifecycle-grid", rows, columns);
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
            cold_via_layout: false,
            grid,
            measured: (0..rows * columns)
                .map(|i| 70.0 + i as f32 * 20.0)
                .collect(),
            first_heights: vec![20.0; rows * columns],
            last_bottoms: vec![50.0; rows * columns],
            bands: vec![[0.0, 110.5, 1080.0, 130.5], [0.0, 230.5, 1080.0, 250.5]],
            warm_bands: vec![vec![], vec![[0.0, 150.5, 1080.0, 180.5]]],
        }
    }

    fn fixture(&self, machine: &mut Machine, recorder: &mut Recorder) -> String {
        machine.initialize_borders(&self.grid);
        frames::configure_layout(machine);
        write(
            machine.engine,
            TABLE_OBJECT + 16,
            &(MODEL + 0xa000).to_le_bytes(),
        );
        write(machine.engine, MODEL + 0xa07c, &1_u32.to_le_bytes());
        write(machine.engine, LAYOUT, &(MODEL + 0xb000).to_le_bytes());
        write(
            machine.engine,
            MODEL + 0xb038,
            &(DRAWING_BASE + 0xaa530).to_le_bytes(),
        );
        splits::initialize_cache(machine);
        write(
            machine.engine,
            LAYOUT + 904,
            &(MODEL + 0xd800).to_le_bytes(),
        );
        write(machine.engine, LAYOUT + 912, &64_u64.to_le_bytes());
        write(machine.engine, LAYOUT + 936, &1_f32.to_le_bytes());
        *recorder.observation = Observation {
            heights: self.measured.clone(),
            ..Observation::default()
        };
        let rows = self.grid.heights.len();
        let columns = self.grid.widths.len();
        splits::supply_bands(machine, &self.bands);
        write(machine.engine, LAYOUT + 649, &[1]);
        machine.call(
            DRAWING_BASE
                + if self.cold_via_layout {
                    0xaa3d4
                } else {
                    0xaa530
                },
            &[LAYOUT],
        );
        assert_eq!(read_u32(machine.engine, LAYOUT + 648) & 0xff00, 0);
        assert_eq!(
            recorder.observation.cold,
            (0..rows * columns).collect::<Vec<_>>()
        );
        let cold = state_json(machine, rows, columns, &recorder.observation.cold);
        machine.call(DRAWING_BASE + 0xaa530, &[LAYOUT]);
        assert_eq!(
            cold,
            state_json(machine, rows, columns, &recorder.observation.cold)
        );
        for slot in 0..rows * columns {
            TextMetrics {
                has_text_layout: true,
                has_text: true,
                first_line_height: self.first_heights[slot],
                top_margin: 0.0,
                measured_height: self.measured[slot],
            }
            .supply_last_line(machine, slot, self.last_bottoms[slot]);
        }
        let mut warm = Vec::new();
        for bands in &self.warm_bands {
            recorder.observation.warm.clear();
            splits::supply_bands(machine, bands);
            machine.call(DRAWING_BASE + 0xaa3d4, &[LAYOUT]);
            machine.call(DRAWING_BASE + 0xac9d4, &[LAYOUT]);
            let minimum = read_register(machine.engine, 136) as u32;
            warm.push(format!(
                "{{\"rectangles\":{bands:?},\"minimum_height_bits\":{minimum},\"state\":{}}}",
                state_json(machine, rows, columns, &recorder.observation.warm),
            ));
        }
        format!(
            "{{\"name\":{:?},\"cold_via_layout\":{},\"heights\":{:?},\"widths\":{:?},\"spans\":{:?},\"measured_heights\":{:?},\"first_line_heights\":{:?},\"last_line_bottoms\":{:?},\"rectangles\":{:?},\"cold\":{cold},\"warm\":[{}]}}",
            self.name,
            self.cold_via_layout,
            self.grid.heights,
            self.grid.widths,
            self.grid.spans,
            self.measured,
            self.first_heights,
            self.last_bottoms,
            self.bands,
            warm.join(","),
        )
    }
}

fn state_json(machine: &Machine, rows: usize, columns: usize, selected: &[usize]) -> String {
    let state = frames::snapshot(machine, rows, columns);
    let bounds: [f32; 4] =
        std::array::from_fn(|i| read_float(machine.engine, LAYOUT + 668 + i as u64 * 4));
    let content: [f32; 4] =
        std::array::from_fn(|i| read_float(machine.engine, LAYOUT + 652 + i as u64 * 4));
    format!(
        "{{\"selected_cells\":{selected:?},\"frames\":{:?},\"pending_gaps\":{:?},\"cell_splits\":[{}],\"measured_bbox\":{bounds:?},\"content_bbox\":{content:?}}}",
        state.grid,
        state.pending_gaps,
        splits::snapshot_json(machine, rows * columns),
    )
}

pub(super) fn capture(machine: &mut Machine, base: &Path, widget: &Path, text: &Path) {
    frames::load_base(machine, base);
    splits::load_lists(machine);
    geometry::load_measurements(machine, widget, text);
    bottom::load_compression(machine);
    let mut recorder = Recorder::new(machine);
    let mut cases = Vec::new();
    for (name, spans) in [
        ("unmerged-lifecycle", vec![]),
        ("column-owner-lifecycle", vec![(0, [1, 2])]),
        ("row-owner-lifecycle", vec![(0, [3, 1])]),
        ("whole-grid-lifecycle", vec![(0, [3, 2])]),
        (
            "covered-owner-chain-lifecycle",
            vec![(0, [2, 1]), (2, [2, 1])],
        ),
    ] {
        let mut case = Case::new(name, 3, 2);
        for (slot, span) in spans {
            case.grid.spans[slot] = span;
        }
        cases.push(case);
    }
    let mut random = 0x6c69_6665_6379_636c_u64;
    for index in 0..64 {
        let rows = 1 + index % 5;
        let columns = 1 + index / 5 % 5;
        let mut case = Case::new(&format!("mixed-lifecycle-{index}"), rows, columns);
        case.cold_via_layout = index % 2 != 0;
        for row in 0..rows {
            case.grid.heights[row] = [20.0, 100.25, 200.0][(index + row) % 3];
            for column in 0..columns {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                let slot = row * columns + column;
                case.grid.spans[slot] = [
                    1 + (random as usize % (rows - row)) as u32,
                    1 + (random as usize / rows % (columns - column)) as u32,
                ];
                case.measured[slot] = [10.0, 40.25, 100.0, 250.5][random as usize % 4];
                case.first_heights[slot] = [10.0, 20.0, 80.0][random as usize / 7 % 3];
                case.last_bottoms[slot] = [5.0, 30.0, 150.25][random as usize / 11 % 3];
            }
        }
        if index % 3 == 0 {
            case.bands.clear();
        }
        if index % 5 == 0 {
            case.bands.reverse();
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
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"widget_library_sha256\":\"{WIDGET_SHA256}\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"measure_address\":\"0xaa530\",\"layout_address\":\"0xaa3d4\",\"measurement_inputs\":\"fixed cached heights and line metrics; native public Measure/Layout drivers; text initialization/update/measurement, padding/font selection, diagnostics and final observers isolated\",\"cases\":[\n{}\n]}}",
        captured.join(",\n"),
    );
}
