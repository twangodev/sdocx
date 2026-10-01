use super::*;
use frames::{BASE, BASE_SHA256, LAYOUT};

const WIDGET: u64 = 0x0400_0000;
pub(super) const WIDGET_SHA256: &str =
    "cfaaccbfd62763f0e514271cc372c0de7b6df41f0d2f991887b8b9584abd1ec9";
const TEXT: u64 = 0x0500_0000;
pub(super) const TEXT_SHA256: &str =
    "5483711673a499743625eb3275e34b37a006919af346212b46b8d8857834308b";
const UPDATE_GEOMETRY: u64 = DRAWING_BASE + 0xab168;
const FIRST_PAGE_MINIMUM: u64 = DRAWING_BASE + 0xac9f0;
const FIND_LAYOUT: u64 = DRAWING_BASE + 0xb3788;
const FIND_FRAME: u64 = DRAWING_BASE + 0xb4d70;
const KEY: u64 = MODEL + 0xe000;
const REGISTER_S0: i32 = 136;

#[derive(Clone, Copy)]
pub(super) struct TextMetrics {
    pub has_text_layout: bool,
    pub has_text: bool,
    pub first_line_height: f32,
    pub top_margin: f32,
    pub measured_height: f32,
}

impl TextMetrics {
    pub fn supply(self, machine: &Machine, slot: usize) {
        let cell = CELL_BASE + slot as u64 * CELL_STRIDE;
        write(machine.engine, KEY, &cell.to_le_bytes());
        let node = machine.call(FIND_LAYOUT, &[LAYOUT + 712, KEY]);
        assert!((HEAP..machine.heap.cursor).contains(&node));
        let layout = read_u64(machine.engine, node + 24);
        assert!((HEAP..machine.heap.cursor).contains(&layout));
        write(
            machine.engine,
            layout + 404,
            &self.measured_height.to_le_bytes(),
        );
        if !self.has_text_layout {
            return;
        }
        let text = machine.call(NEW, &[640]);
        write(machine.engine, text, &[0; 640]);
        let implementation = text + 96;
        let rich_text = text + 416;
        let line = text + 576;
        write(machine.engine, layout + 368, &text.to_le_bytes());
        write(machine.engine, text, &(TEXT + 0xf5508).to_le_bytes());
        write(machine.engine, text + 64, &implementation.to_le_bytes());
        write(machine.engine, implementation, &rich_text.to_le_bytes());
        write(
            machine.engine,
            implementation + 104,
            &u32::from(self.has_text).to_le_bytes(),
        );
        write(machine.engine, implementation + 272, &line.to_le_bytes());
        write(
            machine.engine,
            implementation + 280,
            &(line + 56).to_le_bytes(),
        );
        write(
            machine.engine,
            rich_text + 132,
            &self.top_margin.to_le_bytes(),
        );
        write(
            machine.engine,
            line + 20,
            &self.first_line_height.to_le_bytes(),
        );
        for (function, expected) in [
            (TEXT + 0x8b3ac, self.first_line_height),
            (TEXT + 0x8b90c, self.top_margin),
        ] {
            machine.call(function, &[text, 0]);
            assert_eq!(
                read_register(machine.engine, REGISTER_S0) as u32,
                expected.to_bits()
            );
        }
        assert_eq!(
            machine.call(TEXT + 0x8b104, &[text]),
            u64::from(self.has_text)
        );
    }

    pub fn supply_last_line(self, machine: &Machine, slot: usize, bottom: f32) {
        assert!(bottom.is_finite());
        self.supply(machine, slot);
        if !self.has_text_layout {
            return;
        }
        let cell = CELL_BASE + slot as u64 * CELL_STRIDE;
        write(machine.engine, KEY, &cell.to_le_bytes());
        let node = machine.call(FIND_LAYOUT, &[LAYOUT + 712, KEY]);
        let layout = read_u64(machine.engine, node + 24);
        let text = read_u64(machine.engine, layout + 368);
        let implementation = read_u64(machine.engine, text + 64);
        let lines = machine.call(NEW, &[112]);
        write(machine.engine, lines, &[0; 112]);
        write(
            machine.engine,
            lines + 20,
            &self.first_line_height.to_le_bytes(),
        );
        write(machine.engine, lines + 76, &bottom.to_le_bytes());
        write(machine.engine, implementation + 272, &lines.to_le_bytes());
        write(
            machine.engine,
            implementation + 280,
            &(lines + 112).to_le_bytes(),
        );
        assert_eq!(machine.call(TEXT + 0x8adb0, &[text]), 2);
        machine.call(TEXT + 0x8b4d8, &[text, 1]);
        assert_eq!(
            read_register(machine.engine, REGISTER_S0) as u32,
            bottom.to_bits()
        );
    }

