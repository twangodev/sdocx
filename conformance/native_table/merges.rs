use super::*;
use frames::BASE_SHA256;

const MERGE_CELLS: u64 = 0x3d6038;
const VALID_MERGE_RANGE: u64 = 0x3ca320;

struct Case {
    name: String,
    grid: BorderCase,
    ranges: Vec<[i32; 4]>,
}

impl Case {
    fn new(name: &str, rows: usize, columns: usize, ranges: Vec<[i32; 4]>) -> Self {
        let mut grid = BorderCase::new("merge-grid", rows, columns);
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
            ranges,
        }
    }

    fn fixture(&self, machine: &mut Machine) -> String {
        let rows = self.grid.heights.len();
        let columns = self.grid.widths.len();
        let count = rows * columns;
        machine.initialize_borders(&self.grid);
        write(machine.engine, TABLE_OBJECT + 104, &MODEL.to_le_bytes());
        assert_eq!(read_u64(machine.engine, TABLE_OBJECT + 16), 0);
        let pointers: Vec<_> = (0..count)
            .map(|slot| {
                machine.call(
                    0x3d2be0,
                    &[
                        TABLE_OBJECT,
                        (slot / columns) as u64,
                        (slot % columns) as u64,
                    ],
                )
            })
            .collect();
        let mut actions = Vec::new();
        for range in &self.ranges {
            let indices = range.map(|index| index as u32 as u64);
            let valid = machine.call(
                VALID_MERGE_RANGE,
                &[MODEL, indices[0], indices[1], indices[2], indices[3]],
            );
            assert!(valid <= 1);
            let merged = machine.call(
                MERGE_CELLS,
                &[TABLE_OBJECT, indices[0], indices[1], indices[2], indices[3]],
            );
            assert_eq!(merged, valid);
            let spans: Vec<[u32; 2]> = (0..count)
                .map(|slot| {
                    let cell = CELL_BASE + slot as u64 * CELL_STRIDE;
                    [
                        read_u32(machine.engine, cell + 52),
                        read_u32(machine.engine, cell + 56),
                    ]
                })
                .collect();
            let dirty: Vec<[u8; 2]> = (0..count)
                .map(|slot| {
                    let cell = CELL_BASE + slot as u64 * CELL_STRIDE;
                    let flags = read_u32(machine.engine, cell + 80);
                    [(flags >> 8) as u8, (flags >> 16) as u8]
                })
                .collect();
            let owners: Vec<_> = (0..count)
                .map(|slot| {
                    assert_eq!(
                        machine.call(
                            0x3d2be0,
                            &[
                                TABLE_OBJECT,
                                (slot / columns) as u64,
                                (slot % columns) as u64
                            ]
                        ),
                        pointers[slot]
                    );
                    let owner = machine.call(
                        GET_FRAME_CELL,
                        &[MODEL, (slot / columns) as u64, (slot % columns) as u64],
                    );
                    machine.position(owner, count)
                })
                .collect();
            register(machine.engine, REGISTER_X0 + 8, RETURN_VECTOR);
            machine.call(GET_VISIBLE_CELLS, &[MODEL]);
            let begin = read_u64(machine.engine, RETURN_VECTOR);
            let end = read_u64(machine.engine, RETURN_VECTOR + 8);
            assert!(begin <= end && (end - begin) / 8 <= count as u64);
            let visible: Vec<_> = (0..(end - begin) / 8)
                .map(|index| machine.position(read_u64(machine.engine, begin + index * 8), count))
                .collect();
            actions.push(format!("{{\"range\":{range:?},\"valid\":{},\"spans\":{spans:?},\"dirty_flags\":{dirty:?},\"frame_owners\":{owners:?},\"visible\":{visible:?}}}", valid == 1));
        }
        frames::initialize_layout(machine);
        let state = frames::snapshot(machine, rows, columns);
        format!(
            "{{\"name\":{:?},\"heights\":{:?},\"widths\":{:?},\"initial_spans\":{:?},\"actions\":[{}],\"frames\":{:?}}}",
            self.name,
            self.grid.heights,
            self.grid.widths,
            self.grid.spans,
            actions.join(","),
            state.grid
        )
    }
}

pub(super) fn capture(machine: &mut Machine, base_path: &Path) {
    frames::load_base(machine, base_path);
    for (plt, target) in [
        (0x486df0, VALID_MERGE_RANGE),
        (0x486bf0, 0x3c7328),
        (0x486c20, GET_FRAME_CELL),
        (0x487300, 0x3d2be0),
    ] {
        bind_native(machine.engine, plt, target);
    }
    for plt in [0x47ac10, 0x47ac20] {
        write(machine.engine, plt, &0xd65f03c0_u32.to_le_bytes());
    }
    let mut cases = vec![
        Case::new(
            "row-merge-and-repeat",
            3,
            3,
            vec![[0, 0, 0, 2], [0, 0, 0, 2], [0, 1, 0, 2]],
        ),
        Case::new(
            "column-merge-and-repeat",
            3,
            3,
            vec![[0, 0, 2, 0], [0, 0, 2, 0], [1, 0, 2, 0]],
        ),
        Case::new(
            "rectangle-and-expansion",
            3,
            3,
            vec![[0, 0, 1, 1], [0, 0, 2, 2], [0, 0, 0, 0]],
        ),
        Case::new(
            "disjoint-then-combined",
            3,
            4,
            vec![[0, 0, 1, 1], [0, 2, 1, 3], [0, 0, 1, 3]],
        ),
        Case::new(
            "bounds-and-inverted-ranges",
            3,
            3,
            vec![
                [-1, 0, 0, 0],
                [0, -1, 0, 0],
                [0, 0, 3, 2],
                [0, 0, 2, 3],
                [2, 0, 1, 0],
                [0, 2, 0, 1],
                [2, 2, 2, 2],
            ],
        ),
    ];
    let mut chain = Case::new(
        "initial-covered-span-chain",
        1,
        4,
        vec![[0, 0, 0, 1], [0, 0, 0, 3], [0, 2, 0, 3]],
    );
    chain.grid.spans[0] = [1, 2];
    chain.grid.spans[1] = [1, 3];
    cases.push(chain);
    let mut random = 0x6d65_7267_6573_u64;
    for index in 0..128 {
        let rows = 1 + index % 5;
        let columns = 1 + index / 5 % 5;
        let mut ranges = Vec::new();
        for _ in 0..8 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let top = random as usize % rows;
            let left = random as usize / rows % columns;
            let bottom = top + random as usize / 100 % (rows - top);
            let right = left + random as usize / 10000 % (columns - left);
            ranges.push([top as i32, left as i32, bottom as i32, right as i32]);
        }
        cases.push(Case::new(
            &format!("mixed-merge-sequence-{index}"),
            rows,
            columns,
            ranges,
        ));
    }
    let mut captured = Vec::new();
    for case in &cases {
        machine.heap.allocation_fill = 0;
        let expected = case.fixture(machine);
        for fill in [0xa5, 0xff] {
            machine.heap.allocation_fill = fill;
            assert_eq!(case.fixture(machine), expected, "{}", case.name);
        }
        captured.push(expected);
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"merge_address\":\"0x3d6038\",\"validity_address\":\"0x3ca320\",\"capture_boundary\":\"native merge mutation and ownership with no attached document or history; diagnostics and cold text initialization isolated\",\"raw_cell_identity_preserved\":true,\"cases\":[\n{}\n]}}",
        captured.join(",\n")
    );
}
