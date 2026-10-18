use super::*;
use std::collections::BTreeMap;

const SKIA: u64 = 0x0600_0000;
const BASE: u64 = 0x0400_0000;
const SKIA_SHA256: &str = "42636cb9ac06cc286114b42c1b9d8f4b78d33761843251cde2b443b101ffb88d";
const BASE_SHA256: &str = "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb";
const DRAWING_SHA256: &str = "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd";
const LIBM_SHA256: &str = "6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3";
const HOST: u64 = MODEL + 0x30000;
const PATH_OBJECT: u64 = MODEL + 0x1000;
const RECTANGLE: u64 = MODEL + 0x1100;
const ITERATOR: u64 = MODEL + 0x1200;
const OUTPUT: u64 = MODEL + 0x2000;
const FILLS: [u8; 5] = [0, 85, 165, 255, 0];
const SYMBOLS: [&str; 14] = [
    "_ZdlPv",
    "_Znwm",
    "free",
    "malloc",
    "memcpy",
    "memmove",
    "realloc",
    "sincosf",
    "sinf",
    "cosf",
    "tanf",
    "atan2f",
    "sincos",
    "recursive-mutex",
];

#[link(name = "m")]
unsafe extern "C" {
    fn sincosf(value: f32, sine: *mut f32, cosine: *mut f32);
    fn sinf(value: f32) -> f32;
    fn cosf(value: f32) -> f32;
    fn tanf(value: f32) -> f32;
    fn atan2f(y: f32, x: f32) -> f32;
    fn sincos(value: f64, sine: *mut f64, cosine: *mut f64);
}
#[repr(C)]
struct DynamicSymbol {
    filename: *const std::ffi::c_char,
    base: *mut c_void,
    symbol_name: *const std::ffi::c_char,
    symbol: *mut c_void,
}
#[link(name = "dl")]
unsafe extern "C" {
    fn dladdr(address: *const c_void, information: *mut DynamicSymbol) -> i32;
}
pub(super) fn verify_host_math() {
    for address in [
        sincosf as *const c_void,
        sinf as *const c_void,
        cosf as *const c_void,
        tanf as *const c_void,
        atan2f as *const c_void,
        sincos as *const c_void,
    ] {
        let mut information = DynamicSymbol {
            filename: ptr::null(),
            base: ptr::null_mut(),
            symbol_name: ptr::null(),
            symbol: ptr::null_mut(),
        };
        assert_ne!(unsafe { dladdr(address, &mut information) }, 0);
        assert!(!information.filename.is_null());
        let filename = unsafe { std::ffi::CStr::from_ptr(information.filename) }
            .to_str()
            .unwrap();
        verify_library(Path::new(filename), LIBM_SHA256);
    }
}
fn bytes(engine: Engine, address: u64, length: usize) -> Vec<u8> {
    assert!(length <= 0x100000);
    let mut output = vec![0; length];
    check(unsafe { uc_mem_read(engine, address, output.as_mut_ptr().cast(), length) });
    output
}
fn floats(engine: Engine, address: u64, values: &[f32]) {
    let output: Vec<_> = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    write(engine, address, &output);
}
struct HostState {
    fill: u8,
    cursor: u64,
    allocations: BTreeMap<u64, usize>,
    calls: BTreeMap<&'static str, usize>,
}
impl HostState {
    fn allocate(&mut self, engine: Engine, length: u64) -> u64 {
        assert!(length <= 0x10000, "oversized native allocation {length:#x}");
        let length = length.max(16) as usize;
        let pointer = self.cursor;
        self.cursor += (length as u64 + 15) & !15;
        assert!(self.cursor < TLS);
        write(engine, pointer, &vec![self.fill; length]);
        self.allocations.insert(pointer, length);
        pointer
    }
    fn reset(&mut self, fill: u8) {
        self.fill = fill;
        self.cursor = HEAP;
        self.allocations.clear();
        self.calls.clear();
    }
}
unsafe extern "C" fn imported(engine: Engine, address: u64, _: u32, context: *mut c_void) {
    let state = unsafe { &mut *context.cast::<HostState>() };
    let name = SYMBOLS[((address - HOST) / 16) as usize];
    *state.calls.entry(name).or_default() += 1;
    let first = read_register(engine, REGISTER_X0);
    let second = read_register(engine, REGISTER_X0 + 1);
    let third = read_register(engine, REGISTER_X0 + 2);
    match name {
        "recursive-mutex" => register(engine, REGISTER_X0, 0),
        "_Znwm" | "malloc" => register(engine, REGISTER_X0, state.allocate(engine, first)),
        "realloc" => {
            let pointer = state.allocate(engine, second);
            if first != 0 {
                let previous = state
                    .allocations
                    .remove(&first)
                    .expect("unknown allocation");
                write(
                    engine,
                    pointer,
                    &bytes(engine, first, previous.min(second as usize)),
                );
            }
            register(engine, REGISTER_X0, pointer);
        }
        "free" | "_ZdlPv" => {
            assert!(first == 0 || state.allocations.remove(&first).is_some());
        }
        "memcpy" | "memmove" => {
            assert!(third <= 0x10000);
            write(engine, first, &bytes(engine, second, third as usize));
            register(engine, REGISTER_X0, first);
        }
        "sincosf" => {
            let value = f32::from_bits(read_register(engine, 136) as u32);
            let (mut sine, mut cosine) = (0., 0.);
            unsafe { sincosf(value, &mut sine, &mut cosine) };
            write(engine, first, &sine.to_le_bytes());
            write(engine, second, &cosine.to_le_bytes());
        }
        "sincos" => {
            let value = f64::from_bits(read_register(engine, 40));
            let (mut sine, mut cosine) = (0., 0.);
            unsafe { sincos(value, &mut sine, &mut cosine) };
            write(engine, first, &sine.to_le_bytes());
            write(engine, second, &cosine.to_le_bytes());
        }
        "sinf" | "cosf" | "tanf" | "atan2f" => {
            let value = f32::from_bits(read_register(engine, 136) as u32);
            let other = f32::from_bits(read_register(engine, 137) as u32);
            let result = unsafe {
                match name {
                    "sinf" => sinf(value),
                    "cosf" => cosf(value),
                    "tanf" => tanf(value),
                    "atan2f" => atan2f(value, other),
                    _ => unreachable!(),
                }
            };
            register(engine, 136, u64::from(result.to_bits()));
        }
        _ => unreachable!(),
    }
}
fn reject_unbound_imports(engine: Engine, path: &Path, base: u64) {
    const UNBOUND: u64 = 0xdead_0000;
    let mut probe = 0_u8;
    assert_ne!(
        unsafe { uc_mem_read(engine, UNBOUND, ptr::from_mut(&mut probe).cast(), 1) },
        0
    );
    let binary = fs::read(path).unwrap();
    let section_offset = u64::from_le_bytes(binary[40..48].try_into().unwrap()) as usize;
    let section_bytes = usize::from(u16::from_le_bytes(binary[58..60].try_into().unwrap()));
    let section_count = usize::from(u16::from_le_bytes(binary[60..62].try_into().unwrap()));
    let sections: Vec<_> = (0..section_count)
        .map(|index| &binary[section_offset + index * section_bytes..][..section_bytes])
        .collect();
    for section in &sections {
        if u32::from_le_bytes(section[4..8].try_into().unwrap()) != 4 {
            continue;
        }
        let offset = u64::from_le_bytes(section[24..32].try_into().unwrap()) as usize;
        let length = u64::from_le_bytes(section[32..40].try_into().unwrap()) as usize;
        let symbol_section = u32::from_le_bytes(section[40..44].try_into().unwrap()) as usize;
        let symbols = sections[symbol_section];
        let symbol_offset = u64::from_le_bytes(symbols[24..32].try_into().unwrap()) as usize;
        assert_eq!(u64::from_le_bytes(section[56..64].try_into().unwrap()), 24);
        assert_eq!(u64::from_le_bytes(symbols[56..64].try_into().unwrap()), 24);
        for relocation in binary[offset..offset + length].chunks_exact(24) {
            let information = u64::from_le_bytes(relocation[8..16].try_into().unwrap());
            if !matches!(information as u32, 257 | 1025 | 1026) {
                continue;
            }
            let symbol = &binary[symbol_offset + (information >> 32) as usize * 24..][..24];
            if u16::from_le_bytes(symbol[6..8].try_into().unwrap()) != 0 {
                continue;
            }
            let address = u64::from_le_bytes(relocation[..8].try_into().unwrap());
            write(engine, base + address, &UNBOUND.to_le_bytes());
        }
    }
}
fn bind_imports(engine: Engine) {
    for (library, bindings) in [
        (
            SKIA,
            &[
                (0x29e848, "_ZdlPv"),
                (0x29e878, "_Znwm"),
                (0x29f958, "malloc"),
                (0x2a0358, "realloc"),
                (0x2a0360, "free"),
                (0x2a0828, "memcpy"),
                (0x2a0860, "memmove"),
                (0x2a1280, "sincosf"),
            ][..],
        ),
        (
            0,
            &[
                (0x4a4590, "atan2f"),
                (0x4ac950, "memcpy"),
                (0x4ac960, "cosf"),
                (0x4ac968, "sinf"),
                (0x4ac970, "sincosf"),
                (0x4ac978, "tanf"),
                (0x4a4850, "recursive-mutex"),
                (0x4a4858, "recursive-mutex"),
                (0x4a4ac0, "recursive-mutex"),
                (0x4a4ac8, "recursive-mutex"),
            ][..],
        ),
        (BASE, &[(0xf05c0, "sincos")][..]),
    ] {
        for &(offset, name) in bindings {
            let index = SYMBOLS.iter().position(|symbol| *symbol == name).unwrap();
            write(
                engine,
                library + offset,
                &(HOST + index as u64 * 16).to_le_bytes(),
            );
        }
    }
    write(engine, 0x4a6720, &(BASE + 0xb0f10).to_le_bytes());
}
struct Hooks {
    engine: Engine,
    handles: Vec<usize>,
}
impl Hooks {
    fn new(engine: Engine, state: &mut HostState) -> Self {
        let mut handles = Vec::new();
        for index in 0..SYMBOLS.len() {
            let address = HOST + index as u64 * 16;
            write(engine, address, &0xd65f03c0_u32.to_le_bytes());
            let mut handle = 0;
            check(unsafe {
                uc_hook_add(
                    engine,
                    &mut handle,
                    4,
                    imported as *mut c_void,
                    ptr::from_mut(state).cast(),
                    address,
                    address,
                )
            });
            handles.push(handle);
        }
        Self { engine, handles }
    }
}
impl Drop for Hooks {
    fn drop(&mut self) {
        for handle in &self.handles {
            check(unsafe { uc_hook_del(self.engine, *handle) });
        }
    }
}
#[derive(Clone, Copy)]
enum Operation {
    Oval(u64),
    Arc(f32, f32, bool),
}
struct PathInput {
    name: &'static str,
    rectangle: [f32; 4],
    operation: Operation,
    seeded: bool,
}
impl PathInput {
    const fn oval(name: &'static str, rectangle: [f32; 4], direction: u64) -> Self {
        Self {
            name,
            rectangle,
            operation: Operation::Oval(direction),
            seeded: false,
        }
    }
    const fn arc(
        name: &'static str,
        rectangle: [f32; 4],
        start: f32,
        sweep: f32,
        force: bool,
    ) -> Self {
        Self {
            name,
            rectangle,
            operation: Operation::Arc(start, sweep, force),
            seeded: false,
        }
    }
    const fn seeded(mut self) -> Self {
        self.seeded = true;
        self
    }
}
fn path_output(machine: &Machine, fill: u8, input: &PathInput) -> String {
    let engine = machine.engine;
    write(engine, PATH_OBJECT, &[fill; 64]);
    write(engine, ITERATOR, &[fill; 64]);
    floats(engine, RECTANGLE, &input.rectangle);
    machine.call(SKIA + 0x1d383c, &[PATH_OBJECT]);
    if input.seeded {
        for (function, point) in [(0x1d4e90, [-5f32, -6.]), (0x1d5018, [-7f32, -8.])] {
            register(engine, 136, u64::from(point[0].to_bits()));
            register(engine, 137, u64::from(point[1].to_bits()));
            machine.call(SKIA + function, &[PATH_OBJECT]);
        }
    }
    let (operation, angles, flag) = match input.operation {
        Operation::Oval(direction) => {
            machine.call(SKIA + 0x1d6204, &[PATH_OBJECT, RECTANGLE, direction]);
            ("add_oval", vec![], direction)
        }
        Operation::Arc(start, sweep, force) => {
            register(engine, 136, u64::from(start.to_bits()));
            register(engine, 137, u64::from(sweep.to_bits()));
            machine.call(SKIA + 0x1d6974, &[PATH_OBJECT, RECTANGLE, u64::from(force)]);
            (
                "arc_to",
                vec![start.to_bits(), sweep.to_bits()],
                u64::from(force),
            )
        }
    };
    let point_count = machine.call(SKIA + 0x1d4c20, &[PATH_OBJECT]);
    let verb_count = machine.call(SKIA + 0x1d4c90, &[PATH_OBJECT]);
    machine.call(SKIA + 0x1d855c, &[ITERATOR]);
    machine.call(SKIA + 0x1d858c, &[ITERATOR, PATH_OBJECT]);
    let mut commands = Vec::new();
    let mut done = false;
    for _ in 0..40 {
        write(engine, OUTPUT, &[fill; 128]);
        let verb = machine.call(SKIA + 0x1d41ac, &[ITERATOR, OUTPUT]);
        let points = match verb {
            0 => 1,
            1 => 2,
            2 | 3 => 3,
            4 => 4,
            5 | 6 => 0,
            _ => panic!("unknown native verb {verb}"),
        };
        let point_bits: Vec<_> = (0..points * 2)
            .map(|index| read_u32(engine, OUTPUT + index * 4))
            .collect();
        assert_eq!(bytes(engine, OUTPUT + 32, 96), vec![fill; 96]);
        commands.push(format!("{{\"verb\":{verb},\"point_bits\":{point_bits:?}}}"));
        if verb == 6 {
            done = true;
            break;
        }
    }
    assert!(done);
    machine.call(SKIA + 0x1d3988, &[PATH_OBJECT]);
    format!(
        "{{\"name\":{:?},\"seeded_with_move_line\":{},\"operation\":{operation:?},\"rect_bits\":{:?},\"angle_bits\":{angles:?},\"direction_or_force_move\":{flag},\"point_count\":{point_count},\"verb_count\":{verb_count},\"commands\":[{}]}}",
        input.name,
        input.seeded,
        input.rectangle.map(f32::to_bits),
        commands.join(",")
    )
}
fn point_output(machine: &Machine, fill: u8, function: u64, arguments: &[u64]) -> (u64, Vec<u32>) {
    write(machine.engine, OUTPUT, &[fill; 256]);
    let count = machine.call(function, arguments);
    assert!(count <= 17);
    assert_eq!(
        bytes(
            machine.engine,
            OUTPUT + count * 8,
            (256 - count * 8) as usize
        ),
        vec![fill; (256 - count * 8) as usize]
    );
    (
        count,
        (0..count * 2)
            .map(|index| read_u32(machine.engine, OUTPUT + index * 4))
            .collect(),
    )
}
fn quad_output(
    machine: &Machine,
    fill: u8,
    name: &str,
    start: [f32; 2],
    end: [f32; 2],
    direction: u64,
    matrix: Option<[f32; 9]>,
) -> String {
    floats(machine.engine, PATH_OBJECT, &start);
    floats(machine.engine, RECTANGLE, &end);
    let matrix_pointer = if let Some(values) = matrix {
        floats(machine.engine, ITERATOR, &values);
        write(machine.engine, ITERATOR + 36, &192_u32.to_le_bytes());
        ITERATOR
    } else {
        0
    };
    let (count, bits) = point_output(
        machine,
        fill,
        SKIA + 0x1bb294,
        &[PATH_OBJECT, RECTANGLE, direction, matrix_pointer, OUTPUT],
    );
    format!(
        "{{\"name\":{name:?},\"start_bits\":{:?},\"end_bits\":{:?},\"direction\":{direction},\"matrix_bits\":{:?},\"point_count\":{count},\"point_bits\":{bits:?}}}",
        start.map(f32::to_bits),
        end.map(f32::to_bits),
        matrix
            .map(|values| values.map(f32::to_bits).to_vec())
            .unwrap_or_default()
    )
}
fn outputs(machine: &Machine, fill: u8) -> String {
    let r = [10., 20., 210., 120.];
    let inputs = [
        PathInput::oval("oval-clockwise", r, 1),
        PathInput::oval("oval-counterclockwise", r, 2),
        PathInput::oval("oval-direction-zero", r, 0),
        PathInput::oval("oval-zero-width", [10., 20., 10., 120.], 1),
        PathInput::oval("oval-zero-height", [10., 20., 210., 20.], 2),
        PathInput::oval("oval-seeded-clockwise", r, 1).seeded(),
        PathInput::oval("oval-seeded-counterclockwise", r, 2).seeded(),
        PathInput::oval("oval-point", [10., 20., 10., 20.], 1),
        PathInput::oval("oval-inverted", [210., 120., 10., 20.], 2),
        PathInput::oval("oval-fractional", [-10.125, 20.375, 210.0625, 120.5], 1),
        PathInput::arc("arc-quarter-clockwise", r, 0., 90., true),
        PathInput::arc("arc-quarter-counterclockwise", r, 0., -90., true),
        PathInput::arc("arc-oblique", r, 37., 123., true),
        PathInput::arc("arc-large-sweep", r, -90., 270., false),
        PathInput::arc("arc-nearly-full", r, 0., 359.99, true),
        PathInput::arc("arc-full-counterclockwise", r, 0., -360., true),
        PathInput::arc("arc-two-full", r, 0., 720., true),
        PathInput::arc("arc-two-full-counterclockwise", r, 0., -720., true),
        PathInput::arc("arc-closer-to-full", r, 0., 359.999, true),
        PathInput::arc("arc-oblique-full", r, 37., 360., true),
        PathInput::arc("arc-oblique-two-full", r, 37., 720., true),
        PathInput::arc("arc-oblique-full-counterclockwise", r, 1., -360., true),
        PathInput::arc(
            "arc-negative-start-full-counterclockwise",
            r,
            -37.,
            -360.,
            true,
        ),
        PathInput::arc("arc-full", r, 0., 360., true),
        PathInput::arc("arc-zero", r, 0., 0., true),
        PathInput::arc("arc-zero-oblique", r, 37., 0., false),
        PathInput::arc("arc-inverted", [210., 120., 10., 20.], 37., 123., true),
        PathInput::arc("arc-seeded-connect", r, 37., 123., false).seeded(),
        PathInput::arc("arc-seeded-force-move", r, 37., 123., true).seeded(),
        PathInput::arc("arc-seeded-zero", r, 0., 0., false).seeded(),
        PathInput::arc("arc-point", [10., 20., 10., 20.], 37., 123., true),
    ];
    let paths: Vec<_> = inputs
        .iter()
        .map(|input| path_output(machine, fill, input))
        .collect();
    let d = std::f32::consts::FRAC_1_SQRT_2;
    let quadratics = [
        quad_output(
            machine,
            fill,
            "quarter-clockwise",
            [1., 0.],
            [0., 1.],
            0,
            None,
        ),
        quad_output(
            machine,
            fill,
            "quarter-counterclockwise",
            [1., 0.],
            [0., -1.],
            1,
            None,
        ),
        quad_output(machine, fill, "eighth", [1., 0.], [d, d], 0, None),
        quad_output(machine, fill, "half", [1., 0.], [-1., 0.], 0, None),
        quad_output(machine, fill, "same-vector", [1., 0.], [1., 0.], 0, None),
        quad_output(
            machine,
            fill,
            "quarter-transformed",
            [1., 0.],
            [0., 1.],
            0,
            Some([100., 0., 110., 0., 50., 70., 0., 0., 1.]),
        ),
    ];
    let model_inputs = [
        ("ellipse-quarter", [10., 20., 210., 120., 0., 90.]),
        ("ellipse-oblique", [10., 20., 210., 120., 37., 123.]),
        (
            "ellipse-oblique-negative",
            [10., 20., 210., 120., 37., -123.],
        ),
        ("ellipse-full", [10., 20., 210., 120., 0., 360.]),
        ("ellipse-oblique-full", [10., 20., 210., 120., 37., 360.]),
        ("ellipse-zero", [10., 20., 210., 120., 0., 0.]),
        ("ellipse-zero-oblique", [10., 20., 210., 120., 37., 0.]),
        ("circle-quarter", [10., 20., 110., 120., 0., 90.]),
        ("circle-oblique", [10., 20., 110., 120., 37., 123.]),
        ("circle-full", [10., 20., 110., 120., 0., 360.]),
        ("point", [10., 20., 10., 20., 37., 123.]),
        ("vertical-line", [10., 20., 10., 120., 37., 123.]),
        ("horizontal-line", [10., 20., 210., 20., 37., 123.]),
    ];
    let model: Vec<_> = model_inputs
        .into_iter()
        .map(|(name, values): (&str, [f32; 6])| {
            for (index, value) in values.iter().enumerate() {
                register(machine.engine, 136 + index as i32, u64::from(value.to_bits()));
            }
            let (count, bits) = point_output(machine, fill, 0x211814, &[OUTPUT]);
            format!(
                "{{\"name\":{name:?},\"input_bits\":{:?},\"point_count\":{count},\"point_bits\":{bits:?}}}",
                values.map(f32::to_bits)
            )
        })
        .collect();
    format!(
        "{{\"paths\":[\n{}\n],\"direct_quad_arcs\":[\n{}\n],\"model_arc_helpers\":[\n{}\n]}}",
        paths.join(",\n"),
        quadratics.join(",\n"),
        model.join(",\n")
    )
}
pub(super) fn capture(machine: &mut Machine, model: &Path, base: &Path, skia: &Path) {
    verify_host_math();
    verify_library(model, LIBRARY_SHA256);
    map_library(machine.engine, base, BASE, BASE_SHA256);
    map_library(machine.engine, skia, SKIA, SKIA_SHA256);
    for (path, address) in [(model, 0), (base, BASE), (skia, SKIA)] {
        reject_unbound_imports(machine.engine, path, address);
    }
    bind_imports(machine.engine);
    const WRITABLE_SEGMENTS: [(u64, usize); 6] = [
        (0x48f570, 0x1da90),
        (0x4b0a18, 0x8bd8),
        (BASE + 0xebd30, 0x52d0),
        (BASE + 0xf4630, 0x4178),
        (SKIA + 0x290060, 0x11fa0),
        (SKIA + 0x2a52f8, 0x73a0),
    ];
    let snapshots: Vec<_> = WRITABLE_SEGMENTS
        .into_iter()
        .map(|(address, length)| (address, bytes(machine.engine, address, length)))
        .collect();
    let mut state = Box::new(HostState {
        fill: 0,
        cursor: HEAP,
        allocations: BTreeMap::new(),
        calls: BTreeMap::new(),
    });
    let _hooks = Hooks::new(machine.engine, state.as_mut());
    let mut canonical = None;
    let mut imports = None;
    for fill in FILLS {
        for (address, content) in &snapshots {
            write(machine.engine, *address, content);
            assert_eq!(bytes(machine.engine, *address, content.len()), *content);
        }
        write(machine.engine, MODEL, &vec![fill; 0x30000]);
        write(
            machine.engine,
            HEAP,
            &vec![fill; (MODEL + 0x100000 - HEAP) as usize],
        );
        machine.heap.cursor = HEAP;
        machine.heap.allocation_fill = fill;
        machine.heap.allocations = 0;
        machine.heap.fills = 0;
        machine.heap.deletes = 0;
        state.reset(fill);
        let output = outputs(machine, fill);
        assert_eq!(
            machine.heap.allocations, 0,
            "root allocator unexpectedly reached"
        );
        assert_eq!(
            machine.heap.fills, 0,
            "root fill import unexpectedly reached"
        );
        assert_eq!(
            machine.heap.deletes, 0,
            "root delete import unexpectedly reached"
        );
        if let Some(previous) = &canonical {
            assert_eq!(&output, previous, "native output changed with fill {fill}");
        } else {
            canonical = Some(output);
        }
        if let Some(previous) = &imports {
            assert_eq!(
                &state.calls, previous,
                "native imports changed with fill {fill}"
            );
        } else {
            imports = Some(state.calls.clone());
        }
    }
    let calls = state
        .calls
        .iter()
        .map(|(name, count)| format!("{name:?}:{count}"))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"skia_library_sha256\":\"{SKIA_SHA256}\",\"host_libm_sha256\":\"{LIBM_SHA256}\",\"allocation_fills\":{FILLS:?},\"capture_boundary\":\"Unchanged native SkPath construction, addOval/arcTo, seeded move/line, count/RawIter/destruction, direct SkBuildQuadArc, complete Model arc numeric helper0x211814/internal quadratics/roots and Base PointF rotation0xb0f10 execute on supplied scalar inputs. Native writable segments, caller storage, heap, stack, TLS and allocator state reset between five fills including repeated zero. Allocations and memory copies are bounded host imports; every used Linux libm function is runtime SHA-verified via dladdr. Unbound imported-symbol relocations point to an asserted unmapped guest address; only explicit native and host bindings are admitted. Android libm transcendental bit exactness is not certified. Oval and explicit-vector SkBuildQuadArc cases do not use host transcendental imports. Model helper rectangle is supplied left/top/right/bottom, not serialized Arc fields or route-specific normalizer operands. No common-loader normalization, named Arc/fill/template dispatch, Drawing shape producer, serialization, paint, SDK geometry, SVG/PDF, GPU, rendering or pixels execute.\",\"native_addresses\":{{\"path_construct\":\"Skia:0x1d383c\",\"oval\":\"Skia:0x1d6204\",\"arc_to\":\"Skia:0x1d6974\",\"quad_arc\":\"Skia:0x1bb294\",\"raw_iterator\":\"Skia:0x1d41ac\",\"model_arc_helper\":\"Model:0x211814\",\"model_quadratic_helper\":\"Model:0x211418\",\"point_rotation\":\"Base:0xb0f10\"}},\"reached_host_imports\":{{{calls}}},\"capture\":{}}}",
        canonical.unwrap()
    );
}

