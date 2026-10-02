use super::*;
use frames::{BASE, BASE_SHA256};

const INPUT: u64 = MODEL + 0x9000;
const OUTPUT: u64 = MODEL + 0xa000;
const CURSOR: u64 = MODEL + 0xb000;
const NO_OP: u64 = 0x0300_0a00;
const COPY: u64 = NO_OP + 32;
const LENGTH: u64 = NO_OP + 64;

#[derive(Clone, Copy, Debug)]
enum Kind {
    Foreground,
    FontName,
    ComposingBackground,
    Composing,
    ComposingTag,
    Suggestion,
    Correction,
}

impl Kind {
    fn id(self) -> u32 {
        match self {
            Self::Foreground => 1,
            Self::FontName => 4,
            Self::ComposingBackground => 15,
            Self::Composing => 16,
            Self::ComposingTag => 18,
            Self::Suggestion => 21,
            Self::Correction => 22,
        }
    }

    fn functions(self) -> [u64; 3] {
        match self {
            Self::Foreground => [0x40a728, 0x40a78c, 0x40a858],
            Self::FontName => [0x4096d0, 0x40974c, 0x4097fc],
            Self::ComposingBackground => [0x415674, 0x41567c, 0x415684],
            Self::Composing => [0x415098, 0x4150a0, 0x4150a8],
            Self::ComposingTag => [0x415bfc, 0x415c04, 0x415c0c],
            Self::Suggestion => [0x418908, 0x418a30, 0x418bf8],
            Self::Correction => [0x419760, 0x419768, 0x419770],
        }
    }

    fn snapshot(self, machine: &Machine, object: u64) -> String {
        match self {
            Self::Foreground => format!(
                "{{\"color\":{},\"color_type\":{}}}",
                machine.call(0x40a42c, &[object]),
                machine.call(0x40a484, &[object])
            ),
            Self::ComposingBackground => {
                format!("{{\"color\":{}}}", machine.call(0x415454, &[object]))
            }
            Self::Composing | Self::ComposingTag => {
                let getter = if matches!(self, Self::Composing) {
                    0x414e64
                } else {
                    0x4159c8
                };
                format!("{{\"enabled\":{}}}", machine.call(getter, &[object]) != 0)
            }
            Self::FontName => format!(
                "{{\"name\":{:?}}}",
                string(machine.engine, machine.call(0x40949c, &[object]))
            ),
            Self::Suggestion => {
                let data = read_u64(machine.engine, object + 16);
                let list = data + 8;
                let count = machine.call(BASE + 0x9d120, &[list]);
                assert!(count <= 4);
                let strings: Vec<_> = (0..count)
                    .map(|index| {
                        let value = machine.call(BASE + 0x9d608, &[list, index]);
                        format!("{:?}", string(machine.engine, value))
                    })
                    .collect();
                format!(
                    "{{\"suggestion_type\":{},\"underline\":{},\"strings\":[{}]}}",
                    machine.call(0x41856c, &[object]),
                    machine.call(0x418680, &[object]),
                    strings.join(",")
                )
            }
            Self::Correction => format!(
                "{{\"color\":{},\"underline\":{},\"enabled\":{},\"strike\":{}}}",
                machine.call(0x419174, &[object]),
                machine.call(0x419320, &[object]),
                machine.call(0x4193e0, &[object]) != 0,
                machine.call(0x4194a8, &[object]) != 0
            ),
        }
    }
}

fn string(engine: Engine, object: u64) -> String {
    assert_ne!(object, 0);
    let implementation = read_u64(engine, object + 8);
    let length = read_u32(engine, implementation + 12) as usize;
    assert!(length <= 64);
    let characters = read_u64(engine, implementation + 16);
    let mut bytes = vec![0; length * 2];
    check(unsafe { uc_mem_read(engine, characters, bytes.as_mut_ptr().cast(), bytes.len()) });
    let units: Vec<_> = bytes
        .chunks_exact(2)
        .map(|unit| u16::from_le_bytes(unit.try_into().unwrap()))
        .collect();
    String::from_utf16(&units).unwrap()
}

