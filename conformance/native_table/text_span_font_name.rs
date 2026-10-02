use super::*;

#[path = "text_cell_host.rs"]
pub(super) mod cell_host;
#[path = "text_font_registry.rs"]
mod registry;
use std::collections::BTreeMap;
use text_font_source::{FONT_BYTES, NativeFontEnvironment, TEXT, bytes, json_string};
use text_shaping::{HostIcu, INITIALIZERS, host_import, pinned_host};

const XML: u64 = 0x0a00_0000;
const XML_SHA256: &str = "46753f76c8c007e78777e8fe7de7b57202f966f9494d4fba2b675c3540e35dbd";
const ERRNO: u64 = MODEL + 0x31000;
const OUTPUT: u64 = MODEL + 0x32000;
const SPAN: u64 = MODEL + 0x32400;
const DEFAULT_NAME: u64 = MODEL + 0x32500;
const TEXT_PAINT: u64 = MODEL + 0x32600;
const INPUT: u64 = MODEL + 0x32100;
const NAME: u64 = MODEL + 0x32200;
const FAMILY: u64 = MODEL + 0x32300;
const CONFIG: &str = "<familyset><family name=\"sans-serif\"><font weight=\"400\" style=\"normal\">Roboto-Regular.ttf</font><font weight=\"700\" style=\"normal\">Roboto-Bold.ttf</font><font weight=\"400\" style=\"italic\">Roboto-Italic.ttf</font><font weight=\"700\" style=\"italic\">Roboto-BoldItalic.ttf</font></family></familyset>";

const FONTS: [(&str, &str); 4] = [
    (
        "Roboto-Regular.ttf",
        "56a45233d29f11b4dfb86d248e921939d115778f87325e7ae8cc108383d6664d",
    ),
    (
        "Roboto-Bold.ttf",
        "61f89f8db49261c2f6106e8dccc35df7b2f7ed909020db40a3fc905e95f99334",
    ),
    (
        "Roboto-Italic.ttf",
        "fa0b17bb4aaac4a1b2ee149dd4ca3b55e97d3077aa6ba9bb02541b316e7c46ce",
    ),
    (
        "Roboto-BoldItalic.ttf",
        "40083ed54338397cf49d2c49f59eddcd963a30fdb301813d4bd3abbb37a13d12",
    ),
];

const HOST_PINS: [(&str, &str); 4] = [
    (
        "/usr/lib/x86_64-linux-gnu/libicuuc.so.76.1",
        "a8e433e81075732faf255b17d4a25ce28632e41fef1a75e727ee7f4ed73ab151",
    ),
    (
        "/usr/lib/x86_64-linux-gnu/libicudata.so.76.1",
        "a04b2b906193fa1e40f968a3d16d7d6c844a1fafbdd5bce6e9f67b01c124ff24",
    ),
    (
        "/usr/lib/x86_64-linux-gnu/libc.so.6",
        "fa430b8f298f817a266046af84a77533185ad6fc4406c7d3787b5a0a0c207826",
    ),
    (
        "/usr/lib/x86_64-linux-gnu/libm.so.6",
        "6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3",
    ),
];

