use super::*;
use frames::{BASE, BASE_SHA256};
use geometry::TEXT_SHA256;

const TEXT: u64 = 0x0500_0000;
const SPANS: u64 = MODEL + 0x18000;
const STRINGS: u64 = SPANS + 0x200;
const INPUTS: u64 = SPANS + 0x400;
const JOIN: u64 = TEXT + 0x8dc28;
const COMPARE: u64 = BASE + 0xc49e4;
const UTF16_COMPARE: u64 = BASE + 0xc204c;
const HOST: u64 = 0x0300_1400;
const LENGTH: u64 = HOST;
const COPY: u64 = HOST + 32;

#[derive(Clone, Debug)]
enum FontName {
    Null,
    Utf8(&'static [u8]),
    Utf16(&'static [u16]),
}

impl FontName {
    fn construct(&self, machine: &Machine, index: u64) -> u64 {
        let object = STRINGS + index * 32;
        let input = INPUTS + index * 256;
        match self {
            Self::Null => 0,
            Self::Utf8(bytes) => {
                assert!(bytes.len() < 128);
                write(machine.engine, input, bytes);
                write(machine.engine, input + bytes.len() as u64, &[0]);
                machine.call(BASE + 0xc2a18, &[object]);
                assert_eq!(machine.call(BASE + 0xc3120, &[object, input]), 1);
                object
            }
            Self::Utf16(units) => {
                assert!(units.len() < 64);
                for (index, unit) in units.iter().copied().chain([0]).enumerate() {
                    write(
                        machine.engine,
                        input + index as u64 * 2,
                        &unit.to_le_bytes(),
                    );
                }
                machine.call(BASE + 0xc2a18, &[object]);
                assert_eq!(
                    machine.call(BASE + 0xc2f54, &[object, input, units.len() as u64]),
                    1
                );
                object
            }
        }
    }

    fn json(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Utf8(bytes) => format!("{{\"utf8_or_cesu8_bytes\":{bytes:?}}}"),
            Self::Utf16(units) => format!("{{\"utf16_units\":{units:?}}}"),
        }
    }
}

fn utf16(engine: Engine, object: u64) -> String {
    if object == 0 {
        return "null".into();
    }
    let implementation = read_u64(engine, object + 8);
    let length = read_u32(engine, implementation + 12);
    assert!(length <= 64);
    let source = read_u64(engine, implementation + 16);
    let units: Vec<_> = (0..length)
        .map(|index| {
            let mut bytes = [0; 2];
            check(unsafe {
                uc_mem_read(
                    engine,
                    source + u64::from(index) * 2,
                    bytes.as_mut_ptr().cast(),
                    2,
                )
            });
            u16::from_le_bytes(bytes)
        })
        .collect();
    format!("{units:?}")
}

#[derive(Clone)]
struct Span {
    size_bits: u32,
    foreground: u32,
    style: u8,
    flags: u8,
    name: FontName,
    ignored: Vec<(u64, u32)>,
}

impl Default for Span {
    fn default() -> Self {
        Self {
            size_bits: 20.0_f32.to_bits(),
            foreground: 0x8012_3456,
            style: 0,
            flags: 0,
            name: FontName::Null,
            ignored: Vec::new(),
        }
    }
}

impl Span {
    fn write(&self, machine: &Machine, index: u64) -> u64 {
        let address = SPANS + index * 72;
        for (offset, value) in [
            (0, self.size_bits),
            (4, self.foreground),
            (16, u32::from(self.style)),
            (40, u32::from(self.flags)),
        ]
        .into_iter()
        .chain(self.ignored.iter().copied())
        {
            assert!(offset + 4 <= 72);
            write(machine.engine, address + offset, &value.to_le_bytes());
        }
        let name = self.name.construct(machine, index);
        write(machine.engine, address + 24, &name.to_le_bytes());
        name
    }

