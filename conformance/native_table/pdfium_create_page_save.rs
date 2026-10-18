use super::*;
use std::collections::{BTreeSet, HashMap};

#[link(name = "m")]
unsafe extern "C" {
    fn frexpf(value: f32, exponent: *mut i32) -> f32;
    fn hypotf(left: f32, right: f32) -> f32;
}

#[link(name = "z")]
unsafe extern "C" {
    #[link_name = "compress"]
    fn host_compress(
        destination: *mut u8,
        destination_len: *mut u64,
        source: *const u8,
        source_len: u64,
    ) -> i32;
}

const PDFIUM_BASE: u64 = 0x0100_0000;
const LIBCPP_BASE: u64 = 0x0200_0000;
const STUB: u64 = 0x0800_0000;
const C_LOCALE: u64 = STUB + 0x9200;
const GLOBAL_C_LOCALE: u64 = STUB + 0x9210;
const C_LOCALE_NAME: u64 = STUB + 0x9220;
const TM_BUFFER: u64 = STUB + 0x9300;
const UTC_NAME: u64 = STUB + 0x9380;

struct External {
    names: HashMap<u64, String>,
    handled_imports: BTreeSet<String>,
    platform_events: BTreeSet<String>,
    current_locale: u64,
    posix_cursor: u64,
    host_allocs: HashMap<u64, u64>,
    tls: HashMap<u64, u64>,
    next_tls_key: u64,
    pdf_bytes: Vec<u8>,
    write_calls: usize,
}

