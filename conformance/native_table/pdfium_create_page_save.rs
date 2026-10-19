use super::*;
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::collections::{BTreeSet, HashMap};
use std::ffi::{c_char, c_int, c_uint, c_void, CStr};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;

#[link(name = "m")]
unsafe extern "C" {
    fn frexpf(value: f32, exponent: *mut i32) -> f32;
    fn hypotf(left: f32, right: f32) -> f32;
    fn sincosf(value: f32, sine: *mut f32, cosine: *mut f32);
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
    #[link_name = "inflateInit_"]
    fn host_inflate_init(stream: *mut ZStream, version: *const c_char, stream_size: c_int) -> c_int;
    #[link_name = "inflateInit2_"]
    fn host_inflate_init2(stream: *mut ZStream, window_bits: c_int, version: *const c_char, stream_size: c_int) -> c_int;
    #[link_name = "inflate"]
    fn host_inflate(stream: *mut ZStream, flush: c_int) -> c_int;
    #[link_name = "inflateEnd"]
    fn host_inflate_end(stream: *mut ZStream) -> c_int;
    #[link_name = "zlibVersion"]
    fn host_zlib_version() -> *const c_char;
    #[link_name = "crc32"]
    fn host_crc32(crc: u64, bytes: *const u8, length: c_uint) -> u64;
    #[link_name = "adler32"]
    fn host_adler32(adler: u64, bytes: *const u8, length: c_uint) -> u64;
}

const LIBZ_PATH: &str = "/usr/lib/x86_64-linux-gnu/libz.so.1";
const LIBZ_SHA256: &str = "85590dd58edf5445e18bc7193e5ebc01ac5841f1ae187e97705a662e90c6421e";
const INFLATE_CHUNK_LIMIT: usize = 16 * 1024 * 1024;
const INFLATE_STREAM_LIMIT: u64 = 64 * 1024 * 1024;
const INFLATE_TOTAL_LIMIT: u64 = 128 * 1024 * 1024;

type ZAlloc = Option<unsafe extern "C" fn(*mut c_void, c_uint, c_uint) -> *mut c_void>;
type ZFree = Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>;

// z_stream for the pinned x86_64 host zlib ABI. Guest callback pointers are
// either null or validated against pinned guest thunks, never invoked as host code.
#[repr(C)]
struct ZStream {
    next_in: *mut u8,
    avail_in: c_uint,
    total_in: u64,
    next_out: *mut u8,
    avail_out: c_uint,
    total_out: u64,
    msg: *mut c_char,
    state: *mut c_void,
    zalloc: ZAlloc,
    zfree: ZFree,
    opaque: *mut c_void,
    data_type: c_int,
    adler: u64,
    reserved: u64,
}

impl Default for ZStream {
    fn default() -> Self {
        Self {
            next_in: ptr::null_mut(), avail_in: 0, total_in: 0,
            next_out: ptr::null_mut(), avail_out: 0, total_out: 0,
            msg: ptr::null_mut(), state: ptr::null_mut(), zalloc: None,
            zfree: None, opaque: ptr::null_mut(), data_type: 0, adler: 0,
            reserved: 0,
        }
    }
}

struct InflateRecord {
    id: u64,
    stream: Box<ZStream>,
    zalloc_context: Option<Box<ZallocContext>>,
    guest_zalloc: u64,
    guest_zfree: u64,
    consumed: u64,
    produced: u64,
    calls: usize,
}

#[derive(Default)]
struct InflateSummary {
    id: u64,
    guest_zalloc: u64,
    guest_zfree: u64,
    consumed: u64,
    produced: u64,
    calls: usize,
}

#[derive(Default)]
struct ZallocLedger {
    allocations: HashMap<usize, (Layout, u64)>,
    active_bytes: usize,
    peak_bytes: usize,
}

struct ZallocContext {
    ledger: *mut ZallocLedger,
    stream_id: u64,
    zeroed: bool,
}

const PDFIUM_BASE: u64 = 0x0100_0000;
const LIBCPP_BASE: u64 = 0x0200_0000;
const SPENPDF_BASE: u64 = 0x0300_0000;
const SPENBASE_BASE: u64 = 0x0400_0000;
const MATRIX_DATA_BASE: u64 = 0x6400_0000;
const LIBPNG_BASE: u64 = 0x0500_0000;
const LIBPNG_SHA256: &str = "898abcc6ddbe3a4ad7e076484df666b732913db95a7abc7d6e99bfd835f555eb";
const PDF_WRITE_BLOCK_STUB: u64 = STUB + 0xf000;
const STUB: u64 = 0x0800_0000;
// Pinned PdfiumB offsets: its zalloc thunk forwards to zeroing CallocOrDie2D;
// zfree forwards to free. The pinned libpng png_zalloc/png_zfree callbacks are
// separately recognized below and bridged as ordinary malloc/free allocations.
// Neither bridge invokes a guest function pointer as a host C function.
const PDFIUM_ZALLOC: u64 = PDFIUM_BASE + 0x4e3270;
const PDFIUM_ZFREE: u64 = PDFIUM_BASE + 0x4e3280;
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
    files: HashMap<u64, std::fs::File>,
    next_file: u64,
    tls: HashMap<u64, u64>,
    next_tls_key: u64,
    clock_calls: u64,
    inflates: HashMap<u64, InflateRecord>,
    next_inflate_id: u64,
    inflate_total_produced: u64,
    completed_inflates: Vec<InflateSummary>,
    zalloc_ledger: ZallocLedger,
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

unsafe extern "C" fn zlib_host_alloc(
    opaque: *mut c_void,
    items: c_uint,
    size: c_uint,
) -> *mut c_void {
    let context = unsafe { &*opaque.cast::<ZallocContext>() };
    let Some(bytes) = (items as usize).checked_mul(size as usize) else {
        return ptr::null_mut();
    };
    if bytes > INFLATE_CHUNK_LIMIT {
        return ptr::null_mut();
    }
    let Ok(layout) = Layout::from_size_align(bytes.max(1), 16) else {
        return ptr::null_mut();
    };
    let pointer = unsafe {
        if context.zeroed { alloc_zeroed(layout) } else { std::alloc::alloc(layout) }
    };
    if pointer.is_null() {
        return pointer.cast();
    }
    let ledger = unsafe { &mut *context.ledger };
    assert!(
        ledger
            .allocations
            .insert(pointer as usize, (layout, context.stream_id))
            .is_none(),
        "host zlib allocator reused a live address"
    );
    ledger.active_bytes = ledger.active_bytes.checked_add(layout.size()).unwrap();
    assert!(ledger.active_bytes <= INFLATE_TOTAL_LIMIT as usize, "host zlib allocation budget exceeded");
    ledger.peak_bytes = ledger.peak_bytes.max(ledger.active_bytes);
    pointer.cast()
}

unsafe extern "C" fn zlib_host_free(opaque: *mut c_void, pointer: *mut c_void) {
    if pointer.is_null() {
        return;
    }
    let context = unsafe { &*opaque.cast::<ZallocContext>() };
    let ledger = unsafe { &mut *context.ledger };
    let (layout, stream_id) = ledger
        .allocations
        .remove(&(pointer as usize))
        .expect("host zlib freed an allocation not returned by its bridge");
    assert_eq!(stream_id, context.stream_id, "host zlib freed another stream's allocation");
    ledger.active_bytes = ledger.active_bytes.checked_sub(layout.size()).unwrap();
    unsafe { dealloc(pointer.cast(), layout) };
}