/// Capture Model's unit-quadratic builder directly, before the 0x211814
/// outer mapping/rotation pass. Each case supplies the exact ten floating
/// arguments observed at the native call boundary.
pub(super) fn capture_model_quad_arcs(model: &Path, base: &Path, skia: &Path) {
    const OUTPUT: u64 = MODEL + 0x2000;
    let machine = Machine::new(model);
    verify_host_math();
    verify_library(model, LIBRARY_SHA256);
    map_library(machine.engine, base, BASE, BASE_SHA256);
    map_library(machine.engine, skia, SKIA, SKIA_SHA256);
    for (path, address) in [(model, 0), (base, BASE), (skia, SKIA)] {
        reject_unbound_imports(machine.engine, path, address);
    }
    bind_imports(machine.engine);
    for (got, target) in [
        (0x4a4560, HOST),
        (0x4a4578, HOST + 16),
        (0x4a4580, HOST + 16),
        (0x4a4588, HOST),
        (0x4a4850, HOST + 208),
        (0x4a4858, HOST + 208),
        (0x4a4ac0, HOST + 208),
        (0x4a4ac8, HOST + 208),
    ] {
        write(machine.engine, got, &target.to_le_bytes());
    }
    for (plt, target) in [
        (0x47f240, 0x2ef320),
        (0x47f250, 0x2ef414),
        (0x47f270, 0x2ef584),
        (0x47f280, 0x2ef5d8),
        (0x47f290, 0x2ef630),
        (0x47f3a0, 0x2ef2d8),
        (0x47f3b0, 0x2ef2e0),
        (0x47f3c0, 0x2f1730),
    ] {
        bind_native(machine.engine, plt, target);
    }
    write(machine.engine, 0x4a6720, &(BASE + 0xb0f10).to_le_bytes());
    let mut state = Box::new(HostState {
        fill: 0,
        cursor: HEAP,
        allocations: BTreeMap::new(),
        calls: BTreeMap::new(),
    });
    let _hooks = Hooks::new(machine.engine, state.as_mut());

    let cases: [(&str, [u32; 10]); 16] = [
        ("ellipse-quarter", [0x41200000,0x41a00000,0x43520000,0x42f00000,0x3f800000,0,0,0x3f800000,0,0x42b40000]),
        ("ellipse-oblique", [0x41200000,0x41a00000,0x43520000,0x42f00000,0x3f0d89e1,0x3f55505d,0xbf4ef87d,0x3f16a991,0x4261bd44,0x42af068a]),
        ("ellipse-oblique-negative", [0x41200000,0x41a00000,0x43520000,0x42f00000,0x3f0d89e1,0x3f55505d,0x3d0f1f5f,0xbf7fd7fb,0x4261bd44,0xc3106eb1]),
        ("ellipse-full", [0x41200000,0x41a00000,0x43520000,0x42f00000,0x3f800000,0,0x3f7fffe6,0xbae4ca1a,0,0x43b3f333]),
        ("ellipse-full-negative", [0x41200000,0x41a00000,0x43520000,0x42f00000,0x3f800000,0,0x3f7fffe6,0x3ae4ca1a,0,0xc3b3f333]),
        ("ellipse-oblique-full", [0x41200000,0x41a00000,0x43520000,0x42f00000,0x3f0d89e1,0x3f55505d,0x3f0de920,0x3f55110b,0x4261bd44,0x43b3f333]),
        ("ellipse-zero-oblique", [0x41200000,0x41a00000,0x43520000,0x42f00000,0x3f0d89e1,0x3f55505d,0x3f0d89e1,0x3f55505d,0x4261bd44,0]),
        ("circle-quarter", [0x41200000,0x41a00000,0x42dc0000,0x42f00000,0x3f800000,0,0,0x3f800000,0,0x42b40000]),
        ("circle-oblique", [0x41200000,0x41a00000,0x42dc0000,0x42f00000,0x3f4c7361,0x3f1a108c,0xbf708fb2,0x3eaf1d46,0x42140000,0x42f60000]),
        ("circle-full", [0x41200000,0x41a00000,0x42dc0000,0x42f00000,0x3f800000,0,0x3f800000,0,0,0x43b40000]),
        ("vertical-line", [0x41200000,0x41a00000,0x41200000,0x42f00000,0x3f4c7361,0x3f1a108c,0xbf708fb2,0x3eaf1d46,0x42140000,0x42f60000]),
        ("horizontal-line", [0x41200000,0x41a00000,0x43520000,0x41a00000,0x3f4c7361,0x3f1a108c,0xbf708fb2,0x3eaf1d46,0x42140000,0x42f60000]),
        ("tiny-negative-cross-positive-sweep", [0x41200000,0x41a00000,0x43520000,0x42f00000,0x3f800000,0,0x3f800000,0xb8d1b717,0,0x43b3f333]),
        ("tiny-negative-cross-negative-sweep", [0x41200000,0x41a00000,0x43520000,0x42f00000,0x3f800000,0,0x3f800000,0xb8d1b717,0,0xc3b3f333]),
        ("tiny-positive-cross-positive-sweep", [0x41200000,0x41a00000,0x43520000,0x42f00000,0x3f800000,0,0x3f800000,0x38d1b717,0,0x43b3f333]),
        ("tiny-positive-cross-negative-sweep", [0x41200000,0x41a00000,0x43520000,0x42f00000,0x3f800000,0,0x3f800000,0x38d1b717,0,0xc3b3f333]),
    ];
    let mut outputs = Vec::new();
    for (name, input_bits) in cases {
        for (index, bits) in input_bits[..8].iter().enumerate() {
            register(machine.engine, 136 + index as i32, u64::from(*bits));
        }
        write(machine.engine, STACK, &input_bits[8].to_le_bytes());
        write(machine.engine, STACK + 8, &input_bits[9].to_le_bytes());
        write(machine.engine, OUTPUT, &[0; 256]);
        let point_count = machine.call(0x211418, &[OUTPUT]);
        assert!((1..=17).contains(&point_count), "unexpected native point count for {name}");
        let point_bits: Vec<_> = (0..point_count * 2)
            .map(|index| read_u32(machine.engine, OUTPUT + index * 4))
            .collect();
        outputs.push(format!(
            "{{\"name\":{name:?},\"input_bits\":{input_bits:?},\"point_count\":{point_count},\"point_bits\":{point_bits:?}}}"
        ));
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"host_libm_sha256\":\"{LIBM_SHA256}\",\"boundary\":\"Direct Model:0x211418 entry with observed s0-s7 and stack+0/+8 arguments; returns its pre-0x211814 point buffer.\",\"cases\":[{}]}}",
        outputs.join(",")
    );
}

