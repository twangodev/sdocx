use super::*;
use std::collections::BTreeMap;

const CPP_SHA256: &str = "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4";
const BODY_SHA256: &str = "27324ca3807f07e0c1d0647b23eb9af1296762a8c9d892ee486f37b1eb9543f0";
const BODY: u64 = 0x0700_0000;
const VIEW: u64 = MODEL + 0x19000;
const OWNER: u64 = MODEL + 0x1a000;
const BUCKETS: u64 = MODEL + 0x1b000;
const NODE: u64 = MODEL + 0x1b100;
const INPUT_RECT: u64 = MODEL + 0x18000;

pub(super) struct Paths<'a> {
    pub model: &'a Path,
    pub base: &'a Path,
    pub drawing: &'a Path,
    pub body: &'a Path,
    pub cpp: &'a Path,
}

struct Elf {
    binary: Vec<u8>,
    sections: Vec<[u8; 64]>,
}
impl Elf {
    fn new(path: &Path) -> Self {
        let binary = fs::read(path).unwrap();
        let offset = u64::from_le_bytes(binary[40..48].try_into().unwrap()) as usize;
        let stride = u16::from_le_bytes(binary[58..60].try_into().unwrap()) as usize;
        let count = u16::from_le_bytes(binary[60..62].try_into().unwrap()) as usize;
        let sections = (0..count)
            .map(|i| {
                binary[offset + i * stride..offset + i * stride + 64]
                    .try_into()
                    .unwrap()
            })
            .collect();
        Self { binary, sections }
    }
    fn symbols(&self) -> BTreeMap<String, u64> {
        let mut found = BTreeMap::new();
        for section in &self.sections {
            if u32::from_le_bytes(section[4..8].try_into().unwrap()) != 11 {
                continue;
            }
            let offset = u64::from_le_bytes(section[24..32].try_into().unwrap()) as usize;
            let size = u64::from_le_bytes(section[32..40].try_into().unwrap()) as usize;
            for symbol in self.binary[offset..offset + size].chunks_exact(24) {
                if u16::from_le_bytes(symbol[6..8].try_into().unwrap()) == 0 {
                    continue;
                }
                found.insert(
                    self.name(section, symbol),
                    u64::from_le_bytes(symbol[8..16].try_into().unwrap()),
                );
            }
        }
        found
    }
    fn name(&self, section: &[u8; 64], symbol: &[u8]) -> String {
        let strings =
            &self.sections[u32::from_le_bytes(section[40..44].try_into().unwrap()) as usize];
        let offset = u64::from_le_bytes(strings[24..32].try_into().unwrap()) as usize;
        let name = u32::from_le_bytes(symbol[..4].try_into().unwrap()) as usize;
        let start = &self.binary[offset + name..];
        String::from_utf8(start[..start.iter().position(|b| *b == 0).unwrap()].to_vec()).unwrap()
    }
    fn bind(&self, engine: Engine, base: u64, providers: &BTreeMap<String, u64>) {
        for section in &self.sections {
            if u32::from_le_bytes(section[4..8].try_into().unwrap()) != 4 {
                continue;
            }
            let offset = u64::from_le_bytes(section[24..32].try_into().unwrap()) as usize;
            let size = u64::from_le_bytes(section[32..40].try_into().unwrap()) as usize;
            let symbols =
                &self.sections[u32::from_le_bytes(section[40..44].try_into().unwrap()) as usize];
            let symbol_offset = u64::from_le_bytes(symbols[24..32].try_into().unwrap()) as usize;
            for relocation in self.binary[offset..offset + size].chunks_exact(24) {
                let info = u64::from_le_bytes(relocation[8..16].try_into().unwrap());
                let symbol = &self.binary[symbol_offset + (info >> 32) as usize * 24..][..24];
                if u16::from_le_bytes(symbol[6..8].try_into().unwrap()) != 0
                    || ![257, 1025, 1026].contains(&(info as u32))
                {
                    continue;
                }
                if let Some(target) = providers.get(&self.name(symbols, symbol)) {
                    let addend = i64::from_le_bytes(relocation[16..24].try_into().unwrap());
                    let address = u64::from_le_bytes(relocation[..8].try_into().unwrap());
                    write(
                        engine,
                        base + address,
                        &target.checked_add_signed(addend).unwrap().to_le_bytes(),
                    );
                }
            }
        }
    }
}

