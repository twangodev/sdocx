use std::{ffi::c_void, fs, path::Path, process::Command, ptr};

#[path = "native_table/columns.rs"]
mod columns;

#[path = "native_table/frames.rs"]
mod frames;

#[path = "native_table/rows.rs"]
mod rows;

#[path = "native_table/geometry.rs"]
mod geometry;

#[path = "native_table/splits.rs"]
mod splits;

#[path = "native_table/bottom.rs"]
mod bottom;

#[path = "native_table/control.rs"]
mod control;

#[path = "native_table/cell_inputs.rs"]
mod cell_inputs;

#[path = "native_table/cold_rows.rs"]
mod cold_rows;

#[path = "native_table/merges.rs"]
mod merges;

#[path = "native_table/lifecycle.rs"]
mod lifecycle;

type Engine = *mut c_void;

#[link(name = "unicorn")]
unsafe extern "C" {
    fn uc_open(arch: i32, mode: i32, engine: *mut Engine) -> i32;
    fn uc_close(engine: Engine) -> i32;
    fn uc_mem_map(engine: Engine, address: u64, size: u64, permissions: u32) -> i32;
    fn uc_mem_write(engine: Engine, address: u64, bytes: *const c_void, size: usize) -> i32;
    fn uc_mem_read(engine: Engine, address: u64, bytes: *mut c_void, size: usize) -> i32;
    fn uc_reg_write(engine: Engine, register: i32, value: *const c_void) -> i32;
    fn uc_reg_read(engine: Engine, register: i32, value: *mut c_void) -> i32;
    fn uc_emu_start(engine: Engine, begin: u64, until: u64, timeout: u64, count: usize) -> i32;
    fn uc_hook_add(
        engine: Engine,
        hook: *mut usize,
        kind: i32,
        callback: *mut c_void,
        user_data: *mut c_void,
        begin: u64,
        end: u64,
        ...
    ) -> i32;
    fn uc_hook_del(engine: Engine, hook: usize) -> i32;
}

const LIBRARY_SHA256: &str = "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a";
const DRAWING_SHA256: &str = "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd";
const DRAWING_BASE: u64 = 0x0100_0000;
const MODEL: u64 = 0x1000_0000;
const HEAP: u64 = MODEL + 0x40000;
const STOP: u64 = MODEL + 0xff000;
const STACK: u64 = MODEL + 0xfe000;
const TLS: u64 = MODEL + 0xfd000;
const CELL_BASE: u64 = MODEL + 0x20000;
const CELL_STRIDE: u64 = 256;
const ROW_BASE: u64 = MODEL + 0x2000;
const ROW_STRIDE: u64 = 256;
const CELL_POINTER_BASE: u64 = MODEL + 0x3000;
const RETURN_VECTOR: u64 = MODEL + 0x8000;
const TABLE_OBJECT: u64 = MODEL + 0x9000;
const DRAWING_OBJECT: u64 = MODEL + 0x9100;
const CANVAS: u64 = MODEL + 0x9200;
const RETURN_STYLE: u64 = MODEL + 0x9300;
const CANVAS_VTABLE: u64 = MODEL + 0xa000;
const CANVAS_MATRIX: u64 = MODEL + 0xa300;
const MATRIX_GETTER: u64 = MODEL + 0xa400;
const REGISTER_X0: i32 = 199;
const REGISTER_X30: i32 = 2;
const REGISTER_SP: i32 = 4;
const REGISTER_TPIDR_EL0: i32 = 262;
const GET_FRAME_CELL: u64 = 0x3c75c0;
const GET_VISIBLE_CELLS: u64 = 0x3c7784;
const GET_CELL_BORDER_PATH: u64 = 0x3cad9c;
const GET_BORDER_PATH: u64 = 0x3cb464;
const GET_DRAWING_BORDER_STYLE: u64 = DRAWING_BASE + 0xa6fb4;
const GET_CELL_BACKGROUND: u64 = 0x3c2384;
const GET_TABLE_CELL_BACKGROUND: u64 = 0x3cbf7c;
const NEW: u64 = 0x47ac90;
const DELETE: u64 = 0x47ac00;
const MEMSET: u64 = 0x48b3f0;

fn check(code: i32) {
    assert_eq!(code, 0, "Unicorn error {code}");
}
fn register(engine: Engine, id: i32, value: u64) {
    check(unsafe { uc_reg_write(engine, id, ptr::from_ref(&value).cast()) });
}
fn read_register(engine: Engine, id: i32) -> u64 {
    let mut value = 0;
    check(unsafe { uc_reg_read(engine, id, ptr::from_mut(&mut value).cast()) });
    value
}
fn write(engine: Engine, address: u64, bytes: &[u8]) {
    check(unsafe { uc_mem_write(engine, address, bytes.as_ptr().cast(), bytes.len()) });
}
fn read_u64(engine: Engine, address: u64) -> u64 {
    let mut bytes = [0; 8];
    check(unsafe { uc_mem_read(engine, address, bytes.as_mut_ptr().cast(), bytes.len()) });
    u64::from_le_bytes(bytes)
}
fn read_u32(engine: Engine, address: u64) -> u32 {
    let mut bytes = [0; 4];
    check(unsafe { uc_mem_read(engine, address, bytes.as_mut_ptr().cast(), bytes.len()) });
    u32::from_le_bytes(bytes)
}
fn read_float(engine: Engine, address: u64) -> f32 {
    let value = f32::from_bits(read_u32(engine, address));
    assert!(value.is_finite());
    value
}

