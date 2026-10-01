use super::*;
use frames::{BASE, BASE_SHA256, LAYOUT};

const WIDGET: u64 = 0x0400_0000;
const WIDGET_SHA256: &str = "cfaaccbfd62763f0e514271cc372c0de7b6df41f0d2f991887b8b9584abd1ec9";
const UPDATE_ROWS: u64 = DRAWING_BASE + 0xaecf4;
const GET_MEASURED_HEIGHT: u64 = WIDGET + 0xd3b78;
const FIND_LAYOUT: u64 = DRAWING_BASE + 0xb3788;
const KEY: u64 = MODEL + 0xe000;
const REGISTER_S0: i32 = 136;

struct Case {
    name: String,
    grid: BorderCase,
    minima: Vec<f32>,
    maxima: Vec<f32>,
    measured: Vec<Option<f32>>,
    pending_gaps: Vec<f32>,
    start: usize,
}

impl Case {
    fn new(name: &str, rows: usize, columns: usize) -> Self {
        let mut grid = BorderCase::new("warm-row-grid", rows, columns);
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
            minima: vec![0.0; rows],
            maxima: vec![f32::MAX; rows],
            measured: (0..rows * columns)
                .map(|slot| Some(10.0 + slot as f32 * 5.0))
                .collect(),
            pending_gaps: vec![0.0; rows],
            grid,
            start: 0,
        }
    }

    fn fixture(&self, machine: &mut Machine) -> String {
        frames::initialize(machine, &self.grid);
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
        let mut owners = Vec::new();
        for (slot, measured) in self.measured.iter().enumerate() {
            let cell = CELL_BASE + slot as u64 * CELL_STRIDE;
            write(machine.engine, KEY, &cell.to_le_bytes());
            let node = machine.call(FIND_LAYOUT, &[LAYOUT + 712, KEY]);
            assert!((HEAP..machine.heap.cursor).contains(&node));
            let layout = read_u64(machine.engine, node + 24);
            assert!((HEAP..machine.heap.cursor).contains(&layout));
            match measured {
                Some(height) => {
                    assert!(height.is_finite());
                    write(machine.engine, layout + 404, &height.to_le_bytes());
                    machine.call(GET_MEASURED_HEIGHT, &[layout]);
                    assert_eq!(
                        read_register(machine.engine, REGISTER_S0) as u32,
                        height.to_bits()
                    );
                }
                None => write(machine.engine, node + 24, &0_u64.to_le_bytes()),
            }
            let owner = machine.call(
                GET_FRAME_CELL,
                &[MODEL, (slot / columns) as u64, (slot % columns) as u64],
            );
            owners.push(machine.position(owner, rows * columns));
        }
        machine.call(UPDATE_ROWS, &[LAYOUT, self.start as u64]);
        let final_state = frames::snapshot(machine, rows, columns);
        let measured: Vec<_> = self
            .measured
            .iter()
            .map(|height| height.map_or("null".to_owned(), |height| format!("{height:?}")))
            .collect();
        format!(
            "{{\"name\":{:?},\"heights\":{:?},\"widths\":{:?},\"spans\":{:?},\"frame_owners\":{owners:?},\"minima\":{:?},\"maxima\":{:?},\"measured_heights\":[{}],\"start\":{},\"pending_gaps\":{:?},\"initial_frames\":{:?},\"frames\":{:?},\"cached_frames\":{:?},\"final_pending_gaps\":{:?}}}",
            self.name,
            self.grid.heights,
            self.grid.widths,
            self.grid.spans,
            self.minima,
            self.maxima,
            measured.join(","),
            self.start,
            self.pending_gaps,
            initial.grid,
            final_state.grid,
            final_state.cached,
            final_state.pending_gaps,
        )
    }
}

pub(super) fn capture(machine: &mut Machine, base_path: &Path, widget_path: &Path) {
    frames::load_base(machine, base_path);
    map_library(machine.engine, widget_path, WIDGET, WIDGET_SHA256);
    for (plt, target) in [
        (DRAWING_BASE + 0xbd340, 0x3d2d3c),
        (DRAWING_BASE + 0xbd420, GET_MEASURED_HEIGHT),
        (DRAWING_BASE + 0xbd430, 0x3d3a70),
        (DRAWING_BASE + 0xb8b20, BASE + 0xb109c),
        (DRAWING_BASE + 0xbd1b0, DRAWING_BASE + 0xaff74),
    ] {
        bind_native(machine.engine, plt, target);
    }
    let mut cases = vec![Case::new("unmerged", 3, 2)];
    let mut owner = Case::new("rowspan-owner-measurements", 3, 2);
    owner.grid.spans[0] = [3, 1];
    owner.measured = vec![
        Some(70.0),
        Some(10.0),
        Some(9999.0),
        Some(20.0),
        Some(8888.0),
        Some(30.0),
    ];
    cases.push(owner);
    let mut chain = Case::new("covered-rowspan-chain", 4, 1);
    chain.grid.spans[0] = [2, 1];
    chain.grid.spans[1] = [3, 1];
    chain.measured = vec![Some(10.0), Some(20.0), Some(30.0), Some(40.0)];
    cases.push(chain);
    for height in [
        0.0,
        0.0005,
        f32::from_bits(0.001_f32.to_bits() - 1),
        0.001,
        f32::from_bits(0.001_f32.to_bits() + 1),
    ] {
        let mut case = Case::new("small-height-threshold", 2, 1);
        case.minima = vec![100.0; 2];
        case.measured = vec![Some(height); 2];
        cases.push(case);
    }
    for measured in [vec![Some(50.0), None, Some(20.0), None], vec![None; 4]] {
        let mut case = Case::new("null-layouts", 2, 2);
        case.measured = measured;
        case.minima = vec![12.0, 25.0];
        cases.push(case);
    }
    for start in 0..3 {
        let mut case = Case::new("start-row", 3, 2);
        case.start = start;
        cases.push(case);
    }
    let mut ignored_maximum = Case::new("saved-maximum-ignored", 3, 2);
    ignored_maximum.maxima = vec![1.0; 3];
    cases.push(ignored_maximum);
    let mut random = 0x7761_726d_726f_7773_u64;
    for index in 0..128 {
        let rows = 1 + index % 5;
        let columns = 1 + index / 5 % 5;
        let mut case = Case::new(&format!("mixed-owner-metrics-{index}"), rows, columns);
        case.start = index % rows;
        for row in 0..rows {
            case.minima[row] = [0.0, 12.0, 100.0][(index + row) % 3];
            case.pending_gaps[row] = [0.0, 10.0, 50.0][(index / 3 + row) % 3];
            for column in 0..columns {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                let slot = row * columns + column;
                case.grid.spans[slot] = [
                    1 + (random as usize % (rows - row)) as u32,
                    1 + (random as usize / rows % (columns - column)) as u32,
                ];
                case.measured[slot] = [
                    None,
                    Some(0.0),
                    Some(0.0005),
                    Some(0.001),
                    Some(10.0),
                    Some(100.0),
                    Some(1000.0),
                ][random as usize / 7 % 7];
            }
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
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"widget_library_sha256\":\"{WIDGET_SHA256}\",\"update_rows_address\":\"0xaecf4\",\"measured_height_address\":\"0xd3b78\",\"measurement_inputs\":\"supplied cached heights or null layouts, not native text shaping\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}