fn cstring(engine: Engine, address: u64) -> String {
    let mut bytes = Vec::new();
    for offset in 0..256 {
        let byte = read_u64(engine, address + offset) as u8;
        if byte == 0 {
            break;
        }
        bytes.push(byte);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn cstring_bytes(engine: Engine, address: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    for offset in 0..1_000_000 {
        let byte = read_u64(engine, address + offset) as u8;
        if byte == 0 {
            return bytes;
        }
        bytes.push(byte);
    }
    panic!("unterminated C string at {address:#x}");
}

fn compare_c_strings(engine: Engine, left: u64, right: u64) -> i32 {
    let left = cstring_bytes(engine, left);
    let right = cstring_bytes(engine, right);
    let length = left.len().max(right.len());
    for index in 0..=length {
        let a = *left.get(index).unwrap_or(&0);
        let b = *right.get(index).unwrap_or(&0);
        if a != b {
            return i32::from(a) - i32::from(b);
        }
        if a == 0 {
            break;
        }
    }
    0
}

fn compare_memory(engine: Engine, left: u64, right: u64, length: usize) -> i32 {
    let mut a = vec![0; length];
    let mut b = vec![0; length];
    if length != 0 {
        check(unsafe { uc_mem_read(engine, left, a.as_mut_ptr().cast(), length) });
        check(unsafe { uc_mem_read(engine, right, b.as_mut_ptr().cast(), length) });
    }
    a.into_iter()
        .zip(b)
        .map(|(a, b)| i32::from(a) - i32::from(b))
        .find(|difference| *difference != 0)
        .unwrap_or(0)
}

fn va_integer_args(engine: Engine, va: u64, count: usize) -> Vec<u64> {
    let stack = read_u64(engine, va);
    let gr_top = read_u64(engine, va + 8);
    let mut gr_offs = read_u64(engine, va + 24) as i32;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        if gr_offs < 0 {
            values.push(read_u64(
                engine,
                (gr_top as i64 + i64::from(gr_offs)) as u64,
            ));
            gr_offs += 8;
        } else {
            values.push(read_u64(engine, stack + gr_offs as u64));
            gr_offs += 8;
        }
    }
    values
}

fn format_integer_printf(engine: Engine, format: &str, values: &[u64]) -> String {
    let mut output = String::new();
    let bytes = format.as_bytes();
    let mut offset = 0;
    let mut value_index = 0;
    while offset < bytes.len() {
        if bytes[offset] != b'%' {
            output.push(bytes[offset] as char);
            offset += 1;
            continue;
        }
        offset += 1;
        if bytes.get(offset) == Some(&b'%') {
            output.push('%');
            offset += 1;
            continue;
        }
        let zero_pad = bytes.get(offset) == Some(&b'0');
        if zero_pad {
            offset += 1;
        }
        let width_start = offset;
        while bytes.get(offset).is_some_and(u8::is_ascii_digit) {
            offset += 1;
        }
        let width = std::str::from_utf8(&bytes[width_start..offset])
            .unwrap_or("")
            .parse::<usize>()
            .unwrap_or(0);
        let long = if bytes.get(offset) == Some(&b'l') {
            offset += 1;
            true
        } else {
            false
        };
        let conversion = *bytes.get(offset).expect("printf conversion missing");
        offset += 1;
        if conversion == b'm' {
            output.push_str("Success");
            continue;
        }
        let value = *values.get(value_index).expect("printf vararg missing");
        value_index += 1;
        let rendered = match conversion {
            b'd' | b'i' => {
                let number = if long {
                    value as i64
                } else {
                    value as i32 as i64
                };
                if zero_pad && width > 0 {
                    format!("{number:0width$}")
                } else {
                    number.to_string()
                }
            }
            b'u' => {
                let number = if long { value } else { u64::from(value as u32) };
                if zero_pad && width > 0 {
                    format!("{number:0width$}")
                } else {
                    number.to_string()
                }
            }
            b'x' | b'X' => {
                let number = if long { value } else { u64::from(value as u32) };
                if conversion == b'X' {
                    format!("{number:X}")
                } else {
                    format!("{number:x}")
                }
            }
            b'c' => char::from_u32(value as u32)
                .unwrap_or('\u{fffd}')
                .to_string(),
            b's' => cstring(engine, value),
            other => panic!(
                "unimplemented native vsnprintf conversion {:?} in {format:?}",
                other as char
            ),
        };
        output.push_str(&rendered);
    }
    output
}

unsafe extern "C" fn host_external(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let external = unsafe { &mut *data.cast::<External>() };
    let name = external.names.get(&address).unwrap().clone();
    let x0 = read_register(engine, REGISTER_X0);
    let x1 = read_register(engine, REGISTER_X0 + 1);
    let x2 = read_register(engine, REGISTER_X0 + 2);
    external.handled_imports.insert(name.clone());
    let mut result = 0_u64;
    match name.trim_end_matches("@LIBC") {
        "memset" | "__memset_chk" => {
            let len = if name.starts_with("__memset_chk") {
                x3(engine)
            } else {
                x2
            } as usize;
            assert!(len < 16 * 1024 * 1024);
            write(engine, x0, &vec![x1 as u8; len]);
            result = x0;
        }
        "memcpy" | "memmove" | "__memcpy_chk" | "__memmove_chk" => {
            let len = if name.contains("_chk") {
                x3(engine)
            } else {
                x2
            } as usize;
            assert!(len < 16 * 1024 * 1024);
            let mut bytes = vec![0; len];
            check(unsafe { uc_mem_read(engine, x1, bytes.as_mut_ptr().cast(), len) });
            write(engine, x0, &bytes);
            result = x0;
        }
        "strlen" | "__strlen_chk" => {
            let mut length = 0;
            while length < 1_000_000 && read_u64(engine, x0 + length) as u8 != 0 {
                length += 1;
            }
            assert!(length < 1_000_000);
            result = length;
        }
        "memcmp" | "bcmp" => {
            let len = x2 as usize;
            assert!(len < 16 * 1024 * 1024, "bounded compare exceeded");
            result = compare_memory(engine, x0, x1, len) as i64 as u64;
        }
        "strcmp" => result = compare_c_strings(engine, x0, x1) as i64 as u64,
        "__system_property_get" => {
            let key = cstring(engine, x0);
            let is_sdk = key == "ro.build.version.sdk";
            external.platform_events.insert(format!(
                "android-property:{key}={}",
                if is_sdk { "35" } else { "empty" }
            ));
            let val = if is_sdk {
                b"35\0".as_slice()
            } else {
                b"\0".as_slice()
            };
            write(engine, x1, val);
            result = val.len().saturating_sub(1) as u64;
        }
        "pthread_key_create" => {
            let key = external.next_tls_key;
            external.next_tls_key += 1;
            write(engine, x0, &key.to_le_bytes());
        }
        "pthread_getspecific" => result = external.tls.get(&x0).copied().unwrap_or(0),
        "pthread_setspecific" => {
            external.tls.insert(x0, x1);
        }
        "frexpf" => {
            let value = f32::from_bits(read_register(engine, 136) as u32);
            let mut exponent = 0_i32;
            let fraction = unsafe { frexpf(value, &mut exponent) };
            write(engine, x0, &exponent.to_le_bytes());
            register(engine, 136, u64::from(fraction.to_bits()));
        }
        "hypotf" => {
            let left = f32::from_bits(read_register(engine, 136) as u32);
            let right = f32::from_bits(read_register(engine, 137) as u32);
            let result = unsafe { hypotf(left, right) };
            register(engine, 136, u64::from(result.to_bits()));
        }
        "compress" => {
            let source_len = x3(engine) as usize;
            let mut source = vec![0; source_len];
            if source_len != 0 {
                check(unsafe { uc_mem_read(engine, x2, source.as_mut_ptr().cast(), source_len) });
            }
            let mut destination_len = read_u64(engine, x1);
            let mut destination = vec![0; destination_len as usize];
            let status = unsafe {
                host_compress(
                    destination.as_mut_ptr(),
                    &mut destination_len,
                    source.as_ptr(),
                    source_len as u64,
                )
            };
            assert_eq!(status, 0, "host zlib compression failed");
            write(engine, x0, &destination[..destination_len as usize]);
            write(engine, x1, &destination_len.to_le_bytes());
            result = 0;
        }
        "realloc" => {
            let size = x1.max(1);
            assert!(size < 16 * 1024 * 1024, "bounded realloc size exceeded");
            let pointer = external.posix_cursor.checked_add(15).unwrap() & !15;
            external.posix_cursor = pointer.checked_add(size).unwrap();
            assert!(
                external.posix_cursor < 0x4100_0000,
                "bounded realloc arena exhausted"
            );
            if x0 != 0 {
                let old_size = external.host_allocs.get(&x0).copied().unwrap_or(0);
                let copy_len = old_size.min(size) as usize;
                if copy_len != 0 {
                    let mut bytes = vec![0; copy_len];
                    check(unsafe { uc_mem_read(engine, x0, bytes.as_mut_ptr().cast(), copy_len) });
                    write(engine, pointer, &bytes);
                }
            }
            external.host_allocs.insert(pointer, size);
            result = pointer;
        }
        "android_set_abort_message" => {
            eprintln!("abort message pointer: {x0:x}");
        }
        "syslog" => {
            eprintln!("syslog format: {}", cstring(engine, x1));
        }
        "fwrite" => {
            eprintln!("fwrite bytes: {}", cstring(engine, x0));
        }
        "vfprintf" | "vasprintf" => {
            eprintln!(
                "{name} format={} va={:x} [{:x},{:x},{:x},{:x},{:x}]",
                cstring(engine, x1),
                x2,
                read_u64(engine, x2),
                read_u64(engine, x2 + 8),
                read_u64(engine, x2 + 16),
                read_u64(engine, x2 + 24),
                read_u64(engine, x2 + 32)
            );
            let va0 = read_u64(engine, x2);
            let va1 = read_u64(engine, x2 + 8);
            let va2 = read_u64(engine, x2 + 16);
            eprintln!(
                "exception strings: {:?} {:?} {:?}",
                cstring(engine, va0),
                cstring(engine, va1),
                cstring(engine, va2)
            );
            let gr_top = read_u64(engine, x2 + 8);
            let gr_offs = read_u64(engine, x2 + 24) as i32;
            let args = (0..3)
                .map(|index| {
                    read_u64(
                        engine,
                        (gr_top as i64 + i64::from(gr_offs) + index * 8) as u64,
                    )
                })
                .collect::<Vec<_>>();
            eprintln!(
                "format varargs: {:?}",
                args.iter()
                    .map(|arg| cstring(engine, *arg))
                    .collect::<Vec<_>>()
            );
        }
        "vsnprintf" | "__vsnprintf_chk" => {
            // The shipped `__vsnprintf_chk` call uses the 6-register checked ABI
            // (dst, size, flag, object-size, fmt, va).
            // PDFium first makes a NULL/zero sizing call and then writes the same
            // formatted resource name through this checked entry point.
            let (format_pointer, va, output_size) = if name.starts_with("__vsnprintf_chk") {
                (
                    read_register(engine, REGISTER_X0 + 4),
                    read_register(engine, REGISTER_X0 + 5),
                    x1 as usize,
                )
            } else {
                (x2, x3(engine), x1 as usize)
            };
            let format = cstring(engine, format_pointer);
            let conversion_count = format.matches('%').count();
            let values = va_integer_args(engine, va, conversion_count);
            let formatted = format_integer_printf(engine, &format, &values);
            if x0 != 0 && output_size != 0 {
                let copy_len = formatted.len().min(output_size - 1);
                write(engine, x0, &formatted.as_bytes()[..copy_len]);
                write(engine, x0 + copy_len as u64, &[0]);
            }
            result = formatted.len() as u64;
        }
        "time" => {
            if x0 != 0 {
                write(engine, x0, &0_i64.to_le_bytes());
            }
            external.platform_events.insert("time:epoch-zero".into());
            result = 0;
        }
        "localtime" => {
            let timestamp = read_u64(engine, x0) as i64;
            assert_eq!(
                timestamp, 0,
                "deterministic localtime only admits epoch zero"
            );
            let mut tm = [0_u8; 56];
            tm[12..16].copy_from_slice(&1_i32.to_le_bytes()); // tm_mday
            tm[20..24].copy_from_slice(&70_i32.to_le_bytes()); // tm_year
            tm[24..28].copy_from_slice(&4_i32.to_le_bytes()); // tm_wday
            write(engine, TM_BUFFER, &tm);
            write(engine, UTC_NAME, b"UTC\0");
            write(engine, TM_BUFFER + 48, &UTC_NAME.to_le_bytes()); // tm_zone
            external
                .platform_events
                .insert("localtime:epoch-zero-utc".into());
            result = TM_BUFFER;
        }
        "newlocale" => {
            let locale = cstring(engine, x1);
            assert!(
                locale == "C" || locale == "POSIX",
                "unsupported locale {locale:?}"
            );
            assert_eq!(x0, 0x1fbf, "unexpected newlocale category mask");
            assert_eq!(
                x2, 0,
                "newlocale base-object mutation is outside this probe"
            );
            external
                .platform_events
                .insert(format!("newlocale:{locale}:mask=0x1fbf"));
            write(engine, C_LOCALE_NAME, b"C\0");
            result = C_LOCALE;
        }
        "uselocale" => {
            if x0 == 0 {
                result = external.current_locale;
            } else {
                assert!(x0 == C_LOCALE || x0 == GLOBAL_C_LOCALE);
                result = external.current_locale;
                external.current_locale = x0;
            }
        }
        "setlocale" => {
            if x1 != 0 {
                let locale = cstring(engine, x1);
                assert!(locale == "C" || locale == "POSIX");
                external
                    .platform_events
                    .insert(format!("setlocale:{locale}"));
            }
            write(engine, C_LOCALE_NAME, b"C\0");
            result = C_LOCALE_NAME;
        }
        "mbtowc" => {
            let source = x1;
            let max_bytes = x2;
            if source == 0 || max_bytes == 0 {
                result = 0;
            } else {
                let byte = read_u64(engine, source) as u8;
                if byte == 0 {
                    result = 0;
                } else if byte < 0x80 {
                    if x0 != 0 {
                        write(engine, x0, &u32::from(byte).to_le_bytes());
                    }
                    result = 1;
                } else {
                    result = (-1_i64) as u64;
                }
            }
        }
        "__ctype_get_mb_cur_max" => {
            external
                .platform_events
                .insert("ctype-mb-max:1-c-locale".into());
            result = 1;
        }
        "opendir" => {
            let path = cstring(engine, x0);
            external
                .platform_events
                .insert(format!("opendir-unavailable:{path}"));
            result = 0;
        }
        "syscall" => {
            assert_eq!(x0, 0xb2, "only AArch64 gettid is admitted");
            result = 1; // deterministic single-thread process identity
        }
        "pthread_mutex_lock" | "pthread_mutex_unlock" | "pthread_cond_broadcast" => {
            // This driver executes single-threaded; these synchronization calls
            // are explicit no-ops and provide no concurrency evidence.
            result = 0;
        }
        "__android_log_print"
        | "__cxa_atexit"
        | "__register_atfork"
        | "__cxa_thread_atexit_impl"
        | "pthread_rwlock_rdlock"
        | "pthread_rwlock_wrlock"
        | "pthread_rwlock_unlock"
        | "fflush" => result = 0,
        "getauxval" => {
            external
                .platform_events
                .insert("getauxval:unavailable".into());
            result = 0;
        }
        "sysconf" => result = 4096,
        "posix_memalign" => {
            let alignment = x1.max(16).next_power_of_two();
            let size = x2.max(1).checked_add(alignment - 1).unwrap() & !(alignment - 1);
            eprintln!(
                "posix_memalign align={} size={} x0={x0:x} x1={x1:x} x2={x2:x} lr={:x}",
                alignment,
                size,
                read_register(engine, REGISTER_X30)
            );
            let pointer =
                external.posix_cursor.checked_add(alignment - 1).unwrap() & !(alignment - 1);
            external.posix_cursor = pointer.checked_add(size).unwrap();
            assert!(
                external.posix_cursor < 0x4100_0000,
                "bounded posix allocation arena exhausted"
            );
            write(engine, pointer, &vec![0; size as usize]);
            write(engine, x0, &pointer.to_le_bytes());
            result = 0;
        }
        _ => {
            panic!("unhandled PDFium host import {name}: x0={x0:x} x1={x1:x} x2={x2:x}");
        }
    }
    register(engine, REGISTER_X0, result);
}
fn x3(engine: Engine) -> u64 {
    read_register(engine, REGISTER_X0 + 3)
}

unsafe extern "C" fn native_pdf_write_block(engine: Engine, _: u64, _: u32, data: *mut c_void) {
    let external = unsafe { &mut *data.cast::<External>() };
    let bytes = read_register(engine, REGISTER_X0 + 2) as usize;
    let source = read_register(engine, REGISTER_X0 + 1);
    assert!(bytes < 64 * 1024 * 1024, "bounded PDF write exceeded");
    let mut chunk = vec![0; bytes];
    if bytes != 0 {
        check(unsafe { uc_mem_read(engine, source, chunk.as_mut_ptr().cast(), bytes) });
    }
    external.pdf_bytes.extend_from_slice(&chunk);
    external.write_calls += 1;
    register(engine, REGISTER_X0, 1);
}

fn symbol_version(name: &str) -> &str {
    name.split('@').next().unwrap_or(name)
}

pub(super) fn capture(
    machine: &mut Machine,
    pdfium: &Path,
    source_rotation: u64,
    output_path: &Path,
) {
    map_library(
        machine.engine,
        pdfium,
        PDFIUM_BASE,
        "4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854",
    );
    let libcxx = pdfium.with_file_name("libc++_shared.so");
    map_library(
        machine.engine,
        &libcxx,
        LIBCPP_BASE,
        "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
    );
    check(unsafe { uc_mem_map(machine.engine, STUB, 0x10000, 7) });
    check(unsafe { uc_mem_map(machine.engine, 0x4000_0000, 0x0100_0000, 7) });
    // Minimal addressable stdio globals for error logging only.
    write(
        machine.engine,
        STUB + 0x9010,
        &(STUB + 0x9000).to_le_bytes(),
    );
    // Single-threaded pthread_once: execute the callback only on the first call.
    write(
        machine.engine,
        STUB + 0x80,
        &[
            0xf9, 0x00, 0x00, 0x08, // ldr x8, [x0]
            0xe8, 0x00, 0x00, 0xb5, // cbnz x8, done
            0xfd, 0x7b, 0xbf, 0xa9, // stp x29, x30, [sp, #-16]!
            0xfd, 0x03, 0x00, 0x91, // mov x29, sp
            0x28, 0x00, 0x80, 0xd2, // mov x8, #1
            0x08, 0x00, 0x00, 0xf9, // str x8, [x0]
            0x20, 0x00, 0x3f, 0xd6, // blr x1
            0xfd, 0x7b, 0xc1, 0xa8, // ldp x29, x30, [sp], #16
            0x00, 0x00, 0x80, 0xd2, // done: mov x0, #0
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ],
    );
    let symbols = Command::new("readelf")
        .args(["-Ws"])
        .arg(pdfium)
        .output()
        .unwrap();
    assert!(symbols.status.success());
    let mut local = HashMap::<String, u64>::new();
    for line in String::from_utf8(symbols.stdout).unwrap().lines() {
        let f = line.split_whitespace().collect::<Vec<_>>();
        if f.len() >= 8 && f[6] != "UND" {
            if let Ok(value) = u64::from_str_radix(f[1], 16) {
                local.entry(symbol_version(f[7]).into()).or_insert(value);
            }
        }
    }
    let cpp_symbols = Command::new("readelf")
        .args(["-Ws"])
        .arg(&libcxx)
        .output()
        .unwrap();
    assert!(cpp_symbols.status.success());
    let mut cpp_local = HashMap::<String, u64>::new();
    for line in String::from_utf8(cpp_symbols.stdout).unwrap().lines() {
        let f = line.split_whitespace().collect::<Vec<_>>();
        if f.len() >= 8 && f[6] != "UND" {
            if let Ok(value) = u64::from_str_radix(f[1], 16) {
                cpp_local
                    .entry(symbol_version(f[7]).into())
                    .or_insert(value);
            }
        }
    }
    let reloc = Command::new("readelf")
        .args(["-rW"])
        .arg(pdfium)
        .output()
        .unwrap();
    assert!(reloc.status.success());
    let mut external = Box::new(External {
        names: HashMap::new(),
        handled_imports: BTreeSet::new(),
        platform_events: BTreeSet::new(),
        current_locale: GLOBAL_C_LOCALE,
        posix_cursor: 0x4000_0000,
        host_allocs: HashMap::new(),
        tls: HashMap::new(),
        next_tls_key: 1,
        pdf_bytes: Vec::new(),
        write_calls: 0,
    });
    let mut next_stub = STUB + 0x100;
    write(machine.engine, STUB + 0xa0, &0xd65f03c0_u32.to_le_bytes());
    let mut write_hook = 0;
    check(unsafe {
        uc_hook_add(
            machine.engine,
            &mut write_hook,
            4,
            native_pdf_write_block as *mut c_void,
            ptr::from_mut(external.as_mut()).cast(),
            STUB + 0xa0,
            STUB + 0xa0,
        )
    });
    for line in String::from_utf8(reloc.stdout).unwrap().lines() {
        let f = line.split_whitespace().collect::<Vec<_>>();
        if f.len() < 6 || !f[2].contains("JUMP_SLOT") {
            continue;
        }
        let offset = u64::from_str_radix(f[0], 16).unwrap();
        let value = u64::from_str_radix(f[3], 16).unwrap_or(0);
        let name = symbol_version(f[4]);
        let target = if value != 0 {
            PDFIUM_BASE + value
        } else if local.contains_key(name) {
            PDFIUM_BASE + local[name]
        } else {
            match name {
                "_Znwm" | "_ZnwmRKSt9nothrow_t" | "malloc" | "calloc" => NEW,
                "_ZdlPv" | "_ZdaPv" | "_ZdlPvm" | "_ZdaPvm" | "free" => DELETE,
                "pthread_once" => STUB + 0x80,
                _ if cpp_local.contains_key(name) => LIBCPP_BASE + cpp_local[name],
                _ => {
                    let addr = next_stub;
                    next_stub += 16;
                    external.names.insert(addr, f[4].into());
                    write(machine.engine, addr, &0xd65f03c0_u32.to_le_bytes());
                    let mut hook = 0;
                    check(unsafe {
                        uc_hook_add(
                            machine.engine,
                            &mut hook,
                            4,
                            host_external as *mut c_void,
                            ptr::from_mut(external.as_mut()).cast(),
                            addr,
                            addr,
                        )
                    });
                    addr
                }
            }
        };
        // ELF jump slots are GOT entries: PLT stubs indirect through them.
        write(machine.engine, PDFIUM_BASE + offset, &target.to_le_bytes());
    }
    // Bind libc++'s own jump slots as well. Calls such as __next_prime are native
    // libc++; their internal imports (for example get_new_handler) must resolve too.
    let cpp_reloc = Command::new("readelf")
        .args(["-rW"])
        .arg(&libcxx)
        .output()
        .unwrap();
    assert!(cpp_reloc.status.success());
    for line in String::from_utf8(cpp_reloc.stdout).unwrap().lines() {
        let f = line.split_whitespace().collect::<Vec<_>>();
        if f.len() < 6 || !f[2].contains("JUMP_SLOT") {
            continue;
        }
        let offset = u64::from_str_radix(f[0], 16).unwrap();
        let value = u64::from_str_radix(f[3], 16).unwrap_or(0);
        let name = symbol_version(f[4]);
        let target = if value != 0 {
            LIBCPP_BASE + value
        } else if cpp_local.contains_key(name) {
            LIBCPP_BASE + cpp_local[name]
        } else {
            match name {
                "malloc" | "calloc" | "_Znwm" | "_ZnwmRKSt9nothrow_t" => NEW,
                "free" | "_ZdlPv" | "_ZdaPv" => DELETE,
                "pthread_once" => STUB + 0x80,
                _ => {
                    let addr = next_stub;
                    next_stub += 16;
                    external.names.insert(addr, f[4].into());
                    write(machine.engine, addr, &0xd65f03c0_u32.to_le_bytes());
                    let mut hook = 0;
                    check(unsafe {
                        uc_hook_add(
                            machine.engine,
                            &mut hook,
                            4,
                            host_external as *mut c_void,
                            ptr::from_mut(external.as_mut()).cast(),
                            addr,
                            addr,
                        )
                    });
                    addr
                }
            }
        };
        write(machine.engine, LIBCPP_BASE + offset, &target.to_le_bytes());
    }
    // Resolve undefined GLOB_DATs across the pinned DSOs, including cross-DSO
    // C++ facet/typeinfo data referenced by PDFium's iostream code.
    for (library, base, symbols) in [
        (libcxx.as_path(), LIBCPP_BASE, &cpp_local),
        (pdfium, PDFIUM_BASE, &local),
    ] {
        let relocations = Command::new("readelf")
            .args(["-rW"])
            .arg(library)
            .output()
            .unwrap();
        assert!(relocations.status.success());
        for line in String::from_utf8(relocations.stdout).unwrap().lines() {
            let f = line.split_whitespace().collect::<Vec<_>>();
            let is_glob_dat = f.get(2).is_some_and(|kind| kind.contains("GLOB_DAT"));
            let is_abs64 = f.get(2).is_some_and(|kind| kind.contains("ABS64"));
            if f.len() < 6 || !(is_glob_dat || is_abs64) {
                continue;
            }
            let offset = u64::from_str_radix(f[0], 16).unwrap();
            let symbol_value = u64::from_str_radix(f[3], 16).unwrap_or(0);
            if symbol_value != 0 {
                continue;
            }
            let name = symbol_version(f[4]);
            let target = if let Some(value) = symbols.get(name) {
                base + value
            } else if library == pdfium && cpp_local.contains_key(name) {
                LIBCPP_BASE + cpp_local[name]
            } else {
                match name {
                    "free" => DELETE,
                    "__sF" => STUB + 0x9000,
                    "stdout" => STUB + 0x9010,
                    _ => external
                        .names
                        .iter()
                        .find_map(|(address, imported)| {
                            (symbol_version(imported) == name).then_some(*address)
                        })
                        .unwrap_or_else(|| {
                            panic!(
                                "unresolved GLOB_DAT import {} in {}",
                                f[4],
                                library.display()
                            )
                        }),
                }
            };
            let addend = if is_abs64 && f.len() >= 7 {
                let amount = i64::from_str_radix(f[6], 16).unwrap_or(0);
                match f[5] {
                    "+" => amount,
                    "-" => -amount,
                    _ => 0,
                }
            } else {
                0
            };
            write(
                machine.engine,
                base + offset,
                &target.wrapping_add_signed(addend).to_le_bytes(),
            );
        }
    }
    register(machine.engine, REGISTER_TPIDR_EL0, TLS);

    // Recreate the shared-library initialization order for the C++ runtime and PDFium.
    for (array, count) in [
        (LIBCPP_BASE + 0x1417a8, 3_u64),
        (PDFIUM_BASE + 0x655990, 4_u64),
    ] {
        for index in 0..count {
            let initializer = read_u64(machine.engine, array + index * 8);
            if initializer != 0 && initializer != u64::MAX {
                machine.call(initializer, &[]);
            }
        }
    }

    machine.call(PDFIUM_BASE + 0x5a5504, &[]);
    let doc = machine.call(PDFIUM_BASE + 0x586484, &[]);
    assert_ne!(doc, 0, "FPDF_CreateNewDocument returned null");
    eprintln!("PDFium new document {doc:x}");
    // This pinned API receives the page dimensions in D0/D1 (float64), not X2/X3.
    register(machine.engine, 40, 200.0_f64.to_bits());
    register(machine.engine, 41, 150.0_f64.to_bits());
    let page = machine.call(PDFIUM_BASE + 0x586a10, &[doc, 0]);
    assert_ne!(page, 0, "FPDFPage_New returned null");
    eprintln!("PDFium new page {page:x}");
    machine.call(PDFIUM_BASE + 0x58937c, &[page, source_rotation]);
    let source_rotation = machine.call(PDFIUM_BASE + 0x586ddc, &[page]);
    let form_argument = 4_u64 - source_rotation;
    let object = machine.call(PDFIUM_BASE + 0x58a014, &[page, form_argument]);
    assert_ne!(object, 0, "FPDFPage_NewFormObj returned null");
    eprintln!("PDFium form object {object:x}, rotation {source_rotation}");

    // Build a real filled triangular note path, append it to the form, then execute
    // the native operations used by PdfiumExportHandler::CloseWriter.
    register(machine.engine, 136, f32::to_bits(21.5) as u64);
    register(machine.engine, 137, f32::to_bits(33.25) as u64);
    let path = machine.call(PDFIUM_BASE + 0x58a9b8, &[]);
    assert_ne!(path, 0);
    eprintln!("PDFium path object {path:x}");
    for (x, y) in [(91.5_f32, 44.25_f32), (56.75_f32, 128.5_f32)] {
        register(machine.engine, 136, f32::to_bits(x) as u64);
        register(machine.engine, 137, f32::to_bits(y) as u64);
        machine.call(PDFIUM_BASE + 0x58ada4, &[path]);
    }
    machine.call(PDFIUM_BASE + 0x58af08, &[path]);
    eprintln!("path closed");
    machine.call(PDFIUM_BASE + 0x58af70, &[path, 1, 1]);
    eprintln!("path draw mode set");
    machine.call(PDFIUM_BASE + 0x589504, &[path, 17, 92, 201, 255]);
    eprintln!("path fill color set");
    machine.call(PDFIUM_BASE + 0x58a7fc, &[object, path]);
    eprintln!("path inserted in form");
    let form_object_count = machine.call(PDFIUM_BASE + 0x589f68, &[object]);
    eprintln!("form object count {form_object_count}");
    machine.call(PDFIUM_BASE + 0x58a900, &[object]);
    eprintln!("form content stream generated");
    machine.call(PDFIUM_BASE + 0x587134, &[page, object]);
    eprintln!("form inserted into page");
    let content_result = machine.call(PDFIUM_BASE + 0x588ae4, &[page]);
    eprintln!("page content generated {content_result}");
    let page_object_count = machine.call(PDFIUM_BASE + 0x5874f8, &[page]);
    let rotation_after = machine.call(PDFIUM_BASE + 0x586ddc, &[page]);
    let filewrite = MODEL + 0x8000;
    let mut filewrite_struct = [0_u8; 16];
    filewrite_struct[..4].copy_from_slice(&1_u32.to_le_bytes());
    filewrite_struct[8..].copy_from_slice(&(STUB + 0xa0).to_le_bytes());
    write(machine.engine, filewrite, &filewrite_struct);
    // Samsung's PdfiumExportHandler::Export passes an opaque stack-local
    // adapter at argument four. Pinned FPDF_SaveAsCopy reads its +32 slot and
    // takes the null-hook branch when that field is zero. The adapter's full
    // layout and non-null hook behavior are intentionally not emulated here.
    let empty_facade = MODEL + 0xa000;
    write(machine.engine, empty_facade, &[0; 64]);
    let saved = machine.call(PDFIUM_BASE + 0x59cbbc, &[doc, filewrite, 0, empty_facade]);
    assert_eq!(saved, 1, "FPDF_SaveAsCopy failed");
    assert!(
        !external.pdf_bytes.is_empty(),
        "FPDF_FILEWRITE received no bytes"
    );
    std::fs::write(output_path, &external.pdf_bytes).unwrap();
    let handled_imports = external
        .handled_imports
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(",");
    let platform_events = external
        .platform_events
        .iter()
        .map(|event| format!("\"{event}\""))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"native_path\":\"FPDF_InitLibrary -> FPDF_CreateNewDocument -> FPDFPage_New -> FPDFPage_SetRotation/GetRotation -> FPDFPage_NewFormObj(counterrotation) -> native path create/line/close/fill -> FPDFFormObj_InsertObject/CountObjects/GenarateStream -> FPDFPage_InsertObject/GenerateContent -> FPDF_SaveAsCopy -> FPDF_FILEWRITE.WriteBlock\",\"doc\":{},\"page\":{},\"form_object\":{},\"path_object\":{},\"source_rotation\":{},\"writer_form_argument_4_minus_rotation\":{},\"rotation_after\":{},\"form_object_count\":{},\"page_object_count\":{},\"page_generate_content_result\":{},\"save_result\":{},\"pdf_bytes\":{},\"write_calls\":{},\"handled_imports\":[{}],\"platform_events\":[{}]}}",
        doc,
        page,
        object,
        path,
        source_rotation,
        form_argument,
        rotation_after,
        form_object_count,
        page_object_count,
        content_result,
        saved,
        external.pdf_bytes.len(),
        external.write_calls,
        handled_imports,
        platform_events
    );
}
