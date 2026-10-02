use super::*;

const GET_CELL: u64 = 0x3c7570;
const GET_PUBLIC_CELL: u64 = 0x3d2be0;
const NULL_CANDIDATE_SPAN_LOAD: u64 = 0x3c7678;
const COLUMNS: usize = 3;

unsafe extern "C" {
    fn uc_emu_stop(engine: Engine) -> i32;
}

struct Case {
    name: String,
    rows: usize,
    lengths: Vec<usize>,
    null_slots: Vec<usize>,
    spans: Vec<[u32; 2]>,
}

impl Case {
    fn new(name: String, rows: usize) -> Self {
        Self {
            name,
            rows,
            lengths: vec![COLUMNS; rows],
            null_slots: Vec::new(),
            spans: vec![[1, 1]; rows * COLUMNS],
        }
    }

    fn initialize(&self, machine: &mut Machine, fill: u8) {
        write(machine.engine, MODEL, &vec![fill; 0x100000]);
        machine.heap.cursor = HEAP;
        machine.heap.allocation_fill = fill;
        let row_pointers = MODEL + 0x1000;
        let column_widths = MODEL + 0x1100;
        write(machine.engine, MODEL + 104, &row_pointers.to_le_bytes());
        write(
            machine.engine,
            MODEL + 112,
            &(row_pointers + self.rows as u64 * 8).to_le_bytes(),
        );
        write(machine.engine, MODEL + 32, &column_widths.to_le_bytes());
        write(
            machine.engine,
            MODEL + 40,
            &(column_widths + COLUMNS as u64 * 4).to_le_bytes(),
        );
        write(machine.engine, TABLE_OBJECT + 104, &MODEL.to_le_bytes());
        for row in 0..self.rows {
            let object = ROW_BASE + row as u64 * ROW_STRIDE;
            let pointers = CELL_POINTER_BASE + row as u64 * 128;
            write(
                machine.engine,
                row_pointers + row as u64 * 8,
                &object.to_le_bytes(),
            );
            write(machine.engine, object + 76, &(row as u32).to_le_bytes());
            write(machine.engine, object + 80, &pointers.to_le_bytes());
            write(
                machine.engine,
                object + 88,
                &(pointers + self.lengths[row] as u64 * 8).to_le_bytes(),
            );
            write(
                machine.engine,
                object + 96,
                &(pointers + COLUMNS as u64 * 8).to_le_bytes(),
            );
            for column in 0..COLUMNS {
                let index = row * COLUMNS + column;
                let cell = CELL_BASE + index as u64 * CELL_STRIDE;
                let stored = if self.null_slots.contains(&index) {
                    0
                } else {
                    cell
                };
                write(
                    machine.engine,
                    pointers + column as u64 * 8,
                    &stored.to_le_bytes(),
                );
                write(machine.engine, cell + 48, &(column as u32).to_le_bytes());
                write(
                    machine.engine,
                    cell + 52,
                    &self.spans[index][0].to_le_bytes(),
                );
                write(
                    machine.engine,
                    cell + 56,
                    &self.spans[index][1].to_le_bytes(),
                );
                write(machine.engine, cell + 104, &object.to_le_bytes());
            }
        }
    }
}

struct PointerRead {
    pc: u64,
    row: usize,
    column: usize,
    outside_declared_length: bool,
}

struct Trace {
    lengths: Vec<usize>,
    reads: Vec<PointerRead>,
    null_candidate: bool,
}

unsafe extern "C" fn pointer_read(
    engine: Engine,
    _: i32,
    address: u64,
    size: i32,
    _: i64,
    data: *mut c_void,
) {
    let trace = unsafe { &mut *data.cast::<Trace>() };
    let offset = address - CELL_POINTER_BASE;
    let row = offset as usize / 128;
    let within_row = offset as usize % 128;
    if row >= trace.lengths.len() || within_row >= COLUMNS * 8 {
        return;
    }
    assert_eq!(size, 8);
    assert_eq!(within_row % 8, 0);
    let column = within_row / 8;
    trace.reads.push(PointerRead {
        pc: read_register(engine, 260),
        row,
        column,
        outside_declared_length: column >= trace.lengths[row],
    });
}

unsafe extern "C" fn stop_before_null_span(engine: Engine, _: u64, _: u32, data: *mut c_void) {
    if read_register(engine, REGISTER_X0) == 0 {
        let trace = unsafe { &mut *data.cast::<Trace>() };
        trace.null_candidate = true;
        check(unsafe { uc_emu_stop(engine) });
    }
}

struct Hooks {
    engine: Engine,
    handles: [usize; 2],
    trace: Box<Trace>,
}

impl Hooks {
    fn new(machine: &Machine, case: &Case) -> Self {
        let mut hooks = Self {
            engine: machine.engine,
            handles: [0; 2],
            trace: Box::new(Trace {
                lengths: case.lengths.clone(),
                reads: Vec::new(),
                null_candidate: false,
            }),
        };
        for (index, kind, callback, begin, end) in [
            (
                0,
                1 << 10,
                pointer_read as *mut c_void,
                CELL_POINTER_BASE,
                CELL_POINTER_BASE + case.rows as u64 * 128 - 1,
            ),
            (
                1,
                4,
                stop_before_null_span as *mut c_void,
                NULL_CANDIDATE_SPAN_LOAD,
                NULL_CANDIDATE_SPAN_LOAD,
            ),
        ] {
            check(unsafe {
                uc_hook_add(
                    hooks.engine,
                    &mut hooks.handles[index],
                    kind,
                    callback,
                    ptr::from_mut(hooks.trace.as_mut()).cast(),
                    begin,
                    end,
                )
            });
        }
        hooks
    }
}

