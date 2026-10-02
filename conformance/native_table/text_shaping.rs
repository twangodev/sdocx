use super::*;
use text_font_source::{NativeFontEnvironment, TEXT, bytes, json_string};

const FONT: u64 = MODEL + 0x18000;
const FONT_VECTOR: u64 = MODEL + 0x18200;
const FONT_ITEM: u64 = MODEL + 0x18300;
const FAMILY: u64 = MODEL + 0x18400;
const LOCALE: u64 = MODEL + 0x18500;
const FAMILY_VECTOR: u64 = MODEL + 0x18600;
const FAMILY_ITEM: u64 = MODEL + 0x18700;
const COLLECTION: u64 = MODEL + 0x18800;
const PAINT: u64 = MODEL + 0x18900;
const UTF16: u64 = MODEL + 0x18a00;
const VIEW: u64 = MODEL + 0x18b00;
const RANGE: u64 = MODEL + 0x18b20;
const PIECE: u64 = MODEL + 0x18c00;
const INITIALIZERS: [u64; 9] = [
    0x90e28, 0x90fa4, 0x961bc, 0x9a9b8, 0x9ae28, 0x9da58, 0x9de0c, 0x9f190, 0x9f30c,
];

use std::{
    collections::BTreeMap,
    ffi::{CString, c_char},
};

#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(path: *const c_char, flags: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> i32;
}
#[link(name = "m")]
unsafe extern "C" {
    fn scalbnf(value: f32, power: i32) -> f32;
    fn sqrtf(value: f32) -> f32;
    fn floorf(value: f32) -> f32;
    fn ceilf(value: f32) -> f32;
}

struct HostIcu {
    library: *mut c_void,
    addresses: BTreeMap<String, u64>,
    names: BTreeMap<u64, String>,
    hooks: Vec<usize>,
    calls: Vec<String>,
    sort_calls: Vec<[u64; 4]>,
    engine: Engine,
    comparator: ComparatorEngine,
}
impl HostIcu {
    fn new(machine: &Machine) -> Box<Self> {
        let path = CString::new("/usr/lib/x86_64-linux-gnu/libicuuc.so.76.1").unwrap();
        let library = unsafe { dlopen(path.as_ptr(), 2) };
        assert!(!library.is_null());
        Box::new(Self {
            library,
            addresses: BTreeMap::new(),
            names: BTreeMap::new(),
            hooks: Vec::new(),
            calls: Vec::new(),
            sort_calls: Vec::new(),
            engine: machine.engine,
            comparator: ComparatorEngine::new(machine),
        })
    }
    fn symbol(&self, name: &str) -> *mut c_void {
        let name = CString::new(format!("{name}_76")).unwrap();
        let pointer = unsafe { dlsym(self.library, name.as_ptr()) };
        assert!(!pointer.is_null(), "host ICU missing {name:?}");
        pointer
    }
    fn version(&self, name: &str) -> [u8; 4] {
        let call: unsafe extern "C" fn(*mut u8) = unsafe { std::mem::transmute(self.symbol(name)) };
        let mut version = [0; 4];
        unsafe { call(version.as_mut_ptr()) };
        version
    }
    fn reset(&mut self) {
        self.calls.clear();
        self.sort_calls.clear();
    }
}
impl Drop for HostIcu {
    fn drop(&mut self) {
        for hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, *hook) });
        }
        assert_eq!(unsafe { dlclose(self.library) }, 0);
    }
}
fn guest_c_string(engine: Engine, address: u64) -> CString {
    let value: Vec<_> = (0..0x10000)
        .map(|index| bytes(engine, address + index, 1)[0])
        .take_while(|byte| *byte != 0)
        .collect();
    CString::new(value).unwrap()
}
#[link(name = "c")]
unsafe extern "C" {
    fn __errno_location() -> *mut i32;
    fn strtol(source: *const c_char, end: *mut *mut c_char, radix: i32) -> i64;
    fn gnu_get_libc_version() -> *const c_char;
    fn qsort(
        base: *mut c_void,
        count: usize,
        size: usize,
        compare: unsafe extern "C" fn(*const c_void, *const c_void) -> i32,
    );
}
struct ComparatorEngine {
    engine: Engine,
}
impl ComparatorEngine {
    fn new(machine: &Machine) -> Self {
        let mut engine = ptr::null_mut();
        check(unsafe { uc_open(2, 0, &mut engine) });
        for page in [0xbf000, 0xdd000] {
            check(unsafe { uc_mem_map(engine, TEXT + page, 0x1000, 7) });
            write(
                engine,
                TEXT + page,
                &bytes(machine.engine, TEXT + page, 0x1000),
            );
        }
        check(unsafe { uc_mem_map(engine, 0x20000000, 0x1000, 7) });
        Self { engine }
    }
}
impl Drop for ComparatorEngine {
    fn drop(&mut self) {
        check(unsafe { uc_close(self.engine) });
    }
}
#[derive(Clone, Copy)]
struct GuestSort {
    engine: Engine,
    comparator: u64,
    size: usize,
}
thread_local! { static SORT_COMPARISONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) }; }
thread_local! { static GUEST_SORT: std::cell::Cell<Option<GuestSort>> = const { std::cell::Cell::new(None) }; }
unsafe extern "C" fn compare_guest(left: *const c_void, right: *const c_void) -> i32 {
    SORT_COMPARISONS.with(|count| count.set(count.get() + 1));
    let sort = GUEST_SORT.with(|sort| sort.get().unwrap());
    let left_buffer = 0x20000000;
    let right_buffer = 0x20000100;
    let stop = 0x20000f00;
    write(sort.engine, left_buffer, unsafe {
        std::slice::from_raw_parts(left.cast::<u8>(), sort.size)
    });
    write(sort.engine, right_buffer, unsafe {
        std::slice::from_raw_parts(right.cast::<u8>(), sort.size)
    });
    register(sort.engine, REGISTER_X0, left_buffer);
    register(sort.engine, REGISTER_X0 + 1, right_buffer);
    register(sort.engine, REGISTER_X30, stop);
    check(unsafe { uc_emu_start(sort.engine, sort.comparator, stop, 0, 100) });
    assert_eq!(read_register(sort.engine, 260), stop);
    read_register(sort.engine, REGISTER_X0) as i32
}
fn sort_guest(engine: Engine, args: [u64; 8], comparison_engine: Engine) -> u64 {
    let count = args[1] as usize;
    let size = args[2] as usize;
    assert!(count <= 4096 && size > 0);
    assert!(matches!(
        (args[3] - TEXT, size),
        (0xbfe84, 28) | (0xbfebc, 8) | (0xdd250, 12)
    ));
    let mut records = bytes(engine, args[0], count.checked_mul(size).unwrap());
    GUEST_SORT.with(|state| {
        assert!(
            state
                .replace(Some(GuestSort {
                    engine: comparison_engine,
                    comparator: args[3],
                    size
                }))
                .is_none()
        )
    });
    unsafe { qsort(records.as_mut_ptr().cast(), count, size, compare_guest) };
    GUEST_SORT.with(|state| assert!(state.replace(None).is_some()));
    write(engine, args[0], &records);
    0
}