#[derive(Clone, Copy)]
struct DrawingArcCase {
    name: &'static str,
    rectangle: [f64; 4],
    start: f64,
    sweep: f64,
    seeded: bool,
}
fn capture_drawing_arc_case(
    model: &Path,
    drawing: &Path,
    base: &Path,
    skia: &Path,
    case: DrawingArcCase,
) -> String {
    let machine = Machine::new(model);
    verify_host_math();
    map_library(machine.engine, drawing, DRAWING_BASE, DRAWING_SHA256);
    map_library(machine.engine, base, BASE, BASE_SHA256);
    map_library(machine.engine, skia, SKIA, SKIA_SHA256);
    for (path, address) in [
        (model, 0),
        (drawing, DRAWING_BASE),
        (base, BASE),
        (skia, SKIA),
    ] {
        reject_unbound_imports(machine.engine, path, address);
    }
    bind_imports(machine.engine);
    for (plt, target) in [
        (DRAWING_BASE + 0xbab60, SKIA + 0x1d4e90),
        (DRAWING_BASE + 0xbab70, SKIA + 0x1d5018),
        (DRAWING_BASE + 0xbc430, SKIA + 0x1d51e0),
        (DRAWING_BASE + 0xbc440, SKIA + 0x1d5698),
        (DRAWING_BASE + 0xbc450, SKIA + 0x1d6974),
        (DRAWING_BASE + 0xbc460, SKIA + 0x1d58b8),
        (DRAWING_BASE + 0xbc470, SKIA + 0x1d6204),
    ] {
        bind_native(machine.engine, plt, target);
    }
    let mut state = Box::new(HostState {
        fill: 0,
        cursor: HEAP,
        allocations: BTreeMap::new(),
        calls: BTreeMap::new(),
    });
    let _hooks = Hooks::new(machine.engine, state.as_mut());
    const SEGMENTS: u64 = MODEL + 0x3000;
    let narrow = |value: f64| value as f32;
    let mut records = Vec::<[u32; 7]>::new();
    if case.seeded {
        records.push([1, (-5.0f32).to_bits(), (-6.0f32).to_bits(), 0, 0, 0, 0]);
        records.push([2, (-7.0f32).to_bits(), (-8.0f32).to_bits(), 0, 0, 0, 0]);
    }
    records.push([
        5,
        narrow(case.rectangle[0]).to_bits(),
        narrow(case.rectangle[1]).to_bits(),
        narrow(case.rectangle[2]).to_bits(),
        narrow(case.rectangle[3]).to_bits(),
        narrow(case.start).to_bits(),
        narrow(case.sweep).to_bits(),
    ]);
    let path_object = PATH_OBJECT;
    write(machine.engine, path_object, &[0; 64]);
    write(machine.engine, ITERATOR, &[0; 64]);
    machine.call(SKIA + 0x1d383c, &[path_object]);
    for (index, record) in records.iter().enumerate() {
        let bytes: Vec<u8> = record.iter().flat_map(|v| v.to_le_bytes()).collect();
        write(machine.engine, SEGMENTS + index as u64 * 28, &bytes);
        machine.call(
            DRAWING_BASE + 0x9bd24,
            &[SEGMENTS, index as u64, path_object, RECTANGLE],
        );
    }
    let point_count = machine.call(SKIA + 0x1d4c20, &[path_object]);
    let verb_count = machine.call(SKIA + 0x1d4c90, &[path_object]);
    machine.call(SKIA + 0x1d855c, &[ITERATOR]);
    machine.call(SKIA + 0x1d858c, &[ITERATOR, path_object]);
    let mut commands = Vec::new();
    let mut ended = false;
    for _ in 0..40 {
        write(machine.engine, OUTPUT, &[0; 128]);
        let verb = machine.call(SKIA + 0x1d41ac, &[ITERATOR, OUTPUT]);
        let points = match verb {
            0 => 1,
            1 => 2,
            2 | 3 => 3,
            4 => 4,
            5 => 0,
            6 => {
                ended = true;
                0
            }
            _ => panic!("unknown SkPath verb {verb}"),
        };
        let point_bits: Vec<_> = (0..points * 2)
            .map(|i| read_u32(machine.engine, OUTPUT + i * 4))
            .collect();
        commands.push(format!("{{\"verb\":{verb},\"point_bits\":{point_bits:?}}}"));
        if ended {
            break;
        }
    }
    assert!(ended, "native RawIter did not terminate for {}", case.name);
    machine.call(SKIA + 0x1d3988, &[path_object]);
    let arguments = [
        case.rectangle[0],
        case.rectangle[1],
        case.rectangle[2],
        case.rectangle[3],
        case.start,
        case.sweep,
    ];
    let narrowed = arguments.map(narrow);
    let trig_calls = state.calls.get("sincosf").copied().unwrap_or(0);
    let roots = if case.name == "positive-quarter" {
        format!(
            ",\"native_quad_root_probes\":[{}]",
            native_quad_root_probes(&machine).join(",")
        )
    } else {
        String::new()
    };
    format!(
        "{{\"name\":{:?},\"seeded\":{},\"input_f64_bits\":{:?},\"input_f32_bits\":{:?},\"point_count\":{point_count},\"verb_count\":{verb_count},\"host_sincosf_calls\":{trig_calls},\"commands\":[{}]{roots}}}",
        case.name,
        case.seeded,
        arguments.map(f64::to_bits),
        narrowed.map(f32::to_bits),
        commands.join(",")
    )
}
fn native_quad_root_probes(machine: &Machine) -> Vec<String> {
    let probes = [
        ("fma-rounding", [0x3ede_923c, 0xbf3b_3c44, 0x3e8b_952c]),
        (
            "double-root",
            [1.0f32.to_bits(), (-1.0f32).to_bits(), 0.25f32.to_bits()],
        ),
        (
            "no-real-roots",
            [1.0f32.to_bits(), 0.0f32.to_bits(), 1.0f32.to_bits()],
        ),
        (
            "linear-interior",
            [0.0f32.to_bits(), 1.0f32.to_bits(), (-0.25f32).to_bits()],
        ),
        (
            "linear-below-one",
            [
                0.0f32.to_bits(),
                1.0f32.to_bits(),
                (-f32::from_bits(0x3f7f_ffff)).to_bits(),
            ],
        ),
        (
            "linear-at-one",
            [0.0f32.to_bits(), 1.0f32.to_bits(), (-1.0f32).to_bits()],
        ),
    ];
    probes.into_iter().map(|(name, bits)| {
        for (index, value) in bits.iter().enumerate() {
            register(machine.engine, 136 + index as i32, u64::from(*value));
        }
        write(machine.engine, OUTPUT, &[0; 16]);
        let count = machine.call(SKIA + 0x1b9c6c, &[OUTPUT]);
        assert!(count <= 2);
        let roots: Vec<_> = (0..count).map(|index| read_u32(machine.engine, OUTPUT + index * 4)).collect();
        format!("{{\"name\":{name:?},\"input_bits\":{bits:?},\"count\":{count},\"root_bits\":{roots:?}}}")
    }).collect()
}
pub(super) fn capture_drawing(model: &Path, drawing: &Path, base: &Path, skia: &Path) {
    let r = [10., 20., 210., 120.];
    let cases = [
        DrawingArcCase {
            name: "positive-quarter",
            rectangle: r,
            start: 0.,
            sweep: 90.,
            seeded: false,
        },
        DrawingArcCase {
            name: "negative-quarter",
            rectangle: r,
            start: 0.,
            sweep: -90.,
            seeded: false,
        },
        DrawingArcCase {
            name: "oblique-positive",
            rectangle: r,
            start: 37.,
            sweep: 123.,
            seeded: true,
        },
        DrawingArcCase {
            name: "oblique-negative",
            rectangle: r,
            start: -37.,
            sweep: -123.,
            seeded: false,
        },
        DrawingArcCase {
            name: "start-wrap-negative",
            rectangle: r,
            start: -450.,
            sweep: 30.,
            seeded: false,
        },
        DrawingArcCase {
            name: "start-wrap-positive",
            rectangle: r,
            start: 720.,
            sweep: -30.,
            seeded: false,
        },
        DrawingArcCase {
            name: "fractional-start-small",
            rectangle: r,
            start: 17.25,
            sweep: 0.1,
            seeded: false,
        },
        DrawingArcCase {
            name: "small-sweep-002",
            rectangle: r,
            start: 17.,
            sweep: 0.02,
            seeded: false,
        },
        DrawingArcCase {
            name: "small-sweep-001",
            rectangle: r,
            start: 17.,
            sweep: 0.01,
            seeded: false,
        },
        DrawingArcCase {
            name: "snap-boundary-low",
            rectangle: r,
            start: 0.,
            sweep: 0.01398,
            seeded: false,
        },
        DrawingArcCase {
            name: "snap-boundary-high",
            rectangle: r,
            start: 0.,
            sweep: 0.01399,
            seeded: false,
        },
        DrawingArcCase {
            name: "octant-boundary-below",
            rectangle: r,
            start: 0.,
            sweep: f64::from(f32::from_bits(0x4233_ffff)),
            seeded: false,
        },
        DrawingArcCase {
            name: "octant-boundary-exact",
            rectangle: r,
            start: 0.,
            sweep: 45.,
            seeded: false,
        },
        DrawingArcCase {
            name: "octant-boundary-above",
            rectangle: r,
            start: 0.,
            sweep: f64::from(f32::from_bits(0x4234_0001)),
            seeded: false,
        },
        DrawingArcCase {
            name: "near-full-359",
            rectangle: r,
            start: 0.,
            sweep: 359.,
            seeded: false,
        },
        DrawingArcCase {
            name: "near-full-3595",
            rectangle: r,
            start: 0.,
            sweep: 359.5,
            seeded: false,
        },
        DrawingArcCase {
            name: "near-full-35999",
            rectangle: r,
            start: 0.,
            sweep: 359.99,
            seeded: false,
        },
        DrawingArcCase {
            name: "near-full-359999",
            rectangle: r,
            start: 0.,
            sweep: 359.999,
            seeded: false,
        },
        DrawingArcCase {
            name: "full-exact",
            rectangle: r,
            start: 0.,
            sweep: 360.,
            seeded: false,
        },
        DrawingArcCase {
            name: "full-oblique",
            rectangle: r,
            start: 37.,
            sweep: 360.,
            seeded: false,
        },
        DrawingArcCase {
            name: "double-turn",
            rectangle: r,
            start: 37.,
            sweep: 720.,
            seeded: false,
        },
        DrawingArcCase {
            name: "large-positive",
            rectangle: r,
            start: 11.,
            sweep: 450.,
            seeded: false,
        },
        DrawingArcCase {
            name: "large-negative",
            rectangle: r,
            start: 11.,
            sweep: -450.,
            seeded: false,
        },
        DrawingArcCase {
            name: "zero-sweep-zero-start",
            rectangle: r,
            start: 0.,
            sweep: 0.,
            seeded: false,
        },
        DrawingArcCase {
            name: "zero-sweep-oblique",
            rectangle: r,
            start: 37.,
            sweep: 0.,
            seeded: false,
        },
        DrawingArcCase {
            name: "zero-width",
            rectangle: [10., 20., 10., 120.],
            start: 37.,
            sweep: 123.,
            seeded: false,
        },
        DrawingArcCase {
            name: "zero-height",
            rectangle: [10., 20., 210., 20.],
            start: 37.,
            sweep: 123.,
            seeded: false,
        },
        DrawingArcCase {
            name: "point-rectangle",
            rectangle: [10., 20., 10., 20.],
            start: 37.,
            sweep: 123.,
            seeded: false,
        },
        DrawingArcCase {
            name: "inverted-x",
            rectangle: [210., 20., 10., 120.],
            start: 37.,
            sweep: 123.,
            seeded: false,
        },
        DrawingArcCase {
            name: "inverted-y",
            rectangle: [10., 120., 210., 20.],
            start: 37.,
            sweep: 123.,
            seeded: false,
        },
        DrawingArcCase {
            name: "fractional-rect",
            rectangle: [-10.125, 20.375, 210.0625, 120.5],
            start: 37.,
            sweep: 123.,
            seeded: false,
        },
        DrawingArcCase {
            name: "narrowed-subnormal",
            rectangle: [10., 20., 210., 120.],
            start: f64::from(f32::from_bits(1)),
            sweep: f64::from(f32::from_bits(0x3f800000)),
            seeded: false,
        },
        DrawingArcCase {
            name: "large-finite-coordinates",
            rectangle: [-1.0e38, -1.0e38, 1.0e38, 1.0e38],
            start: 37.,
            sweep: 123.,
            seeded: false,
        },
    ];
    let cases = cases
        .iter()
        .map(|case| capture_drawing_arc_case(model, drawing, base, skia, *case))
        .collect::<Vec<_>>();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"skia_library_sha256\":\"{SKIA_SHA256}\",\"native_addresses\":{{\"drawing_convert_to_path\":\"Drawing:0x9bd24\",\"skia_arc_to\":\"Skia:0x1d6974\",\"skia_arc_helper\":\"Skia:0x1d6ab0\",\"skia_quad_builder\":\"Skia:0x1bb294\",\"skia_raw_iterator\":\"Skia:0x1d41ac\"}},\"transcendental_provider\":{{\"symbol\":\"sincosf\",\"library\":\"host Linux libm\",\"sha256\":\"{LIBM_SHA256}\",\"status\":\"native Skia path construction with substituted host transcendental values; Android libm bits not claimed\"}},\"capture_boundary\":\"Each case creates a fresh emulator and native path state. Drawing converts supplied 28-byte Path::Segment records; native Skia arcTo and RawIter execute. Host allocator/memory imports are bounded; Linux libm sincosf is hash-verified. Common loader/model normalization/serialization, paint, rendering, and pixels are outside scope. Input f64 values are narrowed to saved f32 fields before native conversion; recorded input_f32_bits state those exact native arguments.\",\"cases\":[{}]}}",
        cases.join(",")
    );
}