fn bytes(engine: Engine, address: u64, length: usize) -> Vec<u8> {
    let mut bytes = vec![0; length];
    check(unsafe { uc_mem_read(engine, address, bytes.as_mut_ptr().cast(), bytes.len()) });
    bytes
}

unsafe extern "C" fn imported(engine: Engine, address: u64, _: u32, _: *mut c_void) {
    let first = read_register(engine, REGISTER_X0);
    match address {
        NO_OP => register(engine, REGISTER_X0, 0),
        COPY => {
            let source = read_register(engine, REGISTER_X0 + 1);
            let length = read_register(engine, REGISTER_X0 + 2) as usize;
            assert!(length <= 1024);
            write(engine, first, &bytes(engine, source, length));
        }
        LENGTH => {
            let length = (0..256)
                .find(|index| bytes(engine, first + index, 1)[0] == 0)
                .expect("unterminated supplied string");
            register(engine, REGISTER_X0, length);
        }
        _ => unreachable!(),
    }
}

struct Imports {
    engine: Engine,
    hooks: Vec<usize>,
}

impl Imports {
    fn new(machine: &Machine) -> Self {
        let mut imports = Self {
            engine: machine.engine,
            hooks: Vec::new(),
        };
        for address in [NO_OP, COPY, LENGTH] {
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    imported as *mut c_void,
                    ptr::null_mut(),
                    address,
                    address,
                )
            });
            imports.hooks.push(hook);
        }
        imports
    }
}

impl Drop for Imports {
    fn drop(&mut self) {
        for hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, *hook) });
        }
    }
}

struct Case {
    name: String,
    kind: Kind,
    version: u32,
    payload: Vec<u8>,
    available: Option<usize>,
}

impl Case {
    fn new(name: impl Into<String>, kind: Kind, version: u32, payload: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            kind,
            version,
            payload,
            available: None,
        }
    }

    fn fixture(&self, machine: &mut Machine, fill: u8) -> String {
        write(machine.engine, MODEL, &vec![fill; 0x100000]);
        machine.heap.cursor = HEAP;
        machine.heap.allocation_fill = fill;
        let object = machine.call(0x415db4, &[u64::from(self.kind.id())]);
        assert!((HEAP..TLS).contains(&object));
        let [size, get, apply] = self.kind.functions();
        let mut record: Vec<_> = [self.kind.id(), 70_000, 70_012, 3]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        record.extend_from_slice(&self.payload);
        assert!(record.len() <= 256);
        let available = self.available.unwrap_or(record.len());
        assert!(available <= record.len());
        write(machine.engine, INPUT, &record);
        write(machine.engine, CURSOR, &0_u32.to_le_bytes());
        register(machine.engine, 136, u64::from(1.0_f32.to_bits()));
        let applied = machine.call(
            apply,
            &[
                object,
                INPUT,
                u64::from(self.version),
                CURSOR,
                available as u64,
                2,
            ],
        ) != 0;
        let consumed = read_u32(machine.engine, CURSOR);
        let base = read_u64(machine.engine, object + 8);
        let header: [u32; 4] =
            std::array::from_fn(|index| read_u32(machine.engine, base + index as u64 * 4));
        let decoded = self.kind.snapshot(machine, object);
        let binary_size = machine.call(size, &[object, 2]) as i32;
        assert!((-1..=256).contains(&binary_size));
        write(machine.engine, OUTPUT, &[0; 256]);
        let written = machine.call(get, &[object, OUTPUT, 2]) != 0;
        let output = if written {
            assert!(binary_size >= 0);
            format!("{:?}", bytes(machine.engine, OUTPUT, binary_size as usize))
        } else {
            "null".into()
        };
        format!(
            "{{\"name\":{:?},\"kind\":{},\"version\":{},\"document_type\":2,\"record\":{record:?},\"available\":{available},\"applied\":{applied},\"consumed\":{consumed},\"header\":{header:?},\"decoded\":{decoded},\"binary_size\":{binary_size},\"written\":{written},\"output\":{output}}}",
            self.name,
            self.kind.id(),
            self.version
        )
    }
}

