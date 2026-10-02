use super::*;
use frames::{BASE, BASE_SHA256};
use geometry::TEXT_SHA256;
use std::collections::{BTreeMap, BTreeSet};

pub(super) const TEXT: u64 = 0x0500_0000;
const SKIA: u64 = 0x0600_0000;
const SOURCE_HEAP: u64 = 0x0800_0000;
pub(super) const FONT_BYTES: u64 = 0x0900_0000;
use super::host_thunks::{
    HOST_IMPORT_CAPACITY, HOST_IMPORT_OVERFLOW, HOST_IMPORT_PRIMARY, host_import_address,
};
const HOST_REGION_BYTES: u64 = 0x10000;
pub(super) const SKIA_SHA256: &str =
    "42636cb9ac06cc286114b42c1b9d8f4b78d33761843251cde2b443b101ffb88d";
const FONT: u64 = MODEL + 0x10000;
const INPUT: u64 = MODEL + 0x11000;

pub(super) fn bytes(engine: Engine, address: u64, length: usize) -> Vec<u8> {
    assert!(length <= 0x400000);
    let mut result = vec![0; length];
    if length != 0 {
        check(unsafe { uc_mem_read(engine, address, result.as_mut_ptr().cast(), length) });
    }
    result
}

fn c_string(engine: Engine, address: u64) -> Vec<u8> {
    assert_ne!(address, 0);
    (0..0x20000)
        .map(|offset| bytes(engine, address + offset, 1)[0])
        .take_while(|byte| *byte != 0)
        .collect()
}

fn string(engine: Engine, address: u64) -> String {
    let first = bytes(engine, address, 1)[0];
    let (length, pointer) = if first & 1 == 0 {
        (usize::from(first >> 1), address + 1)
    } else {
        (
            read_u64(engine, address + 8) as usize,
            read_u64(engine, address + 16),
        )
    };
    String::from_utf8(bytes(engine, pointer, length)).unwrap()
}