fn host_import(engine: Engine, name: &str, args: [u64; 8], data: *mut c_void) -> Option<u64> {
    let first = args[0];
    match name {
        "_ZNSt6__ndk111__call_onceERVmPvPFvS2_E" => {
            let first_call = read_u64(engine, first) == 0;
            if first_call {
                write(engine, first, &u64::MAX.to_le_bytes());
            }
            Some(u64::from(first_call))
        }
        "__errno" => Some(MODEL + 0x18fe0),
        "strtol" => {
            let source = guest_c_string(engine, args[0]);
            let mut end = ptr::null_mut();
            unsafe { *__errno_location() = read_u32(engine, MODEL + 0x18fe0) as i32 };
            let result = unsafe { strtol(source.as_ptr(), &mut end, args[2] as i32) };
            let offset = unsafe { end.offset_from(source.as_ptr()) };
            assert!(offset >= 0 && offset as usize <= source.as_bytes().len());
            if args[1] != 0 {
                write(engine, args[1], &(args[0] + offset as u64).to_le_bytes());
            }
            write(
                engine,
                MODEL + 0x18fe0,
                &unsafe { *__errno_location() }.to_le_bytes(),
            );
            Some(result as u64)
        }
        "qsort" => {
            let comparator = unsafe { (*data.cast::<HostIcu>()).comparator.engine };
            SORT_COMPARISONS.with(|count| count.set(0));
            let result = sort_guest(engine, args, comparator);
            let comparisons = SORT_COMPARISONS.with(|count| count.get());
            unsafe {
                (*data.cast::<HostIcu>()).sort_calls.push([
                    args[3] - TEXT,
                    args[1],
                    args[2],
                    comparisons,
                ]);
            }
            Some(result)
        }
        "__cxa_guard_acquire" => Some(u64::from(bytes(engine, first, 1)[0] & 1 == 0)),
        "__cxa_guard_release" => {
            write(engine, first, &[1]);
            Some(0)
        }
        "_ZNSt6__ndk115recursive_mutexC1Ev"
        | "_ZNSt6__ndk115recursive_mutexC2Ev"
        | "_ZNSt6__ndk115recursive_mutex4lockEv"
        | "_ZNSt6__ndk115recursive_mutex6unlockEv"
        | "_ZNSt6__ndk15mutex4lockEv"
        | "_ZNSt6__ndk15mutex6unlockEv" => Some(0),
        "__vsprintf_chk" => {
            let format = guest_c_string(engine, args[3]).into_string().unwrap();
            assert_eq!(format, "%s_%d");
            let top = read_u64(engine, args[4] + 8);
            let offset = read_u32(engine, args[4] + 24) as i32;
            assert!(offset <= -16);
            let slots = top.wrapping_add_signed(i64::from(offset));
            let name = guest_c_string(engine, read_u64(engine, slots))
                .into_string()
                .unwrap();
            let version = read_u64(engine, slots + 8) as i32;
            let result = format!("{name}_{version}");
            assert!(result.len() + 1 <= args[2] as usize);
            write(engine, args[0], result.as_bytes());
            write(engine, args[0] + result.len() as u64, &[0]);
            Some(result.len() as u64)
        }

        "dlopen" => {
            let name = guest_c_string(engine, args[0]).into_string().unwrap();
            assert!(matches!(
                name.as_str(),
                "/apex/com.android.i18n/lib64/libicuuc.so"
                    | "/apex/com.android.art/lib64/libicuuc.so"
                    | "/apex/com.android.runtime/lib64/libicuuc.so"
                    | "/system/lib64/libicuuc.so"
            ));
            assert_eq!(args[1], 2);
            Some(1)
        }
        "dlsym" => {
            let host = unsafe { &mut *data.cast::<HostIcu>() };
            let name = guest_c_string(engine, args[1]).into_string().unwrap();
            let name = if let Some((base, version)) = name
                .rsplit_once('_')
                .filter(|(_, version)| version.parse::<u32>().is_ok())
            {
                if version != "76" {
                    return Some(0);
                }
                base.to_owned()
            } else {
                return Some(0);
            };
            let _ = host.symbol(&name);
            if let Some(address) = host.addresses.get(&name) {
                return Some(*address);
            }
            assert!(host.addresses.len() < 1024);
            let address = 0x07008000 + host.addresses.len() as u64 * 32;
            host.addresses.insert(name.clone(), address);
            host.names.insert(address, name);
            write(engine, address, &0xd65f03c0_u32.to_le_bytes());
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    engine,
                    &mut hook,
                    4,
                    icu_call as *mut c_void,
                    data,
                    address,
                    address,
                )
            });
            host.hooks.push(hook);
            Some(address)
        }
        "scalbnf" | "sqrtf" | "floorf" | "ceilf" => {
            let value = f32::from_bits(read_register(engine, 136) as u32);
            let result = unsafe {
                match name {
                    "scalbnf" => scalbnf(value, first as i32),
                    "sqrtf" => sqrtf(value),
                    "floorf" => floorf(value),
                    _ => ceilf(value),
                }
            };
            register(engine, 136, u64::from(result.to_bits()));
            Some(0)
        }
        _ => None,
    }
}
unsafe extern "C" fn icu_call(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let host = unsafe { &mut *data.cast::<HostIcu>() };
    let name = host.names.get(&address).unwrap().clone();
    host.calls.push(name.clone());
    let function = host.symbol(&name);
    let args: [u64; 8] =
        std::array::from_fn(|index| read_register(engine, REGISTER_X0 + index as i32));
    let a = args[0] as i32;
    let b = args[1] as i32;
    let result = unsafe {
        match name.as_str() {
            "u_charType" => {
                let call: unsafe extern "C" fn(i32) -> i8 = std::mem::transmute(function);
                call(a) as i64 as u64
            }
            "u_charDirection" => {
                let call: unsafe extern "C" fn(i32) -> i32 = std::mem::transmute(function);
                call(a) as i64 as u64
            }
            "u_getCombiningClass" => {
                let call: unsafe extern "C" fn(i32) -> u8 = std::mem::transmute(function);
                u64::from(call(a))
            }
            "u_hasBinaryProperty" => {
                let call: unsafe extern "C" fn(i32, i32) -> i8 = std::mem::transmute(function);
                call(a, b) as i64 as u64
            }
            "u_getIntPropertyValue" => {
                let call: unsafe extern "C" fn(i32, i32) -> i32 = std::mem::transmute(function);
                call(a, b) as i64 as u64
            }
            "u_getIntPropertyMaxValue" | "u_charMirror" | "ublock_getCode" => {
                let call: unsafe extern "C" fn(i32) -> i32 = std::mem::transmute(function);
                call(a) as i64 as u64
            }
            "u_isalpha"
            | "u_isalnum"
            | "u_islower"
            | "u_isupper"
            | "u_isspace"
            | "uscript_isRightToLeft" => {
                let call: unsafe extern "C" fn(i32) -> i8 = std::mem::transmute(function);
                call(a) as i64 as u64
            }
            "u_tolower" | "u_toupper" => {
                let call: unsafe extern "C" fn(i32) -> i32 = std::mem::transmute(function);
                call(a) as i64 as u64
            }
            "uscript_getScript" => {
                let call: unsafe extern "C" fn(i32, *mut i32) -> i32 =
                    std::mem::transmute(function);
                let mut error = read_u32(engine, args[1]) as i32;
                let value = call(a, &mut error);
                write(engine, args[1], &error.to_le_bytes());
                value as i64 as u64
            }
            "uloc_toLanguageTag" => {
                let call: unsafe extern "C" fn(
                    *const c_char,
                    *mut c_char,
                    i32,
                    i8,
                    *mut i32,
                ) -> i32 = std::mem::transmute(function);
                let source = guest_c_string(engine, args[0]);
                let capacity = args[2] as usize;
                assert!(capacity <= 4096);
                let mut target = bytes(engine, args[1], capacity);
                let mut error = read_u32(engine, args[4]) as i32;
                let result = call(
                    source.as_ptr(),
                    if args[1] == 0 {
                        ptr::null_mut()
                    } else {
                        target.as_mut_ptr().cast()
                    },
                    capacity as i32,
                    args[3] as i8,
                    &mut error,
                );
                if !target.is_empty() {
                    write(engine, args[1], &target);
                }
                write(engine, args[4], &error.to_le_bytes());
                result as i64 as u64
            }
            "uloc_canonicalize"
            | "uloc_addLikelySubtags"
            | "uloc_getName"
            | "uloc_forLanguageTag" => {
                let source = guest_c_string(engine, args[0]);
                let capacity = args[2] as usize;
                assert!(capacity <= 4096);
                let mut target = bytes(engine, args[1], capacity);
                let error_pointer = if name == "uloc_forLanguageTag" {
                    args[4]
                } else {
                    args[3]
                };
                let mut error = read_u32(engine, error_pointer) as i32;
                let result = if name == "uloc_forLanguageTag" {
                    let call: unsafe extern "C" fn(
                        *const c_char,
                        *mut c_char,
                        i32,
                        *mut i32,
                        *mut i32,
                    ) -> i32 = std::mem::transmute(function);
                    let mut parsed = if args[3] == 0 {
                        0
                    } else {
                        read_u32(engine, args[3]) as i32
                    };
                    let result = call(
                        source.as_ptr(),
                        if args[1] == 0 {
                            ptr::null_mut()
                        } else {
                            target.as_mut_ptr().cast()
                        },
                        capacity as i32,
                        if args[3] == 0 {
                            ptr::null_mut()
                        } else {
                            &mut parsed
                        },
                        &mut error,
                    );
                    if args[3] != 0 {
                        write(engine, args[3], &parsed.to_le_bytes());
                    }
                    result
                } else {
                    let call: unsafe extern "C" fn(
                        *const c_char,
                        *mut c_char,
                        i32,
                        *mut i32,
                    ) -> i32 = std::mem::transmute(function);
                    call(
                        source.as_ptr(),
                        if args[1] == 0 {
                            ptr::null_mut()
                        } else {
                            target.as_mut_ptr().cast()
                        },
                        capacity as i32,
                        &mut error,
                    )
                };
                if !target.is_empty() {
                    write(engine, args[1], &target);
                }
                write(engine, error_pointer, &error.to_le_bytes());
                result as i64 as u64
            }
            _ => panic!("unsupported actually-called host ICU adapter {name} ({args:x?})"),
        }
    };
    register(engine, REGISTER_X0, result);
}