fn load(machine: &Machine, base: &Path) {
    frames::load_base(machine, base);
    for (plt, target) in [
        (0x47ac30, NEW),
        (0x47ac40, NEW),
        (0x47ac50, DELETE),
        (0x47ac10, NO_OP),
        (0x47ac20, NO_OP),
        (0x48b3e0, COPY),
        (0x47afa0, BASE + 0xc2a18),
        (0x47b0e0, BASE + 0xc3298),
        (0x47b080, BASE + 0xc3934),
        (0x47b0b0, BASE + 0xc36fc),
        (0x47b020, BASE + 0xc49a4),
        (0x47b5d0, BASE + 0xc49cc),
        (0x47af40, BASE + 0xc4924),
        (0x47af50, BASE + 0xc47b0),
        (0x47bed0, BASE + 0x9c868),
        (0x481600, BASE + 0x9c868),
        (0x47bef0, BASE + 0x9c928),
        (0x47b420, BASE + 0x9d120),
        (0x47b430, BASE + 0x9e894),
        (0x47b440, BASE + 0x9d4f0),
        (0x47b450, BASE + 0x9e2ec),
        (0x47b460, BASE + 0x9d000),
        (0x47b470, BASE + 0x9d1c8),
        (0x47b480, BASE + 0x9d73c),
        (BASE + 0xe56f0, NEW),
        (BASE + 0xe55f0, NEW),
        (BASE + 0xe5780, NEW),
        (BASE + 0xe66e0, NEW),
        (BASE + 0xe55d0, DELETE),
        (BASE + 0xe5650, DELETE),
        (BASE + 0xe5f20, NO_OP),
        (BASE + 0xe5f30, NO_OP),
        (BASE + 0xe5f40, NO_OP),
        (BASE + 0xe5f50, NO_OP),
        (BASE + 0xe6130, LENGTH),
        (BASE + 0xe7bd0, MEMSET),
        (BASE + 0xe7be0, COPY),
        (BASE + 0xe7c10, COPY),
    ] {
        bind_native(machine.engine, plt, target);
    }
}