#[derive(Clone, Copy)]
struct NormalizeCase<'a> {
    name: &'static str,
    segments: &'a [[u32; 7]],
    bounds: [f32; 4],
    angle: f32,
    flips: [u64; 2],
    input_frame: u64,
}

fn set_normalize_float_arguments(machine: &Machine, case: NormalizeCase<'_>) {
    for (index, value) in case.bounds.iter().chain([&case.angle]).enumerate() {
        register(
            machine.engine,
            136 + index as i32,
            u64::from(value.to_bits()),
        );
    }
}

fn read_model_path(machine: &Machine, path: u64) -> (u32, Vec<[u32; 7]>) {
    let implementation = read_u64(machine.engine, path);
    assert_ne!(implementation, 0, "native Path has no implementation");
    let segment_pointer = read_u64(machine.engine, implementation);
    let count = read_u32(machine.engine, implementation + 32);
    assert!(count <= 256);
    let segments = (0..count)
        .map(|index| {
            let base = segment_pointer + u64::from(index) * 28;
            std::array::from_fn(|field| read_u32(machine.engine, base + field as u64 * 4))
        })
        .collect();
    (count, segments)
}

fn capture_normalize_case(
    model: &Path,
    base: &Path,
    skia: &Path,
    case: NormalizeCase<'_>,
) -> String {
    const PATH: u64 = MODEL + 0x1000;
    const SEGMENTS: u64 = MODEL + 0x10000;
    const TEMPLATE: u64 = MODEL + 0x3000;
    const TEMPLATE_IMPL: u64 = MODEL + 0x4000;
    let machine = Machine::new(model);
    map_library(machine.engine, base, BASE, BASE_SHA256);
    map_library(machine.engine, skia, SKIA, SKIA_SHA256);
    for (path, address) in [(model, 0), (base, BASE), (skia, SKIA)] {
        reject_unbound_imports(machine.engine, path, address);
    }
    bind_imports(machine.engine);
    for (got, target) in [
        (0x4a4560, HOST),
        (0x4a4578, HOST + 16),
        (0x4a4580, HOST + 16),
        (0x4a4588, HOST),
        (0x4a4850, HOST + 208),
        (0x4a4858, HOST + 208),
        (0x4a4ac0, HOST + 208),
        (0x4a4ac8, HOST + 208),
    ] {
        write(machine.engine, got, &target.to_le_bytes());
    }
    for (plt, target) in [
        (0x47f240, 0x2ef320),
        (0x47f250, 0x2ef414),
        (0x47f270, 0x2ef584),
        (0x47f280, 0x2ef5d8),
        (0x47f290, 0x2ef630),
        (0x47f3a0, 0x2ef2d8),
        (0x47f3b0, 0x2ef2e0),
        (0x47f3c0, 0x2f1730),
    ] {
        bind_native(machine.engine, plt, target);
    }
    let mut state = Box::new(HostState {
        fill: 0,
        cursor: HEAP,
        allocations: BTreeMap::new(),
        calls: BTreeMap::new(),
    });
    let _hooks = Hooks::new(machine.engine, state.as_mut());
    let mut source_bytes = Vec::with_capacity(case.segments.len() * 28);
    for segment in case.segments {
        source_bytes.extend(segment.iter().flat_map(|value| value.to_le_bytes()));
    }
    write(machine.engine, SEGMENTS, &source_bytes);
    write(machine.engine, PATH, &[0; 8]);
    machine.call(0x2ef2d8, &[PATH]);
    let path_result = machine.call(0x2ef414, &[PATH, SEGMENTS, case.segments.len() as u64]);
    assert_ne!(path_result, 0, "native source Path construction failed");
    let mut model_helpers = Vec::new();
    for (index, segment) in case.segments.iter().enumerate() {
        let values = match segment[0] {
            5 => {
                let left = f32::from_bits(segment[1]);
                let top = f32::from_bits(segment[2]);
                let width = f32::from_bits(segment[3]);
                let height = f32::from_bits(segment[4]);
                let start = f32::from_bits(segment[5]);
                let mut sweep = f32::from_bits(segment[6]);
                if sweep.to_bits() == 360.0f32.to_bits() {
                    sweep = f32::from_bits(0x43b3_feb8);
                }
                [left, top, left + width, top + height, start, sweep]
            }
            7 => {
                let x1 = f32::from_bits(segment[1]);
                let y1 = f32::from_bits(segment[2]);
                let x2 = f32::from_bits(segment[3]);
                let y2 = f32::from_bits(segment[4]);
                [
                    x1.min(x2),
                    y1.min(y2),
                    x1.max(x2),
                    y1.max(y2),
                    0.0,
                    f32::from_bits(0x43b3_feb8),
                ]
            }
            _ => continue,
        };
        for (slot, value) in values.iter().enumerate() {
            register(
                machine.engine,
                136 + slot as i32,
                u64::from(value.to_bits()),
            );
        }
        write(machine.engine, OUTPUT, &[0; 256]);
        let count = machine.call(0x211814, &[OUTPUT]);
        assert!(count <= 17);
        let points: Vec<_> = (0..count * 2)
            .map(|slot| read_u32(machine.engine, OUTPUT + slot * 4))
            .collect();
        model_helpers.push(format!(
            "{{\"segment_index\":{index},\"input_bits\":{:?},\"point_count\":{count},\"point_bits\":{points:?}}}",
            values.map(f32::to_bits)
        ));
    }
    write(machine.engine, TEMPLATE, &[0; 16]);
    write(machine.engine, TEMPLATE + 8, &TEMPLATE_IMPL.to_le_bytes());
    write(machine.engine, TEMPLATE_IMPL, &[0; 64]);
    set_normalize_float_arguments(&machine, case);
    let loaded = machine.call(
        0x20bad0,
        &[
            TEMPLATE,
            PATH,
            case.flips[0],
            case.flips[1],
            case.input_frame,
        ],
    );
    assert_eq!(loaded, 1, "Model common path load failed for {}", case.name);
    let base_path = read_u64(machine.engine, TEMPLATE_IMPL + 32);
    let display_path = read_u64(machine.engine, TEMPLATE_IMPL + 24);
    assert_ne!(base_path, 0);
    assert_ne!(display_path, 0);
    let (base_count, base_segments) = read_model_path(&machine, base_path);
    let (display_count, display_segments) = read_model_path(&machine, display_path);
    let (_, source_after) = read_model_path(&machine, PATH);
    set_normalize_float_arguments(&machine, case);
    let repeated_loaded = machine.call(
        0x20bad0,
        &[
            TEMPLATE,
            PATH,
            case.flips[0],
            case.flips[1],
            case.input_frame,
        ],
    );
    assert_eq!(
        repeated_loaded, 1,
        "repeated Model load failed for {}",
        case.name
    );
    let repeated_base_path = read_u64(machine.engine, TEMPLATE_IMPL + 32);
    let repeated_display_path = read_u64(machine.engine, TEMPLATE_IMPL + 24);
    let (repeated_base_count, repeated_base_segments) =
        read_model_path(&machine, repeated_base_path);
    let (repeated_display_count, repeated_display_segments) =
        read_model_path(&machine, repeated_display_path);
    let (_, repeated_source_after) = read_model_path(&machine, PATH);
    format!(
        "{{\"name\":{:?},\"input_bits\":{:?},\"model_helpers\":[{}],\"input_after\":{:?},\"bounds_bits\":{:?},\"angle_bits\":{},\"flip_inputs\":{:?},\"stored_orientation\":[{},{}],\"input_frame\":{},\"base_count\":{base_count},\"base_segments\":{:?},\"display_count\":{display_count},\"display_segments\":{:?},\"repeat_base_count\":{repeated_base_count},\"repeat_base_segments\":{:?},\"repeat_display_count\":{repeated_display_count},\"repeat_display_segments\":{:?},\"repeat_input_after\":{:?},\"host_imports\":{:?}}}",
        case.name,
        case.segments
            .iter()
            .map(|segment| segment.to_vec())
            .collect::<Vec<_>>(),
        model_helpers.join(","),
        source_after
            .iter()
            .map(|segment| segment.to_vec())
            .collect::<Vec<_>>(),
        case.bounds.map(f32::to_bits),
        case.angle.to_bits(),
        case.flips,
        read_u32(machine.engine, TEMPLATE_IMPL + 16),
        read_u32(machine.engine, TEMPLATE_IMPL + 17),
        case.input_frame,
        base_segments
            .iter()
            .map(|segment| segment.to_vec())
            .collect::<Vec<_>>(),
        display_segments
            .iter()
            .map(|segment| segment.to_vec())
            .collect::<Vec<_>>(),
        repeated_base_segments
            .iter()
            .map(|segment| segment.to_vec())
            .collect::<Vec<_>>(),
        repeated_display_segments
            .iter()
            .map(|segment| segment.to_vec())
            .collect::<Vec<_>>(),
        repeated_source_after
            .iter()
            .map(|segment| segment.to_vec())
            .collect::<Vec<_>>(),
        state.calls
    )
}

