use super::*;
use frames::{BASE, BASE_SHA256, LAYOUT};
use geometry::{TEXT_SHA256, TextMetrics, WIDGET_SHA256};

const WIDGET: u64 = 0x0400_0000;
const LAYOUT_CELL: u64 = DRAWING_BASE + 0xb06d4;
const UPDATE_CELL_SPLIT: u64 = DRAWING_BASE + 0xae794;
const FIND_LAYOUT: u64 = DRAWING_BASE + 0xb3788;
const FIND_FRAME: u64 = DRAWING_BASE + 0xb4d70;
const KEY: u64 = MODEL + 0xe000;
const PADDING_BOUNDARY: u64 = 0x0300_0200;
const SHAPING_BOUNDARY: u64 = 0x0300_0220;
const RETURN_ZERO: u64 = 0x0300_0240;
const STATUS_VTABLE: u64 = MODEL + 0xe800;
const REGISTER_S0: i32 = 136;

#[derive(Default)]
struct Observation {
    layout: u64,
    text: u64,
    padding: Option<u64>,
    dimensions: Option<[i32; 2]>,
}

unsafe extern "C" fn observe(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observed = unsafe { &mut *data.cast::<Observation>() };
    assert_eq!(read_register(engine, REGISTER_X0), observed.text);
    match address {
        PADDING_BOUNDARY => {
            assert!(observed.padding.is_none());
            observed.padding = Some(read_register(engine, REGISTER_X0 + 1));
        }
        SHAPING_BOUNDARY => {
            assert!(observed.padding.is_some() && observed.dimensions.is_none());
            for (register_id, expected) in [(3, 0), (4, u32::MAX), (5, u32::MAX)] {
                assert_eq!(
                    read_register(engine, REGISTER_X0 + register_id) as u32,
                    expected
                );
            }
            observed.dimensions = Some(std::array::from_fn(|axis| {
                read_register(engine, REGISTER_X0 + 1 + axis as i32) as u32 as i32
            }));
            assert_eq!(read_u32(engine, observed.layout + 532) >> 24, 0);
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
        for address in [PADDING_BOUNDARY, SHAPING_BOUNDARY] {
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
        }
        write(machine.engine, RETURN_ZERO, &0x52800000_u32.to_le_bytes());
        write(
            machine.engine,
            RETURN_ZERO + 4,
            &0xd65f03c0_u32.to_le_bytes(),
        );
        for (plt, target) in [
            (DRAWING_BASE + 0xbd4a0, WIDGET + 0xd73a0),
            (DRAWING_BASE + 0xbd4b0, WIDGET + 0xd398c),
            (DRAWING_BASE + 0xbd4c0, WIDGET + 0xd399c),
            (DRAWING_BASE + 0xbd4e0, WIDGET + 0xd3b80),
            (DRAWING_BASE + 0xbb4a0, WIDGET + 0xd3b88),
            (DRAWING_BASE + 0xb8b10, BASE + 0xb108c),
            (WIDGET + 0xe9170, PADDING_BOUNDARY),
            (WIDGET + 0xe9ca0, SHAPING_BOUNDARY),
            (WIDGET + 0xe99b0, RETURN_ZERO),
            (WIDGET + 0xe6ed0, RETURN_ZERO),
            (WIDGET + 0xe6ee0, RETURN_ZERO),
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
                PADDING_BOUNDARY,
                SHAPING_BOUNDARY,
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

struct Case {
    name: String,
    rows: usize,
    columns: usize,
    spans: Vec<[u32; 2]>,
    frames: Vec<[f32; 4]>,
    bands: Vec<Vec<[f32; 4]>>,
    measured_heights: Vec<f32>,
}

impl Case {
    fn new(name: &str, rows: usize, columns: usize) -> Self {
        let count = rows * columns;
        Self {
            name: name.to_owned(),
            rows,
            columns,
            spans: vec![[1, 1]; count],
            frames: (0..count)
                .map(|slot| {
                    let left = -10.25 + (slot % columns) as f32 * 60.5;
                    let top = -20.75 + (slot / columns) as f32 * 100.25;
                    [left, top, left + 60.5, top + 100.25]
                })
                .collect(),
            bands: vec![vec![[0.0, 10.0, 1080.0, 30.0], [0.0, 180.0, 1080.0, 220.0]]; count],
            measured_heights: (0..count).map(|slot| 70.125 + slot as f32).collect(),
        }
    }

    fn fixture(&self, machine: &mut Machine, recorder: &mut Recorder) -> String {
        let mut grid = BorderCase::new("cell-input-grid", self.rows, self.columns);
        grid.borders.fill(None);
        grid.spans.clone_from(&self.spans);
        frames::initialize(machine, &grid);
        splits::initialize_cache(machine);
        write(
            machine.engine,
            STATUS_VTABLE + 16,
            &RETURN_ZERO.to_le_bytes(),
        );
        let mut layouts = Vec::new();
        for (slot, frame) in self.frames.iter().enumerate() {
            let cell = CELL_BASE + slot as u64 * CELL_STRIDE;
            write(machine.engine, KEY, &cell.to_le_bytes());
            let node = machine.call(FIND_FRAME, &[LAYOUT + 752, KEY]);
            assert!((HEAP..machine.heap.cursor).contains(&node));
            for (axis, coordinate) in frame.iter().enumerate() {
                write(
                    machine.engine,
                    node + 24 + axis as u64 * 4,
                    &coordinate.to_le_bytes(),
                );
            }
            TextMetrics {
                has_text_layout: true,
                has_text: true,
                first_line_height: 20.0,
                top_margin: 0.0,
                measured_height: self.measured_heights[slot],
            }
            .supply(machine, slot);
            splits::supply_bands(machine, &self.bands[slot]);
            machine.call(UPDATE_CELL_SPLIT, &[LAYOUT, cell]);
            let node = machine.call(FIND_LAYOUT, &[LAYOUT + 712, KEY]);
            assert!((HEAP..machine.heap.cursor).contains(&node));
            let layout = read_u64(machine.engine, node + 24);
            write(machine.engine, layout, &STATUS_VTABLE.to_le_bytes());
            write(machine.engine, layout + 534, &[1, 1]);
            let bullet = machine.call(NEW, &[160]);
            write(machine.engine, bullet, &[0; 160]);
            write(machine.engine, layout + 432, &bullet.to_le_bytes());
            layouts.push(layout);
        }
        let split_cache = splits::snapshot(machine, self.frames.len());
        let mut calls = Vec::new();
        for (slot, layout) in layouts.into_iter().enumerate() {
            *recorder.observed = Observation {
                layout,
                text: read_u64(machine.engine, layout + 368),
                ..Observation::default()
            };
            let cell = CELL_BASE + slot as u64 * CELL_STRIDE;
            machine.call(LAYOUT_CELL, &[LAYOUT, cell]);
            let delta_bits = read_register(machine.engine, REGISTER_S0) as u32;
            assert!(recorder.observed.padding.unwrap() != 0);
            assert!(recorder.observed.dimensions.is_some());
            let dimensions = recorder.observed.dimensions.unwrap();
            let native_dimensions = [
                read_float(machine.engine, layout + 524),
                read_float(machine.engine, layout + 528),
            ];
            let padding = recorder.observed.padding.unwrap();
            assert!((HEAP..machine.heap.cursor).contains(&padding));
            let count = machine.call(BASE + 0x9d120, &[padding]);
            assert!(count <= 8);
            let rectangles: Vec<[f32; 4]> = (0..count)
                .map(|index| {
                    let rect = machine.call(BASE + 0x9d608, &[padding, index]);
                    assert!((HEAP..machine.heap.cursor).contains(&rect));
                    std::array::from_fn(|axis| read_float(machine.engine, rect + axis as u64 * 4))
                })
                .collect();
            assert_eq!(Some(&rectangles), split_cache[slot].as_ref());
            calls.push(format!("{{\"slot\":{slot},\"native_dimensions\":{native_dimensions:?},\"text_dimensions\":{dimensions:?},\"rectangles\":{rectangles:?},\"height_delta_bits\":{delta_bits}}}"));
        }
        for cell in [0, MODEL + 0xf000] {
            *recorder.observed = Observation::default();
            machine.call(LAYOUT_CELL, &[LAYOUT, cell]);
            assert_eq!(read_register(machine.engine, REGISTER_S0) as u32, 0);
            assert!(recorder.observed.padding.is_none() && recorder.observed.dimensions.is_none());
        }
        format!(
            "{{\"name\":{:?},\"rows\":{},\"columns\":{},\"spans\":{:?},\"frames\":{:?},\"bands\":{:?},\"measured_heights\":{:?},\"calls\":[{}]}}",
            self.name,
            self.rows,
            self.columns,
            self.spans,
            self.frames,
            self.bands,
            self.measured_heights,
            calls.join(",")
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
    let mut recorder = Recorder::new(machine);
    let mut cases = Vec::new();
    for (name, spans) in [
        ("unmerged-cell-inputs", vec![]),
        ("column-span-raw-inputs", vec![(0, [1, 3])]),
        ("row-span-raw-inputs", vec![(0, [3, 1])]),
        ("rectangle-span-raw-inputs", vec![(0, [2, 3])]),
        ("covered-owner-chain-inputs", vec![(0, [2, 1]), (3, [2, 1])]),
    ] {
        let mut case = Case::new(name, 3, 3);
        for (slot, span) in spans {
            case.spans[slot] = span;
        }
        cases.push(case);
    }
    for (name, frame) in [
        ("subpixel-frame", [-0.5, -0.25, -0.125, 0.25]),
        ("zero-height", [13.25, -9.5, 70.125, -9.5]),
        (
            "fraction-below-integer",
            [
                0.0,
                0.0,
                f32::from_bits(40_f32.to_bits() - 1),
                f32::from_bits(60_f32.to_bits() - 1),
            ],
        ),
        (
            "fraction-above-integer",
            [
                0.0,
                0.0,
                f32::from_bits(40_f32.to_bits() + 1),
                f32::from_bits(60_f32.to_bits() + 1),
            ],
        ),
        (
            "large-translated-frame",
            [16_777_216.0, -16_777_216.0, 16_777_224.0, -16_777_208.0],
        ),
    ] {
        let mut case = Case::new(name, 1, 1);
        case.frames[0] = frame;
        cases.push(case);
    }
    let mut random = 0x6365_6c6c_696e_7075_u64;
    for index in 0..128 {
        let rows = 1 + index % 4;
        let columns = 1 + index / 4 % 4;
        let mut case = Case::new(&format!("mixed-cell-inputs-{index}"), rows, columns);
        for slot in 0..rows * columns {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let row = slot / columns;
            let column = slot % columns;
            case.spans[slot] = [
                1 + (random as usize % (rows - row)) as u32,
                1 + (random as usize / rows % (columns - column)) as u32,
            ];
            let left = (random % 1000) as f32 * 0.125 - 60.5;
            let top = (random / 1000 % 1000) as f32 * 0.125 - 60.5;
            let width = 0.125 + (random / 1000000 % 1000) as f32 * 0.25;
            let height = (random / 1000000000 % 1000) as f32 * 0.125;
            case.frames[slot] = [left, top, left + width, top + height];
            case.measured_heights[slot] = (random / 100 % 1000) as f32 * 0.125;
            case.bands[slot] = match random % 4 {
                0 => Vec::new(),
                1 => vec![[0.0, top, 1080.0, top + 20.0]],
                2 => vec![
                    [0.0, top - 1.0, 1080.0, top + 10.0],
                    [5.25, top + 20.5, 1080.0, top + 40.75],
                ],
                _ => vec![
                    [0.0, top + 30.5, 1080.0, top + 40.75],
                    [0.0, top + 10.125, 1080.0, top + 15.625],
                ],
            };
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
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"widget_library_sha256\":\"{WIDGET_SHA256}\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"layout_cell_address\":\"0xb06d4\",\"object_layout_address\":\"0xd3b88\",\"split_address\":\"0xae794\",\"measurement_inputs\":\"supplied cached frames and measured heights; native table and object layout, text shaping and padding assignment intercepted\",\"null_and_uncached_cell_delta_bits\":0,\"cases\":[\n{}\n]}}",
        captured.join(",\n")
    );
}