const RET: u64 = 0x0300_0c00;
const UUID_CREATE: u64 = RET + 32;
const UUID_MAKE: u64 = RET + 64;
const UUID_EXPORT: u64 = RET + 96;
const UUID_DESTROY: u64 = RET + 128;
const STRLEN: u64 = RET + 160;
const COPY: u64 = RET + 192;
const COMPARE: u64 = RET + 224;
const GUARD_ACQUIRE: u64 = RET + 256;
const GUARD_RELEASE: u64 = RET + 288;
const MEMCOMPARE: u64 = RET + 320;
unsafe extern "C" fn uuid(engine: Engine, address: u64, _: u32, _: *mut c_void) {
    if address == UUID_CREATE {
        write(
            engine,
            read_register(engine, REGISTER_X0),
            &(MODEL + 0xef400).to_le_bytes(),
        );
    }
    if address == UUID_EXPORT {
        let out = read_u64(engine, read_register(engine, REGISTER_X0 + 2));
        let counter = MODEL + 0xef500;
        let sequence = read_u32(engine, counter) + 1;
        write(engine, counter, &sequence.to_le_bytes());
        write(
            engine,
            out,
            format!("00000000-0000-4000-8000-{sequence:012x}\0").as_bytes(),
        );
    }
    register(engine, REGISTER_X0, 0);
}
unsafe extern "C" fn platform(engine: Engine, address: u64, _: u32, _: *mut c_void) {
    let x0 = read_register(engine, REGISTER_X0);
    let x1 = read_register(engine, REGISTER_X0 + 1);
    let byte = |p| {
        let mut b = 0_u8;
        check(unsafe { uc_mem_read(engine, p, ptr::from_mut(&mut b).cast(), 1) });
        b
    };
    let result = match address {
        STRLEN => {
            let n = (0..4096).find(|i| byte(x0 + i) == 0).unwrap();
            n
        }
        COPY => {
            let n = read_register(engine, REGISTER_X0 + 2) as usize;
            assert!(n < 65536);
            let mut b = vec![0; n];
            check(unsafe { uc_mem_read(engine, x1, b.as_mut_ptr().cast(), n) });
            write(engine, x0, &b);
            x0
        }
        COMPARE => {
            let mut r = 0_i32;
            for i in 0..4096 {
                let a = byte(x0 + i);
                let b = byte(x1 + i);
                r = i32::from(a) - i32::from(b);
                if r != 0 || a == 0 {
                    break;
                }
            }
            r as u32 as u64
        }
        MEMCOMPARE => {
            let n = read_register(engine, REGISTER_X0 + 2);
            assert!(n < 65536);
            let mut r = 0_i32;
            for i in 0..n {
                r = i32::from(byte(x0 + i)) - i32::from(byte(x1 + i));
                if r != 0 {
                    break;
                }
            }
            r as u32 as u64
        }
        GUARD_ACQUIRE => u64::from(byte(x0) == 0),
        GUARD_RELEASE => {
            write(engine, x0, &[1]);
            0
        }
        _ => unreachable!(),
    };
    register(engine, REGISTER_X0, result);
}
pub(super) fn initialize(paths: &Paths, fill: u8) -> Machine {
    let model = paths.model;
    let base = paths.base;
    let body = paths.body;
    let cpp = paths.cpp;
    let mut machine = Machine::new(model);
    machine.heap.allocation_fill = fill;
    frames::load_base(&machine, &base);
    machine.load_drawing(paths.drawing);
    map_library(machine.engine, &body, BODY, BODY_SHA256);
    map_library(machine.engine, cpp, 0x06000000, CPP_SHA256);
    let mut providers: BTreeMap<String, u64> = Elf::new(&cpp)
        .symbols()
        .into_iter()
        .map(|(n, v)| (n, v + 0x06000000))
        .collect();
    providers.extend(
        Elf::new(&base)
            .symbols()
            .into_iter()
            .map(|(name, value)| (name, value + frames::BASE)),
    );
    providers.extend(Elf::new(&model).symbols());
    providers.extend(
        Elf::new(paths.drawing)
            .symbols()
            .into_iter()
            .map(|(n, v)| (n, v + DRAWING_BASE)),
    );
    providers.extend(
        Elf::new(&body)
            .symbols()
            .into_iter()
            .map(|(n, v)| (n, v + 0x07000000)),
    );
    for (name, address) in [
        ("strlen", STRLEN),
        ("memcpy", COPY),
        ("memmove", COPY),
        ("strcmp", COMPARE),
        ("memcmp", MEMCOMPARE),
        ("__cxa_guard_acquire", GUARD_ACQUIRE),
        ("__cxa_guard_release", GUARD_RELEASE),
        ("__cxa_atexit", RET),
    ] {
        providers.insert(name.to_owned(), address);
    }
    for name in [
        "pthread_mutex_lock",
        "pthread_mutex_unlock",
        "pthread_mutex_init",
        "pthread_mutex_destroy",
        "pthread_mutexattr_init",
        "pthread_mutexattr_settype",
        "pthread_mutexattr_destroy",
    ] {
        providers.insert(name.to_owned(), RET);
    }
    Elf::new(&model).bind(machine.engine, 0, &providers);
    Elf::new(&base).bind(machine.engine, frames::BASE, &providers);
    Elf::new(&cpp).bind(machine.engine, 0x06000000, &providers);
    Elf::new(paths.drawing).bind(machine.engine, DRAWING_BASE, &providers);
    Elf::new(&body).bind(machine.engine, 0x07000000, &providers);
    for plt in [0x47ac30, 0x47ac40, 0x020e55f0, 0x020e56f0, 0x020e5780] {
        bind_native(machine.engine, plt, NEW);
    }
    for plt in [0x47ac50, 0x47b090, 0x020e55d0, 0x020e5650] {
        bind_native(machine.engine, plt, DELETE);
    }
    write(
        machine.engine,
        RET,
        &[0x00, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6],
    );
    for plt in [
        0x47ac10, 0x47ac20, 0x47b060, 0x47b6c0, 0x47b1e0, 0x47b1f0, 0x020e5f30,
    ] {
        bind_native(machine.engine, plt, RET);
    }
    for (plt, target) in [
        (0x020e6460, UUID_CREATE),
        (0x020e6470, UUID_MAKE),
        (0x020e6480, UUID_EXPORT),
        (0x020e6490, UUID_DESTROY),
    ] {
        bind_native(machine.engine, plt, target);
        write(machine.engine, target, &0xd65f03c0_u32.to_le_bytes());
        let mut hook = 0;
        check(unsafe {
            uc_hook_add(
                machine.engine,
                &mut hook,
                4,
                uuid as *mut c_void,
                ptr::null_mut(),
                target,
                target,
            )
        });
    }
    for address in [
        STRLEN,
        COPY,
        COMPARE,
        GUARD_ACQUIRE,
        GUARD_RELEASE,
        MEMCOMPARE,
    ] {
        write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
        let mut hook = 0;
        check(unsafe {
            uc_hook_add(
                machine.engine,
                &mut hook,
                4,
                platform as *mut c_void,
                ptr::null_mut(),
                address,
                address,
            )
        });
    }
    for initializer in [0x2a2ed0, 0x2863a0, 0x3c35ec, 0x3c5918] {
        assert!(
            (0x4a3208..0x4a38b0)
                .step_by(8)
                .any(|address| read_u64(machine.engine, address) == initializer)
        );
        machine.call(initializer, &[]);
    }

    machine
}

