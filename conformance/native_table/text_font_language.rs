use super::*;
use frames::{BASE, BASE_SHA256};
use geometry::TEXT_SHA256;
use std::collections::{BTreeMap, BTreeSet};

const TEXT: u64 = 0x0500_0000;
const XML: u64 = 0x0600_0000;
const HOST: u64 = 0x0700_0000;
const XML_SHA256: &str = "46753f76c8c007e78777e8fe7de7b57202f966f9494d4fba2b675c3540e35dbd";
const PARSER: u64 = MODEL + 0x10000;
const INPUT: u64 = MODEL + 0x11000;
const FONT_INPUT: u64 = TEXT + 0xeffb0;

fn bytes(engine: Engine, address: u64, length: usize) -> Vec<u8> {
    assert!(length <= 0x20000);
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

fn json_string(value: &str) -> String {
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
                let next = HOST + imports.len() as u64 * 32;
                let target = exports
                    .get(name)
                    .copied()
                    .unwrap_or_else(|| *imports.entry(name.into()).or_insert(next));
                let addend = i64::from_le_bytes(relocation[16..24].try_into().unwrap());
                let target = target.checked_add_signed(addend).unwrap();
                let address = u64::from_le_bytes(relocation[..8].try_into().unwrap());
                write(engine, base + address, &target.to_le_bytes());
            }
        }
    }
}

#[derive(Default)]
struct Observation {
    imports: BTreeMap<u64, String>,
    allocations: BTreeMap<u64, usize>,
    cursor: u64,
    fill: u8,
    once: BTreeSet<u64>,
    thread_values: BTreeMap<u64, u64>,
    languages: Vec<String>,
    names: Vec<Option<String>>,
    filenames: Vec<String>,
}

