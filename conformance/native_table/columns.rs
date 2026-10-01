use super::*;

const LAYOUT: u64 = MODEL + 0xc000;
const LAYOUT_MAP: u64 = LAYOUT + 712;
const BUCKETS: u64 = MODEL + 0xd000;
const KEY: u64 = MODEL + 0xe000;
const CELL_LAYOUTS: u64 = MODEL + 0x10000;
const FIND_LAYOUT: u64 = DRAWING_BASE + 0xb3788;
const INSERT_LAYOUT: u64 = DRAWING_BASE + 0xb3d5c;
const GET_COLUMN_MINIMUM: u64 = DRAWING_BASE + 0xab7a0;
const REGISTER_S0: i32 = 136;

#[derive(Clone, Copy, Debug)]
enum CachedWidth {
    Missing,
    Unspecified,
    Value(f32),
}

impl CachedWidth {
    fn json(self) -> String {
        match self {
            Self::Missing => "{\"cached\":false,\"width\":null}".to_owned(),
            Self::Unspecified => "{\"cached\":true,\"width\":null}".to_owned(),
            Self::Value(width) => format!("{{\"cached\":true,\"width\":{width:?}}}"),
        }
    }
}

struct Case {
    name: String,
    minima: Vec<f32>,
    global_minimum: f32,
    spans: Vec<[u32; 2]>,
    cells: Vec<CachedWidth>,
    absent_width_bits: u32,
    has_table: bool,
}

impl Case {
    fn new(name: &str, minima: &[f32], cells: &[CachedWidth]) -> Self {
        assert!(!minima.is_empty() && cells.len().is_multiple_of(minima.len()));
        Self {
            name: name.to_owned(),
            minima: minima.to_vec(),
            global_minimum: 10.0,
            spans: vec![[1, 1]; cells.len()],
            cells: cells.to_vec(),
            absent_width_bits: 0,
            has_table: true,
        }
    }

    fn fixture(&self, machine: &mut Machine) -> String {
        let columns = self.minima.len();
        let rows = self.cells.len() / columns;
        machine.initialize_grid(rows, columns, &self.spans);
        write(machine.engine, TABLE_OBJECT + 104, &MODEL.to_le_bytes());
        write(
            machine.engine,
            MODEL + 12,
            &self.global_minimum.to_le_bytes(),
        );
        let minima = MODEL + 0x1800;
        write(machine.engine, MODEL + 56, &minima.to_le_bytes());
        write(
            machine.engine,
            MODEL + 64,
            &(minima + columns as u64 * 4).to_le_bytes(),
        );
        for (column, minimum) in self.minima.iter().enumerate() {
            assert!(minimum.is_finite());
            write(
                machine.engine,
                minima + column as u64 * 4,
                &minimum.to_le_bytes(),
            );
        }
        let table = if self.has_table { TABLE_OBJECT } else { 0 };
        write(machine.engine, LAYOUT + 584, &table.to_le_bytes());
        write(machine.engine, LAYOUT_MAP, &BUCKETS.to_le_bytes());
        write(machine.engine, LAYOUT_MAP + 8, &64_u64.to_le_bytes());
        write(machine.engine, LAYOUT_MAP + 32, &1_f32.to_le_bytes());
        write(machine.engine, KEY + 8, &KEY.to_le_bytes());
        for (slot, cached) in self.cells.iter().enumerate() {
            if matches!(cached, CachedWidth::Missing) {
                continue;
            }
            let cell = CELL_BASE + slot as u64 * CELL_STRIDE;
            write(machine.engine, KEY, &cell.to_le_bytes());
            let node = machine.call(INSERT_LAYOUT, &[LAYOUT_MAP, KEY, 0, KEY + 8, KEY + 16]);
            assert!((HEAP..machine.heap.cursor).contains(&node));
            assert_eq!(read_u64(machine.engine, node + 16), cell);
            assert_eq!(machine.call(FIND_LAYOUT, &[LAYOUT_MAP, KEY]), node);
            let layout = CELL_LAYOUTS + slot as u64 * 640;
            write(machine.engine, node + 24, &layout.to_le_bytes());
            if let CachedWidth::Value(width) = cached {
                assert!(width.is_finite());
                write(machine.engine, layout + 620, &width.to_le_bytes());
                write(machine.engine, layout + 624, &[1]);
            } else {
                write(
                    machine.engine,
                    layout + 620,
                    &self.absent_width_bits.to_le_bytes(),
                );
            }
        }
        let mut result_bits = Vec::new();
        for column in 0..columns {
            machine.call(GET_COLUMN_MINIMUM, &[LAYOUT, column as u64]);
            let bits = read_register(machine.engine, REGISTER_S0) as u32;
            assert!(f32::from_bits(bits).is_finite());
            result_bits.push(bits);
        }
        let widths: Vec<_> = result_bits
            .iter()
            .map(|&bits| f32::from_bits(bits))
            .collect();
        let cells: Vec<_> = self.cells.iter().map(|cell| cell.json()).collect();
        format!(
            "{{\"name\":{:?},\"rows\":{rows},\"columns\":{columns},\"has_table\":{},\"global_minimum\":{:?},\"minima\":{:?},\"spans\":{:?},\"absent_width_bits\":{},\"cells\":[{}],\"minimum_widths\":{widths:?},\"result_bits\":{result_bits:?}}}",
            self.name,
            self.has_table,
            self.global_minimum,
            self.minima,
            self.spans,
            self.absent_width_bits,
            cells.join(",")
        )
    }
}