unsafe extern "C" {
    fn snprintf(output: *mut u8, size: usize, format: *const std::ffi::c_char, ...) -> i32;
}
fn format_one_string(engine: Engine, args: [u64; 8]) -> u64 {
    let format = c_string(engine, args[2]);
    assert!(matches!(format.as_str(), "%s/fonts.xml" | "und%s"));
    let format = std::ffi::CString::new(format).unwrap();
    assert!(args[1] <= 4096);
    let top = read_u64(engine, args[3] + 8);
    let offset = read_u32(engine, args[3] + 24) as i32;
    assert!(offset <= -8);
    let argument = read_u64(engine, top.wrapping_add_signed(i64::from(offset)));
    let argument = std::ffi::CString::new(c_string(engine, argument)).unwrap();
    let mut output = vec![0_u8; args[1] as usize];
    let count = unsafe {
        snprintf(
            output.as_mut_ptr(),
            output.len(),
            format.as_ptr(),
            argument.as_ptr(),
        )
    };
    assert!(count >= 0 && (count as usize) < output.len());
    write(engine, args[0], &output[..count as usize + 1]);
    count as u64
}
struct File {
    path: String,
    content: Vec<u8>,
    mapping: u64,
    digest: &'static str,
}
struct Descriptor {
    file: usize,
    position: usize,
}
pub(super) struct Services {
    pub(super) icu: Box<HostIcu>,
    files: Vec<File>,
    descriptors: BTreeMap<u64, Descriptor>,
    paths: Vec<String>,
}
fn c_string(engine: Engine, address: u64) -> String {
    let mut result = Vec::new();
    for index in 0..4096 {
        let byte = bytes(engine, address + index, 1)[0];
        if byte == 0 {
            return String::from_utf8(result).unwrap();
        }
        result.push(byte);
    }
    panic!("unbounded input string");
}
impl Services {
    fn new(machine: &Machine, font: &Path) -> Self {
        let directory = font.parent().unwrap();
        let mut files = vec![File {
            path: "/system/etc/fonts.xml".into(),
            content: CONFIG.as_bytes().to_vec(),
            mapping: 0,
            digest: "9864ad4db5012ad4b63f82fcd0375b4f6a02a92d762147805ebdc02de0f4ed22",
        }];
        for (index, (name, digest)) in FONTS.into_iter().enumerate() {
            let path = directory.join(name);
            pinned_host(path.to_str().unwrap(), digest);
            let mapping = FONT_BYTES + index as u64 * 0x100000;
            if index != 0 {
                check(unsafe { uc_mem_map(machine.engine, mapping, 0x100000, 7) });
            }
            let content = fs::read(path).unwrap();
            write(machine.engine, mapping, &content);
            files.push(File {
                path: format!("/system/fonts/{name}"),
                content,
                mapping,
                digest,
            });
        }
        Self {
            icu: HostIcu::new(machine),
            files,
            descriptors: BTreeMap::new(),
            paths: Vec::new(),
        }
    }
    fn open(&mut self, engine: Engine, path: u64) -> u64 {
        let name = c_string(engine, path);
        self.paths.push(name.clone());
        assert!(self.paths.len() < 128);
        let file = self.files.iter().position(|f| f.path == name);
        if let Some(file) = file {
            let descriptor = 100 + self.descriptors.len() as u64;
            self.descriptors
                .insert(descriptor, Descriptor { file, position: 0 });
            descriptor
        } else {
            write(engine, ERRNO, &2_u32.to_le_bytes());
            u64::MAX
        }
    }
    fn read(&mut self, engine: Engine, descriptor: u64, destination: u64, count: usize) -> usize {
        assert!(count <= 0x100000);
        let state = self.descriptors.get_mut(&descriptor).unwrap();
        let file = &self.files[state.file];
        let count = count.min(file.content.len() - state.position);
        write(
            engine,
            destination,
            &file.content[state.position..state.position + count],
        );
        state.position += count;
        count
    }
}
fn import(engine: Engine, name: &str, args: [u64; 8], data: *mut c_void) -> Option<u64> {
    let state = unsafe { &mut *data.cast::<Services>() };
    let [a, b, c, d, _, _, _, _] = args;
    let result = match name {
        "__open_2" | "open" | "open64" => {
            assert_eq!(b & 3, 0);
            state.open(engine, a)
        }
        "fopen" => {
            assert_eq!(c_string(engine, b), "r");
            let fd = state.open(engine, a);
            if fd == u64::MAX { 0 } else { fd }
        }
        "fread" => {
            assert_ne!(b, 0);
            let count = b.checked_mul(c).unwrap() as usize;
            state.read(engine, d, a, count) as u64 / b
        }
        "read" => state.read(engine, a, b, c as usize) as u64,
        "__read_chk" => {
            assert!(c <= d);
            state.read(engine, a, b, c as usize) as u64
        }
        "close" | "fclose" => {
            assert!(state.descriptors.contains_key(&a));
            0
        }
        "fstat" => {
            let descriptor = state.descriptors.get(&a).unwrap();
            let f = &state.files[descriptor.file];
            write(engine, b, &[0; 128]);
            write(engine, b + 16, &0o100644_u32.to_le_bytes());
            write(engine, b + 48, &(f.content.len() as u64).to_le_bytes());
            0
        }
        "stat" => {
            let path = c_string(engine, a);
            if let Some(f) = state.files.iter().find(|f| f.path == path) {
                write(engine, b, &[0; 128]);
                write(engine, b + 16, &0o100644_u32.to_le_bytes());
                write(engine, b + 48, &(f.content.len() as u64).to_le_bytes());
                0
            } else {
                write(engine, ERRNO, &2_u32.to_le_bytes());
                u64::MAX
            }
        }
        "mmap" => {
            let descriptor = state.descriptors.get(&args[4]).unwrap();
            let f = &state.files[descriptor.file];
            assert_eq!(a, 0);
            assert_eq!(b, f.content.len() as u64);
            assert_eq!(args[5], 0);
            assert_ne!(f.mapping, 0);
            f.mapping
        }
        "munmap" => {
            assert!(
                state
                    .files
                    .iter()
                    .any(|f| f.mapping == a && f.content.len() as u64 == b)
            );
            0
        }
        "clock_gettime" => {
            assert!(a <= 1);
            write(engine, b, &[0; 16]);
            0
        }
        "vsnprintf" => format_one_string(engine, args),
        "__errno" => ERRNO,
        "pthread_mutexattr_init"
        | "pthread_mutexattr_settype"
        | "pthread_mutexattr_destroy"
        | "pthread_mutex_init"
        | "pthread_mutex_destroy"
        | "pthread_mutex_lock"
        | "pthread_mutex_unlock"
        | "pthread_cond_init"
        | "pthread_cond_destroy"
        | "pthread_cond_signal"
        | "pthread_cond_broadcast"
        | "pthread_key_delete" => 0,
        "__stack_chk_fail" => panic!(
            "native stack check failed LR={:x} SP={:x}",
            read_register(engine, REGISTER_X30),
            read_register(engine, REGISTER_SP)
        ),
        "syscall" => {
            assert_eq!(a, 178);
            1
        }
        "pthread_self" => 1,
        "pthread_equal" => u64::from(a == b),
        _ => return host_import(engine, name, args, ptr::from_mut(state.icu.as_mut()).cast()),
    };
    Some(result)
}