fn infos_json(engine: Engine, buffer: u64, output: bool) -> String {
    let count = read_u32(engine, buffer + 96) as usize;
    assert!(count <= 4096);
    let info = read_u64(engine, buffer + 112);
    let key = if output { "glyph_id" } else { "codepoint" };
    format!(
        "[{}]",
        (0..count)
            .map(|i| format!(
                "{{\"{key}\":{},\"cluster\":{}}}",
                read_u32(engine, info + i as u64 * 20),
                read_u32(engine, info + i as u64 * 20 + 8)
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn context(engine: Engine, buffer: u64, offset: u64, count_offset: u64) -> Vec<u32> {
    let count = read_u32(engine, buffer + count_offset) as usize;
    assert!(count <= 5);
    (0..count)
        .map(|i| read_u32(engine, buffer + offset + i as u64 * 4))
        .collect()
}
#[derive(Default)]
struct ShapeTrace {
    calls: Vec<String>,
    pending: Option<String>,
    scalar_glyphs: Vec<u32>,
    scalar_raw_bits: Vec<u32>,
    scalar_quantized: Vec<i32>,
    vector_glyphs: Vec<Vec<u32>>,
    vector_raw_bits: Vec<u32>,
    vector_quantized: Vec<i32>,
    bounds: Vec<[u32; 4]>,
}
unsafe extern "C" fn trace_shape(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let trace = unsafe { &mut *data.cast::<ShapeTrace>() };
    match address - TEXT {
        0xecdcc => {
            assert!(trace.pending.is_none());
            let font = read_register(engine, REGISTER_X0);
            assert_eq!(
                read_u32(engine, read_register(engine, REGISTER_X0 + 1) + 52),
                1
            );
            let buffer = read_register(engine, REGISTER_X0 + 1);
            let feature = read_register(engine, REGISTER_X0 + 2);
            let count = read_register(engine, REGISTER_X0 + 3) as usize;
            assert!(count <= 256);
            let features = (0..count)
                .map(|i| {
                    let base = feature + i as u64 * 16;
                    format!(
                        "{{\"tag\":{},\"value\":{},\"start\":{},\"end\":{}}}",
                        read_u32(engine, base),
                        read_u32(engine, base + 4),
                        read_u32(engine, base + 8),
                        read_u32(engine, base + 12)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let language = read_u64(engine, buffer + 64);
            let language = if language == 0 {
                "null".to_owned()
            } else {
                json_string(guest_c_string(engine, language).to_str().unwrap())
            };
            trace.pending = Some(format!(
                "{{\"nativefont_scale\":{:?},\"nativefont_ppem\":{:?},\"input\":{{\"infos\":{},\"direction\":{},\"script\":{},\"language\":{},\"flags\":{},\"cluster_level\":{},\"content_type\":{},\"pre_context\":{:?},\"post_context\":{:?},\"features\":[{}]}},",
                [
                    read_u32(engine, font + 32) as i32,
                    read_u32(engine, font + 36) as i32
                ],
                [read_u32(engine, font + 56), read_u32(engine, font + 60)],
                infos_json(engine, buffer, false),
                read_u32(engine, buffer + 56),
                read_u32(engine, buffer + 60),
                language,
                read_u32(engine, buffer + 24),
                read_u32(engine, buffer + 28),
                read_u32(engine, buffer + 52),
                context(engine, buffer, 140, 180),
                context(engine, buffer, 160, 184),
                features
            ));
        }
        0x9c764 => {
            let buffer = read_u64(engine, read_register(engine, REGISTER_SP) + 240);
            let count = read_u32(engine, buffer + 96) as usize;
            assert!(count <= 4096);
            let position = read_u64(engine, buffer + 128);
            let positions: Vec<[i32; 4]> = (0..count)
                .map(|i| {
                    std::array::from_fn(|j| {
                        read_u32(engine, position + i as u64 * 20 + j as u64 * 4) as i32
                    })
                })
                .collect();
            assert_eq!(read_u32(engine, buffer + 52), 2);
            let prefix = trace.pending.take().unwrap();
            trace.calls.push(format!(
                "{prefix}\"output\":{{\"infos\":{},\"positions\":{:?},\"content_type\":{}}}}}",
                infos_json(engine, buffer, true),
                positions,
                read_u32(engine, buffer + 52)
            ));
        }
        0x9d728 => trace
            .scalar_glyphs
            .push(read_register(engine, REGISTER_X0 + 2) as u32),
        0x9d74c => trace
            .scalar_raw_bits
            .push(read_register(engine, 136) as u32),
        0x9d768 => trace
            .scalar_quantized
            .push(read_register(engine, REGISTER_X0) as i32),
        0x9d770 => {
            let count = read_register(engine, REGISTER_X0 + 2) as usize;
            let glyphs = read_register(engine, REGISTER_X0 + 3);
            let stride = read_register(engine, REGISTER_X0 + 4);
            assert!(count <= 4096 && stride <= 64);
            trace.vector_glyphs.push(
                (0..count)
                    .map(|i| read_u32(engine, glyphs + i as u64 * stride))
                    .collect(),
            );
        }
        0x9d858 => trace
            .vector_raw_bits
            .push(read_u32(engine, read_register(engine, REGISTER_X0 + 22))),
        0x9d86c => trace
            .vector_quantized
            .push(read_register(engine, REGISTER_X0 + 8) as i32),
        0x9cc5c => trace.bounds.push(std::array::from_fn(|i| {
            read_u32(engine, read_register(engine, 1) - 240 + i as u64 * 4)
        })),
        _ => unreachable!(),
    }
}
struct TraceRecorder {
    engine: Engine,
    hooks: Vec<usize>,
    state: Box<ShapeTrace>,
}
impl TraceRecorder {
    fn new(machine: &Machine) -> Self {
        let mut recorder = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            state: Box::default(),
        };
        for offset in [
            0xecdcc, 0x9c764, 0x9d728, 0x9d74c, 0x9d768, 0x9d770, 0x9d858, 0x9d86c, 0x9cc5c,
        ] {
            let address = TEXT + offset;
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    trace_shape as *mut c_void,
                    ptr::from_mut(recorder.state.as_mut()).cast(),
                    address,
                    address,
                )
            });
            recorder.hooks.push(hook);
        }
        recorder
    }
}
impl Drop for TraceRecorder {
    fn drop(&mut self) {
        for hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, *hook) });
        }
    }
}