    fn json(self) -> String {
        format!(
            "{{\"has_text_layout\":{},\"has_text\":{},\"first_line_height\":{:?},\"top_margin\":{:?},\"measured_height\":{:?}}}",
            self.has_text_layout,
            self.has_text,
            self.first_line_height,
            self.top_margin,
            self.measured_height,
        )
    }
}

struct Case {
    name: String,
    grid: BorderCase,
    metrics: Vec<TextMetrics>,
    replacements: Option<Vec<[f32; 4]>>,
}

impl Case {
    fn new(name: &str, rows: usize, columns: usize) -> Self {
        let mut grid = BorderCase::new("measured-grid", rows, columns);
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
            metrics: (0..rows * columns)
                .map(|slot| TextMetrics {
                    has_text_layout: true,
                    has_text: true,
                    first_line_height: 10.25 + slot as f32 * 3.5,
                    top_margin: slot as f32 * 0.125,
                    measured_height: 300.0 + slot as f32 * 10.0,
                })
                .collect(),
            replacements: None,
        }
    }

    fn fixture(&self, machine: &mut Machine) -> String {
        frames::initialize(machine, &self.grid);
        let rows = self.grid.heights.len();
        let columns = self.grid.widths.len();
        if let Some(replacements) = &self.replacements {
            let grid = read_u64(machine.engine, LAYOUT + 832);
            for (slot, rectangle) in replacements.iter().enumerate() {
                let cells = read_u64(machine.engine, grid + (slot / columns) as u64 * 24);
                let cell = CELL_BASE + slot as u64 * CELL_STRIDE;
                write(machine.engine, KEY, &cell.to_le_bytes());
                let node = machine.call(FIND_FRAME, &[LAYOUT + 752, KEY]);
                for (axis, coordinate) in rectangle.iter().enumerate() {
                    write(
                        machine.engine,
                        cells + (slot % columns) as u64 * 16 + axis as u64 * 4,
                        &coordinate.to_le_bytes(),
                    );
                    write(
                        machine.engine,
                        node + 24 + axis as u64 * 4,
                        &coordinate.to_le_bytes(),
                    );
                }
            }
        }
        for (slot, metrics) in self.metrics.iter().enumerate() {
            metrics.supply(machine, slot);
        }
        machine.call(UPDATE_GEOMETRY, &[LAYOUT]);
        let content: [f32; 4] =
            std::array::from_fn(|axis| read_float(machine.engine, LAYOUT + 652 + axis as u64 * 4));
        let measured: [f32; 4] =
            std::array::from_fn(|axis| read_float(machine.engine, LAYOUT + 668 + axis as u64 * 4));
        let widths: [f32; 4] = std::array::from_fn(|edge| {
            machine.call(
                [0x3dbba4, 0x3dbc30, 0x3dbcbc, 0x3dbd48][edge],
                &[TABLE_OBJECT],
            );
            f32::from_bits(read_register(machine.engine, REGISTER_S0) as u32)
        });
        let mut minima = Vec::new();
        for row in 0..rows {
            minima.push(std::array::from_fn::<_, 2, _>(|offset| {
                machine.call(FIRST_PAGE_MINIMUM, &[LAYOUT, row as u64, offset as u64]);
                read_register(machine.engine, REGISTER_S0) as u32
            }));
        }
        machine.call(DRAWING_BASE + 0xac9d4, &[LAYOUT]);
        assert_eq!(
            read_register(machine.engine, REGISTER_S0) as u32,
            minima[0][1]
        );
        let snapshot = frames::snapshot(machine, rows, columns);
        format!(
            "{{\"name\":{:?},\"heights\":{:?},\"widths\":{:?},\"spans\":{:?},\"native_defaults\":{},\"outer_border\":{},\"default_border\":{},\"borders\":[{}],\"frames\":{:?},\"metrics\":[{}],\"drawn_widths\":{widths:?},\"content_bbox\":{content:?},\"measured_bbox\":{measured:?},\"minimum_height_bits\":{minima:?}}}",
            self.name,
            self.grid.heights,
            self.grid.widths,
            self.grid.spans,
            self.grid.native_defaults,
            border_json(self.grid.outer_border),
            border_json(self.grid.default_border),
            self.grid
                .borders
                .iter()
                .map(|border| border_json(*border))
                .collect::<Vec<_>>()
                .join(","),
            snapshot.grid,
            self.metrics
                .iter()
                .map(|metrics| metrics.json())
                .collect::<Vec<_>>()
                .join(","),
        )
    }
}