fn write_stat(engine: Engine, address: u64, size: u64) {
    let mut bytes = [0_u8; 128];
    bytes[16..20].copy_from_slice(&(0o100444_u32).to_le_bytes());
    bytes[48..56].copy_from_slice(&size.to_le_bytes());
    bytes[56..60].copy_from_slice(&(4096_u32).to_le_bytes());
    bytes[64..72].copy_from_slice(&size.div_ceil(512).to_le_bytes());
    write(engine, address, &bytes);
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
    if name == "inflateInit2_" {
        let caller = read_register(engine, REGISTER_X30);
        assert!((LIBPNG_BASE..LIBPNG_BASE + 0x34_000).contains(&caller), "successful overlay decode expected pinned libpng caller, got {caller:#x}");
        external.platform_events.insert(format!("pinned-libpng-inflateInit2-callsite:{caller:#x}"));
    }
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
        "setjmp" => {
            // Only the successful PNG decode branch is in scope; a subsequent
            // longjmp is rejected rather than turned into a normal return.
            external.platform_events.insert("setjmp-initial-zero-only".into());
            result = 0;
        }
        "longjmp" => panic!("libpng longjmp error recovery is outside the successful-decode capture"),
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
        "memchr" => {
            let len = x2 as usize;
            assert!(len < 16 * 1024 * 1024, "bounded memchr exceeded");
            result = (0..len)
                .find(|offset| read_u64(engine, x0 + *offset as u64) as u8 == x1 as u8)
                .map_or(0, |offset| x0 + offset as u64);
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
        "strchr" => {
            let bytes = cstring_bytes(engine, x0);
            let needle = x1 as u8;
            result = if let Some(index) = bytes.iter().position(|byte| *byte == needle) {
                x0 + index as u64
            } else if needle == 0 {
                x0 + bytes.len() as u64
            } else {
                0
            };
        }
        "atoi" => {
            let bytes = cstring_bytes(engine, x0);
            let mut index = 0;
            while bytes.get(index).is_some_and(u8::is_ascii_whitespace) { index += 1; }
            let sign = match bytes.get(index) { Some(b'-') => { index += 1; -1_i64 }, Some(b'+') => { index += 1; 1_i64 }, _ => 1_i64 };
            let mut value = 0_i64;
            while let Some(byte @ b'0'..=b'9') = bytes.get(index) {
                value = value.checked_mul(10).and_then(|v| v.checked_add(i64::from(*byte - b'0'))).expect("bounded atoi overflow");
                index += 1;
            }
            result = i32::try_from(value * sign).expect("bounded atoi result overflow") as i64 as u64;
        }
        "open" | "open64" | "__open_2" | "openat" => {
            let (path_pointer, flags) = if name == "openat" { (x1, x2 as u32) } else { (x0, x1 as u32) };
            let path = cstring(engine, path_pointer);
            if name == "openat" {
                assert!(path.starts_with('/') || x0 as i32 == -100, "unsupported relative openat directory");
            }
            let access = flags & 3;
            let opened = if access == 0 { std::fs::File::open(&path) } else {
                assert!(path.starts_with("/tmp/"), "native probe write escaped /tmp: {path}");
                let mut options = std::fs::OpenOptions::new();
                options.read(access == 2).write(true).create(flags & 0x40 != 0).truncate(flags & 0x200 != 0).append(flags & 0x400 != 0);
                options.open(&path)
            };
            if let Ok(file) = opened {
                let handle = 0x7000_0000_u64 + external.next_file * 0x100;
                external.next_file += 1;
                external.files.insert(handle, file);
                external.platform_events.insert(format!("host-file-open:{path}:flags={flags:#x}"));
                result = handle;
            } else { result = u64::MAX; }
        }
        "fstat" | "fstat64" => {
            let size = external.files.get(&x0).expect("unknown host fd").metadata().expect("fstat").len();
            write_stat(engine, x1, size);
            result = 0;
        }
        "read" | "__read_chk" => {
            let length = x2 as usize;
            assert!(length < 16 * 1024 * 1024);
            if name.starts_with("__read_chk") { assert!(length <= x3(engine) as usize); }
            let file = external.files.get_mut(&x0).expect("unknown host fd");
            let mut buffer = vec![0; length];
            let read = file.read(&mut buffer).expect("host read");
            write(engine, x1, &buffer[..read]);
            result = read as u64;
        }
        "lseek" | "lseek64" => {
            let seek = match x2 as u32 { 0 => SeekFrom::Start(x1), 1 => SeekFrom::Current(x1 as i64), 2 => SeekFrom::End(x1 as i64), other => panic!("unsupported lseek origin {other}") };
            result = external.files.get_mut(&x0).expect("unknown host fd").seek(seek).expect("host seek");
        }
        "close" => { external.files.remove(&x0).expect("unknown host fd"); result = 0; }
        "fopen" | "fopen64" => {
            let path = cstring(engine, x0);
            let mode = cstring(engine, x1);
            let opened = if mode.contains('w') || mode.contains('a') {
                assert!(path.starts_with("/tmp/"), "native probe write escaped /tmp: {path}");
                let mut options = std::fs::OpenOptions::new();
                options.write(true).create(true).truncate(mode.contains('w')).append(mode.contains('a'));
                options.open(&path)
            } else { std::fs::File::open(&path) };
            if let Ok(file) = opened {
                let handle = 0x7000_0000_u64 + external.next_file * 0x100;
                external.next_file += 1;
                external.files.insert(handle, file);
                external.platform_events.insert(format!("host-file-open:{path}:mode={mode}"));
                result = handle;
            }
        }
        "fwrite" => {
            let unit = x1 as usize;
            let count = x2 as usize;
            let length = unit.checked_mul(count).expect("fwrite overflow");
            assert!(length < 64 * 1024 * 1024);
            let mut bytes = vec![0; length];
            check(unsafe { uc_mem_read(engine, x0, bytes.as_mut_ptr().cast(), length) });
            external.files.get_mut(&x3(engine)).expect("unknown FILE handle").write_all(&bytes).expect("host fwrite");
            result = count as u64;
        }
        "fread" => {
            let length = (x1 as usize).checked_mul(x2 as usize).expect("fread overflow");
            assert!(length < 16 * 1024 * 1024);
            let file = external.files.get_mut(&x3(engine)).expect("unknown FILE handle");
            let mut bytes = vec![0; length];
            let read = file.read(&mut bytes).expect("host fread");
            write(engine, x0, &bytes[..read]);
            result = (read / (x1 as usize).max(1)) as u64;
        }
        "fclose" => { external.files.remove(&x0).expect("unknown FILE handle"); result = 0; }
        "feof" | "ferror" | "fflush" => result = 0,
        "setbuf" => {
            assert!(external.files.contains_key(&x0), "setbuf received unknown FILE handle {x0:#x}");
            assert_eq!(x1, 0, "only unbuffered setbuf is admitted");
            external.platform_events.insert("setbuf:unbuffered-host-file".into());
        }
        "fileno" => result = external.files.get(&x0).expect("unknown FILE handle").as_raw_fd() as u64,
        "access" | "faccessat" => {
            let path = if name == "faccessat@LIBC" { cstring(engine, x1) } else { cstring(engine, x0) };
            external.platform_events.insert(format!("host-access-unavailable:{path}"));
            result = u64::MAX;
        }
        "mkdir" | "mkdirat" => {
            let path = if name == "mkdirat@LIBC" { cstring(engine, x1) } else { cstring(engine, x0) };
            external.platform_events.insert(format!("host-mkdir-noop:{path}"));
            result = 0;
        }
        "stat" | "stat64" | "__stat64" => {
            let path = cstring(engine, x0);
            if let Ok(metadata) = std::fs::metadata(path) { write_stat(engine, x1, metadata.len()); result = 0; } else { result = u64::MAX; }
        }
        // PDF syntax classification here is the C-locale ASCII digit test.
        "iswdigit" => result = u64::from((0x30..=0x39).contains(&x0)),
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
        "calloc" => {
            let size = x0.checked_mul(x1).expect("calloc size overflow").max(1);
            assert!(size < 16 * 1024 * 1024, "bounded calloc size exceeded");
            let pointer = external.posix_cursor.checked_add(15).unwrap() & !15;
            external.posix_cursor = pointer.checked_add(size).unwrap();
            assert!(external.posix_cursor < 0x4100_0000, "bounded calloc arena exhausted");
            write(engine, pointer, &vec![0; size as usize]);
            external.host_allocs.insert(pointer, size);
            result = pointer;
        }
        "free" => {
            if x0 != 0 && external.host_allocs.remove(&x0).is_none() {
                assert!((HEAP..0x11f0_0000).contains(&x0) || (PDF_HEAP_BASE..PDF_HEAP_BASE + PDF_HEAP_SIZE).contains(&x0), "free of pointer outside bounded guest/host allocators: {x0:#x}");
            }
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
        "inflateInit_" | "inflateInit2_" => {
            let init2 = name == "inflateInit2_";
            let version_pointer = if init2 { x2 } else { x1 };
            let stream_size = if init2 { x3(engine) } else { x2 };
            assert_eq!(stream_size as usize, std::mem::size_of::<ZStream>(), "guest z_stream size does not match host ABI");
            let guest_version = cstring(engine, version_pointer);
            assert!(guest_version.starts_with("1."), "unsupported guest zlib version {guest_version:?}");
            let guest_zalloc = read_u64(engine, x0 + 64);
            let guest_zfree = read_u64(engine, x0 + 72);
            let guest_opaque = read_u64(engine, x0 + 80);
            let host_version = std::ffi::CString::new(guest_version.as_bytes()).unwrap();
            let mut stream = Box::<ZStream>::default();
            let id = external.next_inflate_id;
            external.next_inflate_id = external.next_inflate_id.checked_add(1).unwrap();
            let zalloc_context = if guest_zalloc == 0 && guest_zfree == 0 {
                assert_eq!(guest_opaque, 0, "guest zlib opaque must be null with default allocators");
                None
            } else {
                let zeroed = guest_zalloc == PDFIUM_ZALLOC && guest_zfree == PDFIUM_ZFREE && guest_opaque == 0;
                // libSPenLibpng's pinned png_zalloc multiplies items*size then
                // calls png_malloc_warn; png_zfree calls png_free.
                let libpng_allocator = guest_zalloc == LIBPNG_BASE + 0x10e48
                    && guest_zfree == LIBPNG_BASE + 0x10e58
                    && (PDF_HEAP_BASE..PDF_HEAP_BASE + PDF_HEAP_SIZE).contains(&guest_opaque);
                assert!(zeroed || libpng_allocator, "unsupported guest zlib allocator callbacks: alloc={guest_zalloc:#x} free={guest_zfree:#x} opaque={guest_opaque:#x}");
                let mut context = Box::new(ZallocContext {
                    ledger: ptr::from_mut(&mut external.zalloc_ledger),
                    stream_id: id,
                    zeroed,
                });
                stream.zalloc = Some(zlib_host_alloc);
                stream.zfree = Some(zlib_host_free);
                stream.opaque = context.as_mut() as *mut ZallocContext as *mut c_void;
                Some(context)
            };
            result = unsafe {
                (if init2 {
                    host_inflate_init2(stream.as_mut(), x1 as c_int, host_version.as_ptr(), stream_size as c_int)
                } else {
                    host_inflate_init(stream.as_mut(), host_version.as_ptr(), stream_size as c_int)
                }) as i64 as u64
            };
            assert_eq!(result as i64, 0, "pinned host inflateInit_ failed");
            external.inflates.insert(x0, InflateRecord {
                id,
                stream,
                zalloc_context,
                guest_zalloc,
                guest_zfree,
                consumed: 0,
                produced: 0,
                calls: 0,
            });
        }
        "inflate" => {
            let input_pointer = read_u64(engine, x0);
            let input_length = read_u32(engine, x0 + 8) as usize;
            let output_pointer = read_u64(engine, x0 + 24);
            let output_length = read_u32(engine, x0 + 32) as usize;
            assert!(input_length <= INFLATE_CHUNK_LIMIT && output_length <= INFLATE_CHUNK_LIMIT, "zlib chunk exceeds bounded host bridge");
            let record = external.inflates.get_mut(&x0).expect("inflate called without matching init");
            let next_consumed = record.consumed.checked_add(input_length as u64).expect("zlib input counter overflow");
            assert!(next_consumed <= INFLATE_STREAM_LIMIT, "zlib input stream limit exceeded");
            let mut input = vec![0_u8; input_length];
            let mut output = vec![0_u8; output_length];
            if input_length != 0 {
                check(unsafe { uc_mem_read(engine, input_pointer, input.as_mut_ptr().cast(), input_length) });
            }
            let stream = record.stream.as_mut();
            stream.next_in = input.as_mut_ptr();
            stream.avail_in = input_length as c_uint;
            stream.next_out = output.as_mut_ptr();
            stream.avail_out = output_length as c_uint;
            let status = unsafe { host_inflate(stream, x1 as c_int) };
            assert!(matches!(status, -5 | 0 | 1), "unsupported pinned zlib inflate status {status}");
            let consumed = input_length - stream.avail_in as usize;
            let produced = output_length - stream.avail_out as usize;
            record.consumed = record.consumed.checked_add(consumed as u64).unwrap();
            record.produced = record.produced.checked_add(produced as u64).unwrap();
            record.calls += 1;
            external.inflate_total_produced = external.inflate_total_produced.checked_add(produced as u64).unwrap();
            assert!(record.produced <= INFLATE_STREAM_LIMIT, "zlib output stream limit exceeded");
            assert!(external.inflate_total_produced <= INFLATE_TOTAL_LIMIT, "total zlib output limit exceeded");
            if produced != 0 {
                write(engine, output_pointer, &output[..produced]);
            }
            write(engine, x0, &(input_pointer + consumed as u64).to_le_bytes());
            write(engine, x0 + 8, &stream.avail_in.to_le_bytes());
            write(engine, x0 + 16, &stream.total_in.to_le_bytes());
            write(engine, x0 + 24, &(output_pointer + produced as u64).to_le_bytes());
            write(engine, x0 + 32, &stream.avail_out.to_le_bytes());
            write(engine, x0 + 40, &stream.total_out.to_le_bytes());
            write(engine, x0 + 88, &stream.data_type.to_le_bytes());
            write(engine, x0 + 96, &stream.adler.to_le_bytes());
            result = status as i64 as u64;
        }
        "inflateEnd" => {
            if !external.inflates.contains_key(&x0) {
                assert_eq!(read_u64(engine, x0 + 56), 0, "untracked inflateEnd has a live native zlib state outside the host bridge");
                result = (-2_i32) as i64 as u64; // Z_STREAM_ERROR for an uninitialized z_stream.
                register(engine, REGISTER_X0, result);
                return;
            }
            let mut record = external.inflates.remove(&x0).expect("inflateEnd called without matching init");
            assert!(record.calls > 0, "zlib stream ended without any inflate calls");
            assert!(record.zalloc_context.as_ref().is_none_or(|context| context.stream_id == record.id));
            result = unsafe { host_inflate_end(record.stream.as_mut()) as i64 as u64 };
            assert_eq!(result as i64, 0, "pinned host inflateEnd failed");
            assert!(
                external.zalloc_ledger.allocations.values().all(|(_, owner)| *owner != record.id),
                "zlib stream leaked allocations through the host callback bridge"
            );
            external.completed_inflates.push(InflateSummary {
                id: record.id,
                guest_zalloc: record.guest_zalloc,
                guest_zfree: record.guest_zfree,
                consumed: record.consumed,
                produced: record.produced,
                calls: record.calls,
            });
        }
        "crc32" | "adler32" => {
            let len = x2 as usize;
            assert!(len <= INFLATE_CHUNK_LIMIT, "checksum input chunk exceeds bound");
            assert!(len == 0 || x1 != 0, "checksum received null input with nonzero length");
            let mut bytes = vec![0_u8; len];
            if len != 0 { check(unsafe { uc_mem_read(engine, x1, bytes.as_mut_ptr().cast(), len) }); }
            let input = if x1 == 0 { ptr::null() } else { bytes.as_ptr() };
            result = unsafe {
                if name == "crc32" { host_crc32(x0, input, len as c_uint) }
                else { host_adler32(x0, input, len as c_uint) }
            };
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
        "sincosf" => {
            let angle = f32::from_bits(read_register(engine, 136) as u32);
            let mut sine = 0.0;
            let mut cosine = 0.0;
            unsafe { sincosf(angle, &mut sine, &mut cosine) };
            write(engine, x0, &sine.to_le_bytes());
            write(engine, x1, &cosine.to_le_bytes());
            result = 0;
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
        "clock_gettime" => {
            let ticks = external.clock_calls;
            external.clock_calls += 1;
            write(engine, x1, &(1 + ticks / 1000).to_le_bytes());
            write(engine, x1 + 8, &((ticks % 1000) * 1_000_000).to_le_bytes());
            external.platform_events.insert(format!("clock_gettime:deterministic-monotonic:clockid={x0}"));
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
        "pthread_mutex_init" | "pthread_mutexattr_init" | "pthread_mutexattr_destroy" | "pthread_mutexattr_settype" | "pthread_mutex_lock" | "pthread_mutex_unlock" | "pthread_cond_broadcast" => {
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
        | "pthread_rwlock_unlock" => result = 0,
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

fn verify_loaded_host_zlib() {
    let version = unsafe { CStr::from_ptr(host_zlib_version()) }
        .to_str()
        .expect("host zlib version is UTF-8");
    assert_eq!(version, "1.3.1", "pinned host zlib version changed");
    let symbol_address = host_inflate as *const () as usize;
    let maps = std::fs::read_to_string("/proc/self/maps").expect("read host process mappings");
    let loaded_path = maps
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            let range = fields.next()?;
            let (start, end) = range.split_once('-')?;
            let start = usize::from_str_radix(start, 16).ok()?;
            let end = usize::from_str_radix(end, 16).ok()?;
            if !(start <= symbol_address && symbol_address < end) {
                return None;
            }
            let path = line.split_whitespace().skip(5).collect::<Vec<_>>().join(" ");
            (!path.is_empty() && path.starts_with('/')).then_some(path)
        })
        .expect("locate loaded host zlib mapping");
    let expected = std::fs::canonicalize(LIBZ_PATH).expect("canonicalize pinned host zlib");
    let actual = std::fs::canonicalize(&loaded_path).expect("canonicalize loaded host zlib");
    assert_eq!(actual, expected, "linked zlib resolved to an unexpected library");
    verify_library(&actual, LIBZ_SHA256);
}

fn sha256_file(path: &Path) -> String {
    let digest = Command::new("sha256sum").arg(path).output().expect("hash native PDF output");
    assert!(digest.status.success(), "sha256sum failed for {}", path.display());
    String::from_utf8(digest.stdout)
        .expect("sha256sum output is UTF-8")
        .split_whitespace()
        .next()
        .expect("sha256sum digest present")
        .to_owned()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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

fn capture_loaded_source(
    machine: &mut Machine,
    external: &mut External,
    source_pdf: &[u8],
    output_path: &Path,
) {
    const SOURCE_BASE: u64 = 0x3000_0000;
    check(unsafe { uc_mem_map(machine.engine, SOURCE_BASE, 0x10000, 7) });
    assert!(source_pdf.len() < 0x10000, "synthetic source PDF too large");
    write(machine.engine, SOURCE_BASE, source_pdf);
    let doc = machine.call(
        PDFIUM_BASE + 0x5a5c50,
        &[SOURCE_BASE, source_pdf.len() as u64, 0],
    );
    assert_ne!(doc, 0, "FPDF_LoadMemDocument returned null");
    let page = machine.call(PDFIUM_BASE + 0x5a6148, &[doc, 0]);
    assert_ne!(page, 0, "FPDF_LoadPage returned null");
    machine.call(PDFIUM_BASE + 0x5a6c18, &[page]);
    let width = f32::from_bits(read_register(machine.engine, 136) as u32);
    machine.call(PDFIUM_BASE + 0x5a6c5c, &[page]);
    let height = f32::from_bits(read_register(machine.engine, 136) as u32);
    let source_rotation = machine.call(PDFIUM_BASE + 0x586ddc, &[page]);
    let form_argument = 4_u64 - source_rotation;
    let object = machine.call(PDFIUM_BASE + 0x58a014, &[page, form_argument]);
    assert_ne!(object, 0, "FPDFPage_NewFormObj returned null");

    register(machine.engine, 136, f32::to_bits(17.25) as u64);
    register(machine.engine, 137, f32::to_bits(29.5) as u64);
    let path = machine.call(PDFIUM_BASE + 0x58a9b8, &[]);
    assert_ne!(path, 0);
    for (x, y) in [(93.75_f32, 46.5_f32), (61.25_f32, 109.75_f32)] {
        register(machine.engine, 136, f32::to_bits(x) as u64);
        register(machine.engine, 137, f32::to_bits(y) as u64);
        machine.call(PDFIUM_BASE + 0x58ada4, &[path]);
    }
    machine.call(PDFIUM_BASE + 0x58af08, &[path]);
    machine.call(PDFIUM_BASE + 0x58af70, &[path, 1, 1]);
    machine.call(PDFIUM_BASE + 0x589504, &[path, 17, 92, 201, 255]);
    machine.call(PDFIUM_BASE + 0x58a7fc, &[object, path]);
    let form_object_count = machine.call(PDFIUM_BASE + 0x589f68, &[object]);
    machine.call(PDFIUM_BASE + 0x58a900, &[object]);
    machine.call(PDFIUM_BASE + 0x587134, &[page, object]);
    let content_result = machine.call(PDFIUM_BASE + 0x588ae4, &[page]);
    let page_object_count = machine.call(PDFIUM_BASE + 0x5874f8, &[page]);
    let rotation_after = machine.call(PDFIUM_BASE + 0x586ddc, &[page]);

    let filewrite = MODEL + 0x8000;
    let mut filewrite_struct = [0_u8; 16];
    filewrite_struct[..4].copy_from_slice(&1_u32.to_le_bytes());
    filewrite_struct[8..].copy_from_slice(&PDF_WRITE_BLOCK_STUB.to_le_bytes());
    write(machine.engine, filewrite, &filewrite_struct);
    let empty_facade = MODEL + 0xa000;
    write(machine.engine, empty_facade, &[0; 64]);
    let saved = machine.call(PDFIUM_BASE + 0x59cbbc, &[doc, filewrite, 0, empty_facade]);
    assert_eq!(saved, 1, "FPDF_SaveAsCopy failed");
    assert!(!external.pdf_bytes.is_empty(), "native writer emitted no bytes");
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
        "{{\"native_path\":\"FPDF_InitLibrary -> FPDF_LoadMemDocument -> FPDF_LoadPage -> GetPageWidthF/GetPageHeightF/GetRotation -> FPDFPage_NewFormObj(counterrotation) -> native path/form generation -> FPDF_SaveAsCopy -> FPDF_FILEWRITE.WriteBlock\",\"doc\":{},\"page\":{},\"loaded_width\":{},\"loaded_height\":{},\"form_object\":{},\"path_object\":{},\"source_rotation\":{},\"writer_form_argument_4_minus_rotation\":{},\"rotation_after\":{},\"form_object_count\":{},\"page_object_count\":{},\"page_generate_content_result\":{},\"save_result\":{},\"pdf_bytes\":{},\"write_calls\":{},\"handled_imports\":[{}],\"platform_events\":[{}]}}",
        doc,
        page,
        width,
        height,
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

fn symbol_version(name: &str) -> &str {
    name.split('@').next().unwrap_or(name)
}

fn exported_symbols(path: &Path, base: u64) -> HashMap<String, u64> {
    let output = Command::new("readelf").args(["-Ws"]).arg(path).output().unwrap();
    assert!(output.status.success());
    let mut symbols = HashMap::new();
    for line in String::from_utf8(output.stdout).unwrap().lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() >= 8 && fields[6] != "UND" {
            if let Ok(value) = u64::from_str_radix(fields[1], 16) {
                symbols.entry(symbol_version(fields[7]).to_owned()).or_insert(base + value);
            }
        }
    }
    symbols
}

fn install_unresolved_host_stub(engine: Engine, name: &str, external: &mut External, next_stub: &mut u64) -> u64 {
    if let Some((address, _)) = external.names.iter().find(|(_, imported)| symbol_version(imported) == name) {
        return *address;
    }
    let address = *next_stub;
    *next_stub += 16;
    external.names.insert(address, name.to_owned());
    write(engine, address, &0xd65f03c0_u32.to_le_bytes());
    let mut hook = 0;
    check(unsafe { uc_hook_add(engine, &mut hook, 4, host_external as *mut c_void, ptr::from_mut(external).cast(), address, address) });
    address
}

fn bind_extra_dso(engine: Engine, path: &Path, base: u64, providers: &HashMap<String, u64>, external: &mut External, next_stub: &mut u64) {
    let relocations = Command::new("readelf").args(["-rW"]).arg(path).output().unwrap();
    assert!(relocations.status.success());
    let symbols = Command::new("readelf").args(["-Ws"]).arg(path).output().unwrap();
    assert!(symbols.status.success());
    let function_imports = String::from_utf8(symbols.stdout).unwrap().lines().filter_map(|line| {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        (fields.len() >= 8 && fields[3] == "FUNC" && fields[6] == "UND").then(|| symbol_version(fields[7]).to_owned())
    }).collect::<BTreeSet<_>>();
    for line in String::from_utf8(relocations.stdout).unwrap().lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 6 { continue; }
        let kind = fields[2];
        if !(kind.contains("JUMP_SLOT") || kind.contains("GLOB_DAT") || kind.contains("ABS64")) { continue; }
        let offset = u64::from_str_radix(fields[0], 16).unwrap();
        let symbol_value = u64::from_str_radix(fields[3], 16).unwrap_or(0);
        let name = symbol_version(fields[4]);
        let jump_slot = kind.contains("JUMP_SLOT");
        let png_zlib_call = path.file_name().is_some_and(|name| name == "libSPenLibpng.so")
            && matches!(name, "inflateInit_" | "inflateInit2_" | "inflate" | "inflateEnd" | "crc32" | "adler32");
        let mut target = if png_zlib_call || matches!(name, "calloc" | "free") { install_unresolved_host_stub(engine, fields[4], external, next_stub) }
        else if symbol_value != 0 { base + symbol_value }
        else if let Some(address) = providers.get(name) { *address }
        else if name == "__sF" { STUB + 0x9000 }
        else if name == "stdout" { STUB + 0x9010 }
        else if name == "stderr" { STUB + 0x9020 }
        else if matches!(name, "malloc" | "_Znwm" | "_ZnwmRKSt9nothrow_t") { NEW }
        else if matches!(name, "free" | "_ZdlPv" | "_ZdaPv" | "_ZdlPvm" | "_ZdaPvm") { DELETE }
        else if jump_slot || function_imports.contains(name) { install_unresolved_host_stub(engine, fields[4], external, next_stub) }
        else { panic!("unresolved {kind} data import {name} in {}", path.display()); };
        if kind.contains("ABS64") && fields.len() >= 7 {
            let amount = i64::from_str_radix(fields[6], 16).unwrap_or(0);
            target = target.wrapping_add_signed(match fields[5] { "+" => amount, "-" => -amount, _ => 0 });
        }
        write(engine, base + offset, &target.to_le_bytes());
    }
}

fn run_init_array(engine: Engine, path: &Path, base: u64, mut call: impl FnMut(u64)) {
    let sections = Command::new("readelf").args(["-W", "-S"]).arg(path).output().unwrap();
    assert!(sections.status.success());
    for line in String::from_utf8(sections.stdout).unwrap().lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.get(1) != Some(&".init_array") || fields.len() < 7 { continue; }
        let address = u64::from_str_radix(fields[3], 16).unwrap();
        let size = u64::from_str_radix(fields[5], 16).unwrap();
        assert_eq!(size % 8, 0);
        for offset in (0..size).step_by(8) {
            let initializer = read_u64(engine, base + address + offset);
            if initializer != 0 && initializer != u64::MAX { call(initializer); }
        }
        return;
    }
}

pub(super) fn capture(
    machine: &mut Machine,
    model: &Path,
    pdfium: &Path,
    source_rotation: u64,
    output_path: &Path,
    source_pdf: Option<&[u8]>,
    standard_export_path: Option<&Path>,
    expected_source_sha256: Option<&str>,
    note_page_scale: Option<f32>,
    overlay_png_path: Option<&Path>,
    expected_overlay_png_sha256: Option<&str>,
    body_writer_matrix: Option<(&Path, &Path, u32, u32)>,
    pdf_image_adapter: Option<super::pdf_image_adapter::Input<'_>>,
) {
    verify_loaded_host_zlib();
    machine.prepare_pdfium_arena();
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
    let libpng = overlay_png_path.map(|_| pdfium.with_file_name("libSPenLibpng.so"));
    if let Some(libpng) = &libpng { verify_library(libpng, LIBPNG_SHA256); }
    if let Some(libpng) = &libpng {
        map_library(
            machine.engine,
            libpng,
            LIBPNG_BASE,
            "898abcc6ddbe3a4ad7e076484df666b732913db95a7abc7d6e99bfd835f555eb",
        );
    }
    let (spenpdf, spenbase) = if standard_export_path.is_some() {
        let spenpdf = pdfium.with_file_name("libSPenPdf.so");
        let spenbase = pdfium.with_file_name("libSPenBase.so");
        map_library(machine.engine, &spenpdf, SPENPDF_BASE, "cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04");
        map_library(machine.engine, &spenbase, SPENBASE_BASE, "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb");
        (Some(spenpdf), Some(spenbase))
    } else { (None, None) };
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
    let libpng_symbols = libpng.as_ref().map(|path| exported_symbols(path, LIBPNG_BASE)).unwrap_or_default();
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
        files: HashMap::new(),
        next_file: 1,
        tls: HashMap::new(),
        next_tls_key: 1,
        clock_calls: 0,
        inflates: HashMap::new(),
        next_inflate_id: 1,
        inflate_total_produced: 0,
        completed_inflates: Vec::new(),
        zalloc_ledger: ZallocLedger::default(),
        pdf_bytes: Vec::new(),
        write_calls: 0,
    });
    let mut next_stub = STUB + 0x100;
    write(machine.engine, PDF_WRITE_BLOCK_STUB, &0xd65f03c0_u32.to_le_bytes());
    let mut write_hook = 0;
    check(unsafe {
        uc_hook_add(
            machine.engine,
            &mut write_hook,
            4,
            native_pdf_write_block as *mut c_void,
            ptr::from_mut(external.as_mut()).cast(),
            PDF_WRITE_BLOCK_STUB,
            PDF_WRITE_BLOCK_STUB,
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
        } else if let Some(address) = libpng_symbols.get(name) {
            *address
        } else {
            match name {
                "_Znwm" | "_ZnwmRKSt9nothrow_t" | "malloc" => NEW,
                "calloc" | "free" => install_unresolved_host_stub(machine.engine, f[4], external.as_mut(), &mut next_stub),
                "_ZdlPv" | "_ZdaPv" | "_ZdlPvm" | "_ZdaPvm" => DELETE,
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
                "malloc" | "_Znwm" | "_ZnwmRKSt9nothrow_t" => NEW,
                "calloc" | "free" => install_unresolved_host_stub(machine.engine, f[4], external.as_mut(), &mut next_stub),
                "_ZdlPv" | "_ZdaPv" => DELETE,
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
                    "calloc" | "free" => install_unresolved_host_stub(machine.engine, f[4], external.as_mut(), &mut next_stub),
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
    if let (Some(spenpdf), Some(spenbase)) = (&spenpdf, &spenbase) {
        let mut providers = HashMap::new();
        let mut provider_sources = vec![
            exported_symbols(spenpdf, SPENPDF_BASE),
            exported_symbols(spenbase, SPENBASE_BASE),
            exported_symbols(pdfium, PDFIUM_BASE),
            exported_symbols(&libcxx, LIBCPP_BASE),
            exported_symbols(model, 0),
        ];
        if let Some(libpng) = &libpng { provider_sources.push(exported_symbols(libpng, LIBPNG_BASE)); }
        for source in provider_sources {
            for (name, address) in source { providers.entry(name).or_insert(address); }
        }
        bind_extra_dso(machine.engine, spenbase, SPENBASE_BASE, &providers, external.as_mut(), &mut next_stub);
        bind_extra_dso(machine.engine, spenpdf, SPENPDF_BASE, &providers, external.as_mut(), &mut next_stub);
        if let Some(libpng) = &libpng { bind_extra_dso(machine.engine, libpng, LIBPNG_BASE, &providers, external.as_mut(), &mut next_stub); }
    }
    register(machine.engine, REGISTER_TPIDR_EL0, machine.tls_pointer);

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

    if let (Some(spenpdf), Some(spenbase), Some(source_path)) = (&spenpdf, &spenbase, standard_export_path) {
        if let Some(expected) = expected_source_sha256 {
            verify_library(source_path, expected);
        }
        run_init_array(machine.engine, spenbase, SPENBASE_BASE, |initializer| { machine.call(initializer, &[]); });
        run_init_array(machine.engine, spenpdf, SPENPDF_BASE, |initializer| { machine.call(initializer, &[]); });
        machine.call(PDFIUM_BASE + 0x5a5504, &[]);
        const IMPL_OBJECT: u64 = 0x5000_0000;
        const HANDLER_OBJECT: u64 = IMPL_OBJECT + 0xf000;
        const SOURCE_BASE: u64 = 0x3000_0000;
        check(unsafe { uc_mem_map(machine.engine, IMPL_OBJECT, 0x10000, 7) });
        if overlay_png_path.is_some() {
            check(unsafe { uc_mem_map(machine.engine, 0x0901_0000, 0x10000, 7) });
        }
        machine.call(SPENPDF_BASE + 0x84354, &[IMPL_OBJECT]);
        check(unsafe { uc_mem_map(machine.engine, SOURCE_BASE, 0x10000, 7) });
        let source_name = source_path.to_string_lossy();
        write(machine.engine, SOURCE_BASE, source_name.as_bytes());
        write(machine.engine, SOURCE_BASE + source_name.len() as u64, &[0]);
        machine.call(SPENPDF_BASE + 0x85184, &[IMPL_OBJECT, SOURCE_BASE, 0, 0]);
        let load_result = machine.call(SPENPDF_BASE + 0x857fc, &[IMPL_OBJECT, 0]);
        assert_eq!(load_result, 1, "PdfiumImpl::LoadPage failed");
        let page_cache_begin = read_u64(machine.engine, IMPL_OBJECT + 1520);
        let page = read_u64(machine.engine, page_cache_begin);
        assert_ne!(page, 0, "PdfiumImpl page cache is empty");
        machine.call(PDFIUM_BASE + 0x5a6c18, &[page]);
        let loaded_width = f32::from_bits(read_register(machine.engine, 136) as u32);
        machine.call(PDFIUM_BASE + 0x5a6c5c, &[page]);
        let loaded_height = f32::from_bits(read_register(machine.engine, 136) as u32);
        let source_rotation = machine.call(PDFIUM_BASE + 0x586ddc, &[page]);
        if let Some((composer, base, note_width, note_height)) = body_writer_matrix {
            check(unsafe { uc_mem_map(machine.engine, MATRIX_DATA_BASE, 0x10000, 7) });
            let page_rect = [0.0_f32, 0.0, loaded_width, loaded_height];
            let (overlay_rect, note_scale) = super::pdf_paper_body_writer_matrix::transform(
                model,
                composer,
                base,
                page_rect,
                note_width,
                note_height,
            );
            let host_expected_note_scale = loaded_width / note_width as f32;
            let info = MATRIX_DATA_BASE;
            write(machine.engine, info, &[0; 128]);
            write(machine.engine, info + 44, unsafe { std::slice::from_raw_parts(overlay_rect.as_ptr().cast::<u8>(), 16) });
            write(machine.engine, info + 60, &1.0_f32.to_bits().to_le_bytes());
            write(machine.engine, info + 64, unsafe { std::slice::from_raw_parts(overlay_rect.as_ptr().cast::<u8>(), 16) });
            let matrix = MATRIX_DATA_BASE + 0x1000;
            let matrix_result = machine.call(SPENPDF_BASE + 0x89ff8, &[IMPL_OBJECT, page, info, matrix]);
            let matrix_values = (0..6).map(|index| read_float(machine.engine, matrix + index * 4)).collect::<Vec<_>>();
            let position = MATRIX_DATA_BASE + 0x1100;
            let position_result = machine.call(SPENPDF_BASE + 0x89f14, &[IMPL_OBJECT, page, info, position]);
            let position_values = (0..4).map(|index| read_float(machine.engine, position + index * 4)).collect::<Vec<_>>();
            assert_eq!(matrix_result, 1, "PdfiumImpl::GetObjectMatrix rejected the BodyText rect");
            assert_eq!(position_result, 1, "PdfiumImpl::GetObjectPosition rejected the BodyText rect");
            println!("{{\"native_path\":\"PdfiumImpl Open/LoadPage -> Composer addBodyText/BodyTextPDFWriter -> RectF Offset/Scale -> separate PdfiumImpl::GetObjectMatrix/GetObjectPosition\",\"loaded_page_rect\":[0,0,{},{}],\"source_rotation_quarter_turns\":{},\"note_page_wh\":[{},{}],\"native_write_body_text_scale\":{},\"host_expected_note_scale\":{},\"synthetic_marker_note_rect\":[10,20,40,60],\"synthetic_marker_position\":[23,17],\"body_writer_draw_object_rect\":[{},{},{},{}],\"matrix_input_rect\":[{},{},{},{}],\"matrix_ratio\":1,\"get_object_matrix_executed\":true,\"get_object_matrix_result\":{},\"matrix\":{:?},\"get_object_position_executed\":true,\"get_object_position_result\":{},\"position_rect\":{:?},\"host_boundaries\":[\"WPage dimensions and one DrawnText marker are host supplied to native Composer\",\"The native BodyTextPDFWriter scale field is observed at the real function entry; the independently computed expected ratio is host-derived\",\"Drawing::DrawObject is a host sink; its captured native rect is passed to the separate native PDFium matrix helpers\"],\"boundary\":\"PDFium loads the generated source PDF through native PdfiumImpl; its loaded dimensions and rotation feed actual Composer addBodyText and BodyTextPDFWriter calls. A read-only hook observes the scale field passed to the real BodyTextPDFWriter::WriteBodyText entry. Native Base RectF operations transform the host synthetic marker before the host DrawObject sink. The actual PdfiumImpl AddImage matrix/position callees execute on that rect, but Drawing::DrawObject to AddImage, FPDF insertion, content generation, serialization and pixels do not execute.\"}}", loaded_width, loaded_height, source_rotation, note_width, note_height, note_scale, host_expected_note_scale, overlay_rect[0], overlay_rect[1], overlay_rect[2], overlay_rect[3], overlay_rect[0], overlay_rect[1], overlay_rect[2], overlay_rect[3], matrix_result, matrix_values, position_result, position_values);
            return;
        }
        write(machine.engine, HANDLER_OBJECT, &IMPL_OBJECT.to_le_bytes());
        let writer_open = machine.call(SPENPDF_BASE + 0x7c844, &[HANDLER_OBJECT, 0, 0]);
        assert_eq!(writer_open, 1, "PdfiumExportHandler::OpenWriter failed");
        let mut adapter_capture = None;
        let image_result = if let Some(png_path) = overlay_png_path {
            let expected = expected_overlay_png_sha256.expect("PNG capture requires an expected source hash");
            verify_library(png_path, expected);
            let png_name = png_path.to_string_lossy();
            assert!(png_name.len() <= 22, "PDFExportInfo short path supports at most 22 ASCII bytes (11 UTF-16 code units)");
            assert!(png_name.is_ascii(), "PDFExportInfo short path must be ASCII");
            const PROBE_DATA_BASE: u64 = 0x0901_0000;
            adapter_capture = if let Some(input) = pdf_image_adapter {
                assert_eq!(input.path, png_path, "native adapter path must be the pinned PNG input");
                Some(super::pdf_image_adapter::populate(machine, input))
            } else {
                None
            };
            let image_info = if let Some(capture) = &adapter_capture {
                capture.info
            } else {
                let image_info = PROBE_DATA_BASE;
                write(machine.engine, image_info, &[0; 128]);
                let note_rect = [100.0_f32, 100.0, 300.0, 300.0];
                write(machine.engine, image_info + 44, unsafe { std::slice::from_raw_parts(note_rect.as_ptr().cast::<u8>(), 16) });
                let scale = note_page_scale.expect("PNG capture requires the page scale");
                write(machine.engine, image_info + 60, &scale.to_bits().to_le_bytes());
                write(machine.engine, image_info + 64, unsafe { std::slice::from_raw_parts(note_rect.as_ptr().cast::<u8>(), 16) });
                write(machine.engine, image_info + 80, &[(png_name.len() as u8) * 2]);
                write(machine.engine, image_info + 81, png_name.as_bytes());
                write(machine.engine, image_info + 81 + png_name.len() as u64, &[0]);
                image_info
            };
            let note_rect = adapter_capture.as_ref().map_or([100.0_f32, 100.0, 300.0, 300.0], |capture| capture.destination_rect);
            let result = if let Some(capture) = adapter_capture.as_mut() {
                super::pdf_image_adapter::add_image(machine, HANDLER_OBJECT, capture)
            } else {
                machine.call(SPENPDF_BASE + 0x82810, &[HANDLER_OBJECT, 0, image_info])
            };
            assert_eq!(result, 1, "PdfiumImageHandler::AddImage failed");
            let generated = machine.call(PDFIUM_BASE + 0x588ae4, &[page]);
            assert_eq!(generated, 1, "FPDFPage_GenerateContent after image insert failed");
            Some((result, generated, note_rect))
        } else { None };
        const PAINT: u64 = IMPL_OBJECT + 0xe000;
        const POINTS: u64 = IMPL_OBJECT + 0xd100;
        const VECTOR: u64 = IMPL_OBJECT + 0xd000;
        machine.call(SPENPDF_BASE + 0x7571c, &[PAINT]);
        machine.call(SPENPDF_BASE + 0x75854, &[PAINT, 0xff11_5cc9]);
        register(machine.engine, 136, f32::to_bits(0.0) as u64);
        machine.call(SPENPDF_BASE + 0x759f0, &[PAINT, 0]);
        let synthetic_points = [(17.25_f32, 29.5_f32), (93.75_f32, 46.5_f32), (61.25_f32, 109.75_f32)];
        let note_points = [(100.0_f32, 100.0_f32), (300.0, 100.0), (200.0, 350.0)];
        let emitted_points = note_page_scale.map_or(synthetic_points, |scale| {
            assert!(scale.is_finite() && scale > 0.0);
            note_points.map(|(x, y)| (x * scale, y * scale))
        });
        for (index, (x, y)) in emitted_points.into_iter().enumerate() {
            write(machine.engine, POINTS + index as u64 * 8, &x.to_le_bytes());
            write(machine.engine, POINTS + index as u64 * 8 + 4, &y.to_le_bytes());
        }
        write(machine.engine, VECTOR, &POINTS.to_le_bytes());
        write(machine.engine, VECTOR + 8, &(POINTS + 24).to_le_bytes());
        write(machine.engine, VECTOR + 16, &(POINTS + 24).to_le_bytes());
        machine.call(SPENPDF_BASE + 0x9a470, &[HANDLER_OBJECT, IMPL_OBJECT]);
        machine.call(SPENPDF_BASE + 0x9aa38, &[HANDLER_OBJECT, PAINT, VECTOR]);
        let writer_close = machine.call(SPENPDF_BASE + 0x7c93c, &[HANDLER_OBJECT]);
        assert_eq!(writer_close, 1, "PdfiumExportHandler::CloseWriter failed");
        const EXPORT_BASE: u64 = SOURCE_BASE + 0x1000;
        write(machine.engine, EXPORT_BASE, output_path.to_string_lossy().as_bytes());
        write(machine.engine, EXPORT_BASE + output_path.to_string_lossy().len() as u64, &[0]);
        let export_result = machine.call(SPENPDF_BASE + 0x7ca1c, &[HANDLER_OBJECT, EXPORT_BASE, 0, 0]);
        assert_eq!(export_result, 0, "PdfiumExportHandler::Export failed");
        if let Some(expected) = expected_source_sha256 {
            verify_library(source_path, expected);
        }
        let output = std::fs::read(output_path).expect("Samsung Export output");
        assert!(!output.is_empty(), "Samsung Export emitted no file bytes");
        assert!(external.inflates.is_empty(), "native export left host zlib streams active");
        assert!(external.zalloc_ledger.allocations.is_empty(), "native export leaked host zlib allocations");
        assert_eq!(external.zalloc_ledger.active_bytes, 0, "native export left zlib allocator bytes live");
        let output_sha256 = sha256_file(output_path);
        let source_sha256 = sha256_file(source_path);
        let inflate_streams = external.completed_inflates.iter().map(|stream| {
            let host_bridge = if stream.guest_zalloc == 0 && stream.guest_zfree == 0 {
                "host_zlib_default"
            } else if stream.guest_zalloc == PDFIUM_ZALLOC && stream.guest_zfree == PDFIUM_ZFREE {
                "host_zeroed_alloc_free"
            } else {
                assert_eq!(stream.guest_zalloc, LIBPNG_BASE + 0x10e48);
                assert_eq!(stream.guest_zfree, LIBPNG_BASE + 0x10e58);
                "host_alloc_free"
            };
            format!(
                "{{\"id\":{},\"guest_zalloc\":\"0x{:x}\",\"guest_zfree\":\"0x{:x}\",\"host_bridge\":\"{host_bridge}\",\"consumed_bytes\":{},\"produced_bytes\":{},\"calls\":{}}}",
                stream.id, stream.guest_zalloc, stream.guest_zfree, stream.consumed, stream.produced, stream.calls
            )
        }).collect::<Vec<_>>().join(",");
        let note_points_json = note_points.iter().map(|(x, y)| format!("[{x},{y}]")).collect::<Vec<_>>().join(",");
        let emitted_points_json = emitted_points.iter().map(|(x, y)| format!("[{x},{y}]")).collect::<Vec<_>>().join(",");
        let scale_json = note_page_scale.map_or_else(|| "null".to_owned(), |scale| scale.to_string());
        if let (Some((add_image_result, image_generate_content_result, image_rect)), Some(png_path)) = (image_result, overlay_png_path) {
            let png_sha256 = sha256_file(png_path);
            let image_adapter_field = adapter_capture.as_ref().map_or_else(String::new, |capture| format!(",\"image_adapter\":{{\"native_constructor\":true,\"native_set_image_string_rect\":true,\"native_set_rect\":true,\"native_set_scale\":true,\"info_pointer\":\"0x{:x}\",\"info_pointer_passed_to_add_image\":true,\"add_image_entry_info_pointer\":\"0x{:x}\",\"constructor_info_128_hex\":\"{}\",\"after_setters_info_128_hex\":\"{}\",\"destination_rect\":{:?},\"source_clip\":{:?},\"scale\":{},\"path_host_input\":{:?},\"path_provenance\":\"host input; native path field not decoded\",\"owns_path\":{}}}", capture.info, capture.add_image_entry_info.expect("AddImage entry pointer not captured"), hex(&capture.constructor_info), hex(&capture.after_setters_info), capture.destination_rect, capture.source_clip, capture.scale, capture.path, capture.owns_path));
            println!("{{\"native_path\":\"PdfiumImpl ctor/Open/LoadPage -> PdfiumExportHandler::OpenWriter -> PdfiumImageHandler::AddImage -> FPDFPage_GenerateContent -> PdfiumPathHandler::DrawPath -> CloseWriter -> Export(range 0..0) -> FPDF_SaveAsCopy\",\"source_pdf_sha256\":\"{source_sha256}\",\"overlay_png_sha256\":\"{png_sha256}\",\"loaded_page_geometry\":[{loaded_width},{loaded_height}],\"source_rotation\":{source_rotation},\"writer_open\":{writer_open},\"image_add_result\":{add_image_result},\"image_generate_content_result\":{image_generate_content_result},\"note_page_image_rect\":{image_rect:?},\"page_scale\":{scale_json}{image_adapter_field},\"writer_close\":{writer_close},\"export_executed\":true,\"export_result\":{export_result},\"export_result_semantics\":\"0 means FPDF_SaveAsCopy succeeded; the native wrapper closes the document on nonzero save status\",\"export_output_path\":{:?},\"export_output_bytes\":{},\"export_output_sha256\":\"{}\",\"host_zlib_sha256\":\"{}\",\"inflate_total_produced_bytes\":{},\"inflate_allocator_peak_bytes\":{},\"inflate_streams\":[{}],\"note_path_points\":[{}],\"page_path_points\":[{}],\"handled_imports\":[{}],\"platform_events\":[{}],\"host_boundaries\":{{\"source_pdf_and_png\":\"external files admitted only by pinned SHA-256\",\"png_decode\":\"pinned APK libSPenLibpng executes; zlib inflate/checksum callbacks use pinned host libz\",\"image_rect\":\"synthetic note-space coordinates supplied by this harness\",\"path_points\":\"synthetic note-space coordinates composed on host using separately captured ratio\",\"filesystem\":\"bounded host file callbacks\",\"page_parse_insert_export\":\"pinned native PDFium and SPenPdf calls\"}}}}", output_path.display(), output.len(), output_sha256, LIBZ_SHA256, external.inflate_total_produced, external.zalloc_ledger.peak_bytes, inflate_streams, note_points_json, emitted_points_json, external.handled_imports.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(","), external.platform_events.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(","));
            return;
        }
        println!("{{\"native_path\":\"PdfiumImpl ctor/Open/LoadPage -> PdfiumExportHandler::OpenWriter -> PdfiumPathHandler::DrawPath -> CloseWriter -> PdfiumExportHandler::Export(inclusive page range 0..0) -> PDFFileWriter/FDPF_SaveAsCopy\",\"load_result\":{},\"loaded_width\":{},\"loaded_height\":{},\"source_rotation\":{},\"writer_open\":{},\"writer_close\":{},\"export_executed\":true,\"export_result\":{},\"output_bytes\":{},\"source_file_sha256\":\"{}\",\"output_file_sha256\":\"{}\",\"host_zlib_sha256\":\"{}\",\"inflate_total_produced_bytes\":{},\"inflate_allocator_peak_bytes\":{},\"inflate_streams\":[{}],\"note_page_scale\":{},\"note_points\":[{}],\"pdf_points\":[{}],\"coordinate_composition\":\"host composed supplied note points with the independently captured Composer ratio; body writer transform not executed\",\"handled_imports\":[{}],\"platform_events\":[{}]}}", load_result, loaded_width, loaded_height, source_rotation, writer_open, writer_close, export_result, output.len(), source_sha256, output_sha256, LIBZ_SHA256, external.inflate_total_produced, external.zalloc_ledger.peak_bytes, inflate_streams, scale_json, note_points_json, emitted_points_json, external.handled_imports.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(","), external.platform_events.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(","));
        return;
    }

    machine.call(PDFIUM_BASE + 0x5a5504, &[]);
    if let Some(source_pdf) = source_pdf {
        external.pdf_bytes.clear();
        external.write_calls = 0;
        capture_loaded_source(machine, external.as_mut(), source_pdf, output_path);
        return;
    }
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
    filewrite_struct[8..].copy_from_slice(&PDF_WRITE_BLOCK_STUB.to_le_bytes());
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