pub(super) fn capture(machine: &mut Machine) {
    for (plt, target) in [
        (DRAWING_BASE + 0xbd320, 0x3da870),
        (DRAWING_BASE + 0xbcd90, 0x3d2b20),
        (DRAWING_BASE + 0xbcdb0, 0x3d2be0),
        (0x4872e0, 0x3d2b20),
        (0x4872f0, 0x3d2b80),
    ] {
        bind_native(machine.engine, plt, target);
    }
    use CachedWidth::{Missing, Unspecified, Value};
    let mut cases = vec![
        Case::new("no-rows", &[10.0, 25.0], &[]),
        Case::new("no-values", &[10.0], &[Unspecified; 4]),
        Case::new("baseline-larger", &[100.0], &[Value(25.0), Value(50.0)]),
        Case::new("descending-values", &[10.0], &[Value(100.0), Value(25.0)]),
        Case::new("ascending-values", &[10.0], &[Value(25.0), Value(100.0)]),
        Case::new("fractional-widths", &[10.3], &[Value(20.7), Value(20.6)]),
        Case::new("signed-widths", &[0.0], &[Value(-1.0), Value(-0.0)]),
        Case::new("missing-first", &[10.0], &[Missing, Value(100.0)]),
        Case::new(
            "missing-middle",
            &[10.0],
            &[Value(100.0), Missing, Value(200.0)],
        ),
        Case::new("missing-last", &[10.0], &[Value(100.0), Missing]),
        Case::new(
            "independent-columns",
            &[10.0, 30.0, 50.0],
            &[
                Value(100.0),
                Value(20.0),
                Value(30.0),
                Value(40.0),
                Unspecified,
                Value(60.0),
                Value(50.0),
                Value(25.0),
                Missing,
            ],
        ),
    ];
    let mut detached = Case::new("no-table", &[10.0], &[Value(100.0)]);
    detached.has_table = false;
    cases.push(detached);
    for bits in [1000_f32.to_bits(), 0x7fc01234, 0xffff_ffff] {
        let mut case = Case::new(
            "unset-width-payload",
            &[10.0],
            &[Unspecified, Value(25.0), Unspecified],
        );
        case.absent_width_bits = bits;
        cases.push(case);
    }
    for global_minimum in [0.0, 1000.0] {
        let mut case = Case::new("global-minimum-independent", &[5.0], &[Value(3.0)]);
        case.global_minimum = global_minimum;
        cases.push(case);
    }
    for pattern in 0..81 {
        let mut value = pattern;
        let cells: Vec<_> = (0..4)
            .map(|row| {
                let state = match value % 3 {
                    0 => Missing,
                    1 => Unspecified,
                    _ => Value([20.0, 50.0, 100.0, 75.0][row]),
                };
                value /= 3;
                state
            })
            .collect();
        for merged in [false, true] {
            let mut case = Case::new(
                &format!("cache-pattern-{pattern}-{merged}"),
                &[10.0],
                &cells,
            );
            if merged {
                case.spans[0] = [4, 1];
            }
            cases.push(case);
        }
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
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"minimum_width_address\":\"0xab7a0\",\"metric_inputs\":\"supplied cached optional widths, not native text measurement\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}