pub(super) fn load_measurements(machine: &Machine, widget_path: &Path, text_path: &Path) {
    map_library(machine.engine, widget_path, WIDGET, WIDGET_SHA256);
    map_library(machine.engine, text_path, TEXT, TEXT_SHA256);
    for (plt, target) in [
        (DRAWING_BASE + 0xbd340, 0x3d2d3c),
        (DRAWING_BASE + 0xbd330, BASE + 0xb1724),
        (DRAWING_BASE + 0xbd460, 0x3dbba4),
        (DRAWING_BASE + 0xbd470, 0x3dbc30),
        (DRAWING_BASE + 0xbd480, 0x3dbcbc),
        (DRAWING_BASE + 0xbd490, 0x3dbd48),
        (0x486ba0, 0x3c6d5c),
        (0x486bb0, 0x3c6dd8),
        (0x486bc0, 0x3c6e54),
        (0x486bd0, 0x3c6ee0),
        (0x486c10, 0x3c7510),
        (DRAWING_BASE + 0xbce90, WIDGET + 0xd39ac),
        (DRAWING_BASE + 0xbd420, WIDGET + 0xd3b78),
        (DRAWING_BASE + 0xb8b20, BASE + 0xb109c),
        (DRAWING_BASE + 0xbd070, FIRST_PAGE_MINIMUM),
        (TEXT + 0xeee20, BASE + 0xb109c),
        (TEXT + 0xef430, TEXT + 0x74058),
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
    load_measurements(machine, widget_path, text_path);
    let mut cases = Vec::new();
    for (name, rows, columns, spans) in [
        ("unmerged", 3, 2, vec![]),
        ("last-column-owner", 2, 3, vec![(3, [1, 3])]),
        ("last-row-owner", 3, 2, vec![(1, [3, 1])]),
        ("all-slots-owned", 3, 2, vec![(0, [3, 2])]),
        ("covered-owner-chain", 4, 1, vec![(0, [2, 1]), (1, [3, 1])]),
    ] {
        let mut case = Case::new(name, rows, columns);
        for (slot, span) in spans {
            case.grid.spans[slot] = span;
        }
        cases.push(case);
    }
    for (name, has_text_layout, has_text, measured) in [
        ("empty-text-measured-height", true, false, 75.5),
        ("missing-text-layout", false, true, 93.25),
        ("zero-height-first-row-fallback", false, true, 0.0),
    ] {
        let mut case = Case::new(name, 3, 2);
        for metrics in &mut case.metrics {
            metrics.has_text_layout = has_text_layout;
            metrics.has_text = has_text;
            metrics.measured_height = measured;
        }
        cases.push(case);
    }
    let mut union = Case::new("endpoint-cache-union", 1, 2);
    union.replacements = Some(vec![[50.0, 30.0, 100.0, 70.0], [-20.0, -10.0, 40.0, 10.0]]);
    cases.push(union);
    let mut asymmetric = Case::new("asymmetric-outer-and-cell-widths", 3, 2);
    asymmetric.grid.native_defaults = false;
    asymmetric.grid.outer_border = Some(distinct_border(4));
    asymmetric.grid.default_border = Some(distinct_border(9));
    asymmetric.grid.borders[0] = Some(distinct_border(14));
    asymmetric.grid.borders[5] = Some(distinct_border(17));
    cases.push(asymmetric);
    let mut random = 0x6765_6f6d_6574_7279_u64;
    for index in 0..128 {
        let rows = 1 + index % 5;
        let columns = 1 + index / 5 % 5;
        let mut case = Case::new(&format!("mixed-cached-geometry-{index}"), rows, columns);
        case.grid.native_defaults = false;
        case.grid.outer_border = Some(distinct_border(index % 7));
        case.grid.default_border = Some(distinct_border(index % 3));
        for slot in 0..rows * columns {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let row = slot / columns;
            let column = slot % columns;
            case.grid.spans[slot] = [
                1 + (random as usize % (rows - row)) as u32,
                1 + (random as usize / rows % (columns - column)) as u32,
            ];
            case.grid.borders[slot] = (slot % 3 == 0).then(|| distinct_border(slot));
            case.metrics[slot] = TextMetrics {
                has_text_layout: random & 3 != 0,
                has_text: random & 4 != 0,
                first_line_height: [0.0, 0.1, 0.001, 10.25, 101.125][random as usize / 7 % 5],
                top_margin: [0.0, 0.3, 12.5][random as usize / 13 % 3],
                measured_height: [0.0, 1.25, 87.5][random as usize / 17 % 3],
            };
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
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"widget_library_sha256\":\"{WIDGET_SHA256}\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"update_geometry_address\":\"0xab168\",\"minimum_height_address\":\"0xac9f0\",\"measurement_inputs\":\"supplied cached frames and text metrics, not native text shaping\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}