fn rectangle(machine: &Machine, address: u64) -> [f32; 4] {
    std::array::from_fn(|index| read_float(machine.engine, address + index as u64 * 4))
}

fn write_rectangle(machine: &Machine, address: u64, values: [f32; 4]) {
    for (index, value) in values.into_iter().enumerate() {
        write(
            machine.engine,
            address + index as u64 * 4,
            &value.to_le_bytes(),
        );
    }
}

fn float_arguments(machine: &Machine, values: [f32; 4]) {
    for (index, value) in values.into_iter().enumerate() {
        register(
            machine.engine,
            136 + index as i32,
            u64::from(value.to_bits()),
        );
    }
}

fn frame(machine: &Machine, cell: u64) -> [f32; 4] {
    machine.call(DRAWING_BASE + 0xab4bc, &[frames::LAYOUT, cell]);
    std::array::from_fn(|index| {
        f32::from_bits(read_register(machine.engine, 136 + index as i32) as u32)
    })
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    rows: usize,
    columns: usize,
    bounds: [f32; 4],
    caller_rect: [f32; 4],
    change: f32,
    merged: bool,
    clean: bool,
    divergent: bool,
    raw_table_translation: bool,
}

impl Case {
    fn new(name: &'static str) -> Self {
        let bounds = [13.25, -19.5, 173.25, 180.5];
        Self {
            name,
            rows: 2,
            columns: 2,
            bounds,
            caller_rect: bounds,
            change: 37.25,
            merged: false,
            clean: false,
            divergent: false,
            raw_table_translation: false,
        }
    }