fn map_library(engine: Engine, path: &Path, base: u64, expected_sha256: &str) {
    let digest = Command::new("sha256sum").arg(path).output().unwrap();
    assert!(digest.status.success());
    assert_eq!(
        String::from_utf8(digest.stdout)
            .unwrap()
            .split_whitespace()
            .next(),
        Some(expected_sha256)
    );
    let binary = fs::read(path).unwrap();
    assert_eq!(&binary[..4], b"\x7fELF");
    let ph_offset = u64::from_le_bytes(binary[32..40].try_into().unwrap()) as usize;
    let ph_size = u16::from_le_bytes(binary[54..56].try_into().unwrap()) as usize;
    let ph_count = u16::from_le_bytes(binary[56..58].try_into().unwrap()) as usize;
    let loads: Vec<_> = (0..ph_count)
        .map(|index| &binary[ph_offset + index * ph_size..][..ph_size])
        .filter(|header| u32::from_le_bytes(header[..4].try_into().unwrap()) == 1)
        .collect();
    assert_eq!(u64::from_le_bytes(loads[0][8..16].try_into().unwrap()), 0);
    assert_eq!(u64::from_le_bytes(loads[0][16..24].try_into().unwrap()), 0);
    for load in loads {
        let offset = u64::from_le_bytes(load[8..16].try_into().unwrap()) as usize;
        let address = base + u64::from_le_bytes(load[16..24].try_into().unwrap());
        let file_size = u64::from_le_bytes(load[32..40].try_into().unwrap()) as usize;
        let memory_size = u64::from_le_bytes(load[40..48].try_into().unwrap());
        assert!(file_size as u64 <= memory_size && memory_size > 0);
        let begin = address & !0xfff;
        let end = (address + memory_size + 0xfff) & !0xfff;
        check(unsafe { uc_mem_map(engine, begin, end - begin, 7) });
        write(engine, address, &binary[offset..offset + file_size]);
    }
    relocate_local_symbols(engine, &binary, base);
}

fn relocate_local_symbols(engine: Engine, binary: &[u8], base: u64) {
    let section_offset = u64::from_le_bytes(binary[40..48].try_into().unwrap()) as usize;
    let section_size = u16::from_le_bytes(binary[58..60].try_into().unwrap()) as usize;
    let section_count = u16::from_le_bytes(binary[60..62].try_into().unwrap()) as usize;
    let sections: Vec<_> = (0..section_count)
        .map(|index| &binary[section_offset + index * section_size..][..section_size])
        .collect();
    for section in &sections {
        if u32::from_le_bytes(section[4..8].try_into().unwrap()) != 4 {
            continue;
        }
        let offset = u64::from_le_bytes(section[24..32].try_into().unwrap()) as usize;
        let size = u64::from_le_bytes(section[32..40].try_into().unwrap()) as usize;
        let symbol_section = u32::from_le_bytes(section[40..44].try_into().unwrap()) as usize;
        assert_eq!(u64::from_le_bytes(section[56..64].try_into().unwrap()), 24);
        let symbols = sections[symbol_section];
        let symbol_offset = u64::from_le_bytes(symbols[24..32].try_into().unwrap()) as usize;
        assert_eq!(u64::from_le_bytes(symbols[56..64].try_into().unwrap()), 24);
        for relocation in binary[offset..offset + size].chunks_exact(24) {
            let address = u64::from_le_bytes(relocation[..8].try_into().unwrap());
            let info = u64::from_le_bytes(relocation[8..16].try_into().unwrap());
            let addend = i64::from_le_bytes(relocation[16..24].try_into().unwrap());
            let symbol = &binary[symbol_offset + (info >> 32) as usize * 24..][..24];
            let value = match info as u32 {
                1027 => 0,
                257 | 1025 | 1026 if u16::from_le_bytes(symbol[6..8].try_into().unwrap()) != 0 => {
                    u64::from_le_bytes(symbol[8..16].try_into().unwrap())
                }
                _ => continue,
            };
            let target = base
                .checked_add(value)
                .unwrap()
                .checked_add_signed(addend)
                .unwrap();
            write(engine, base + address, &target.to_le_bytes());
        }
    }
}

fn bind_native(engine: Engine, plt: u64, target: u64) {
    let displacement = target as i64 - plt as i64;
    assert!(displacement % 4 == 0 && (-0x8000000..0x8000000).contains(&displacement));
    let branch = 0x14000000_u32 | ((displacement / 4) as u32 & 0x03ffffff);
    write(engine, plt, &branch.to_le_bytes());
}

struct Heap {
    cursor: u64,
    allocation_fill: u8,
    allocations: usize,
    fills: usize,
    deletes: usize,
}
unsafe extern "C" fn imported_call(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let heap = unsafe { &mut *data.cast::<Heap>() };
    let first = read_register(engine, REGISTER_X0);
    match address {
        NEW => {
            let allocation = first.max(16).checked_add(15).unwrap() & !15;
            let pointer = heap.cursor;
            heap.cursor = pointer.checked_add(allocation).unwrap();
            assert!(heap.cursor < TLS);
            write(
                engine,
                pointer,
                &vec![heap.allocation_fill; allocation as usize],
            );
            register(engine, REGISTER_X0, pointer);
            heap.allocations += 1;
        }
        DELETE => {
            heap.deletes += 1;
        }
        MEMSET => {
            let byte = read_register(engine, REGISTER_X0 + 1) as u8;
            let count = usize::try_from(read_register(engine, REGISTER_X0 + 2)).unwrap();
            assert!(count < 0x10000);
            write(engine, first, &vec![byte; count]);
            heap.fills += 1;
        }
        _ => unreachable!(),
    }
}