fn words(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn font(name: &str, version: u32) -> Vec<u8> {
    font_bytes(name.as_bytes(), version)
}

fn font_bytes(name: &[u8], version: u32) -> Vec<u8> {
    assert!(name.len() <= 64);
    let mut payload = vec![0x7b; if version >= 8 { 8 } else { 4 }];
    payload.extend_from_slice(&((name.len() + 1) as u16).to_le_bytes());
    payload.extend_from_slice(name);
    payload.push(0);
    payload
}

fn suggestions(strings: &[&str]) -> Vec<u8> {
    assert!(strings.len() <= 4);
    let mut payload = words(&[8192, 0x8012_3456, strings.len() as u32]);
    for text in strings {
        let units: Vec<_> = text.encode_utf16().collect();
        assert!(units.len() <= 64);
        payload.extend_from_slice(&(units.len() as u16).to_le_bytes());
        payload.extend(units.into_iter().flat_map(u16::to_le_bytes));
    }
    payload
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for version in [7, 8] {
        for (name, kind, mut payload) in [
            ("foreground", Kind::Foreground, words(&[0x8012_3456])),
            (
                "composing-background",
                Kind::ComposingBackground,
                words(&[0x8065_4321]),
            ),
            (
                "composing-false",
                Kind::Composing,
                vec![0, 0xab, 0xcd, 0xef],
            ),
            ("composing-true", Kind::Composing, vec![1, 0xab, 0xcd, 0xef]),
            (
                "composing-tag-false",
                Kind::ComposingTag,
                vec![0, 0xab, 0xcd, 0xef],
            ),
            (
                "composing-tag-noncanonical-true",
                Kind::ComposingTag,
                vec![0xff, 0xab, 0xcd, 0xef],
            ),
        ] {
            if version >= 8 {
                payload.extend_from_slice(&0x0000_0001_u32.to_le_bytes());
            }
            cases.push(Case::new(
                format!("{name}-v{version}"),
                kind,
                version,
                payload,
            ));
        }
        for (name, text) in [("empty", ""), ("ascii", "Roboto"), ("utf8", "字体𝄞")] {
            cases.push(Case::new(
                format!("font-{name}-v{version}"),
                Kind::FontName,
                version,
                font(text, version),
            ));
        }
        cases.push(Case::new(
            format!("font-cesu8-v{version}"),
            Kind::FontName,
            version,
            font_bytes(
                &[
                    0xe5, 0xad, 0x97, 0xe4, 0xbd, 0x93, 0xed, 0xa0, 0xb4, 0xed, 0xb4, 0x9e,
                ],
                version,
            ),
        ));
        cases.push(Case::new(
            format!("font-embedded-nul-v{version}"),
            Kind::FontName,
            version,
            font_bytes(b"Ro\0boto", version),
        ));
        for (name, strings) in [
            ("empty", vec![]),
            ("one", vec!["word"]),
            ("two", vec!["word", "字体𝄞"]),
            ("zero-length-entry", vec!["", "word"]),
        ] {
            cases.push(Case::new(
                format!("suggestion-{name}-v{version}"),
                Kind::Suggestion,
                version,
                suggestions(&strings),
            ));
        }
        cases.push(Case::new(
            format!("spell-correction-unsupported-v{version}"),
            Kind::Correction,
            version,
            words(&[0xff12_3456, 0xff65_4321, 1, 1]),
        ));
        cases.push(Case::new(
            format!("suggestion-negative-count-v{version}"),
            Kind::Suggestion,
            version,
            words(&[8192, 0x8012_3456, u32::MAX]),
        ));
    }
    cases.push(Case::new(
        "foreground-unknown-color-type",
        Kind::Foreground,
        8,
        words(&[0x8012_3456, 77]),
    ));
    cases.push(Case::new(
        "composing-background-ignored-tail",
        Kind::ComposingBackground,
        8,
        words(&[0x8065_4321, 0xdead_beef]),
    ));
    cases.push(Case::new(
        "composing-tag-ignored-tail",
        Kind::ComposingTag,
        8,
        vec![1, 0x77, 0x55, 0x33, 0xef, 0xbe, 0xad, 0xde],
    ));
    let mut truncated = Vec::new();
    for source in &cases {
        if source.version != 8 {
            continue;
        }
        let length = 16 + source.payload.len();
        for available in [0, 3, 4, 7, 8, 11, 12, 15, 16, length - 1] {
            truncated.push(Case {
                name: format!("{}-available-{available}", source.name),
                kind: source.kind,
                version: source.version,
                payload: source.payload.clone(),
                available: Some(available),
            });
        }
    }
    cases.extend(truncated);
    cases
}

pub(super) fn capture(machine: &mut Machine, base: &Path) {
    load(machine, base);
    let _imports = Imports::new(machine);
    let output: Vec<_> = cases()
        .into_iter()
        .map(|case| {
            let expected = case.fixture(machine, 0);
            for fill in [0xa5, 0xff] {
                assert_eq!(
                    case.fixture(machine, fill),
                    expected,
                    "memory fill changed {}",
                    case.name
                );
            }
            expected
        })
        .collect();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"memory_fills\":[0,165,255],\"create_span\":\"0x415db4\",\"base_apply\":\"0x40cec4\",\"wdoc_header\":\"0x40cfb4\",\"check_read_buffer\":\"0x2784a4\",\"capture_boundary\":\"Native TextStyleFactory construction, complete Model span ApplyBinary/GetBinary/GetBinarySize, native WDoc header and buffer checks, property getters, Base String construction/UTF conversion and native List operations execute unchanged. Supplied bounded records use document type 2 and span versions 7/8; available byte counts exercise truncation. Allocation/deletion, copy/move, memset, strlen and single-thread recursive mutex operations are host supplied; diagnostics are isolated. Output buffers are explicitly zero initialized, including FontName's bytes that the native writer does not initialize. Partial native mutations after failed decoding remain observable. No whole document parsing, Widget conversion, shaping, run grouping or rendering executes.\",\"cases\":[\n{}\n]}}",
        output.join(",\n")
    );
}