    fn cells(self, machine: &Machine) -> Vec<u64> {
        (0..self.rows)
            .flat_map(|row| {
                (0..self.columns).map(move |column| {
                    machine.call(0x3d2be0, &[TABLE_OBJECT, row as u64, column as u64])
                })
            })
            .collect()
    }

    fn snapshot(self, machine: &Machine, stage: &str, prepared: bool) -> String {
        let slots = self.cells(machine);
        let table_impl = read_u64(machine.engine, TABLE_OBJECT + 104);
        let table_data = read_u64(
            machine.engine,
            read_u64(machine.engine, TABLE_OBJECT + 16) + 24,
        );
        let cells:Vec<String>=slots.iter().enumerate().map(|(index,&cell)|{
            let source=read_u64(machine.engine,cell+88);
            let implementation=read_u64(machine.engine,source+16);
            let data=read_u64(machine.engine,implementation+24);
            let flags=read_u32(machine.engine,cell+80);
            let owner=machine.call(GET_FRAME_CELL,&[table_impl,(index/self.columns) as u64,(index%self.columns) as u64]);
            let owner=slots.iter().position(|&slot|slot==owner).unwrap();
            let cell_frame=if prepared {format!("{:?}",frame(machine,cell))}else{"null".into()};
            format!("{{\"slot\":{index},\"owner\":{owner},\"span\":[{},{}],\"cell_rect\":{:?},\"content_model_rect\":{:?},\"content_drawn_rect\":{:?},\"drawing_frame\":{cell_frame},\"cell_changed\":{},\"cell_unsaved\":{},\"content_changed\":{},\"content_unsaved\":{},\"shape_type\":{}}}",read_u32(machine.engine,cell+52),read_u32(machine.engine,cell+56),rectangle(machine,cell+64),rectangle(machine,data+8),rectangle(machine,data+24),(flags>>8)&255,(flags>>16)&255,read_u32(machine.engine,implementation+240)&255,(read_u32(machine.engine,implementation+240)>>8)&255,machine.call(0x396e14,&[source]))
        }).collect();
        let cached = if prepared {
            format!("{:?}", rectangle(machine, frames::LAYOUT + 652))
        } else {
            "null".into()
        };
        format!(
            "{{\"stage\":{stage:?},\"table_model_rect\":{:?},\"table_drawn_rect\":{:?},\"table_content_rect\":{:?},\"drawing_content_rect\":{cached},\"cells\":[{}]}}",
            rectangle(machine, table_data + 8),
            rectangle(machine, table_data + 24),
            rectangle(machine, table_impl + 16),
            cells.join(",")
        )
    }