struct Machine {
    engine: Engine,
    heap: Box<Heap>,
}
impl Machine {
    fn new(path: &Path) -> Self {
        let mut engine = ptr::null_mut();
        check(unsafe { uc_open(2, 0, &mut engine) });
        map_library(engine, path, 0, LIBRARY_SHA256);
        check(unsafe { uc_mem_map(engine, MODEL, 0x100000, 7) });
        for (plt, target) in [
            (0x486bf0_u64, 0x3c7328_u64),
            (0x486c30, 0x3c77b0),
            (0x486c20, GET_FRAME_CELL),
            (0x486c10, 0x3c7510),
            (0x486c00, 0x3c7474),
            (0x4872b0, 0x3dc280),
            (0x4872c0, 0x3dc420),
            (0x486930, 0x3dc670),
            (0x486810, GET_CELL_BACKGROUND),
        ] {
            bind_native(engine, plt, target);
        }
        let mut machine = Self {
            engine,
            heap: Box::new(Heap {
                cursor: HEAP,
                allocation_fill: 0,
                allocations: 0,
                fills: 0,
                deletes: 0,
            }),
        };
        for address in [NEW, DELETE, MEMSET] {
            write(engine, address, &0xd65f03c0_u32.to_le_bytes());
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    engine,
                    &mut hook,
                    4,
                    imported_call as *mut c_void,
                    ptr::from_mut(machine.heap.as_mut()).cast(),
                    address,
                    address,
                )
            });
        }
        register(engine, REGISTER_TPIDR_EL0, TLS);
        machine
    }
    fn load_drawing(&self, path: &Path) {
        map_library(self.engine, path, DRAWING_BASE, DRAWING_SHA256);
        for (plt, target) in [
            (DRAWING_BASE + 0xbcd50, 0x3dc280),
            (DRAWING_BASE + 0xbcd60, 0x3d8098),
            (DRAWING_BASE + 0xb8b00, DELETE),
            (DRAWING_BASE + 0xb8ae0, NEW),
            (0x486ed0, GET_BORDER_PATH),
        ] {
            bind_native(self.engine, plt, target);
        }
    }
    fn call(&self, function: u64, arguments: &[u64]) -> u64 {
        for (index, &value) in arguments.iter().enumerate() {
            register(self.engine, REGISTER_X0 + index as i32, value);
        }
        register(self.engine, REGISTER_X30, STOP);
        register(self.engine, REGISTER_SP, STACK);
        let error = unsafe { uc_emu_start(self.engine, function, STOP, 1_000_000, 1_000_000) };
        assert_eq!(
            error,
            0,
            "native function {function:x} failed at PC {:x}, LR {:x}, x0 {:x}",
            read_register(self.engine, 260),
            read_register(self.engine, REGISTER_X30),
            read_register(self.engine, REGISTER_X0)
        );
        assert_eq!(
            read_register(self.engine, 260),
            STOP,
            "native call did not return"
        );
        read_register(self.engine, REGISTER_X0)
    }
    fn capture(&mut self, name: &str, rows: usize, columns: usize, spans: &[[u32; 2]]) -> String {
        self.heap.allocation_fill = 0;
        let expected = self.fixture(name, rows, columns, spans);
        for fill in [0xa5, 0xff] {
            self.heap.allocation_fill = fill;
            assert_eq!(
                self.fixture(name, rows, columns, spans),
                expected,
                "allocation-fill changed native output for {name}"
            );
        }
        expected
    }
    fn initialize_grid(&mut self, rows: usize, columns: usize, spans: &[[u32; 2]]) {
        assert_eq!(spans.len(), rows * columns);
        assert!(rows <= 8 && columns <= 8 && spans.len() <= 64);
        for (index, &[row_span, column_span]) in spans.iter().enumerate() {
            assert!(row_span > 0 && row_span as usize <= rows - index / columns);
            assert!(column_span > 0 && column_span as usize <= columns - index % columns);
        }
        write(self.engine, MODEL, &vec![0; 0x100000]);
        self.heap.cursor = HEAP;
        self.heap.allocations = 0;
        self.heap.fills = 0;
        self.heap.deletes = 0;
        let rows_pointer = MODEL + 0x1000;
        let widths_pointer = MODEL + 0x1100;
        write(self.engine, MODEL + 104, &rows_pointer.to_le_bytes());
        write(
            self.engine,
            MODEL + 112,
            &(rows_pointer + rows as u64 * 8).to_le_bytes(),
        );
        write(self.engine, MODEL + 32, &widths_pointer.to_le_bytes());
        write(
            self.engine,
            MODEL + 40,
            &(widths_pointer + columns as u64 * 4).to_le_bytes(),
        );
        for row in 0..rows {
            let row_pointer = ROW_BASE + row as u64 * ROW_STRIDE;
            let cells_pointer = CELL_POINTER_BASE + row as u64 * 128;
            write(
                self.engine,
                rows_pointer + row as u64 * 8,
                &row_pointer.to_le_bytes(),
            );
            write(self.engine, row_pointer + 76, &(row as u32).to_le_bytes());
            write(self.engine, row_pointer + 80, &cells_pointer.to_le_bytes());
            for column in 0..columns {
                let position = row * columns + column;
                let cell_pointer = CELL_BASE + position as u64 * CELL_STRIDE;
                write(
                    self.engine,
                    cells_pointer + column as u64 * 8,
                    &cell_pointer.to_le_bytes(),
                );
                write(
                    self.engine,
                    cell_pointer + 48,
                    &(column as u32).to_le_bytes(),
                );
                write(
                    self.engine,
                    cell_pointer + 52,
                    &spans[position][0].to_le_bytes(),
                );
                write(
                    self.engine,
                    cell_pointer + 56,
                    &spans[position][1].to_le_bytes(),
                );
                write(self.engine, cell_pointer + 104, &row_pointer.to_le_bytes());
            }
        }
    }
    fn fixture(&mut self, name: &str, rows: usize, columns: usize, spans: &[[u32; 2]]) -> String {
        self.initialize_grid(rows, columns, spans);
        let owners: Vec<_> = (0..rows * columns)
            .map(|position| {
                let pointer = self.call(
                    GET_FRAME_CELL,
                    &[
                        MODEL,
                        (position / columns) as u64,
                        (position % columns) as u64,
                    ],
                );
                self.position(pointer, rows * columns)
            })
            .collect();
        register(self.engine, REGISTER_X0 + 8, RETURN_VECTOR);
        self.call(GET_VISIBLE_CELLS, &[MODEL]);
        let begin = read_u64(self.engine, RETURN_VECTOR);
        let end = read_u64(self.engine, RETURN_VECTOR + 8);
        assert!(begin <= end && (end - begin) / 8 <= (rows * columns) as u64);
        let visible: Vec<_> = (0..(end - begin) / 8)
            .map(|index| self.position(read_u64(self.engine, begin + index * 8), rows * columns))
            .collect();
        format!(
            "{{\"name\":{name:?},\"rows\":{rows},\"columns\":{columns},\"spans\":{spans:?},\"frame_owners\":{owners:?},\"visible\":{visible:?}}}"
        )
    }
    fn capture_borders(&mut self, case: &BorderCase) -> String {
        self.heap.allocation_fill = 0;
        let expected = self.border_fixture(case);
        for fill in [0xa5, 0xff] {
            self.heap.allocation_fill = fill;
            assert_eq!(self.border_fixture(case), expected, "{}", case.name);
        }
        expected
    }
    fn initialize_borders(&mut self, case: &BorderCase) -> [f32; 4] {
        let rows = case.heights.len();
        let columns = case.widths.len();
        assert_eq!(case.borders.len(), rows * columns);
        self.initialize_grid(rows, columns, &case.spans);
        for (axis, value) in case.origin.iter().enumerate() {
            write(
                self.engine,
                MODEL + 16 + axis as u64 * 4,
                &value.to_le_bytes(),
            );
        }
        for (row, height) in case.heights.iter().enumerate() {
            write(
                self.engine,
                ROW_BASE + row as u64 * ROW_STRIDE + 72,
                &height.to_le_bytes(),
            );
        }
        let width = case
            .widths
            .iter()
            .copied()
            .fold(case.origin[0], |sum, value| sum + value);
        let height = case
            .heights
            .iter()
            .copied()
            .fold(case.origin[1], |sum, value| sum + value);
        write(self.engine, MODEL + 24, &width.to_le_bytes());
        write(self.engine, MODEL + 28, &height.to_le_bytes());
        for (column, width) in case.widths.iter().enumerate() {
            write(
                self.engine,
                MODEL + 0x1100 + column as u64 * 4,
                &width.to_le_bytes(),
            );
        }
        if let Some(border) = case.default_border {
            if case.native_defaults {
                self.call(0x3dc670, &[MODEL + 0x7000]);
            } else {
                self.write_border(MODEL + 0x7000, border);
            }
            write(self.engine, MODEL + 160, &(MODEL + 0x7000).to_le_bytes());
        }
        if let Some(border) = case.outer_border {
            if case.native_defaults {
                self.call(0x3dc670, &[MODEL + 0x7100]);
            } else {
                self.write_border(MODEL + 0x7100, border);
            }
            write(self.engine, MODEL + 152, &(MODEL + 0x7100).to_le_bytes());
        }
        for (position, border) in case.borders.iter().enumerate() {
            if let Some(border) = border {
                let pointer = MODEL + 0x4000 + position as u64 * 128;
                self.write_border(pointer, *border);
                write(
                    self.engine,
                    CELL_BASE + position as u64 * CELL_STRIDE + 96,
                    &pointer.to_le_bytes(),
                );
            }
        }
        [case.origin[0], case.origin[1], width, height]
    }
    fn border_fixture(&mut self, case: &BorderCase) -> String {
        let bbox = self.initialize_borders(case);
        let rows = case.heights.len();
        let columns = case.widths.len();
        let mut paths = Vec::new();
        for position in 0..rows * columns {
            register(self.engine, REGISTER_X0 + 8, RETURN_VECTOR);
            self.call(
                GET_CELL_BORDER_PATH,
                &[
                    MODEL,
                    (position / columns) as u64,
                    (position % columns) as u64,
                ],
            );
            paths.push(self.read_border_paths(2 * (rows + columns)));
        }
        let borders: Vec<_> = case
            .borders
            .iter()
            .map(|border| border_json(*border))
            .collect();
        register(self.engine, REGISTER_X0 + 8, RETURN_VECTOR);
        self.call(GET_BORDER_PATH, &[MODEL]);
        let outer_paths = self.read_border_paths(4);
        format!(
            "{{\"name\":{:?},\"native_defaults\":{},\"content_bbox\":{:?},\"heights\":{:?},\"widths\":{:?},\"spans\":{:?},\"default_border\":{},\"outer_border\":{},\"borders\":[{}],\"paths\":[{}],\"outer_paths\":{outer_paths}}}",
            case.name,
            case.native_defaults,
            bbox,
            case.heights,
            case.widths,
            case.spans,
            border_json(case.default_border),
            border_json(case.outer_border),
            borders.join(","),
            paths.join(",")
        )
    }
    fn drawing_border_fixture(&mut self, case: &BorderCase, scale: f32) -> String {
        self.initialize_borders(case);
        write(self.engine, TABLE_OBJECT + 104, &MODEL.to_le_bytes());
        write(
            self.engine,
            DRAWING_OBJECT + 104,
            &TABLE_OBJECT.to_le_bytes(),
        );
        write(self.engine, CANVAS, &CANVAS_VTABLE.to_le_bytes());
        write(
            self.engine,
            CANVAS_VTABLE + 80,
            &MATRIX_GETTER.to_le_bytes(),
        );
        write(self.engine, CANVAS_MATRIX, &scale.to_le_bytes());
        write(self.engine, MATRIX_GETTER, &0x58000040_u32.to_le_bytes());
        write(
            self.engine,
            MATRIX_GETTER + 4,
            &0xd65f03c0_u32.to_le_bytes(),
        );
        write(self.engine, MATRIX_GETTER + 8, &CANVAS_MATRIX.to_le_bytes());
        register(self.engine, REGISTER_X0 + 8, RETURN_STYLE);
        self.call(GET_DRAWING_BORDER_STYLE, &[DRAWING_OBJECT, CANVAS]);
        let outline = format!(
            "{{\"color\":{},\"width\":{:?},\"rx\":{:?},\"ry\":{:?}}}",
            read_u32(self.engine, RETURN_STYLE),
            read_float(self.engine, RETURN_STYLE + 4),
            read_float(self.engine, RETURN_STYLE + 8),
            read_float(self.engine, RETURN_STYLE + 12)
        );
        format!(
            "{{\"name\":{:?},\"canvas_scale\":{scale:?},\"native_defaults\":{},\"outer_border\":{},\"outline\":{outline}}}",
            case.name,
            case.native_defaults,
            border_json(case.outer_border)
        )
    }
    fn capture_drawing_border(&mut self, case: &BorderCase, scale: f32) -> String {
        self.heap.allocation_fill = 0;
        let expected = self.drawing_border_fixture(case, scale);
        for fill in [0xa5, 0xff] {
            self.heap.allocation_fill = fill;
            assert_eq!(
                self.drawing_border_fixture(case, scale),
                expected,
                "{}, scale {scale}",
                case.name
            );
        }
        expected
    }
    fn background_fixture(
        &mut self,
        flags: u8,
        heading: u32,
        default: u32,
        merged: bool,
    ) -> String {
        let mut spans = vec![[1, 1]; 9];
        if merged {
            spans[0] = [2, 2];
        }
        self.initialize_grid(3, 3, &spans);
        write(self.engine, MODEL + 184, &[flags & 1, (flags >> 1) & 1]);
        write(self.engine, MODEL + 188, &heading.to_le_bytes());
        write(self.engine, MODEL + 192, &default.to_le_bytes());
        let table_callback = MODEL + 0xb800;
        let table_vtable = MODEL + 0xb900;
        let row_vtable = MODEL + 0xba00;
        write(self.engine, table_callback, &table_vtable.to_le_bytes());
        write(self.engine, table_callback + 8, &0x3c8cfc_u64.to_le_bytes());
        write(self.engine, table_callback + 24, &MODEL.to_le_bytes());
        write(self.engine, table_vtable + 48, &0x3d19bc_u64.to_le_bytes());
        write(self.engine, row_vtable + 48, &0x3c5aa0_u64.to_le_bytes());
        let mut cells = Vec::new();
        let mut colors = [Vec::new(), Vec::new()];
        for position in 0..9 {
            let row = position / 3;
            let column = position % 3;
            let row_pointer = ROW_BASE + row as u64 * ROW_STRIDE;
            let row_callback = MODEL + 0xb000 + row as u64 * 256;
            write(self.engine, row_callback, &row_vtable.to_le_bytes());
            write(self.engine, row_callback + 8, &0x3c3d3c_u64.to_le_bytes());
            write(self.engine, row_callback + 24, &row_pointer.to_le_bytes());
            write(self.engine, row_pointer + 48, &table_callback.to_le_bytes());
            let pointer = CELL_BASE + position as u64 * CELL_STRIDE;
            let owned = position % 2 == 0;
            let color: u32 = [
                0xff102030, 0, 0x80112233, 0x00123456, 0xffffffff, 0xff000000, 0x01000000,
                0xff405060, 0,
            ][position];
            write(self.engine, pointer + 32, &row_callback.to_le_bytes());
            write(self.engine, pointer + 60, &color.to_le_bytes());
            write(self.engine, pointer + 128, &[u8::from(owned)]);
            cells.push(format!("{{\"color\":{color},\"owned\":{owned}}}"));
            for honor_heading in 0..2 {
                let selected = self.call(GET_CELL_BACKGROUND, &[pointer, honor_heading]);
                assert_eq!(
                    selected,
                    self.call(
                        GET_TABLE_CELL_BACKGROUND,
                        &[MODEL, row as u64, column as u64, honor_heading]
                    )
                );
                colors[honor_heading as usize].push(selected.to_string());
            }
        }
        format!(
            "{{\"heading_row\":{},\"heading_column\":{},\"heading_color\":{heading},\"default_color\":{default},\"spans\":{spans:?},\"cells\":[{}],\"without_heading_override\":[{}],\"with_heading_override\":[{}]}}",
            flags & 1 != 0,
            flags & 2 != 0,
            cells.join(","),
            colors[0].join(","),
            colors[1].join(",")
        )
    }
    fn read_border_paths(&self, limit: usize) -> String {
        let begin = read_u64(self.engine, RETURN_VECTOR);
        let end = read_u64(self.engine, RETURN_VECTOR + 8);
        assert!(begin <= end && (end - begin) % 52 == 0);
        assert!((end - begin) / 52 <= limit as u64);
        let mut segments = Vec::new();
        for index in 0..(end - begin) / 52 {
            let pointer = begin + index * 52;
            let style = BorderStyle {
                color: read_u32(self.engine, pointer),
                width: read_float(self.engine, pointer + 4),
                start_radius: read_float(self.engine, pointer + 8),
                end_radius: read_float(self.engine, pointer + 12),
            };
            let endpoints =
                [20, 24, 28, 32].map(|offset| read_float(self.engine, pointer + offset));
            let equation = [36, 40, 44].map(|offset| read_float(self.engine, pointer + offset));
            let edge = read_u32(self.engine, pointer + 48);
            assert!([1, 2, 4, 8].contains(&edge));
            segments.push(format!("{{\"edge\":{edge},\"style\":{},\"endpoints\":{endpoints:?},\"equation\":{equation:?}}}", style.json()));
        }
        format!("[{}]", segments.join(","))
    }
    fn write_border(&self, pointer: u64, border: [BorderStyle; 4]) {
        for (edge, style) in border.iter().enumerate() {
            let pointer = pointer + edge as u64 * 20;
            write(self.engine, pointer, &style.color.to_le_bytes());
            for (offset, value) in [
                (4, style.width),
                (8, style.start_radius),
                (12, style.end_radius),
            ] {
                write(self.engine, pointer + offset, &value.to_le_bytes());
            }
        }
    }
    fn position(&self, pointer: u64, count: usize) -> usize {
        assert!(pointer >= CELL_BASE && pointer < CELL_BASE + count as u64 * CELL_STRIDE);
        assert_eq!((pointer - CELL_BASE) % CELL_STRIDE, 0);
        ((pointer - CELL_BASE) / CELL_STRIDE) as usize
    }
}
impl Drop for Machine {
    fn drop(&mut self) {
        check(unsafe { uc_close(self.engine) });
    }
}

