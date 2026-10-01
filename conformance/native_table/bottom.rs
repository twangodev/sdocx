use super::*;
use frames::{BASE, BASE_SHA256, LAYOUT};
use geometry::{TEXT_SHA256, TextMetrics, WIDGET_SHA256};

const INTERSECT_SPLIT: u64 = DRAWING_BASE + 0xb304c;
const ROW_OFFSET: u64 = DRAWING_BASE + 0xb294c;
const UPDATE_BOTTOM: u64 = DRAWING_BASE + 0xb28ec;
const REGISTER_S0: i32 = 136;

struct Case {
    name: String,
    grid: BorderCase,
    rectangles: Vec<[f32; 4]>,
    last_bottoms: Vec<Option<f32>>,
    pending_gaps: Vec<f32>,
    cache_rows: Vec<usize>,
    rows: Vec<usize>,
}

impl Case {
    fn new(name: &str, rows: usize, columns: usize) -> Self {
        let mut grid = BorderCase::new("row-bottom-grid", rows, columns);
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
            rectangles: vec![[0.0, 40.0, 1080.0, 60.0], [0.0, 150.0, 1080.0, 180.0]],
            last_bottoms: vec![Some(10.0); rows * columns],
            pending_gaps: vec![0.0; rows],
            cache_rows: (0..rows).collect(),
            rows: (0..rows).collect(),
        }
    }

    fn fixture(&self, machine: &mut Machine) -> String {
        frames::initialize(machine, &self.grid);
        splits::initialize_cache(machine);
        splits::supply_bands(machine, &self.rectangles);
        let rows = self.grid.heights.len();
        let columns = self.grid.widths.len();
        let gaps = read_u64(machine.engine, LAYOUT + 856);
        for (row, gap) in self.pending_gaps.iter().enumerate() {
            write(machine.engine, gaps + row as u64 * 4, &gap.to_le_bytes());
        }
        for (slot, bottom) in self.last_bottoms.iter().enumerate() {
            TextMetrics {
                has_text_layout: bottom.is_some(),
                has_text: true,
                first_line_height: 10.25,
                top_margin: 12.5,
                measured_height: 300.0,
            }
            .supply_last_line(machine, slot, bottom.unwrap_or(0.0));
        }
        for &row in &self.cache_rows {
            machine.call(splits::UPDATE_SPLIT, &[LAYOUT, row as u64]);
        }
        let cell_splits = splits::snapshot_json(machine, rows * columns);
        let initial = frames::snapshot(machine, rows, columns);
        let mut changes = Vec::new();
        for &row in &self.rows {
            machine.call(INTERSECT_SPLIT, &[LAYOUT, row as u64]);
            let selected: [u32; 4] = std::array::from_fn(|axis| {
                read_register(machine.engine, REGISTER_S0 + axis as i32) as u32
            });
            machine.call(ROW_OFFSET, &[LAYOUT, row as u64]);
            let amount = read_register(machine.engine, REGISTER_S0) as u32;
            machine.call(UPDATE_BOTTOM, &[LAYOUT, row as u64]);
            let state = frames::snapshot(machine, rows, columns);
            assert_eq!(splits::snapshot_json(machine, rows * columns), cell_splits);
            changes.push(format!(
                "{{\"row\":{row},\"selected_band_bits\":{selected:?},\"offset_bits\":{amount},\"frames\":{:?},\"pending_gaps\":{:?}}}",
                state.grid, state.pending_gaps
            ));
        }
        let last_bottoms = self
            .last_bottoms
            .iter()
            .map(|bottom| bottom.map_or("null".to_owned(), |bottom| format!("{bottom:?}")))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"name\":{:?},\"heights\":{:?},\"widths\":{:?},\"spans\":{:?},\"rectangles\":{:?},\"last_line_bottoms\":[{last_bottoms}],\"cache_rows\":{:?},\"cell_splits\":[{cell_splits}],\"initial_frames\":{:?},\"pending_gaps\":{:?},\"changes\":[{}]}}",
            self.name,
            self.grid.heights,
            self.grid.widths,
            self.grid.spans,
            self.rectangles,
            self.cache_rows,
            initial.grid,
            self.pending_gaps,
            changes.join(",")
        )
    }
}