pub(super) fn capture_normalization(model: &Path, base: &Path, skia: &Path) {
    verify_host_math();
    let r = [0.0f32, 0.0, 300.0, 200.0];
    let move_line_arc: [[u32; 7]; 3] = [
        [1, 0.0f32.to_bits(), 0.0f32.to_bits(), 0, 0, 0, 0],
        [2, 204.9997f32.to_bits(), 60.0f32.to_bits(), 0, 0, 0, 0],
        [
            5,
            5.0f32.to_bits(),
            10.0f32.to_bits(),
            200.0f32.to_bits(),
            100.0f32.to_bits(),
            0.0f32.to_bits(),
            90.0f32.to_bits(),
        ],
    ];
    let move_line_arc_miss = [
        [1, 0.0f32.to_bits(), 0.0f32.to_bits(), 0, 0, 0, 0],
        [2, 204.9994f32.to_bits(), 60.0f32.to_bits(), 0, 0, 0, 0],
        [
            5,
            5.0f32.to_bits(),
            10.0f32.to_bits(),
            200.0f32.to_bits(),
            100.0f32.to_bits(),
            0.0f32.to_bits(),
            90.0f32.to_bits(),
        ],
    ];
    let close_arc = [
        [6, 0, 0, 0, 0, 0, 0],
        [
            5,
            5.0f32.to_bits(),
            10.0f32.to_bits(),
            200.0f32.to_bits(),
            100.0f32.to_bits(),
            0.0f32.to_bits(),
            90.0f32.to_bits(),
        ],
    ];
    let consecutive_arcs = [
        [
            5,
            5.0f32.to_bits(),
            10.0f32.to_bits(),
            200.0f32.to_bits(),
            100.0f32.to_bits(),
            0.0f32.to_bits(),
            90.0f32.to_bits(),
        ],
        [
            5,
            5.0f32.to_bits(),
            10.0f32.to_bits(),
            200.0f32.to_bits(),
            100.0f32.to_bits(),
            90.0f32.to_bits(),
            90.0f32.to_bits(),
        ],
    ];
    let oval_then_close = [
        [
            7,
            10.0f32.to_bits(),
            20.0f32.to_bits(),
            210.0f32.to_bits(),
            120.0f32.to_bits(),
            0,
            0,
        ],
        [6, 0, 0, 0, 0, 0, 0],
    ];
    let quad_arc = [
        [
            3,
            1.0f32.to_bits(),
            2.0f32.to_bits(),
            0,
            0,
            205.0f32.to_bits(),
            60.0f32.to_bits(),
        ],
        [
            5,
            5.0f32.to_bits(),
            10.0f32.to_bits(),
            200.0f32.to_bits(),
            100.0f32.to_bits(),
            0.0f32.to_bits(),
            90.0f32.to_bits(),
        ],
    ];
    let cases = [
        NormalizeCase {
            name: "arc-quarter-extent",
            segments: &[[
                5,
                5.0f32.to_bits(),
                10.0f32.to_bits(),
                200.0f32.to_bits(),
                100.0f32.to_bits(),
                0.0f32.to_bits(),
                90.0f32.to_bits(),
            ]],
            bounds: r,
            angle: 0.0,
            flips: [0, 0],
            input_frame: 0,
        },
        NormalizeCase {
            name: "arc-positive-full-turn",
            segments: &[[
                5,
                10.0f32.to_bits(),
                20.0f32.to_bits(),
                200.0f32.to_bits(),
                100.0f32.to_bits(),
                0.0f32.to_bits(),
                360.0f32.to_bits(),
            ]],
            bounds: r,
            angle: 0.0,
            flips: [0, 0],
            input_frame: 0,
        },
        NormalizeCase {
            name: "arc-connected-snap",
            segments: &move_line_arc,
            bounds: r,
            angle: 0.0,
            flips: [0, 0],
            input_frame: 0,
        },
        NormalizeCase {
            name: "arc-connected-outside-epsilon",
            segments: &move_line_arc_miss,
            bounds: r,
            angle: 0.0,
            flips: [0, 0],
            input_frame: 0,
        },
        NormalizeCase {
            name: "arc-after-close",
            segments: &close_arc,
            bounds: r,
            angle: 0.0,
            flips: [0, 0],
            input_frame: 0,
        },
        NormalizeCase {
            name: "consecutive-linked-arcs",
            segments: &consecutive_arcs,
            bounds: r,
            angle: 0.0,
            flips: [0, 0],
            input_frame: 0,
        },
        NormalizeCase {
            name: "oval-then-explicit-close",
            segments: &oval_then_close,
            bounds: r,
            angle: 0.0,
            flips: [0, 0],
            input_frame: 0,
        },
        NormalizeCase {
            name: "quadratic-endpoint-exact-link",
            segments: &quad_arc,
            bounds: r,
            angle: 0.0,
            flips: [0, 0],
            input_frame: 0,
        },
        NormalizeCase {
            name: "arc-inverse-frame-rotation",
            segments: &[[
                5,
                5.0f32.to_bits(),
                10.0f32.to_bits(),
                200.0f32.to_bits(),
                100.0f32.to_bits(),
                37.0f32.to_bits(),
                123.0f32.to_bits(),
            ]],
            bounds: r,
            angle: 15.0,
            flips: [0, 0],
            input_frame: 1,
        },
        NormalizeCase {
            name: "reversed-bounds-toggle-flips",
            segments: &[[
                5,
                5.0f32.to_bits(),
                10.0f32.to_bits(),
                200.0f32.to_bits(),
                100.0f32.to_bits(),
                0.0f32.to_bits(),
                90.0f32.to_bits(),
            ]],
            bounds: [300.0, 200.0, 0.0, 0.0],
            angle: 0.0,
            flips: [0, 1],
            input_frame: 0,
        },
        NormalizeCase {
            name: "arc-negative-full-turn",
            segments: &[[
                5,
                10.0f32.to_bits(),
                20.0f32.to_bits(),
                200.0f32.to_bits(),
                100.0f32.to_bits(),
                0.0f32.to_bits(),
                (-360.0f32).to_bits(),
            ]],
            bounds: r,
            angle: 0.0,
            flips: [0, 0],
            input_frame: 0,
        },
        NormalizeCase {
            name: "oval-ordered-corners",
            segments: &[[
                7,
                210.0f32.to_bits(),
                120.0f32.to_bits(),
                10.0f32.to_bits(),
                20.0f32.to_bits(),
                0,
                0,
            ]],
            bounds: r,
            angle: 0.0,
            flips: [0, 0],
            input_frame: 0,
        },
    ];
    let cases = cases
        .iter()
        .map(|case| capture_normalize_case(model, base, skia, *case))
        .collect::<Vec<_>>();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"skia_library_sha256\":\"{SKIA_SHA256}\",\"normalizer\":\"Model:0x20bad0 + installer Model:0x20c688\",\"model_arc_helper\":\"Model:0x211814 called directly on each normalized Arc/Oval rectangle/start/sweep before common load; points are independent helper evidence, not Drawing/Skia output\",\"case_boundary\":\"Each case uses a fresh Unicorn machine, one newly constructed native SPen::Path and one fresh zeroed template implementation. One load records the baseline, then the same Path and template are loaded again to expose source mutation and repeated-load behavior. The common Model normalizer and installer execute; input-frame rotation reaches Base PointF rotation. Recursive-mutex constructor/destructor/lock/unlock are bounded single-thread no-op stubs; they do not model locking, concurrency, or object lifecycle. Host allocations, bounded copies and pinned Linux libm are explicit; Android libm transcendental bit parity is unverified. No file loader/serializer, Drawing renderer, paint, SVG/PDF or pixels execute.\",\"cases\":[{}]}}",
        cases.join(",")
    );
}
