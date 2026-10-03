use super::*;
use std::collections::BTreeMap;

const SKIA: u64 = 0x0600_0000;
const BASE: u64 = 0x0400_0000;
const SKIA_SHA256: &str = "42636cb9ac06cc286114b42c1b9d8f4b78d33761843251cde2b443b101ffb88d";
const BASE_SHA256: &str = "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb";
const LIBM_SHA256: &str = "6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3";
const HOST: u64 = MODEL + 0x30000;
const PATH_OBJECT: u64 = MODEL + 0x1000;
const RECTANGLE: u64 = MODEL + 0x1100;
const ITERATOR: u64 = MODEL + 0x1200;
const OUTPUT: u64 = MODEL + 0x2000;
const FILLS: [u8; 5] = [0, 85, 165, 255, 0];
const SYMBOLS: [&str; 13] = [
    "_ZdlPv", "_Znwm", "free", "malloc", "memcpy", "memmove", "realloc", "sincosf", "sinf", "cosf",
    "tanf", "atan2f", "sincos",
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
fn verify_host_math() {
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
        assert!(length <= 0x10000);
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