    fn fixture(self, paths: &Paths, fill: u8) -> String {
        let machine = initialize(paths, fill);
        write_rectangle(&machine, INPUT_RECT, self.bounds);
        machine.call(0x3d2690, &[TABLE_OBJECT]);
        assert_eq!(
            machine.call(
                0x3d27d8,
                &[
                    TABLE_OBJECT,
                    INPUT_RECT,
                    self.rows as u64,
                    self.columns as u64
                ]
            ),
            1
        );
        let mut stages = vec![self.snapshot(&machine, "constructed", false)];
        if self.raw_table_translation {
            float_arguments(&machine, self.caller_rect);
            machine.call(0x2d2b18, &[TABLE_OBJECT]);
            stages.push(self.snapshot(&machine, "table_base_data_translation", false));
        }
        if self.merged {
            assert_eq!(
                machine.call(
                    0x3d6038,
                    &[
                        TABLE_OBJECT,
                        0,
                        0,
                        (self.rows - 1) as u64,
                        (self.columns - 1) as u64
                    ]
                ),
                1
            );
            stages.push(self.snapshot(&machine, "native_merge", false));
        }
        let table_impl = read_u64(machine.engine, TABLE_OBJECT + 104);
        frames::configure_layout(&machine);
        write(
            machine.engine,
            TABLE_OBJECT + 104,
            &table_impl.to_le_bytes(),
        );
        write(
            machine.engine,
            frames::LAYOUT,
            &(DRAWING_BASE + 0xc4b30).to_le_bytes(),
        );
        machine.call(DRAWING_BASE + 0xaa6b4, &[frames::LAYOUT]);
        machine.call(DRAWING_BASE + 0xab168, &[frames::LAYOUT]);
        write(machine.engine, VIEW + 1048, &OWNER.to_le_bytes());
        let map = OWNER + 496;
        let handle = machine.call(0x2cb0d0, &[TABLE_OBJECT]);
        for (address, value) in [
            (map, BUCKETS),
            (map + 8, 1),
            (map + 16, NODE),
            (map + 24, 1),
            (BUCKETS, map + 16),
            (NODE + 8, handle),
            (NODE + 24, frames::LAYOUT),
        ] {
            write(machine.engine, address, &value.to_le_bytes());
        }
        write(machine.engine, map + 32, &1_f32.to_le_bytes());
        write(machine.engine, NODE + 16, &(handle as u32).to_le_bytes());
        if self.divergent {
            let cell = self.cells(&machine)[0];
            let source = read_u64(machine.engine, cell + 88);
            write_rectangle(&machine, INPUT_RECT, frame(&machine, cell));
            float_arguments(&machine, self.caller_rect);
            machine.call(frames::BASE + 0xb11a4, &[INPUT_RECT]);
            float_arguments(&machine, rectangle(&machine, INPUT_RECT));
            assert_eq!(machine.call(0x3c20cc, &[cell]), 1);
            float_arguments(&machine, [-40.5, -41.25, -20.5, -11.25]);
            assert_eq!(machine.call(0x399954, &[source]), 1);
        }
        if self.clean {
            for cell in self.cells(&machine) {
                machine.call(0x3c28e4, &[cell]);
            }
        }
        stages.push(self.snapshot(&machine, "cold_frame_preparation", true));
        let cold_gate = machine.call(BODY + 0xd76e0, &[VIEW, TABLE_OBJECT]);
        assert!(cold_gate <= 1);
        float_arguments(&machine, self.caller_rect);
        machine.call(BODY + 0xd78ec, &[VIEW, TABLE_OBJECT]);
        stages.push(self.snapshot(&machine, "cold_explicit_bridge", true));
        register(machine.engine, 136, u64::from(self.change.to_bits()));
        machine.call(DRAWING_BASE + 0xaff74, &[frames::LAYOUT, 0]);
        if self.rows > 1 {
            register(machine.engine, 136, u64::from(self.change.to_bits()));
            machine.call(DRAWING_BASE + 0xade0c, &[frames::LAYOUT, 1]);
        }
        machine.call(DRAWING_BASE + 0xab168, &[frames::LAYOUT]);
        stages.push(self.snapshot(&machine, "warm_frame_resize", true));
        let warm_gate = machine.call(BODY + 0xd76e0, &[VIEW, TABLE_OBJECT]);
        assert!(warm_gate <= 1);
        float_arguments(&machine, self.caller_rect);
        machine.call(BODY + 0xd78ec, &[VIEW, TABLE_OBJECT]);
        stages.push(self.snapshot(&machine, "warm_explicit_bridge", true));
        format!(
            "{{\"name\":{:?},\"rows\":{},\"columns\":{},\"bounds\":{:?},\"caller_rect\":{:?},\"row_change\":{:?},\"merged\":{},\"initially_clean\":{},\"divergent_source\":{},\"table_base_data_translation\":{},\"cold_size_gate\":{cold_gate},\"warm_size_gate\":{warm_gate},\"stages\":[{}]}}",
            self.name,
            self.rows,
            self.columns,
            self.bounds,
            self.caller_rect,
            self.change,
            self.merged,
            self.clean,
            self.divergent,
            self.raw_table_translation,
            stages.join(",")
        )
    }
}