fn vector(engine: Engine, address: u64, storage: u64, length: usize) {
    for (offset, value) in [
        (0, storage),
        (8, storage + length as u64),
        (16, storage + length as u64),
    ] {
        write(engine, address + offset, &value.to_le_bytes());
    }
}

struct Case {
    name: &'static str,
    text: &'static str,
    font_size: f32,
    flags: u32,
    rtl: bool,
    range: Option<[u32; 2]>,
    skew: f32,
    letter_spacing: f32,
    features: &'static str,
}
impl Case {
    fn regular(name: &'static str, text: &'static str, size: f32) -> Self {
        Self {
            name,
            text,
            font_size: size,
            flags: 0x20000,
            rtl: false,
            range: None,
            skew: 0.0,
            letter_spacing: 0.0,
            features: "",
        }
    }
    fn execute(
        &self,
        machine: &mut Machine,
        environment: &mut NativeFontEnvironment,
        trace: &mut TraceRecorder,
        host: &mut HostIcu,
        fill: u8,
    ) -> String {
        host.reset();
        *trace.state = ShapeTrace::default();
        environment.reset(machine, fill, 0);
        write(machine.engine, FONT, &[0; 0x1000]);
        environment.construct_font(machine, FONT, "", 400, false);
        let font_impl = read_u64(machine.engine, FONT + 8);
        assert_eq!(machine.call(TEXT + 0x87824, &[font_impl]), 400);
        assert_eq!(machine.call(TEXT + 0x8782c, &[font_impl]), 0);
        let source_id = machine.call(TEXT + 0x85d7c, &[FONT]);
        let bitmap = machine.call(TEXT + 0x85d98, &[FONT]);
        let font_language = machine.call(TEXT + 0x85db0, &[FONT]);
        assert_eq!(bytes(machine.engine, font_language, 1), vec![0]);
        assert_eq!(bitmap, 0);
        for initializer in INITIALIZERS {
            machine.call(TEXT + initializer, &[]);
        }
        write(machine.engine, FONT_ITEM, &FONT.to_le_bytes());
        vector(machine.engine, FONT_VECTOR, FONT_ITEM, 16);
        write(machine.engine, LOCALE, b"en-Latn\0");
        machine.call(TEXT + 0x8609c, &[FAMILY, LOCALE, 0, FONT_VECTOR]);
        let implementation = read_u64(machine.engine, FAMILY + 8);
        write(
            machine.engine,
            FAMILY_ITEM,
            &bytes(machine.engine, implementation + 8, 16),
        );
        vector(machine.engine, FAMILY_VECTOR, FAMILY_ITEM, 16);
        machine.call(TEXT + 0x8f7a8, &[COLLECTION, FAMILY_VECTOR]);
        let paint_size = self.font_size * 100.0;
        write(machine.engine, PAINT, &paint_size.to_bits().to_le_bytes());
        write(machine.engine, PAINT + 4, &1_f32.to_bits().to_le_bytes());
        write(
            machine.engine,
            PAINT + 8,
            &self.skew.to_bits().to_le_bytes(),
        );
        write(
            machine.engine,
            PAINT + 12,
            &self.letter_spacing.to_bits().to_le_bytes(),
        );
        write(machine.engine, PAINT + 20, &self.flags.to_le_bytes());
        write(machine.engine, PAINT + 28, &400_u16.to_le_bytes());
        let features = MODEL + 0x18f00;
        write(machine.engine, features, self.features.as_bytes());
        write(machine.engine, features + self.features.len() as u64, &[0]);
        machine.call(TEXT + 0x77e2c, &[PAINT + 40, features]);
        write(machine.engine, PAINT + 64, &COLLECTION.to_le_bytes());
        let source: Vec<_> = self.text.encode_utf16().collect();
        assert!(source.len() <= 128);
        for (index, code) in source.iter().enumerate() {
            write(
                machine.engine,
                UTF16 + index as u64 * 2,
                &code.to_le_bytes(),
            );
        }
        write(machine.engine, VIEW, &UTF16.to_le_bytes());
        write(
            machine.engine,
            VIEW + 8,
            &(source.len() as u32).to_le_bytes(),
        );
        let range = self.range.unwrap_or([0, source.len() as u32]);
        write(machine.engine, RANGE, &range[0].to_le_bytes());
        write(machine.engine, RANGE + 4, &range[1].to_le_bytes());
        machine.call(
            TEXT + 0x9b150,
            &[PIECE, VIEW, RANGE, u64::from(self.rtl), PAINT, 0, 0],
        );
        assert!(trace.state.pending.is_none());
        let read_vector = |offset: u64, width: usize| {
            let start = read_u64(machine.engine, PIECE + offset);
            let end = read_u64(machine.engine, PIECE + offset + 8);
            assert!(end >= start && (end - start) as usize % width == 0 && end - start <= 65536);
            bytes(machine.engine, start, (end - start) as usize)
        };
        let words = |offset| {
            read_vector(offset, 4)
                .chunks_exact(4)
                .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
                .collect::<Vec<_>>()
        };
        let points = |offset| {
            read_vector(offset, 8)
                .chunks_exact(8)
                .map(|v| {
                    std::array::from_fn::<_, 2, _>(|i| {
                        u32::from_le_bytes(v[i * 4..i * 4 + 4].try_into().unwrap())
                    })
                })
                .collect::<Vec<_>>()
        };
        let rectangles: Vec<[u32; 4]> = read_vector(120, 16)
            .chunks_exact(16)
            .map(|v| {
                std::array::from_fn(|i| u32::from_le_bytes(v[i * 4..i * 4 + 4].try_into().unwrap()))
            })
            .collect();
        let fakery: Vec<u16> = read_vector(184, 16)
            .chunks_exact(16)
            .map(|record| u16::from_le_bytes(record[8..10].try_into().unwrap()))
            .collect();
        assert_eq!(
            trace.state.vector_raw_bits.len(),
            trace.state.vector_quantized.len()
        );
        assert_eq!(
            trace.state.vector_raw_bits.len(),
            trace.state.vector_glyphs.iter().map(Vec::len).sum()
        );
        assert_eq!(trace.state.bounds.len(), words(24).len());
        let mut counts = BTreeMap::new();
        for name in &host.calls {
            *counts.entry(name).or_insert(0_u32) += 1;
        }
        let counts = counts
            .iter()
            .map(|(name, count)| format!("{}:{count}", json_string(name)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"name\":{},\"text_utf8\":{},\"range_utf16\":{:?},\"font_size_bits\":{},\"font_source\":{{\"id\":{},\"bitmap\":false,\"language\":\"\",\"face_index\":0}},\"family_locale\":\"en-Latn\",\"hyphen_edits\":[0,0],\"paint\":{{\"size_bits\":{},\"scale_x_bits\":{},\"skew_x_bits\":{},\"letter_spacing_bits\":{},\"word_spacing_bits\":0,\"packed_flags\":{},\"locale_list_id\":0,\"weight\":400,\"italic\":false,\"variant\":0,\"feature_settings\":{}}},\"hb_calls\":[{}],\"layout_piece\":{{\"font_indices\":{:?},\"glyph_ids\":{:?},\"full_positions_bits\":{:?},\"owner_positions_bits\":{:?},\"owners_utf16\":{:?},\"ink_bounds_bits\":{:?},\"character_advances_bits\":{:?},\"total_advance_bits\":{},\"extent_bits\":{:?},\"font_fakery_bits\":{:?}}},\"callbacks\":{{\"scalar_glyphs\":{:?},\"scalar_raw_bits\":{:?},\"scalar_quantized\":{:?},\"vector_glyphs\":{:?},\"vector_raw_bits\":{:?},\"vector_quantized\":{:?},\"raw_skia_bounds_bits\":{:?}}},\"host_calls\":{{\"icu\":{{{}}},\"qsort\":{:?}}}}}",
            json_string(self.name),
            json_string(self.text),
            range,
            self.font_size.to_bits(),
            source_id,
            paint_size.to_bits(),
            1_f32.to_bits(),
            self.skew.to_bits(),
            self.letter_spacing.to_bits(),
            self.flags,
            json_string(self.features),
            trace.state.calls.join(","),
            read_vector(0, 1),
            words(24),
            points(48),
            points(72),
            words(96),
            rectangles,
            words(144),
            read_u32(machine.engine, PIECE + 168),
            [
                read_u32(machine.engine, PIECE + 172),
                read_u32(machine.engine, PIECE + 176)
            ],
            fakery,
            trace.state.scalar_glyphs,
            trace.state.scalar_raw_bits,
            trace.state.scalar_quantized,
            trace.state.vector_glyphs,
            trace.state.vector_raw_bits,
            trace.state.vector_quantized,
            trace.state.bounds,
            counts,
            host.sort_calls
        )
    }
}
fn pinned_host(path: &str, expected: &str) {
    let digest = Command::new("sha256sum").arg(path).output().unwrap();
    assert!(digest.status.success());
    assert_eq!(
        std::str::from_utf8(&digest.stdout)
            .unwrap()
            .split_whitespace()
            .next(),
        Some(expected)
    );
}
pub(super) fn capture(machine: &mut Machine, base: &Path, text: &Path, skia: &Path, font: &Path) {
    pinned_host(
        "/usr/lib/x86_64-linux-gnu/libicuuc.so.76.1",
        "a8e433e81075732faf255b17d4a25ce28632e41fef1a75e727ee7f4ed73ab151",
    );
    pinned_host(
        "/usr/lib/x86_64-linux-gnu/libicudata.so.76.1",
        "a04b2b906193fa1e40f968a3d16d7d6c844a1fafbdd5bce6e9f67b01c124ff24",
    );
    pinned_host(
        "/usr/lib/x86_64-linux-gnu/libc.so.6",
        "fa430b8f298f817a266046af84a77533185ad6fc4406c7d3787b5a0a0c207826",
    );
    pinned_host(
        "/usr/lib/x86_64-linux-gnu/libm.so.6",
        "6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3",
    );
    machine.call_timeout_micros = 0;
    machine.call_instruction_limit = 10_000_000;
    let mut environment = NativeFontEnvironment::new(machine, base, text, skia, font);
    let mut trace = TraceRecorder::new(machine);
    let mut host = HostIcu::new(machine);
    let icu_version = host.version("u_getVersion");
    let unicode_version = host.version("u_getUnicodeVersion");
    assert_eq!(icu_version, [76, 1, 0, 0]);
    let libc_version = unsafe { std::ffi::CStr::from_ptr(gnu_get_libc_version()) }
        .to_str()
        .unwrap();
    environment.set_host_import_handler(host_import, ptr::from_mut(host.as_mut()).cast());
    let mut cases = vec![
        Case::regular("av_default", "AV", 17.0),
        Case::regular("to_default", "To", 17.0),
        Case::regular("office_default", "office", 17.0),
        Case::regular("fi_default", "fi", 17.0),
        Case::regular("combining", "x\u{327}\u{301}y", 17.125),
        Case::regular("supplementary_combining", "A😀x\u{327}\u{301}B", 17.0),
        Case::regular("at_normalization_limit", "AV", 20.48),
        Case::regular("above_normalization", "AV", 20.49),
        Case::regular("large_default", "AV", 23.0),
    ];
    let mut nohint = Case::regular("av_nohint", "AV", 17.0);
    nohint.flags = 0;
    cases.push(nohint);
    let mut linear = Case::regular("av_linear", "AV", 17.0);
    linear.flags = 0x20040;
    cases.push(linear);
    let mut rtl = Case::regular("av_rtl", "AV", 17.0);
    rtl.rtl = true;
    cases.push(rtl);
    let mut context = Case::regular("partial_context", "xAVy", 17.0);
    context.range = Some([1, 3]);
    cases.push(context);
    let mut skew = Case::regular("combining_skew", "x\u{327}\u{301}y", 17.0);
    skew.skew = -0.25;
    cases.push(skew);
    let mut features = Case::regular("requested_ligatures", "office", 17.0);
    features.features = "liga=1,clig=1";
    cases.push(features);
    let mut spaced = Case::regular("letter_spacing", "AV", 17.0);
    spaced.letter_spacing = 0.04;
    cases.push(spaced);
    let outputs = cases
        .iter()
        .map(|case| {
            eprintln!("capture {}", case.name);
            let expected = case.execute(machine, &mut environment, &mut trace, &mut host, 0);
            for fill in [0xa5, 0xff, 0] {
                assert_eq!(
                    case.execute(machine, &mut environment, &mut trace, &mut host, fill),
                    expected,
                    "{} fill {fill}",
                    case.name
                );
            }
            expected
        })
        .collect::<Vec<_>>();
    println!(
        "{{\"font_sha256\":\"{}\",\"font_face_index\":0,\"memory_fills\":[0,165,255],\"repeat_zero_fill\":true,\"context_order\":{{\"pre\":\"nearest_first\",\"post\":\"source_order\"}},\"model_library_sha256\":\"{}\",\"base_library_sha256\":\"{}\",\"text_library_sha256\":\"{}\",\"skia_library_sha256\":\"{}\",\"host_icu_uc_sha256\":\"a8e433e81075732faf255b17d4a25ce28632e41fef1a75e727ee7f4ed73ab151\",\"host_icu_data_sha256\":\"a04b2b906193fa1e40f968a3d16d7d6c844a1fafbdd5bce6e9f67b01c124ff24\",\"host_libm_sha256\":\"6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3\",\"host_icu_version\":{:?},\"host_unicode_version\":{:?},\"host_libc_version\":{},\"resolved_icu_symbols\":[{}],\"native_initializers\":{:?},\"layout_piece_address\":\"0x9b150\",\"hb_shape_address\":\"0xecdcc\",\"instruction_limit\":10000000,\"native_icu_resolved_suffix\":76,\"host_libc_sha256\":\"fa430b8f298f817a266046af84a77533185ad6fc4406c7d3787b5a0a0c207826\",\"capture_boundary\":\"Actual file Font, FontFamily, FontCollection, LayoutPiece, bundled HarfBuzz Unicode tables and shaping, Skia and bundled FreeType execute on supplied pinned Roboto. Supplied MinikinPaint, locale en-Latn, range/direction and zero hyphen edits are caller inputs. Actual Android ICU dynamic loader resolves version 76 through its fallback search; dlopen routes to pinned host ICU76.1 code/data for property and locale calls, not bundled HB Unicode functions. Host libc qsort uses exact allowlisted leaf ARM64 comparators in a separate emulator. Host allocation/file/memory, single-thread synchronization, libc numeric feature parsing, formatter and libm calls are service boundaries. No Android device ICU/fontconfig selection, system fallback, whole SpanRunFunctor, all-library static initialization, bitmap/fallback fonts, wrapping/composition, canvas pixels, vector output or general Rust metric parity is established.\",\"cases\":[\n{}\n]}}",
        text_font_source::FONT_SHA256,
        LIBRARY_SHA256,
        frames::BASE_SHA256,
        geometry::TEXT_SHA256,
        text_font_source::SKIA_SHA256,
        icu_version,
        unicode_version,
        json_string(libc_version),
        host.addresses
            .keys()
            .map(|name| json_string(name))
            .collect::<Vec<_>>()
            .join(","),
        INITIALIZERS,
        outputs.join(",\n")
    );
}