#[derive(Clone, Copy)]
pub(super) struct Case {
    pub(super) span_name: Option<&'static str>,
    pub(super) default_name: Option<&'static str>,
    pub(super) flags: u8,
    pub(super) direction: bool,
    pub(super) size: f32,
}

#[derive(Default)]
struct FactoryTrace {
    name: Option<String>,
    direction: bool,
    parser_name: Option<String>,
    parser_result: bool,
    parser_style: u32,
    family: String,
    requested_weight: i32,
    requested_italic: i32,
    skia_typeface: u64,
    pending_parser: Option<u64>,
    factory_calls: usize,
}
fn cxx_string(engine: Engine, address: u64) -> String {
    let first = bytes(engine, address, 1)[0];
    let (length, pointer) = if first & 1 == 0 {
        (usize::from(first >> 1), address + 1)
    } else {
        (
            read_u64(engine, address + 8) as usize,
            read_u64(engine, address + 16),
        )
    };
    assert!(length <= 4096);
    String::from_utf8(bytes(engine, pointer, length)).unwrap()
}
fn spen_string(machine: &Machine, address: u64, input: Option<&str>) -> u64 {
    if let Some(input) = input {
        write(machine.engine, INPUT + 64, input.as_bytes());
        write(machine.engine, INPUT + 64 + input.len() as u64, &[0]);
        machine.call(TEXT + 0xeeb50, &[address]);
        machine.call(TEXT + 0xeeb70, &[address, INPUT + 64]);
        address
    } else {
        0
    }
}
fn read_spen_string(engine: Engine, pointer: u64) -> Option<String> {
    if pointer == 0 {
        None
    } else {
        let implementation = read_u64(engine, pointer + 8);
        assert_ne!(implementation, 0);
        let length = read_u32(engine, implementation + 12) as usize;
        assert!(length <= 4096);
        let data = read_u64(engine, implementation + 16);
        let units: Vec<_> = bytes(engine, data, length * 2)
            .chunks_exact(2)
            .map(|unit| u16::from_le_bytes(unit.try_into().unwrap()))
            .collect();
        Some(String::from_utf16(&units).unwrap())
    }
}
unsafe extern "C" fn trace_call(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let trace = unsafe { &mut *data.cast::<FactoryTrace>() };
    match address - TEXT {
        0x8a614 => {
            assert_eq!(trace.factory_calls, 0);
            trace.factory_calls += 1;
            trace.name = read_spen_string(engine, read_register(engine, REGISTER_X0 + 1));
            trace.direction = read_register(engine, REGISTER_X0 + 2) != 0;
        }
        0x8a42c => {
            assert!(trace.pending_parser.is_none());
            trace.parser_name = read_spen_string(engine, read_register(engine, REGISTER_X0));
            trace.pending_parser = Some(read_register(engine, REGISTER_X0 + 1));
        }
        0x8a668 => {
            let destination = trace.pending_parser.take().unwrap();
            trace.parser_result = read_register(engine, REGISTER_X0) & 1 != 0;
            trace.parser_style = read_u32(engine, destination) & 0x1ffff;
        }
        0x8a278 => {
            trace.family = cxx_string(engine, read_register(engine, REGISTER_X0 + 1));
            trace.requested_weight = read_register(engine, REGISTER_X0 + 2) as i32;
            trace.requested_italic = read_register(engine, REGISTER_X0 + 3) as i32;
        }
        0xefe40 => {
            assert_eq!(trace.skia_typeface, 0);
            trace.skia_typeface = read_register(engine, REGISTER_X0 + 1);
        }
        _ => unreachable!(),
    }
}
pub(super) struct TraceRecorder {
    trace: Box<FactoryTrace>,
    hooks: Vec<usize>,
    engine: Engine,
}
impl TraceRecorder {
    pub(super) fn new(machine: &Machine) -> Self {
        let mut result = Self {
            trace: Box::default(),
            hooks: Vec::new(),
            engine: machine.engine,
        };
        for offset in [0x8a614, 0x8a42c, 0x8a668, 0x8a278, 0xefe40] {
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    trace_call as *mut c_void,
                    ptr::from_mut(result.trace.as_mut()).cast(),
                    TEXT + offset,
                    TEXT + offset,
                )
            });
            result.hooks.push(hook);
        }
        result
    }
}
impl Drop for TraceRecorder {
    fn drop(&mut self) {
        for hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, *hook) });
        }
    }
}
fn window(machine: &Machine, begin: u64, end: u64) {
    let result = unsafe { uc_emu_start(machine.engine, TEXT + begin, TEXT + end, 0, 1_000_000) };
    assert_eq!(
        result,
        0,
        "window failed at PC {:x}",
        read_register(machine.engine, 260)
    );
    assert_eq!(read_register(machine.engine, 260), TEXT + end);
}
pub(super) struct ResultProfile {
    pub(super) paint: u64,
    selected_name: Option<String>,
    factory: FactoryTrace,
    typeface_style: u32,
    family: String,
    physical_path: String,
    physical_digest: &'static str,
    physical_style: u32,
    physical_fakery: u64,
    pub(super) physical_source_id: u64,
    physical_source: u64,
    physical_face_index: u64,
    physical_data_size: u64,
    physical_mapping_is_pinned: bool,
    setter_selects_same_physical_face: bool,
    paint_size_bits: u32,
    paint_scale_bits: u32,
    paint_skew_bits: u32,
    paint_flags: u32,
    paint_weight: u16,
    paint_italic: bool,
}
fn optional_json(input: Option<&str>) -> String {
    input.map_or_else(|| "null".to_owned(), json_string)
}
impl ResultProfile {
    pub(super) fn json(&self, case: Case) -> String {
        format!(
            "{{\"span_name\":{},\"default_name\":{},\"source_flags\":{},\"caller_direction\":{},\"source_size_bits\":{},\"selected_name\":{},\"factory_name\":{},\"factory_direction\":{},\"parser_name\":{},\"parser_matched\":{},\"parser_style\":{},\"factory_family\":{},\"requested_weight\":{},\"requested_italic\":{},\"typeface_style\":{},\"resolved_family\":{},\"physical_font_path\":{},\"physical_font_sha256\":{},\"physical_font_style\":{},\"physical_fakery\":{},\"physical_source_id\":{},\"physical_face_index\":{},\"physical_data_size\":{},\"physical_mapping_is_pinned\":{},\"set_typeface_selects_same_physical_face\":{},\"paint_size_bits\":{},\"paint_scale_x_bits\":{},\"paint_skew_x_bits\":{},\"paint_flags\":{},\"paint_weight\":{},\"paint_italic\":{}}}",
            optional_json(case.span_name),
            optional_json(case.default_name),
            case.flags,
            case.direction,
            case.size.to_bits(),
            optional_json(self.selected_name.as_deref()),
            optional_json(self.factory.name.as_deref()),
            self.factory.direction,
            optional_json(self.factory.parser_name.as_deref()),
            self.factory.parser_result,
            self.factory.parser_style,
            json_string(&self.factory.family),
            self.factory.requested_weight,
            self.factory.requested_italic,
            self.typeface_style,
            json_string(&self.family),
            json_string(&self.physical_path),
            json_string(self.physical_digest),
            self.physical_style,
            self.physical_fakery,
            self.physical_source_id,
            self.physical_face_index,
            self.physical_data_size,
            self.physical_mapping_is_pinned,
            self.setter_selects_same_physical_face,
            self.paint_size_bits,
            self.paint_scale_bits,
            self.paint_skew_bits,
            self.paint_flags,
            self.paint_weight,
            self.paint_italic,
        )
    }
}
pub(super) fn execute(
    machine: &Machine,
    services: &Services,
    case: Case,
    recorder: &mut TraceRecorder,
) -> ResultProfile {
    *recorder.trace = FactoryTrace::default();
    let name = spen_string(machine, NAME, case.span_name);
    let default = spen_string(machine, DEFAULT_NAME, case.default_name);
    write(machine.engine, SPAN, &[0; 72]);
    write(machine.engine, SPAN, &case.size.to_bits().to_le_bytes());
    write(machine.engine, SPAN + 4, &0xff112233_u32.to_le_bytes());
    write(machine.engine, SPAN + 16, &[case.flags]);
    write(machine.engine, SPAN + 24, &name.to_le_bytes());
    register(machine.engine, REGISTER_SP, STACK - 0x3000);
    for (index, value) in [OUTPUT, SPAN, 0, 1, u64::from(case.direction), default, 0]
        .into_iter()
        .enumerate()
    {
        register(machine.engine, REGISTER_X0 + index as i32, value);
    }
    window(machine, 0x7710c, 0x7719c);
    let selected = read_register(machine.engine, REGISTER_X0 + 25);
    let selected_name = read_spen_string(machine.engine, selected);
    let typeface = read_register(machine.engine, REGISTER_X0 + 20);
    assert_ne!(typeface, 0);
    let typeface_style = machine.call(TEXT + 0x89f40, &[typeface]) as u32 & 0x1ffff;
    assert_eq!(machine.call(TEXT + 0x89ef8, &[typeface, FAMILY]), 1);
    let family = c_string(machine.engine, FAMILY);
    register(machine.engine, REGISTER_X0 + 8, OUTPUT);
    machine.call(TEXT + 0x76ad4, &[SPAN, typeface, 0]);
    let paint = read_u64(machine.engine, OUTPUT);
    assert_ne!(paint, 0);
    let collection = read_u64(machine.engine, paint + 64);
    let native_family = read_u64(machine.engine, read_u64(machine.engine, collection + 8));
    let font_record = machine.call(TEXT + 0x913d4, &[native_family, u64::from(typeface_style)]);
    let physical_fakery = read_register(machine.engine, REGISTER_X0 + 1);
    let native_font = read_u64(machine.engine, font_record);
    let physical_style = read_u32(machine.engine, native_font + 16) & 0x1ffff;
    let physical_holder = machine.call(TEXT + 0x8eebc, &[native_font]);
    let physical = read_u64(machine.engine, physical_holder);
    let physical_path = cxx_string(machine.engine, machine.call(TEXT + 0x88ab4, &[physical]));
    let physical_data = machine.call(TEXT + 0x88abc, &[physical]);
    let physical_data_size = machine.call(TEXT + 0x88ac4, &[physical]);
    let physical_face_index = machine.call(TEXT + 0x88acc, &[physical]);
    let physical_source_id = machine.call(TEXT + 0x88ad4, &[physical]);
    let skia_typeface = machine.call(TEXT + 0x88b68, &[physical]);
    machine.call(TEXT + 0x7b700, &[TEXT_PAINT]);
    assert_eq!(machine.call(TEXT + 0x7b8bc, &[TEXT_PAINT, typeface]), 1);
    let physical_mapping_is_pinned = services.files.iter().any(|file| {
        file.path == physical_path
            && file.mapping == physical_data
            && file.content.len() as u64 == physical_data_size
    });
    assert!(physical_mapping_is_pinned);
    let physical_digest = services
        .files
        .iter()
        .find(|file| file.path == physical_path)
        .unwrap()
        .digest;
    assert_eq!(physical_style, typeface_style);
    assert_eq!(physical_fakery, 0);
    assert_eq!(recorder.trace.skia_typeface, skia_typeface);
    assert_eq!(recorder.trace.factory_calls, 1);
    assert!(recorder.trace.pending_parser.is_none());
    assert_eq!(selected_name, recorder.trace.name);
    assert_eq!(selected_name, recorder.trace.parser_name);
    assert_eq!(recorder.trace.direction, case.direction);
    assert_eq!(
        selected_name,
        case.span_name.or(case.default_name).map(str::to_owned)
    );
    let paint_weight = u16::from_le_bytes(bytes(machine.engine, paint + 28, 2).try_into().unwrap());
    let paint_italic = bytes(machine.engine, paint + 30, 1)[0] != 0;
    assert_eq!(u32::from(paint_weight), typeface_style & 0xffff);
    assert_eq!(paint_italic, typeface_style & 0x10000 != 0);
    ResultProfile {
        paint,
        selected_name,
        factory: std::mem::take(recorder.trace.as_mut()),
        typeface_style,
        family,
        physical_path,
        physical_digest,
        physical_style,
        physical_fakery,
        physical_source_id,
        physical_source: physical,
        physical_face_index,
        physical_data_size,
        physical_mapping_is_pinned,
        setter_selects_same_physical_face: true,
        paint_size_bits: read_u32(machine.engine, paint),
        paint_scale_bits: read_u32(machine.engine, paint + 4),
        paint_skew_bits: read_u32(machine.engine, paint + 8),
        paint_flags: read_u32(machine.engine, paint + 20),
        paint_weight,
        paint_italic,
    }
}
fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for span_name in [
        Some("Roboto"),
        Some("Roboto-Regular"),
        Some("Roboto-Bold"),
        Some("Roboto Bold"),
        Some("Roboto-Italic"),
        Some("Roboto-BoldItalic"),
        Some("missing"),
        Some(""),
        None,
    ] {
        for direction in [false, true] {
            for flags in [0, 1, 2, 4, 7] {
                cases.push(Case {
                    span_name,
                    default_name: Some("Roboto-Bold"),
                    flags,
                    direction,
                    size: 17.125,
                });
            }
        }
    }
    for span_name in [None, Some("")] {
        for default_name in [
            None,
            Some(""),
            Some("Roboto-Regular"),
            Some("Roboto-Italic"),
            Some("Roboto-BoldItalic"),
        ] {
            for direction in [false, true] {
                for flags in [0, 1] {
                    cases.push(Case {
                        span_name,
                        default_name,
                        flags,
                        direction,
                        size: 17.125,
                    });
                }
            }
        }
    }
    cases
}
pub(super) fn capture(
    machine: &mut Machine,
    base: &Path,
    text: &Path,
    skia: &Path,
    font: &Path,
    xml: &Path,
    cpp: &Path,
) {
    let (mut environment, mut services) = setup(machine, base, text, skia, font, xml, cpp);
    let cases = cases();
    let mut canonical = None;
    for fill in [0, 0xa5, 0xff, 0] {
        reset(machine, &mut environment, &mut services, fill);
        let mut recorder = TraceRecorder::new(machine);
        let outputs: Vec<_> = cases
            .iter()
            .map(|&case| execute(machine, &services, case, &mut recorder).json(case))
            .collect();
        if let Some(expected) = &canonical {
            assert_eq!(&outputs, expected);
        } else {
            canonical = Some(outputs);
        }
    }
    let fonts = services
        .files
        .iter()
        .skip(1)
        .map(|file| {
            format!(
                "{{\"path\":{},\"sha256\":{},\"byte_length\":{}}}",
                json_string(&file.path),
                json_string(file.digest),
                file.content.len()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    println!(
        concat!(
            "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"model_library_sha256\":\"{LIBRARY_SHA256}\",",
            "\"base_library_sha256\":\"{}\",\"text_library_sha256\":\"{}\",\"skia_library_sha256\":\"{}\",",
            "\"xml_library_sha256\":\"{XML_SHA256}\",\"cpp_library_sha256\":\"4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4\",",
            "\"host_icu_uc_sha256\":\"a8e433e81075732faf255b17d4a25ce28632e41fef1a75e727ee7f4ed73ab151\",",
            "\"host_icu_data_sha256\":\"a04b2b906193fa1e40f968a3d16d7d6c844a1fafbdd5bce6e9f67b01c124ff24\",",
            "\"host_libc_sha256\":\"fa430b8f298f817a266046af84a77533185ad6fc4406c7d3787b5a0a0c207826\",",
            "\"host_libm_sha256\":\"6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3\",",
            "\"memory_fills\":[0,165,255],\"repeat_zero_fill\":true,\"native_initializers\":{:?},\"instruction_limit\":10000000,",
            "\"source_counter_seed\":0,\"source_foreground_argb\":4279312947,\"supplied_fonts\":[{}],",
            "\"supplied_config_path\":\"/system/etc/fonts.xml\",\"supplied_config_sha256\":\"9864ad4db5012ad4b63f82fcd0375b4f6a02a92d762147805ebdc02de0f4ed22\",\"supplied_config\":{},",
            "\"native_addresses\":{{\"span_name_selection_window\":[\"Text+0x7710c\",\"Text+0x7719c\"],\"span_name_selection\":[\"Text+0x7715c\",\"Text+0x77164\"],",
            "\"factory_virtual_call\":\"Text+0x77184\",\"font_manager_get_instance\":\"Text+0x849e8\",\"font_manager_constructor\":\"Text+0x84684\",",
            "\"font_list_parser_constructor\":\"Text+0x7c8e0\",\"font_list_parser_parse\":\"Text+0x7cc10\",\"xml_read_file_call\":\"Text+0x7cc60\",",
            "\"get_family_by_font_name\":\"Text+0x84f8c\",\"create_from_font_name\":\"Text+0x8a614\",\"resolve_font_style\":\"Text+0x8a42c\",\"parser_result_observation\":\"Text+0x8a668\",",
            "\"create_from_family_name\":\"Text+0x8a278\",\"get_family_name\":\"Text+0x89ef8\",\"get_typeface_style\":\"Text+0x89f40\",\"complete_span_paint_helper\":\"Text+0x76ad4\",",
            "\"match_physical_font\":\"Text+0x913d4\",\"unwrap_physical_font\":\"Text+0x8eebc\",\"physical_source_getters\":[\"Text+0x88ab4\",\"Text+0x88abc\",\"Text+0x88ac4\",\"Text+0x88acc\",\"Text+0x88ad4\",\"Text+0x88b68\"],",
            "\"text_paint_constructor\":\"Text+0x7b700\",\"text_paint_set_typeface\":\"Text+0x7b8bc\",\"skia_set_typeface_observation\":\"Text+0xefe40\",\"string_utf16_layout_reference\":\"Base+0xc49a4\"}},",
            "\"capture_boundary\":{},\"cases\":[\n{}\n]}}"
        ),
        frames::BASE_SHA256,
        geometry::TEXT_SHA256,
        text_font_source::SKIA_SHA256,
        INITIALIZERS,
        fonts,
        json_string(CONFIG),
        json_string(
            "Actual FontManager constructor and FontListParser parse execute the caller-supplied XML through hash-pinned bundled libSPenLibxml2; actual Font/FreeType constructors load four pinned Roboto physical files and native family registration/name mapping executes. Actual SpanRun prologue/name/default selection and virtual CreateFromFontName call execute until immediately before the paint-helper call; the complete helper is then called on the returned Typeface. Actual suffix parser, family cache, Typeface style getters, physical family best-match and TextPaint/SkPaint typeface setter execute. Source size17.125, ARGB foreground, flags, nullable NAME/default and caller direction are supplied; returned MinikinPaint fields, physical path/low17 style/source ID/face index/data length and pinned mapping plus actual setter pointer retention are observed. Typeface style byte19 is unwritten padding and excluded; physical style is the native record metadata consumed by best-match, not an independent OS/font-name convention. XML is a fixture FontManager configuration, not device configuration. Native CSC/system-property probes receive absent properties; host file services provide only the listed XML/fonts, allocator/byte operations, single-thread synchronization, gettid1, zero-time clock_gettime and a host snprintf restricted to the native %s/fonts.xml format; actual APK libc++ executes C++ string/map algorithms. Pinned host ICU property/locale services and libc/libm are boundaries. Source IDs are process-local with seed0 and fixed file order. No host XML parser, suffix parser or physical font selector is substituted. Capture excludes device font equivalence, alternate XML/schema/error paths, fallback glyph selection, shaping/metrics, fake-bold metric support, whole SpanRun execution after paint conversion, wrapping, composition or SVG output."
        ),
        canonical.unwrap().join(",\n"),
        LIBRARY_SHA256 = LIBRARY_SHA256,
        XML_SHA256 = XML_SHA256,
    );
}

pub(super) fn setup(
    machine: &mut Machine,
    base: &Path,
    text: &Path,
    skia: &Path,
    font: &Path,
    xml: &Path,
    cpp: &Path,
) -> (NativeFontEnvironment, Box<Services>) {
    setup_with_libraries(machine, base, text, skia, font, xml, cpp, &[])
}

pub(super) fn setup_with_libraries(
    machine: &mut Machine,
    base: &Path,
    text: &Path,
    skia: &Path,
    font: &Path,
    xml: &Path,
    cpp: &Path,
    additional: &[(&Path, u64, &str)],
) -> (NativeFontEnvironment, Box<Services>) {
    setup_with_preloaded_libraries(machine, base, text, skia, font, xml, cpp, additional, &[])
}

pub(super) fn setup_with_preloaded_libraries(
    machine: &mut Machine,
    base: &Path,
    text: &Path,
    skia: &Path,
    font: &Path,
    xml: &Path,
    cpp: &Path,
    additional: &[(&Path, u64, &str)],
    preloaded: &[(&Path, u64, &str)],
) -> (NativeFontEnvironment, Box<Services>) {
    for (path, digest) in HOST_PINS {
        pinned_host(path, digest);
    }
    machine.call_instruction_limit = 10_000_000;
    machine.call_timeout_micros = 0;
    let mut libraries = vec![
        (xml, XML, XML_SHA256),
        (
            cpp,
            0x0b00_0000,
            "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
        ),
    ];
    libraries.extend_from_slice(additional);
    let mut environment = NativeFontEnvironment::with_preloaded_libraries(
        machine, base, text, skia, font, &libraries, preloaded,
    );
    let mut services = Box::new(Services::new(machine, font));
    environment.set_host_import_handler(import, ptr::from_mut(services.as_mut()).cast());
    (environment, services)
}
pub(super) fn reset(
    machine: &Machine,
    environment: &mut NativeFontEnvironment,
    services: &mut Services,
    fill: u8,
) {
    environment.reset(machine, fill, 0);
    services.descriptors.clear();
    services.paths.clear();
    for initializer in INITIALIZERS {
        machine.call(TEXT + initializer, &[]);
    }
    machine.call(TEXT + 0x849e8, &[]);
    assert_eq!(
        services.paths,
        vec![
            "/system/etc/fonts.xml",
            "/system/fonts/Roboto-Regular.ttf",
            "/system/fonts/Roboto-Bold.ttf",
            "/system/fonts/Roboto-Italic.ttf",
            "/system/fonts/Roboto-BoldItalic.ttf"
        ]
    );
}

pub(super) fn capture_registry(
    machine: &mut Machine,
    base: &Path,
    text: &Path,
    skia: &Path,
    font: &Path,
    xml: &Path,
    cpp: &Path,
) {
    registry::capture(machine, base, text, skia, font, xml, cpp);
}

pub(super) fn dependency_metadata() -> String {
    let fonts = FONTS
        .iter()
        .map(|(name, digest)| {
            format!(
                "{{\"file\":{},\"sha256\":{}}}",
                json_string(name),
                json_string(digest)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let host = HOST_PINS
        .iter()
        .map(|(path, digest)| {
            format!(
                "{{\"path\":{},\"sha256\":{}}}",
                json_string(path),
                json_string(digest)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"caller_font_xml\":{},\"caller_font_xml_sha256\":{},\"font_files\":[{fonts}],\"xml_library_sha256\":{},\"cpp_library_sha256\":{},\"host_libraries\":[{host}],\"icu_version\":[76,1,0,0],\"unicode_version\":[16,0,0,0],\"text_initializers\":{INITIALIZERS:?},\"per_native_call_instruction_limit\":10000000,\"wall_timeout_micros\":0}}",
        json_string(CONFIG),
        json_string("9864ad4db5012ad4b63f82fcd0375b4f6a02a92d762147805ebdc02de0f4ed22"),
        json_string(XML_SHA256),
        json_string("4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4")
    )
}