pub(super) fn json_string(value: &str) -> String {
    use std::fmt::Write;
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            character if character < '\u{20}' => {
                write!(output, "\\u{:04x}", u32::from(character)).unwrap();
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn json_strings(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| json_string(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

struct Binary {
    bytes: Vec<u8>,
    sections: Vec<[u8; 64]>,
}

impl Binary {
    fn new(path: &Path) -> Self {
        let bytes = fs::read(path).unwrap();
        let offset = u64::from_le_bytes(bytes[40..48].try_into().unwrap()) as usize;
        let count = u16::from_le_bytes(bytes[60..62].try_into().unwrap()) as usize;
        assert_eq!(u16::from_le_bytes(bytes[58..60].try_into().unwrap()), 64);
        let sections = bytes[offset..offset + count * 64]
            .chunks_exact(64)
            .map(|section| section.try_into().unwrap())
            .collect();
        Self { bytes, sections }
    }

    fn section(&self, index: usize) -> &[u8] {
        let section = self.sections[index];
        let start = u64::from_le_bytes(section[24..32].try_into().unwrap()) as usize;
        let length = u64::from_le_bytes(section[32..40].try_into().unwrap()) as usize;
        &self.bytes[start..start + length]
    }

    fn symbol(&self, section: usize, index: usize) -> (&str, u64, bool) {
        let symbol = &self.section(section)[index * 24..][..24];
        let names = u32::from_le_bytes(self.sections[section][40..44].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(symbol[..4].try_into().unwrap()) as usize;
        let name = self.section(names)[offset..]
            .split(|byte| *byte == 0)
            .next()
            .unwrap();
        (
            std::str::from_utf8(name).unwrap(),
            u64::from_le_bytes(symbol[8..16].try_into().unwrap()),
            u16::from_le_bytes(symbol[6..8].try_into().unwrap()) != 0,
        )
    }

    fn exports(&self, base: u64, exports: &mut BTreeMap<String, u64>) {
        for (index, section) in self.sections.iter().enumerate() {
            if u32::from_le_bytes(section[4..8].try_into().unwrap()) != 11 {
                continue;
            }
            for symbol in 0..self.section(index).len() / 24 {
                let (name, value, defined) = self.symbol(index, symbol);
                if defined && !name.is_empty() {
                    exports.insert(name.into(), base + value);
                }
            }
        }
    }

    fn bind(
        &self,
        engine: Engine,
        base: u64,
        exports: &BTreeMap<String, u64>,
        imports: &mut BTreeMap<String, u64>,
    ) {
        for (index, section) in self.sections.iter().enumerate() {
            if u32::from_le_bytes(section[4..8].try_into().unwrap()) != 4 {
                continue;
            }
            let symbols = u32::from_le_bytes(section[40..44].try_into().unwrap()) as usize;
            for relocation in self.section(index).chunks_exact(24) {
                let info = u64::from_le_bytes(relocation[8..16].try_into().unwrap());
                if !matches!(info as u32, 257 | 1025 | 1026) {
                    continue;
                }
                let (name, _, defined) = self.symbol(symbols, (info >> 32) as usize);
                if defined || name.is_empty() {
                    continue;
                }
                let thunk_index = imports.len();
                let target = exports.get(name).copied().unwrap_or_else(|| {
                    *imports
                        .entry(name.into())
                        .or_insert_with(|| host_import_address(thunk_index))
                });
                let addend = i64::from_le_bytes(relocation[16..24].try_into().unwrap());
                let target = target.checked_add_signed(addend).unwrap();
                let address = u64::from_le_bytes(relocation[..8].try_into().unwrap());
                write(engine, base + address, &target.to_le_bytes());
            }
        }
    }
}

pub(super) type HostImportHandler = fn(Engine, &str, [u64; 8], *mut c_void) -> Option<u64>;

#[derive(Default)]
struct Observation {
    imports: BTreeMap<u64, String>,
    allocations: BTreeMap<u64, usize>,
    cursor: u64,
    fill: u8,
    once: BTreeSet<u64>,
    next_thread_key: u32,
    thread_values: BTreeMap<u64, u64>,
    file_size: usize,
    opened_paths: Vec<String>,
    external_import: Option<(HostImportHandler, *mut c_void)>,
}

impl Observation {
    fn allocate(&mut self, engine: Engine, size: usize, zero: bool) -> u64 {
        let size = size.max(16).next_multiple_of(16);
        let pointer = self.cursor;
        self.cursor += size as u64;
        assert!(self.cursor < SOURCE_HEAP + 0x800000);
        write(
            engine,
            pointer,
            &vec![if zero { 0 } else { self.fill }; size],
        );
        self.allocations.insert(pointer, size);
        pointer
    }
}

unsafe extern "C" fn imported(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let (name, external_handler) = {
        let state = unsafe { &*data.cast::<Observation>() };
        (
            state.imports.get(&address).unwrap().clone(),
            state.external_import,
        )
    };
    let first = read_register(engine, REGISTER_X0);
    let second = read_register(engine, REGISTER_X0 + 1);
    let third = read_register(engine, REGISTER_X0 + 2);
    let external = external_handler.and_then(|(handler, data)| {
        handler(
            engine,
            &name,
            std::array::from_fn(|index| read_register(engine, REGISTER_X0 + index as i32)),
            data,
        )
    });
    if let Some(value) = external {
        register(engine, REGISTER_X0, value);
        return;
    }
    let state = unsafe { &mut *data.cast::<Observation>() };
    let result = match name.as_str() {
        "malloc" | "_Znwm" | "_Znam" | "_ZnwmRKSt9nothrow_t" | "_ZnamRKSt9nothrow_t" => {
            state.allocate(engine, first as usize, false)
        }
        "calloc" => state.allocate(engine, first.checked_mul(second).unwrap() as usize, true),
        "realloc" => {
            let old = state.allocations.get(&first).copied().unwrap_or(0);
            let content = bytes(engine, first, old.min(second as usize));
            let pointer = state.allocate(engine, second as usize, false);
            write(engine, pointer, &content);
            pointer
        }
        "free" | "_ZdlPv" | "_ZdaPv" => 0,
        "memset" => {
            write(engine, first, &vec![second as u8; third as usize]);
            first
        }
        "memcpy" | "__memcpy_chk" | "memmove" | "__memmove_chk" => {
            write(engine, first, &bytes(engine, second, third as usize));
            first
        }
        "strlen" | "__strlen_chk" => c_string(engine, first).len() as u64,
        "strcmp" | "strncmp" | "memcmp" => {
            let (left, right) = if name == "memcmp" {
                (
                    bytes(engine, first, third as usize),
                    bytes(engine, second, third as usize),
                )
            } else {
                (c_string(engine, first), c_string(engine, second))
            };
            let count = if name == "strcmp" {
                left.len().max(right.len()) + 1
            } else {
                third as usize
            };
            (0..count)
                .map(|index| {
                    i32::from(*left.get(index).unwrap_or(&0))
                        - i32::from(*right.get(index).unwrap_or(&0))
                })
                .find(|difference| *difference != 0)
                .unwrap_or(0) as i64 as u64
        }
        "strstr" => {
            let haystack = c_string(engine, first);
            let needle = c_string(engine, second);
            if needle.is_empty() {
                first
            } else {
                haystack
                    .windows(needle.len())
                    .position(|window| window == needle)
                    .map_or(0, |index| first + index as u64)
            }
        }
        "strchr" | "memchr" => {
            let content = if name == "strchr" {
                let mut content = c_string(engine, first);
                content.push(0);
                content
            } else {
                bytes(engine, first, third as usize)
            };
            content
                .iter()
                .position(|byte| *byte == second as u8)
                .map_or(0, |index| first + index as u64)
        }
        "strcpy" | "strncpy" | "__strncpy_chk" => {
            if name == "__strncpy_chk" {
                assert!(third <= read_register(engine, REGISTER_X0 + 3));
            }
            let mut content = c_string(engine, second);
            content.push(0);
            if name != "strcpy" {
                content.resize(third as usize, 0);
            }
            write(engine, first, &content);
            first
        }
        "__open_2" => {
            assert_eq!(c_string(engine, first), b"supplied/Roboto-Regular.ttf");
            state
                .opened_paths
                .push(String::from_utf8(c_string(engine, first)).unwrap());
            assert_eq!(second, 0);
            13
        }
        "fstat" => {
            assert_eq!(first, 13);
            write(engine, second, &[0; 128]);
            write(engine, second + 48, &(state.file_size as u64).to_le_bytes());
            0
        }
        "mmap" => {
            assert_eq!(first, 0);
            assert_eq!(second, state.file_size as u64);
            assert_eq!(third, 1);
            assert_eq!(read_register(engine, REGISTER_X0 + 3), 1);
            assert_eq!(read_register(engine, REGISTER_X0 + 4), 13);
            assert_eq!(read_register(engine, REGISTER_X0 + 5), 0);
            FONT_BYTES
        }
        "close" => {
            assert_eq!(first, 13);
            0
        }
        "munmap" => {
            assert_eq!(first, FONT_BYTES);
            assert_eq!(second, state.file_size as u64);
            0
        }
        "setjmp" | "getenv" => 0,
        "__system_property_get" => {
            write(engine, second, &[0]);
            0
        }
        "atoi" => String::from_utf8(c_string(engine, first))
            .unwrap()
            .parse::<i32>()
            .unwrap_or(0) as i64 as u64,
        "pthread_once" => u64::from(state.once.insert(first)),
        "pthread_key_create" => {
            state.next_thread_key = state.next_thread_key.checked_add(1).unwrap();
            write(engine, first, &state.next_thread_key.to_le_bytes());
            0
        }
        "pthread_getspecific" => state.thread_values.get(&first).copied().unwrap_or(0),
        "pthread_setspecific" => {
            state.thread_values.insert(first, second);
            0
        }
        "pthread_self" => 1,
        "pthread_equal" => u64::from(first == second),
        "pthread_mutex_init"
        | "pthread_mutex_destroy"
        | "pthread_mutex_lock"
        | "pthread_mutex_unlock"
        | "pthread_cond_init"
        | "pthread_cond_destroy"
        | "__cxa_atexit"
        | "__android_log_print" => 0,
        "time" => {
            if first != 0 {
                write(engine, first, &123456_u64.to_le_bytes());
            }
            123456
        }
        "rand_r" => {
            write(engine, first, &123_u32.to_le_bytes());
            123
        }
        _ => panic!("unsupported native font-source import {name} at {address:x}"),
    };
    register(engine, REGISTER_X0, result);
}

struct Recorder {
    engine: Engine,
    hooks: Vec<usize>,
    state: Box<Observation>,
}

impl Recorder {
    fn new(machine: &Machine, imports: BTreeMap<String, u64>) -> Self {
        let mut recorder = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            state: Box::default(),
        };
        recorder.state.imports = imports
            .into_iter()
            .map(|(name, address)| (address, name))
            .collect();
        let addresses: Vec<_> = recorder.state.imports.keys().copied().collect();
        for address in addresses {
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
            let callback_words: &[u32] = match recorder.state.imports[&address].as_str() {
                "pthread_once" => &[
                    0xb40000a0, 0xa9bf7bfd, 0xd63f0020, 0xa8c17bfd, 0x52800000, 0xd65f03c0,
                ],
                "_ZNSt6__ndk111__call_onceERVmPvPFvS2_E" => &[
                    0xb40000c0, 0xa9bf7bfd, 0xaa0103e0, 0xd63f0040, 0xa8c17bfd, 0x52800000,
                    0xd65f03c0,
                ],
                _ => &[],
            };
            for (index, word) in callback_words.iter().enumerate() {
                write(
                    machine.engine,
                    address + index as u64 * 4,
                    &word.to_le_bytes(),
                );
            }
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    imported as *mut c_void,
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

impl Drop for Recorder {
    fn drop(&mut self) {
        for hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, *hook) });
        }
    }
}

fn writable_snapshots(binary: &Binary, engine: Engine, base: u64) -> Vec<(u64, Vec<u8>)> {
    let offset = u64::from_le_bytes(binary.bytes[32..40].try_into().unwrap()) as usize;
    let size = u16::from_le_bytes(binary.bytes[54..56].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(binary.bytes[56..58].try_into().unwrap()) as usize;
    assert_eq!(size, 56);
    binary.bytes[offset..offset + size * count]
        .chunks_exact(size)
        .filter_map(|header| {
            let kind = u32::from_le_bytes(header[..4].try_into().unwrap());
            let flags = u32::from_le_bytes(header[4..8].try_into().unwrap());
            if kind != 1 || flags & 2 == 0 {
                return None;
            }
            let address = u64::from_le_bytes(header[16..24].try_into().unwrap());
            let length = u64::from_le_bytes(header[40..48].try_into().unwrap());
            Some((
                base + address,
                bytes(engine, base + address, length as usize),
            ))
        })
        .collect()
}

fn native_string(machine: &Machine, destination: u64, value: &str) {
    write(machine.engine, INPUT, value.as_bytes());
    write(machine.engine, INPUT + value.len() as u64, &[0]);
    machine.call(TEXT + 0x77e2c, &[destination, INPUT]);
}

fn font_output(machine: &Machine, font: u64, expected_language: &str) -> String {
    let implementation = read_u64(machine.engine, font + 8);
    let source = read_u64(machine.engine, implementation + 8);
    let typeface = read_u64(machine.engine, source + 112);
    assert_ne!(typeface, 0);
    let id = machine.call(TEXT + 0x85d7c, &[font]);
    let bitmap = machine.call(TEXT + 0x85d98, &[font]);
    let language = machine.call(TEXT + 0x85db0, &[font]);
    let language = string(machine.engine, language);
    assert_eq!(language, expected_language);
    assert_eq!(id, u64::from(read_u32(machine.engine, typeface + 16)));
    let index = machine.call(TEXT + 0x85d64, &[font]);
    let length = machine.call(TEXT + 0x85d4c, &[font]);
    let pointer = machine.call(TEXT + 0x85d34, &[font]);
    let tag_count = machine.call(SKIA + 0x21a05c, &[typeface, FONT + 0x800]) as usize;
    assert!(tag_count <= 128);
    let tags = bytes(machine.engine, FONT + 0x800, tag_count * 4)
        .chunks_exact(4)
        .map(|tag| format!("\"{:08x}\"", u32::from_le_bytes(tag.try_into().unwrap())))
        .collect::<Vec<_>>()
        .join(",");
    register(machine.engine, REGISTER_X0 + 8, FONT + 0x900);
    machine.call(TEXT + 0x85dd0, &[font]);
    let copied_implementation = read_u64(machine.engine, FONT + 0x908);
    assert_eq!(copied_implementation, implementation);
    let copied_implementation_id = machine.call(TEXT + 0x877f8, &[copied_implementation]);
    register(machine.engine, REGISTER_X0 + 8, FONT + 0x940);
    machine.call(TEXT + 0x87850, &[implementation]);
    let copied_source = read_u64(machine.engine, FONT + 0x948);
    assert_eq!(copied_source, source);
    let copied_source_id = machine.call(TEXT + 0x88ad4, &[copied_source]);
    assert_eq!(id, copied_implementation_id);
    assert_eq!(id, copied_source_id);
    assert_eq!(machine.call(TEXT + 0x85d7c, &[font]), id);
    format!(
        "{{\"source_id\":{id},\"bitmap\":{},\"language\":{},\"face_index\":{index},\"data_size\":{length},\"data_is_supplied_mapping\":{},\"native_table_tags\":[{tags}],\"copied_implementation_retains_pointer\":true,\"copied_implementation_source_id\":{copied_implementation_id},\"copied_source_retains_pointer\":true,\"copied_source_id\":{copied_source_id}}}",
        bitmap != 0,
        json_string(&language),
        pointer == FONT_BYTES
    )
}

pub(super) const FONT_SHA256: &str =
    "56a45233d29f11b4dfb86d248e921939d115778f87325e7ae8cc108383d6664d";
pub(super) struct NativeFontEnvironment {
    recorder: Recorder,
    snapshots: Vec<(u64, Vec<u8>)>,
    file_size: usize,
}
impl NativeFontEnvironment {
    pub(super) fn new(
        machine: &Machine,
        base: &Path,
        text: &Path,
        skia: &Path,
        font: &Path,
    ) -> Self {
        Self::with_libraries(machine, base, text, skia, font, &[])
    }
    pub(super) fn with_libraries(
        machine: &Machine,
        base: &Path,
        text: &Path,
        skia: &Path,
        font: &Path,
        additional: &[(&Path, u64, &str)],
    ) -> Self {
        Self::with_preloaded_libraries(machine, base, text, skia, font, additional, &[])
    }

    pub(super) fn with_preloaded_libraries(
        machine: &Machine,
        base: &Path,
        text: &Path,
        skia: &Path,
        font: &Path,
        additional: &[(&Path, u64, &str)],
        preloaded: &[(&Path, u64, &str)],
    ) -> Self {
        let digest = Command::new("sha256sum").arg(font).output().unwrap();
        assert!(digest.status.success());
        assert_eq!(
            std::str::from_utf8(&digest.stdout)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap(),
            FONT_SHA256
        );
        let mut libraries = vec![
            (base, BASE, BASE_SHA256),
            (text, TEXT, TEXT_SHA256),
            (skia, SKIA, SKIA_SHA256),
        ];
        libraries.extend_from_slice(additional);
        for &(path, address, hash) in &libraries {
            map_library(machine.engine, path, address, hash);
        }
        for &(path, _, hash) in preloaded {
            verify_library(path, hash);
        }
        libraries.extend_from_slice(preloaded);
        check(unsafe {
            uc_mem_map(
                machine.engine,
                HOST_IMPORT_PRIMARY.start(),
                HOST_REGION_BYTES,
                7,
            )
        });
        check(unsafe {
            uc_mem_map(
                machine.engine,
                HOST_IMPORT_OVERFLOW.start(),
                HOST_IMPORT_OVERFLOW.byte_len(),
                7,
            )
        });
        check(unsafe { uc_mem_map(machine.engine, SOURCE_HEAP, 0x800000, 7) });
        check(unsafe { uc_mem_map(machine.engine, FONT_BYTES, 0x100000, 7) });
        let data = fs::read(font).unwrap();
        assert_eq!(data.len(), 515100);
        write(machine.engine, FONT_BYTES, &data);
        let binaries: Vec<_> = libraries
            .iter()
            .map(|(path, address, _)| (Binary::new(path), *address))
            .collect();
        let mut exports = BTreeMap::new();
        for (binary, address) in &binaries {
            binary.exports(*address, &mut exports);
        }
        let mut imports = BTreeMap::new();
        for (binary, address) in &binaries {
            binary.bind(machine.engine, *address, &exports, &mut imports);
        }
        assert!(imports.len() <= HOST_IMPORT_CAPACITY);
        let recorder = Recorder::new(machine, imports);
        let snapshots: Vec<_> = binaries
            .iter()
            .flat_map(|(binary, address)| writable_snapshots(binary, machine.engine, *address))
            .collect();
        Self {
            recorder,
            snapshots,
            file_size: data.len(),
        }
    }
    pub(super) fn set_host_import_handler(
        &mut self,
        handler: HostImportHandler,
        data: *mut c_void,
    ) {
        self.recorder.state.external_import = Some((handler, data));
    }
    pub(super) fn reset(&mut self, machine: &Machine, fill: u8, seed: u32) {
        for (address, snapshot) in &self.snapshots {
            write(machine.engine, *address, snapshot);
        }
        write(machine.engine, MODEL, &vec![fill; 0x100000]);
        write(machine.engine, FONT, &[0; 0x1000]);
        write(machine.engine, SKIA + 0x2ac318, &seed.to_le_bytes());
        self.recorder.state.allocations.clear();
        self.recorder.state.cursor = SOURCE_HEAP;
        self.recorder.state.fill = fill;
        self.recorder.state.once.clear();
        self.recorder.state.next_thread_key = 0;
        self.recorder.state.thread_values.clear();
        self.recorder.state.opened_paths.clear();
        self.recorder.state.file_size = self.file_size;
    }
    pub(super) fn construct_font(
        &mut self,
        machine: &Machine,
        destination: u64,
        language: &str,
        weight: u64,
        italic: bool,
    ) {
        write(machine.engine, destination, &[0; 24]);
        native_string(machine, FONT + 0x100, "supplied/Roboto-Regular.ttf");
        native_string(machine, FONT + 0x120, language);
        write(machine.engine, FONT + 0x140, &[0; 24]);
        machine.call(
            TEXT + 0x85604,
            &[
                destination,
                FONT + 0x100,
                0,
                FONT + 0x140,
                weight,
                u64::from(italic),
                FONT + 0x120,
            ],
        );
    }
}

pub(super) fn capture(machine: &mut Machine, base: &Path, text: &Path, skia: &Path, font: &Path) {
    let mut environment = NativeFontEnvironment::new(machine, base, text, skia, font);
    let languages = [
        "",
        "und",
        "und-Deva",
        "und-deva",
        "हिन्दी𝄞",
        "und-Deva,en-Latn-US,hi-Deva-IN",
    ];
    let mut outputs = Vec::new();
    for language in languages {
        for (weight, italic) in [(400, false), (700, true)] {
            for seed in [0_u32, 41] {
                let mut canonical = None;
                for fill in [0, 0xa5, 0xff, 0] {
                    environment.reset(machine, fill, seed);
                    environment.construct_font(machine, FONT, language, weight, italic);
                    let first = font_output(machine, FONT, language);
                    assert_eq!(machine.call(TEXT + 0x85d7c, &[FONT]), u64::from(seed + 1));
                    let reserved = machine.call(SKIA + 0x21a85c, &[]);
                    assert_eq!(reserved, u64::from(seed + 2));
                    environment.construct_font(machine, FONT + 0x20, language, weight, italic);
                    let second = font_output(machine, FONT + 0x20, language);
                    assert_eq!(
                        machine.call(TEXT + 0x85d7c, &[FONT + 0x20]),
                        u64::from(seed + 3)
                    );
                    let first_impl = read_u64(machine.engine, FONT + 8);
                    let second_impl = read_u64(machine.engine, FONT + 0x28);
                    assert_ne!(first_impl, second_impl);
                    assert_ne!(
                        read_u64(machine.engine, first_impl + 8),
                        read_u64(machine.engine, second_impl + 8)
                    );
                    assert_eq!(
                        environment.recorder.state.opened_paths,
                        vec!["supplied/Roboto-Regular.ttf"; 2]
                    );
                    let output = format!(
                        "{{\"language_input\":{},\"requested_weight\":{weight},\"requested_italic\":{italic},\"source_counter_seed\":{seed},\"interleaved_reserved_id\":{reserved},\"opened_paths\":{},\"independent_instances_have_distinct_sources\":true,\"instances\":[{first},{second}]}}",
                        json_string(language),
                        json_strings(&environment.recorder.state.opened_paths)
                    );
                    if let Some(previous) = &canonical {
                        assert_eq!(&output, previous);
                    } else {
                        canonical = Some(output);
                    }
                }
                outputs.push(canonical.unwrap());
            }
        }
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"skia_library_sha256\":\"{SKIA_SHA256}\",\"supplied_font_sha256\":\"{FONT_SHA256}\",\"supplied_font_size\":515100,\"memory_fills\":[0,165,255],\"repeat_zero_fill\":true,\"file_font_constructor\":\"0x85604\",\"skia_typeface_factory\":\"0x2197b8\",\"skia_source_counter\":\"0x2ac318\",\"native_bitmap_scan\":\"0x88ae4\",\"capture_boundary\":\"Actual native file Font/FontImplMinikin/MinikinFontImplSkia constructors, Skia CreateFromStream and bundled FreeType execute on hash-pinned supplied Roboto-Regular.ttf. Actual native table parsing, bitmap predicate, raw language getters and shared-pointer copies execute. Host file adapters supply a fixed fd/stat/mmap of pinned bytes; host imports supply allocation, byte/string operations, single-thread bookkeeping and setjmp success with no longjmp path. Counter seeds and an actual NewFontID call explicitly bound process-history IDs. Weight/italic are requested metadata; this capture does not establish synthesis or shaping. No device font XML, family selection, font-manager cache reuse, missing-file/parser-error path, bitmap font file, rendering or cross-process source identity is captured.\",\"cases\":[\n{}\n]}}",
        outputs.join(",\n")
    );
}