#[derive(Clone, Copy)]
struct BorderStyle {
    color: u32,
    width: f32,
    start_radius: f32,
    end_radius: f32,
}
impl BorderStyle {
    fn json(self) -> String {
        format!(
            "{{\"color\":{},\"width\":{:?},\"start_radius\":{:?},\"end_radius\":{:?}}}",
            self.color, self.width, self.start_radius, self.end_radius
        )
    }
}

struct BorderCase {
    name: &'static str,
    native_defaults: bool,
    origin: [f32; 2],
    heights: Vec<f32>,
    widths: Vec<f32>,
    spans: Vec<[u32; 2]>,
    default_border: Option<[BorderStyle; 4]>,
    outer_border: Option<[BorderStyle; 4]>,
    borders: Vec<Option<[BorderStyle; 4]>>,
}
impl BorderCase {
    fn new(name: &'static str, rows: usize, columns: usize) -> Self {
        Self {
            name,
            native_defaults: false,
            origin: [13.25, -19.5],
            heights: (0..rows).map(|row| 31.25 + row as f32 * 36.5).collect(),
            widths: (0..columns)
                .map(|column| 41.125 + column as f32 * 42.75)
                .collect(),
            spans: vec![[1, 1]; rows * columns],
            default_border: None,
            outer_border: None,
            borders: (0..rows * columns)
                .map(|position| Some(distinct_border(position)))
                .collect(),
        }
    }
}
fn distinct_border(position: usize) -> [BorderStyle; 4] {
    std::array::from_fn(|edge| {
        let index = (position * 4 + edge) as u32;
        BorderStyle {
            color: 0xff102000 + index,
            width: 0.25 + index as f32 * 0.5,
            start_radius: 0.125 + index as f32 * 0.25,
            end_radius: 0.375 + index as f32 * 0.75,
        }
    })
}
fn border_json(border: Option<[BorderStyle; 4]>) -> String {
    border.map_or_else(
        || "null".into(),
        |styles| {
            format!(
                "[{}]",
                styles
                    .into_iter()
                    .map(BorderStyle::json)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        },
    )
}
fn border_cases(machine: &mut Machine) {
    let mut cases = Vec::new();
    cases.push(BorderCase::new("unit-distinct-styles", 2, 3));
    let mut outer = BorderCase::new("outer-border-independent-of-cell-styles", 2, 3);
    outer.outer_border = Some(distinct_border(32));
    cases.push(outer);
    let mut native = BorderCase::new("native-constructor-default-borders", 2, 3);
    native.native_defaults = true;
    native.borders.fill(None);
    let style = BorderStyle {
        color: 0xff000000,
        width: 1.0,
        start_radius: 0.0,
        end_radius: 0.0,
    };
    native.default_border = Some([style; 4]);
    native.outer_border = Some([style; 4]);
    cases.push(native);
    for (name, span) in [
        ("column-span-perimeter", [1, 3]),
        ("row-span-perimeter", [2, 1]),
        ("rectangle-span-perimeter", [2, 3]),
    ] {
        let mut case = BorderCase::new(name, 2, 3);
        case.spans[0] = span;
        cases.push(case);
    }
    let mut chain = BorderCase::new("covered-column-chain", 1, 4);
    chain.spans[0] = [1, 2];
    chain.spans[1] = [1, 3];
    cases.push(chain);
    let mut defaults = BorderCase::new("missing-cell-borders-use-default", 2, 3);
    defaults.borders.fill(None);
    defaults.default_border = Some(distinct_border(64));
    cases.push(defaults);
    let mut replaced = BorderCase::new("owned-border-replaces-all-default-edges", 2, 3);
    replaced.borders.fill(None);
    replaced.default_border = Some(distinct_border(64));
    replaced.borders[1] = Some(distinct_border(1));
    replaced.borders[4] = Some(distinct_border(4));
    replaced.spans[0] = [2, 3];
    cases.push(replaced);
    let mut absent = BorderCase::new("no-border-pointer-emits-no-path", 2, 3);
    absent.borders.fill(None);
    cases.push(absent);
    let mut suppressed = BorderCase::new("transparent-and-nonpositive-styles-retained", 1, 2);
    suppressed.borders[0] = Some(std::array::from_fn(|edge| BorderStyle {
        color: [0, 0x00123456, 0xff654321, 0xffabcdef][edge],
        width: [12.0, 2.0, 0.0, -5.0][edge],
        start_radius: 0.0,
        end_radius: 0.0,
    }));
    cases.push(suppressed);
    let mut rounded = BorderCase::new("fractional-prefix-rounding", 3, 3);
    rounded.origin = [100000.1, -100000.1];
    rounded.widths = vec![0.1, 0.3, 0.7];
    rounded.heights = vec![0.2, 0.4, 0.8];
    rounded.spans[0] = [2, 2];
    cases.push(rounded);
    let captures: Vec<_> = cases
        .iter()
        .map(|case| machine.capture_borders(case))
        .collect();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"library_sha256\":\"{LIBRARY_SHA256}\",\"cell_border_path_address\":\"0x3cad9c\",\"outer_border_path_address\":\"0x3cb464\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}

fn drawing_border_cases(machine: &mut Machine) {
    let mut cases = Vec::new();
    cases.push(BorderCase::new("absent-outer-border", 1, 1));
    let mut native = BorderCase::new("native-constructor-default", 1, 1);
    native.native_defaults = true;
    native.outer_border = Some(
        [BorderStyle {
            color: 0xff000000,
            width: 1.0,
            start_radius: 0.0,
            end_radius: 0.0,
        }; 4],
    );
    cases.push(native);
    for mask in 0..16 {
        let mut case = BorderCase::new("active-edge-mask", 1, 1);
        case.outer_border = Some(std::array::from_fn(|edge| BorderStyle {
            color: if mask & (1 << edge) == 0 {
                0
            } else {
                0xff102030 + edge as u32
            },
            width: [0.25, 1.0, 2.5, 4.0][edge],
            start_radius: [3.0, 7.0, 11.0, 13.0][edge],
            end_radius: [5.0, 2.0, 1.0, 17.0][edge],
        }));
        cases.push(case);
    }
    for (name, colors, widths, starts, ends) in [
        (
            "all-thin-widths",
            [0xff102030; 4],
            [0.1, 0.25, 0.75, 0.99],
            [0.0; 4],
            [0.0; 4],
        ),
        (
            "nonpositive-widths",
            [0xff102030; 4],
            [0.0, -1.0, -2.5, -100.0],
            [3.0, 7.0, 11.0, 13.0],
            [5.0, 2.0, 1.0, 17.0],
        ),
        (
            "mixed-widths",
            [0xff102030, 0xff405060, 0xff708090, 0xffabcdef],
            [-5.0, 0.0, 0.5, 3.0],
            [3.0, 7.0, 11.0, 13.0],
            [5.0, 2.0, 1.0, 17.0],
        ),
        (
            "zero-alpha-nonzero-colors",
            [0x00123456, 0x80112233, 0x00445566, 0x00000001],
            [1.0, 2.0, 3.0, 4.0],
            [0.0; 4],
            [0.0; 4],
        ),
        (
            "negative-radius-maxima",
            [0xff102030; 4],
            [1.0; 4],
            [-8.0, -4.0, -7.0, -3.0],
            [-2.0, -6.0, -5.0, -1.0],
        ),
        (
            "inactive-radius-maxima",
            [0xff102030, 0, 0xff708090, 0],
            [2.0, 0.0, 0.25, -10.0],
            [0.0, 21.0, 0.0, 30.0],
            [0.0, 22.0, 0.0, 29.0],
        ),
        (
            "one-rounded-axis",
            [0xff102030; 4],
            [1.0; 4],
            [3.0, 0.0, 9.0, 0.0],
            [5.0, 0.0, 8.0, 0.0],
        ),
    ] {
        let mut case = BorderCase::new(name, 1, 1);
        case.outer_border = Some(std::array::from_fn(|edge| BorderStyle {
            color: colors[edge],
            width: widths[edge],
            start_radius: starts[edge],
            end_radius: ends[edge],
        }));
        cases.push(case);
    }
    let mut captures = Vec::new();
    for case in &cases {
        for scale in [0.125, 0.3, 0.5, 0.75, 1.0, 2.0, 4.0] {
            captures.push(machine.capture_drawing_border(case, scale));
        }
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"border_style_address\":\"0xa6fb4\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}

fn background_cases(machine: &mut Machine) {
    let mut captures = Vec::new();
    for flags in 0..4 {
        for (heading, default) in [
            (0xff223344, 0xff667788),
            (0, 0xff667788),
            (0xff223344, 0),
            (0x80223344, 0x00667788),
            (0, 0),
        ] {
            for merged in [false, true] {
                captures.push(machine.background_fixture(flags, heading, default, merged));
            }
        }
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"library_sha256\":\"{LIBRARY_SHA256}\",\"cell_background_address\":\"0x3c2384\",\"table_background_address\":\"0x3cbf7c\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}

fn main() {
    let path = std::env::args_os()
        .nth(1)
        .expect("libSPenModel.so path required");
    let mut machine = Machine::new(Path::new(&path));
    let mode = std::env::args_os().nth(2);
    match mode.as_deref().and_then(|mode| mode.to_str()) {
        Some("--border-paths") => {
            border_cases(&mut machine);
            return;
        }
        Some("--drawing-borders") => {
            let drawing_path = std::env::args_os()
                .nth(3)
                .expect("libSPenDrawing.so path required");
            machine.load_drawing(Path::new(&drawing_path));
            drawing_border_cases(&mut machine);
            return;
        }
        Some("--backgrounds") => {
            background_cases(&mut machine);
            return;
        }
        Some("--column-minima") => {
            let drawing_path = std::env::args_os()
                .nth(3)
                .expect("libSPenDrawing.so path required");
            machine.load_drawing(Path::new(&drawing_path));
            columns::capture(&mut machine);
            return;
        }
        Some("--cold-frames") => {
            let drawing_path = std::env::args_os()
                .nth(3)
                .expect("libSPenDrawing.so path required");
            let base_path = std::env::args_os()
                .nth(4)
                .expect("libSPenBase.so path required");
            machine.load_drawing(Path::new(&drawing_path));
            frames::capture(&mut machine, Path::new(&base_path));
            return;
        }
        Some("--merge-cells") => {
            let drawing_path = std::env::args_os()
                .nth(3)
                .expect("libSPenDrawing.so path required");
            let base_path = std::env::args_os()
                .nth(4)
                .expect("libSPenBase.so path required");
            machine.load_drawing(Path::new(&drawing_path));
            merges::capture(&mut machine, Path::new(&base_path));
            return;
        }
        Some(mode @ ("--warm-rows" | "--cold-rows")) => {
            let drawing_path = std::env::args_os()
                .nth(3)
                .expect("libSPenDrawing.so path required");
            let base_path = std::env::args_os()
                .nth(4)
                .expect("libSPenBase.so path required");
            let widget_path = std::env::args_os()
                .nth(5)
                .expect("libSPenWidget.so path required");
            machine.load_drawing(Path::new(&drawing_path));
            let capture = if mode == "--cold-rows" {
                cold_rows::capture
            } else {
                rows::capture
            };
            capture(&mut machine, Path::new(&base_path), Path::new(&widget_path));
            return;
        }
        Some(
            mode @ ("--measured-geometry"
            | "--row-bottom"
            | "--warm-control"
            | "--cell-inputs"
            | "--lifecycle"),
        ) => {
            let drawing_path = std::env::args_os()
                .nth(3)
                .expect("libSPenDrawing.so path required");
            let base_path = std::env::args_os()
                .nth(4)
                .expect("libSPenBase.so path required");
            let widget_path = std::env::args_os()
                .nth(5)
                .expect("libSPenWidget.so path required");
            let text_path = std::env::args_os()
                .nth(6)
                .expect("libSPenText.so path required");
            machine.load_drawing(Path::new(&drawing_path));
            let capture = match mode {
                "--row-bottom" => bottom::capture,
                "--warm-control" => control::capture,
                "--cell-inputs" => cell_inputs::capture,
                "--lifecycle" => lifecycle::capture,
                _ => geometry::capture,
            };
            capture(
                &mut machine,
                Path::new(&base_path),
                Path::new(&widget_path),
                Path::new(&text_path),
            );
            return;
        }
        Some("--row-splits") => {
            let drawing_path = std::env::args_os()
                .nth(3)
                .expect("libSPenDrawing.so path required");
            let base_path = std::env::args_os()
                .nth(4)
                .expect("libSPenBase.so path required");
            machine.load_drawing(Path::new(&drawing_path));
            splits::capture(&mut machine, Path::new(&base_path));
            return;
        }
        None => {}
        _ => panic!(
            "expected --border-paths, --drawing-borders, --backgrounds, --column-minima, --cold-frames, --merge-cells, --cold-rows, --warm-rows, --measured-geometry, --row-splits, --row-bottom, --warm-control, --cell-inputs, --lifecycle or no capture mode"
        ),
    }
    let mut cases = Vec::new();
    for (name, rows, columns, changes) in [
        ("unit", 3, 4, vec![]),
        ("column-span", 3, 4, vec![(0, [1, 3])]),
        ("row-span", 3, 4, vec![(1, [3, 1])]),
        ("rectangle-span", 3, 4, vec![(0, [2, 3])]),
        ("latent-column-chain", 1, 4, vec![(0, [1, 2]), (1, [1, 3])]),
        ("latent-row-chain", 4, 1, vec![(0, [2, 1]), (1, [3, 1])]),
        (
            "overlapping-rectangles",
            3,
            4,
            vec![(0, [2, 2]), (1, [3, 3])],
        ),
    ] {
        let mut spans = vec![[1, 1]; rows * columns];
        for (position, span) in changes {
            spans[position] = span;
        }
        cases.push(machine.capture(name, rows, columns, &spans));
    }
    for first in [[1, 1], [1, 2], [2, 1], [2, 2]] {
        for second in [[1, 1], [2, 1]] {
            for third in [[1, 1], [1, 2]] {
                let spans = [first, second, third, [1, 1]];
                cases.push(machine.capture(
                    &format!("exhaustive-2x2-{}", cases.len() - 7),
                    2,
                    2,
                    &spans,
                ));
            }
        }
    }
    let mut random = 0x6d_6572_6765_64_u64;
    for index in 0..256 {
        let rows = 1 + index % 5;
        let columns = 1 + index / 5 % 5;
        let mut spans = Vec::new();
        for row in 0..rows {
            for column in 0..columns {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                let row_span = 1 + random as usize % (rows - row);
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                let column_span = 1 + random as usize % (columns - column);
                spans.push([row_span as u32, column_span as u32]);
            }
        }
        cases.push(machine.capture(&format!("mixed-{index}"), rows, columns, &spans));
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"allocation_fills\":[0,165,255],\"library_sha256\":\"{LIBRARY_SHA256}\",\"frame_owner_address\":\"0x3c75c0\",\"visible_cells_address\":\"0x3c7784\",\"cases\":[\n{}\n]}}",
        cases.join(",\n")
    );
}