    fn json(&self) -> String {
        let ignored: Vec<_> = self
            .ignored
            .iter()
            .map(|&(offset, value)| [offset, u64::from(value)])
            .collect();
        format!(
            "{{\"font_size_bits\":{},\"foreground\":{},\"style\":{},\"flags\":{},\"font_name\":{},\"ignored_member_writes\":{:?}}}",
            self.size_bits,
            self.foreground,
            self.style,
            self.flags,
            self.name.json(),
            ignored
        )
    }
}

#[derive(Default)]
struct Observation {
    string_compares: usize,
    utf16_compares: usize,
}

unsafe extern "C" fn imported(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observation = unsafe { &mut *data.cast::<Observation>() };
    let first = read_register(engine, REGISTER_X0);
    match address {
        COMPARE => observation.string_compares += 1,
        UTF16_COMPARE => observation.utf16_compares += 1,
        LENGTH => {
            let length = (0..128)
                .find(|offset| {
                    let mut byte = 0;
                    check(unsafe {
                        uc_mem_read(engine, first + offset, ptr::from_mut(&mut byte).cast(), 1)
                    });
                    byte == 0
                })
                .unwrap();
            register(engine, REGISTER_X0, length);
        }
        COPY => {
            let source = read_register(engine, REGISTER_X0 + 1);
            let length = usize::try_from(read_register(engine, REGISTER_X0 + 2)).unwrap();
            assert!(length <= 512);
            let mut bytes = vec![0; length];
            check(unsafe { uc_mem_read(engine, source, bytes.as_mut_ptr().cast(), length) });
            write(engine, first, &bytes);
        }
        _ => unreachable!(),
    }
}

struct Recorder {
    engine: Engine,
    hooks: Vec<usize>,
    observation: Box<Observation>,
}

impl Recorder {
    fn new(machine: &Machine) -> Self {
        let mut recorder = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            observation: Box::default(),
        };
        for address in [LENGTH, COPY] {
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
        }
        for address in [LENGTH, COPY, COMPARE, UTF16_COMPARE] {
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    imported as *mut c_void,
                    ptr::from_mut(recorder.observation.as_mut()).cast(),
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

struct Case {
    name: String,
    left: Span,
    right: Span,
    same_name_pointer: bool,
}

impl Case {
    fn new(name: impl Into<String>, right: Span) -> Self {
        Self {
            name: name.into(),
            left: Span::default(),
            right,
            same_name_pointer: false,
        }
    }

    fn fixture(&self, machine: &mut Machine, fill: u8) -> String {
        write(machine.engine, MODEL, &vec![fill; 0x100000]);
        write(machine.engine, SPANS, &[0; 144]);
        write(machine.engine, STRINGS, &[0; 64]);
        machine.heap.cursor = HEAP;
        machine.heap.allocation_fill = fill;
        let recorder = Recorder::new(machine);
        let left = self.left.write(machine, 0);
        let mut right = self.right.write(machine, 1);
        if self.same_name_pointer {
            right = left;
            write(machine.engine, SPANS + 72 + 24, &right.to_le_bytes());
        }
        let left_units = utf16(machine.engine, left);
        let right_units = utf16(machine.engine, right);
        let results = [
            machine.call(JOIN, &[SPANS, SPANS + 72]),
            machine.call(JOIN, &[SPANS + 72, SPANS]),
            machine.call(JOIN, &[SPANS, SPANS]),
            machine.call(JOIN, &[SPANS + 72, SPANS + 72]),
        ];
        assert!(results.into_iter().all(|value| value <= 1));
        assert_eq!(
            results[0], results[1],
            "{} asymmetric native comparison",
            self.name
        );
        let observation = recorder.observation.as_ref();
        format!(
            "{{\"name\":{:?},\"left\":{},\"right\":{},\"same_name_pointer\":{},\"left_font_utf16\":{left_units},\"right_font_utf16\":{right_units},\"join_forward_reverse_self_left_self_right\":{:?},\"string_compares\":{},\"utf16_compares\":{}}}",
            self.name,
            self.left.json(),
            self.right.json(),
            self.same_name_pointer,
            results.map(|result| result != 0),
            observation.string_compares,
            observation.utf16_compares
        )
    }
}

pub(super) fn capture(machine: &mut Machine, base: &Path, text: &Path) {
    map_library(machine.engine, base, BASE, BASE_SHA256);
    map_library(machine.engine, text, TEXT, TEXT_SHA256);
    check(unsafe { uc_mem_map(machine.engine, HOST & !0xfff, 0x1000, 7) });
    for (plt, target) in [
        (TEXT + 0xf06d0, COMPARE),
        (BASE + 0xe56f0, NEW),
        (BASE + 0xe55f0, NEW),
        (BASE + 0xe5780, NEW),
        (BASE + 0xe66e0, NEW),
        (BASE + 0xe55d0, DELETE),
        (BASE + 0xe5650, DELETE),
        (BASE + 0xe6130, LENGTH),
        (BASE + 0xe7bd0, MEMSET),
        (BASE + 0xe7be0, COPY),
        (BASE + 0xe7c10, COPY),
    ] {
        bind_native(machine.engine, plt, target);
    }
    let mut cases = vec![Case::new("equal-null-fonts", Span::default())];
    for style in 0..=255 {
        cases.push(Case::new(
            format!("style-mask-{style:02x}"),
            Span {
                style,
                ..Span::default()
            },
        ));
    }
    for (name, left_bits, right_bits) in [
        ("positive-negative-zero", 0, 0x80000000),
        ("same-negative-zero", 0x80000000, 0x80000000),
        (
            "adjacent-f32-size",
            20.0_f32.to_bits(),
            20.0_f32.to_bits() + 1,
        ),
        ("same-quiet-nan", 0x7fc00001, 0x7fc00001),
        ("different-quiet-nan", 0x7fc00001, 0x7fc00002),
        ("nan-finite", 0x7fc00001, 20.0_f32.to_bits()),
        ("same-positive-infinity", 0x7f800000, 0x7f800000),
        ("opposite-infinity", 0x7f800000, 0xff800000),
    ] {
        let mut case = Case::new(
            name,
            Span {
                size_bits: right_bits,
                ..Span::default()
            },
        );
        case.left.size_bits = left_bits;
        cases.push(case);
    }
    for (name, foreground) in [
        ("foreground-alpha-only", 0xff123456),
        ("foreground-rgb-only", 0x80654321),
        ("foreground-transparent", 0x00123456),
    ] {
        cases.push(Case::new(
            name,
            Span {
                foreground,
                ..Span::default()
            },
        ));
    }
    for (name, left_flags, right_flags) in [
        ("left-object", 2, 0),
        ("right-object", 0, 2),
        ("both-object", 2, 2),
        ("hyperlink-ignored", 0, 1),
        ("other-flags-ignored", 0, 0xfc),
    ] {
        let mut case = Case::new(
            name,
            Span {
                flags: right_flags,
                ..Span::default()
            },
        );
        case.left.flags = left_flags;
        cases.push(case);
    }
    for (name, offset) in [
        ("background-ignored", 8),
        ("composing-background-ignored", 12),
        ("underline-color-ignored", 32),
        ("correction-foreground-ignored", 36),
        ("link-metadata-ignored", 44),
        ("link-string-metadata-ignored", 52),
        ("inline-overpages-correction-math-flags-ignored", 64),
    ] {
        cases.push(Case::new(
            name,
            Span {
                ignored: vec![(offset, 0xa5123456)],
                ..Span::default()
            },
        ));
    }
    for (name, left, right) in [
        ("null-versus-empty", FontName::Null, FontName::Utf8(b"")),
        (
            "independent-empty",
            FontName::Utf8(b""),
            FontName::Utf8(b""),
        ),
        (
            "independent-equal",
            FontName::Utf8(b"Roboto"),
            FontName::Utf8(b"Roboto"),
        ),
        (
            "different-case",
            FontName::Utf8(b"Roboto"),
            FontName::Utf8(b"roboto"),
        ),
        (
            "different-length",
            FontName::Utf8(b"Roboto"),
            FontName::Utf8(b"Roboto Mono"),
        ),
        (
            "utf8-versus-utf16",
            FontName::Utf8("字体𝄞".as_bytes()),
            FontName::Utf16(&[0x5b57, 0x4f53, 0xd834, 0xdd1e]),
        ),
        (
            "cesu8-versus-utf16",
            FontName::Utf8(&[
                0xe5, 0xad, 0x97, 0xe4, 0xbd, 0x93, 0xed, 0xa0, 0xb4, 0xed, 0xb4, 0x9e,
            ]),
            FontName::Utf16(&[0x5b57, 0x4f53, 0xd834, 0xdd1e]),
        ),
        (
            "utf8-versus-cesu8",
            FontName::Utf8("𝄞".as_bytes()),
            FontName::Utf8(&[0xed, 0xa0, 0xb4, 0xed, 0xb4, 0x9e]),
        ),
        (
            "utf16-embedded-nul-prefix",
            FontName::Utf16(&[0x52, 0x6f, 0, 0x62]),
            FontName::Utf16(&[0x52, 0x6f]),
        ),
        (
            "utf8-embedded-nul-prefix",
            FontName::Utf8(b"Ro\0boto"),
            FontName::Utf8(b"Ro"),
        ),
    ] {
        let mut case = Case::new(
            name,
            Span {
                name: right,
                ..Span::default()
            },
        );
        case.left.name = left;
        cases.push(case);
    }
    let mut shared = Case::new(
        "same-font-name-pointer",
        Span {
            name: FontName::Utf8(b"Roboto"),
            ..Span::default()
        },
    );
    shared.left.name = FontName::Utf8(b"Roboto");
    shared.same_name_pointer = true;
    cases.push(shared);
    let captures: Vec<_> = cases
        .into_iter()
        .map(|case| {
            let expected = case.fixture(machine, 0);
            for fill in [0xa5, 0xff] {
                assert_eq!(
                    case.fixture(machine, fill),
                    expected,
                    "{} memory fill",
                    case.name
                );
            }
            expected
        })
        .collect();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"base_library_sha256\":\"{BASE_SHA256}\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"memory_fills\":[0,165,255],\"joinable_for_measure\":\"0x8dc28\",\"string_compare\":\"0xc49e4\",\"utf16_compare\":\"0xc204c\",\"capture_boundary\":\"Complete native RichTextSpan::JoinableForMeasureTo, Base String construction from UTF-8/CESU-8 or explicit UTF-16 units, String::CompareTo and StringImplBase::Wcscmp execute unchanged. Span member values and bounded string inputs are supplied. Allocation/deletion, memset, strlen and memory copy are host supplied. Forward/reverse and self comparisons are captured. No Widget conversion, document parsing, shaping, font resolution, producer run grouping or painting execute.\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}