impl Drop for Hooks {
    fn drop(&mut self) {
        for handle in self.handles {
            check(unsafe { uc_hook_del(self.engine, handle) });
        }
    }
}

fn call(machine: &Machine, case: &Case, function: u64, row: usize, column: usize) -> String {
    let hooks = Hooks::new(machine, case);
    let object = if function == GET_PUBLIC_CELL {
        TABLE_OBJECT
    } else {
        MODEL
    };
    for (index, value) in [object, row as u64, column as u64].into_iter().enumerate() {
        register(machine.engine, REGISTER_X0 + index as i32, value);
    }
    register(machine.engine, REGISTER_X30, STOP);
    register(machine.engine, REGISTER_SP, STACK);
    check(unsafe { uc_emu_start(machine.engine, function, STOP, 1_000_000, 1_000_000) });
    let pc = read_register(machine.engine, 260);
    let (status, result) = if hooks.trace.null_candidate {
        assert_eq!(function, GET_FRAME_CELL);
        assert_eq!(pc, NULL_CANDIDATE_SPAN_LOAD);
        ("stopped-before-null-span", "null".to_owned())
    } else {
        assert_eq!(pc, STOP);
        let pointer = read_register(machine.engine, REGISTER_X0);
        let position = if pointer == 0 {
            "null".to_owned()
        } else {
            machine.position(pointer, case.rows * COLUMNS).to_string()
        };
        ("returned", position)
    };
    let reads = hooks
        .trace
        .reads
        .iter()
        .map(|read| {
            format!(
                "{{\"pc\":\"0x{:x}\",\"position\":[{},{}],\"outside_declared_length\":{}}}",
                read.pc, read.row, read.column, read.outside_declared_length,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"status\":{status:?},\"result_slot\":{result},\"pointer_reads\":[{reads}]}}")
}

fn sample(machine: &mut Machine, case: &Case, fill: u8) -> String {
    case.initialize(machine, fill);
    let mut queries = Vec::new();
    for row in 0..case.rows {
        for column in 0..COLUMNS {
            let raw = call(machine, case, GET_CELL, row, column);
            let public = call(machine, case, GET_PUBLIC_CELL, row, column);
            assert_eq!(
                raw.replace("0x3c75b0", "0x3d2c60"),
                public,
                "{}: public/raw cell selection differs",
                case.name,
            );
            let owner = call(machine, case, GET_FRAME_CELL, row, column);
            queries.push(format!(
                "{{\"position\":[{row},{column}],\"get_cell\":{raw},\"public_get_cell\":{public},\"get_frame_cell\":{owner}}}"
            ));
        }
    }
    format!(
        "  {{\"name\":{:?},\"rows\":{},\"columns\":{COLUMNS},\"declared_row_lengths\":{:?},\"backing_row_capacity\":{COLUMNS},\"null_slots\":{:?},\"spans\":{:?},\"queries\":[{}]}}",
        case.name,
        case.rows,
        case.lengths,
        case.null_slots,
        case.spans,
        queries.join(","),
    )
}

pub(super) fn capture(machine: &mut Machine) {
    bind_native(machine.engine, 0x4872e0, 0x3d2b20);
    bind_native(machine.engine, 0x4872f0, 0x3d2b80);
    let mut cases = Vec::new();
    for length in 0..=COLUMNS {
        let mut case = Case::new(format!("one-row-length-{length}"), 1);
        case.lengths[0] = length;
        cases.push(case);
    }
    for row in 0..2 {
        for length in 0..COLUMNS {
            let mut case = Case::new(format!("two-rows-row-{row}-length-{length}"), 2);
            case.lengths[row] = length;
            cases.push(case);
        }
    }
    for mask in 1..1 << COLUMNS {
        let mut case = Case::new(format!("one-row-null-mask-{mask}"), 1);
        case.null_slots = (0..COLUMNS)
            .filter(|column| mask & (1 << column) != 0)
            .collect();
        cases.push(case);
    }
    for slot in 0..2 * COLUMNS {
        let mut case = Case::new(format!("two-rows-null-slot-{slot}"), 2);
        case.null_slots.push(slot);
        cases.push(case);
    }
    for null_slots in [vec![1, 2, 3, 4, 5], vec![2, 4]] {
        let mut case = Case::new(format!("merged-owner-null-slots-{null_slots:?}"), 2);
        case.spans[0] = [2, 3];
        case.null_slots = null_slots;
        cases.push(case);
    }
    let results = cases
        .iter()
        .map(|case| {
            let expected = sample(machine, case, 0);
            for fill in [0xa5, 0xff] {
                assert_eq!(sample(machine, case, fill), expected, "{}", case.name);
            }
            expected
        })
        .collect::<Vec<_>>()
        .join(",\n");
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"model_sha256\":\"{LIBRARY_SHA256}\",\"memory_fills\":[0,165,255],\"get_cell_address\":\"0x3c7570\",\"public_get_cell_address\":\"0x3d2be0\",\"get_frame_cell_address\":\"0x3c75c0\",\"null_candidate_stop_address\":\"0x3c7678\",\"limits\":{{\"topology\":\"supplied row and cell backing buffers\",\"native_construction\":false,\"native_binary_loading\":false,\"sparse_editor_support\":false,\"null_candidates\":\"stopped before native span dereference\",\"short_rows\":\"initialized backing capacity extends beyond declared vector length\"}},\"cases\":[\n{results}\n]}}"
    );
}