pub(super) fn load_compression(machine: &Machine) {
    for (plt, target) in [
        (DRAWING_BASE + 0xbd440, BASE + 0x9e3f0),
        (DRAWING_BASE + 0xb92a0, BASE + 0xb11bc),
        (DRAWING_BASE + 0xbd2f0, INTERSECT_SPLIT),
        (DRAWING_BASE + 0xbd2c0, ROW_OFFSET),
        (DRAWING_BASE + 0xbd1b0, DRAWING_BASE + 0xaff74),
    ] {
        bind_native(machine.engine, plt, target);
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
    load_compression(machine);
    let mut cases = vec![Case::new("unmerged-compression", 3, 2)];
    for (name, spans) in [
        ("first-column-owner-cache", vec![(0, [3, 1])]),
        ("other-column-owner-last-line", vec![(1, [3, 1])]),
        ("whole-grid-owner", vec![(0, [3, 2])]),
        ("covered-owner-chain", vec![(0, [2, 1]), (2, [2, 1])]),
    ] {
        let mut case = Case::new(name, 3, 2);
        for (slot, span) in spans {
            case.grid.spans[slot] = span;
        }
        if name == "other-column-owner-last-line" {
            case.last_bottoms[3] = Some(8888.0);
            case.last_bottoms[5] = Some(6666.0);
        } else {
            case.last_bottoms[2] = Some(9999.0);
            case.last_bottoms[4] = Some(7777.0);
        }
        cases.push(case);
    }
    for bottom in [
        f32::from_bits(39.5_f32.to_bits() - 1),
        39.5,
        f32::from_bits(39.5_f32.to_bits() + 1),
        -1.0,
        0.0,
    ] {
        let mut case = Case::new("last-line-band-boundary", 1, 2);
        case.last_bottoms.fill(Some(bottom));
        cases.push(case);
    }
    for top in [
        f32::from_bits(100_f32.to_bits() - 1),
        100.0,
        f32::from_bits(100_f32.to_bits() + 1),
    ] {
        let mut case = Case::new("physical-height-band-boundary", 1, 1);
        case.rectangles = vec![[0.0, top + 0.5, 1080.0, 150.0]];
        cases.push(case);
    }
    for rectangles in [
        vec![],
        vec![[0.0, 40.0, 0.0, 60.0]],
        vec![[0.0, 40.0, 1080.0, 40.0]],
        vec![[0.0, 90.0, 1080.0, 110.0], [0.0, 40.0, 1080.0, 60.0]],
    ] {
        let mut case = Case::new("empty-or-unsorted-bands", 3, 2);
        case.rectangles = rectangles;
        cases.push(case);
    }
    let mut missing = Case::new("missing-split-caches", 3, 2);
    missing.cache_rows.clear();
    cases.push(missing);
    let mut missing = Case::new("missing-text-layouts", 3, 2);
    missing.last_bottoms.fill(None);
    cases.push(missing);
    let mut pending = Case::new("compression-consumes-later-pending-gaps", 3, 2);
    pending.pending_gaps = vec![0.0, 50.0, 10.0];
    cases.push(pending);
    let mut random = 0x726f_772d_626f_7474_u64;
    for index in 0..128 {
        let rows = 1 + index % 5;
        let columns = 1 + index / 5 % 5;
        let mut case = Case::new(&format!("mixed-row-bottom-{index}"), rows, columns);
        for row in 0..rows {
            case.grid.heights[row] = [0.0, 10.0, 100.0, 400.25][(index + row) % 4];
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
                case.last_bottoms[slot] = [
                    None,
                    Some(-1.0),
                    Some(0.0),
                    Some(20.0),
                    Some(39.5),
                    Some(1000.0),
                ][random as usize / 7 % 6];
            }
        }
        if index % 7 == 0 {
            case.cache_rows.truncate(rows / 2);
        }
        if index % 11 == 0 {
            case.rectangles.clear();
        }
        if index % 3 == 0 {
            case.rows.reverse();
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
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"widget_library_sha256\":\"{WIDGET_SHA256}\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"intersect_split_address\":\"0xb304c\",\"row_offset_address\":\"0xb294c\",\"update_bottom_address\":\"0xb28ec\",\"measurement_inputs\":\"supplied cached two-line text metrics or null layouts, not native text shaping\",\"capture_boundary\":\"native row compression, owner lookup, lists and text getters; text initialization, single-thread mutex operations and diagnostic interfaces isolated\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}