impl Observation {
    fn allocate(&mut self, engine: Engine, size: usize, zero: bool) -> u64 {
        let size = size.max(16).next_multiple_of(16);
        let pointer = self.cursor;
        self.cursor += size as u64;
        assert!(self.cursor < TLS);
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
    let state = unsafe { &mut *data.cast::<Observation>() };
    let first = read_register(engine, REGISTER_X0);
    let second = read_register(engine, REGISTER_X0 + 1);
    let third = read_register(engine, REGISTER_X0 + 2);
    if address == FONT_INPUT {
        state
            .languages
            .push(string(engine, read_register(engine, REGISTER_X0 + 4)));
        state
            .names
            .push((third != 0).then(|| String::from_utf8(c_string(engine, third)).unwrap()));
        let node = second;
        let child = read_u64(engine, node + 24);
        state
            .filenames
            .push(String::from_utf8(c_string(engine, read_u64(engine, child + 80))).unwrap());
        register(engine, REGISTER_X0, 0);
        return;
    }
    let name = state.imports.get(&address).unwrap().clone();
    let result = match name.as_str() {
        "malloc" | "_Znwm" | "_Znam" => state.allocate(engine, first as usize, false),
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
        "memcpy" | "memmove" | "__memmove_chk" => {
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
        "strcpy" | "strncpy" => {
            let mut content = c_string(engine, second);
            content.push(0);
            if name == "strncpy" {
                content.resize(third as usize, 0);
            }
            write(engine, first, &content);
            first
        }
        "getenv" => 0,
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
            write(engine, first, &1_u32.to_le_bytes());
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
        _ => panic!("unsupported native font-language import {name} at {address:x}"),
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
        let mut addresses: Vec<_> = recorder.state.imports.keys().copied().collect();
        addresses.push(FONT_INPUT);
        for address in addresses {
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
            if recorder
                .state
                .imports
                .get(&address)
                .is_some_and(|name| name == "pthread_once")
            {
                for (index, word) in [
                    0xb40000a0_u32,
                    0xa9bf7bfd,
                    0xd63f0020,
                    0xa8c17bfd,
                    0x52800000,
                    0xd65f03c0,
                ]
                .into_iter()
                .enumerate()
                {
                    write(
                        machine.engine,
                        address + index as u64 * 4,
                        &word.to_le_bytes(),
                    );
                }
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

pub(super) fn capture(machine: &mut Machine, base: &Path, text: &Path, xml: &Path) {
    let libraries = [
        (base, BASE, BASE_SHA256),
        (text, TEXT, TEXT_SHA256),
        (xml, XML, XML_SHA256),
    ];
    for (path, address, hash) in libraries {
        map_library(machine.engine, path, address, hash);
    }
    check(unsafe { uc_mem_map(machine.engine, HOST, 0x10000, 7) });
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
    assert!(imports.len() < 2048);
    let mut recorder = Recorder::new(machine, imports);
    let snapshots: Vec<_> = [(XML + 0x10e000, 0x4000), (XML + 0x115000, 0x3000)]
        .into_iter()
        .map(|(address, length)| (address, bytes(machine.engine, address, length)))
        .collect();
    let cases = [
        ("missing", "", ""),
        ("empty", " lang=\"\"", ""),
        ("und", " lang=\"und\"", "und"),
        ("deva", " lang=\"und-Deva\"", "und-Deva"),
        ("deva-lower", " lang=\"und-deva\"", "und-deva"),
        ("deva-upper", " lang=\"UND-DEVA\"", "UND-DEVA"),
        ("space", " lang=\" und-Deva \"", " und-Deva "),
        ("multiple", " lang=\"und-Deva,en\"", "und-Deva,en"),
        ("utf8", " lang=\"हिन्दी𝄞\"", "हिन्दी𝄞"),
        (
            "allocated",
            " lang=\"und-Deva,en-Latn-US,hi-Deva-IN\"",
            "und-Deva,en-Latn-US,hi-Deva-IN",
        ),
        (
            "allocated-utf8",
            " lang=\"हिन्दी𝄞-native-language\"",
            "हिन्दी𝄞-native-language",
        ),
        (
            "xml-attribute-whitespace",
            " lang=\"und&#xA;Deva\"",
            "und\nDeva",
        ),
        ("entity", " lang=\"und&#45;Deva\"", "und-Deva"),
    ];
    let mut outputs = Vec::new();
    for (name, attribute, expected) in cases {
        for count in [1, 2] {
            let source = format!(
                "<familyset><family name=\"Roboto\"{attribute}>{}</family></familyset>",
                "<font weight=\"400\">Roboto-Regular.ttf</font>".repeat(count)
            );
            let mut canonical = None;
            for fill in [0, 0xa5, 0xff] {
                for (address, snapshot) in &snapshots {
                    write(machine.engine, *address, snapshot);
                }
                write(machine.engine, MODEL, &vec![fill; 0x100000]);
                write(machine.engine, PARSER, &[0; 8]);
                write(machine.engine, INPUT, source.as_bytes());
                recorder.state.allocations.clear();
                recorder.state.cursor = HEAP;
                recorder.state.fill = fill;
                recorder.state.once.clear();
                recorder.state.thread_values.clear();
                recorder.state.languages.clear();
                recorder.state.names.clear();
                recorder.state.filenames.clear();
                machine.call(TEXT + 0x7c8e0, &[PARSER]);
                let document = machine.call(XML + 0x75890, &[INPUT, source.len() as u64, 0, 0, 0]);
                assert_ne!(document, 0);
                let root = machine.call(XML + 0x7cfdc, &[document]);
                let family = read_u64(machine.engine, root + 24);
                let result = machine.call(TEXT + 0x7ce7c, &[PARSER, family]);
                assert_eq!(result, 0);
                assert_eq!(recorder.state.languages, vec![expected; count]);
                assert_eq!(recorder.state.names, vec![Some("Roboto".into()); count]);
                assert_eq!(recorder.state.filenames, vec!["Roboto-Regular.ttf"; count]);
                let names = recorder
                    .state
                    .names
                    .iter()
                    .map(|name| {
                        name.as_ref()
                            .map_or_else(|| "null".into(), |name| json_string(name))
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let output = format!(
                    "{{\"name\":{},\"xml\":{},\"languages\":{},\"family_names\":[{names}],\"font_filenames\":{},\"family_return\":{result}}}",
                    json_string(&format!("{name}-{count}-records")),
                    json_string(&source),
                    json_strings(&recorder.state.languages),
                    json_strings(&recorder.state.filenames)
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
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"xml_library_sha256\":\"{XML_SHA256}\",\"memory_fills\":[0,165,255],\"read_memory\":\"0x75890\",\"read_family\":\"0x7ce7c\",\"read_font_intercept\":\"0xeffb0\",\"capture_boundary\":\"Actual pinned native libxml parses supplied UTF-8 XML; actual Text FontListParser constructor and readFamily execute. readFont is intercepted before execution and returns false without creating font records, so readFamily returns false. The interception records actual producer family language, name and parsed font text. Host imports supply allocation, byte/string operations, single-thread bookkeeping and fixed time/random seed. No font loading, Skia typeface construction, shaping, selection, numeric source ID, bitmap or rendering is captured.\",\"cases\":[\n{}\n]}}",
        outputs.join(",\n")
    );
}