pub(super) fn capture(paths: Paths) {
    let cases = [
        Case::new("fractional-origin-grow"),
        Case {
            name: "clean-grow",
            clean: true,
            ..Case::new("")
        },
        Case {
            name: "shrink",
            change: -37.25,
            ..Case::new("")
        },
        Case {
            name: "zero-change",
            change: 0.0,
            ..Case::new("")
        },
        Case {
            name: "below-size-epsilon",
            change: 0.0005,
            ..Case::new("")
        },
        Case {
            name: "above-size-epsilon",
            change: 0.002,
            ..Case::new("")
        },
        Case {
            name: "one-cell",
            rows: 1,
            columns: 1,
            ..Case::new("")
        },
        Case {
            name: "one-row",
            rows: 1,
            columns: 3,
            ..Case::new("")
        },
        Case {
            name: "one-column",
            rows: 3,
            columns: 1,
            ..Case::new("")
        },
        Case {
            name: "zero-origin",
            bounds: [0.0, 0.0, 160.0, 200.0],
            caller_rect: [0.0, 0.0, 160.0, 200.0],
            ..Case::new("")
        },
        Case {
            name: "large-origin",
            bounds: [1_000_000.0, -1_000_000.0, 1_000_160.0, -999_800.0],
            caller_rect: [1_000_000.0, -1_000_000.0, 1_000_160.0, -999_800.0],
            ..Case::new("")
        },
        Case {
            name: "translated-caller",
            caller_rect: [123.75, -31.125, 283.75, 168.875],
            ..Case::new("")
        },
        Case {
            name: "translated-table-base-data",
            caller_rect: [123.75, -31.125, 283.75, 168.875],
            raw_table_translation: true,
            ..Case::new("")
        },
        Case {
            name: "merged-owner",
            merged: true,
            ..Case::new("")
        },
        Case {
            name: "clean-merged-owner",
            merged: true,
            clean: true,
            ..Case::new("")
        },
        Case {
            name: "equal-cell-divergent-source",
            divergent: true,
            ..Case::new("")
        },
        Case {
            name: "clean-equal-cell-divergent-source",
            divergent: true,
            clean: true,
            ..Case::new("")
        },
    ];
    let output: Vec<_> = cases
        .into_iter()
        .map(|case| {
            let expected = case.fixture(&paths, 0);
            for fill in [0xa5, 0xff] {
                assert_eq!(
                    case.fixture(&paths, fill),
                    expected,
                    "allocation-fill changed {}",
                    case.name
                );
            }
            expected
        })
        .collect();
    println!(
        concat!(
            "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",",
            "\"model_sha256\":\"{}\",\"base_sha256\":\"{}\",\"drawing_sha256\":\"{}\",\"bodytext_sha256\":\"{}\",\"cpp_sha256\":\"{}\",",
            "\"allocation_fills\":[0,165,255],\"allocation_fill_scope\":\"native heap allocations throughout Model construction, registry, template, effect and container execution; supplied table object, Drawing layout, Bodytext owner and view storage starts zeroed\",",
            "\"native_initializers\":[\"0x2a2ed0\",\"0x2863a0\",\"0x3c35ec\",\"0x3c5918\"],",
            "\"native_functions\":{{\"table_constructor\":\"0x3d2690\",\"table_construct\":\"0x3d27d8\",\"cell_constructor\":\"0x3c1cac\",\"content_constructor\":\"0x3c17f0\",\"table_base_rect_only\":\"0x2d2b18\",\"cold_frames_drawing\":\"0xaa6b4\",\"update_geometry_drawing\":\"0xab168\",\"extend_row_drawing\":\"0xaff74\",\"offset_rows_drawing\":\"0xade0c\",\"size_gate_bodytext\":\"0xd76e0\",\"cell_model_bridge_bodytext\":\"0xd78ec\"}},",
            "\"host_boundaries\":[\"bounded allocation and free\",\"memory and byte string operations\",\"deterministic UUID service\",\"single-thread mutex and C++ guard services\",\"destructor registration\",\"Android logging and Error::SetError\",\"zeroed ObjectTextLayout storage adapter; SetObject and SetTextScale omitted\"],",
            "\"capture_boundary\":\"Complete native Model table/row/cell/content construction and geometry setters execute, with native registry initialization. Native Bodytext size gate and complete bridge execute with real runtime-handle owner lookup and bundled C++ dynamic_cast. Drawing layout storage, native-vtable identity and one-element Bodytext owner association are supplied. Caller rectangle is explicit input; explicit bridge calls execute even when the separately captured size gate is false. Native text shaping, first measurement, complete TableLayout/Bodytext construction, listener callbacks, document ObjectSpan rectangle production, parsing/cloning and final PDF run clipping do not execute. Warm operations resize Drawing frames before an explicit native Model bridge; these are bounded function captures, not full application lifecycle parity.\",",
            "\"cases\":[\n{}\n]}}"
        ),
        LIBRARY_SHA256,
        frames::BASE_SHA256,
        DRAWING_SHA256,
        BODY_SHA256,
        CPP_SHA256,
        output.join(",\n")
    );
}
