use super::*;
use std::{
    collections::BTreeMap,
    ffi::{CString, c_char},
};
use text_font_source::{NativeFontEnvironment, TEXT, bytes, json_string};

const LAYOUT: u64 = MODEL + 0x20000;
const PARAGRAPH: u64 = MODEL + 0x20200;
const RICH_TEXT: u64 = MODEL + 0x20400;
const RICH_PARAGRAPH: u64 = MODEL + 0x20800;
const SOURCE: u64 = MODEL + 0x20a00;
const ENTRIES: u64 = MODEL + 0x21000;
const ENTRY_VECTOR: u64 = MODEL + 0x22000;
const CHANGED: u64 = MODEL + 0x22100;

#[derive(Default)]
struct ParagraphTrace {
    calls: [u32; 4],
    pre_layout_map: Vec<u32>,
    pre_layout_annotations: Vec<[u32; 2]>,
    natural_fold_bits: Option<u32>,
}

unsafe extern "C" fn paragraph_trace(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let trace = unsafe { &mut *data.cast::<ParagraphTrace>() };
    match address - TEXT {
        0x73cd0 => trace.calls[0] += 1,
        0x6a5c0 => trace.calls[1] += 1,
        0x6ab9c => trace.calls[2] += 1,
        0x6b4a4 => trace.calls[3] += 1,
        0x72dd0 | 0x72b98 => trace.natural_fold_bits = Some(read_register(engine, 147) as u32),
        0x72e0c => {
            let count = read_u32(engine, PARAGRAPH + 8) as usize;
            let map = read_u64(engine, PARAGRAPH + 112);
            assert!(count <= 64);
            trace.pre_layout_map = (0..count)
                .map(|index| read_u32(engine, map + index as u64 * 4))
                .collect();
            trace.pre_layout_annotations = (0..count)
                .map(|index| {
                    [52, 56].map(|offset| read_u32(engine, ENTRIES + index as u64 * 80 + offset))
                })
                .collect();
        }
        _ => unreachable!(),
    }
}

struct TraceRecorder {
    state: Box<ParagraphTrace>,
    hooks: Vec<usize>,
    engine: Engine,
}
impl TraceRecorder {
    fn new(machine: &Machine) -> Self {
        let mut recorder = Self {
            state: Box::default(),
            hooks: Vec::new(),
            engine: machine.engine,
        };
        for offset in [
            0x73cd0, 0x6a5c0, 0x6ab9c, 0x6b4a4, 0x72e0c, 0x72dd0, 0x72b98,
        ] {
            let address = TEXT + offset;
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    paragraph_trace as *mut c_void,
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

#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(path: *const c_char, flags: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> i32;
}

pub(super) struct ParagraphIcu {
    library: *mut c_void,
    fallback: Box<text_shaping::HostIcu>,
    names: BTreeMap<u64, String>,
    hooks: Vec<usize>,
    texts: BTreeMap<u64, Vec<u16>>,
    pub(super) calls: Vec<String>,
    engine: Engine,
}

impl ParagraphIcu {
    pub(super) fn new(machine: &Machine) -> Box<Self> {
        let path = CString::new("/usr/lib/x86_64-linux-gnu/libicuuc.so.76.1").unwrap();
        let library = unsafe { dlopen(path.as_ptr(), 2) };
        assert!(!library.is_null());
        Box::new(Self {
            library,
            fallback: text_shaping::HostIcu::new(machine),
            names: BTreeMap::new(),
            hooks: Vec::new(),
            texts: BTreeMap::new(),
            calls: Vec::new(),
            engine: machine.engine,
        })
    }

    fn symbol(&self, name: &str) -> *mut c_void {
        let name = CString::new(format!("{name}_76")).unwrap();
        let symbol = unsafe { dlsym(self.library, name.as_ptr()) };
        assert!(!symbol.is_null());
        symbol
    }

    pub(super) fn version(&self, name: &str) -> [u8; 4] {
        let call: unsafe extern "C" fn(*mut u8) = unsafe { std::mem::transmute(self.symbol(name)) };
        let mut version = [0; 4];
        unsafe { call(version.as_mut_ptr()) };
        version
    }
}

impl Drop for ParagraphIcu {
    fn drop(&mut self) {
        assert!(self.texts.is_empty());
        for hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, *hook) });
        }
        assert_eq!(unsafe { dlclose(self.library) }, 0);
    }
}

fn c_string(engine: Engine, address: u64) -> String {
    let bytes: Vec<_> = (0..1024)
        .map(|offset| bytes(engine, address + offset, 1)[0])
        .take_while(|&byte| byte != 0)
        .collect();
    String::from_utf8(bytes).unwrap()
}

fn host_import(engine: Engine, name: &str, args: [u64; 8], data: *mut c_void) -> Option<u64> {
    let host = unsafe { &mut *data.cast::<ParagraphIcu>() };
    if let Some(result) = paragraph_icu_import(engine, name, args, host) {
        return Some(result);
    }
    if matches!(
        name,
        "pthread_mutexattr_init" | "pthread_mutexattr_destroy" | "pthread_mutexattr_settype"
    ) {
        return Some(0);
    }
    if name == "clock_gettime" {
        write(engine, args[1], &[0; 16]);
        return Some(0);
    }
    if name == "gettid" {
        return Some(1);
    }
    if name == "syscall" {
        assert_eq!(args[0], 178);
        return Some(1);
    }
    text_shaping::host_import(
        engine,
        name,
        args,
        ptr::from_mut(host.fallback.as_mut()).cast(),
    )
}

pub(super) fn paragraph_icu_import(
    engine: Engine,
    name: &str,
    args: [u64; 8],
    host: &mut ParagraphIcu,
) -> Option<u64> {
    if name == "dlsym" {
        let name = c_string(engine, args[1]);
        if let Some(name) = name
            .strip_suffix("_76")
            .filter(|name| name.starts_with("ubidi_") || name.starts_with("ubrk_"))
        {
            let _ = host.symbol(name);
            if let Some((&address, _)) = host.names.iter().find(|(_, saved)| saved.as_str() == name)
            {
                return Some(address);
            }
            assert!(host.names.len() < 64);
            let address = 0x0700_f000 + host.names.len() as u64 * 32;
            host.names.insert(address, name.into());
            write(engine, address, &0xd65f03c0_u32.to_le_bytes());
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    engine,
                    &mut hook,
                    4,
                    icu_call as *mut c_void,
                    ptr::from_mut(host).cast(),
                    address,
                    address,
                )
            });
            host.hooks.push(hook);
            return Some(address);
        }
    }
    None
}

fn utf16(engine: Engine, address: u64, length: u64) -> Vec<u16> {
    assert!(length <= 256);
    bytes(engine, address, length as usize * 2)
        .chunks_exact(2)
        .map(|slot| u16::from_le_bytes(slot.try_into().unwrap()))
        .collect()
}

unsafe extern "C" fn icu_call(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let host = unsafe { &mut *data.cast::<ParagraphIcu>() };
    let name = host.names.get(&address).unwrap().clone();
    host.calls.push(name.clone());
    let function = host.symbol(&name);
    let a: [u64; 8] =
        std::array::from_fn(|index| read_register(engine, REGISTER_X0 + index as i32));
    let result = unsafe {
        match name.as_str() {
            "ubidi_open" => {
                let call: unsafe extern "C" fn() -> *mut c_void = std::mem::transmute(function);
                call() as u64
            }
            "ubidi_close" | "ubrk_close" => {
                let call: unsafe extern "C" fn(*mut c_void) = std::mem::transmute(function);
                call(a[0] as *mut c_void);
                host.texts.remove(&a[0]);
                0
            }
            "ubidi_getBaseDirection" => {
                let text = utf16(engine, a[0], a[1]);
                let call: unsafe extern "C" fn(*const u16, i32) -> i32 =
                    std::mem::transmute(function);
                call(text.as_ptr(), text.len() as i32) as u64
            }
            "ubidi_setPara" => {
                assert_eq!(a[4], 0);
                let text = utf16(engine, a[1], a[2]);
                let mut error = read_u32(engine, a[5]) as i32;
                let call: unsafe extern "C" fn(
                    *mut c_void,
                    *const u16,
                    i32,
                    u8,
                    *const u8,
                    *mut i32,
                ) = std::mem::transmute(function);
                call(
                    a[0] as *mut c_void,
                    text.as_ptr(),
                    text.len() as i32,
                    a[3] as u8,
                    ptr::null(),
                    &mut error,
                );
                write(engine, a[5], &error.to_le_bytes());
                host.texts.insert(a[0], text);
                0
            }
            "ubidi_getLogicalMap" => {
                let mut map = vec![0_i32; host.texts.get(&a[0]).unwrap().len()];
                let mut error = read_u32(engine, a[2]) as i32;
                let call: unsafe extern "C" fn(*mut c_void, *mut i32, *mut i32) =
                    std::mem::transmute(function);
                call(a[0] as *mut c_void, map.as_mut_ptr(), &mut error);
                for (index, value) in map.iter().enumerate() {
                    write(engine, a[1] + index as u64 * 4, &value.to_le_bytes());
                }
                write(engine, a[2], &error.to_le_bytes());
                0
            }
            "ubidi_getLevelAt" => {
                let call: unsafe extern "C" fn(*mut c_void, i32) -> u8 =
                    std::mem::transmute(function);
                u64::from(call(a[0] as *mut c_void, a[1] as i32))
            }
            "ubrk_open" => {
                assert_eq!(a[0], 2);
                let text = utf16(engine, a[2], a[3]);
                let locale = CString::new(c_string(engine, a[1])).unwrap();
                assert!(locale.as_bytes().is_empty());
                let mut error = read_u32(engine, a[4]) as i32;
                let call: unsafe extern "C" fn(
                    i32,
                    *const c_char,
                    *const u16,
                    i32,
                    *mut i32,
                ) -> *mut c_void = std::mem::transmute(function);
                let iterator = call(
                    a[0] as i32,
                    locale.as_ptr(),
                    text.as_ptr(),
                    text.len() as i32,
                    &mut error,
                ) as u64;
                write(engine, a[4], &error.to_le_bytes());
                host.texts.insert(iterator, text);
                iterator
            }
            "ubrk_following" => {
                let call: unsafe extern "C" fn(*mut c_void, i32) -> i32 =
                    std::mem::transmute(function);
                call(a[0] as *mut c_void, a[1] as i32) as i64 as u64
            }
            _ => panic!("unsupported paragraph ICU call {name}"),
        }
    };
    register(engine, REGISTER_X0, result);
}

fn float(engine: Engine, address: u64, value: f32) {
    write(engine, address, &value.to_le_bytes());
}
fn scalar(engine: Engine, index: i32, value: f32) {
    register(engine, 136 + index, u64::from(value.to_bits()));
}
fn pointer(engine: Engine, address: u64, value: u64) {
    write(engine, address, &value.to_le_bytes());
}

struct Case {
    name: &'static str,
    text: &'static str,
    advances: &'static [f32],
    width: u32,
    margins: [f32; 2],
    alignment: u32,
    single_line: bool,
    word_wrap: bool,
}

impl Case {
    fn ordinary(
        name: &'static str,
        text: &'static str,
        advances: &'static [f32],
        width: u32,
    ) -> Self {
        Self {
            name,
            text,
            advances,
            width,
            margins: [0.0; 2],
            alignment: 0,
            single_line: false,
            word_wrap: true,
        }
    }

    fn execute(
        &self,
        machine: &Machine,
        host: &mut ParagraphIcu,
        trace: &mut TraceRecorder,
    ) -> String {
        let source: Vec<_> = self.text.encode_utf16().collect();
        assert_eq!(source.len(), self.advances.len());
        assert!(source.len() <= 64);
        host.calls.clear();
        *trace.state = ParagraphTrace::default();
        write(machine.engine, LAYOUT, &[0; 320]);
        machine.call(TEXT + 0x6fcbc, &[LAYOUT]);
        write(machine.engine, PARAGRAPH, &[0; 136]);
        write(machine.engine, RICH_TEXT, &[0; 8]);
        machine.call(TEXT + 0x61c50, &[RICH_TEXT]);
        assert_eq!(machine.call(TEXT + 0x61d7c, &[RICH_TEXT]), 1);
        assert_eq!(machine.call(TEXT + 0x628d4, &[RICH_TEXT]), 0);
        assert_eq!(machine.call(TEXT + 0x62908, &[RICH_TEXT]), 1);
        let rich_data = read_u64(machine.engine, RICH_TEXT);
        machine.call(TEXT + 0x628c0, &[RICH_TEXT, u64::from(self.single_line)]);
        machine.call(TEXT + 0x628f4, &[RICH_TEXT, u64::from(self.word_wrap)]);
        write(machine.engine, RICH_PARAGRAPH, &[0; 100]);
        register(machine.engine, REGISTER_X0 + 8, RICH_PARAGRAPH);
        machine.call(TEXT + 0x62544, &[RICH_TEXT, u64::MAX]);
        write(
            machine.engine,
            RICH_PARAGRAPH + 8,
            &(source.len() as u32).to_le_bytes(),
        );
        write(
            machine.engine,
            RICH_PARAGRAPH + 28,
            &self.alignment.to_le_bytes(),
        );
        float(machine.engine, RICH_PARAGRAPH + 36, 1.35);
        machine.call(TEXT + 0x72720, &[PARAGRAPH, RICH_PARAGRAPH]);
        pointer(machine.engine, rich_data + 32, RICH_PARAGRAPH);
        pointer(machine.engine, rich_data + 40, RICH_PARAGRAPH + 100);
        pointer(machine.engine, rich_data + 48, RICH_PARAGRAPH + 100);
        float(machine.engine, rich_data + 128, self.margins[0]);
        float(machine.engine, rich_data + 136, self.margins[1]);
        pointer(machine.engine, LAYOUT + 48, RICH_TEXT);
        pointer(machine.engine, LAYOUT + 64, SOURCE);
        pointer(machine.engine, LAYOUT + 72, ENTRY_VECTOR);
        pointer(machine.engine, ENTRY_VECTOR, ENTRIES);
        pointer(
            machine.engine,
            ENTRY_VECTOR + 8,
            ENTRIES + source.len() as u64 * 80,
        );
        pointer(
            machine.engine,
            ENTRY_VECTOR + 16,
            ENTRIES + source.len() as u64 * 80,
        );
        write(machine.engine, CHANGED, &[0; 24]);
        for (index, (&character, &advance)) in source.iter().zip(self.advances).enumerate() {
            write(
                machine.engine,
                SOURCE + index as u64 * 2,
                &character.to_le_bytes(),
            );
            let entry = ENTRIES + index as u64 * 80;
            write(machine.engine, entry, &[0; 80]);
            float(machine.engine, entry, advance);
            float(machine.engine, entry + 4, 17.125);
            float(machine.engine, entry + 60, 17.125);
            for (axis, value) in [3.0, -12.0, 11.0, 6.0].into_iter().enumerate() {
                float(machine.engine, entry + 32 + axis as u64 * 4, value);
            }
            let kind: u32 = match character {
                32 => 1,
                9 => 2,
                10 => 3,
                _ => 0,
            };
            write(machine.engine, entry + 48, &kind.to_le_bytes());
        }
        scalar(machine.engine, 0, 0.0);
        scalar(machine.engine, 1, 0.0);
        scalar(machine.engine, 2, 1_000_000.0);
        assert_eq!(
            machine.call(
                TEXT + 0x7278c,
                &[LAYOUT, PARAGRAPH, u64::from(self.width), 1, CHANGED]
            ),
            1
        );
        assert!(host.texts.is_empty());
        assert_eq!(&trace.state.calls[..2], &[1, 1]);
        if source.is_empty() {
            assert_eq!(&trace.state.calls[2..], &[0, 0]);
        } else {
            assert!(trace.state.calls[2] > 0 && trace.state.calls[3] > 0);
        }
        let begin = read_u64(machine.engine, LAYOUT + 248);
        let end = read_u64(machine.engine, LAYOUT + 256);
        assert_eq!((end - begin) % 56, 0);
        assert!((end - begin) / 56 <= 64);
        let lines = (0..(end - begin) / 56).map(|index| {
            let line = begin + index * 56;
            let slots: [u32; 14] = std::array::from_fn(|slot| read_u32(machine.engine, line + slot as u64 * 4));
            format!("{{\"point_bits\":{:?},\"layout_rect_bits\":{:?},\"ink_rect_bits\":{:?},\"range_utf16_inclusive\":{:?},\"flag\":{},\"metric_bits\":{}}}", &slots[..2], &slots[2..6], &slots[6..10], &slots[10..12], bytes(machine.engine, line + 48, 1)[0], slots[13])
        }).collect::<Vec<_>>().join(",");
        let entries = (0..source.len()).map(|index| {
            let entry = ENTRIES + index as u64 * 80;
            let slots: [u32; 20] = std::array::from_fn(|slot| read_u32(machine.engine, entry + slot as u64 * 4));
            format!("{{\"advance_bits\":{},\"size_bits\":{},\"position_bits\":{:?},\"layout_rect_bits\":{:?},\"ink_rect_bits\":{:?},\"kind\":{},\"direction\":{},\"break_end_utf16\":{},\"metric_bits\":{}}}", slots[0], slots[1], &slots[2..4], &slots[4..8], &slots[8..12], slots[12], slots[13], slots[14], slots[15])
        }).collect::<Vec<_>>().join(",");
        let map = read_u64(machine.engine, PARAGRAPH + 112);
        let logical_map: Vec<_> = (0..source.len())
            .map(|index| read_u32(machine.engine, map + index as u64 * 4))
            .collect();
        format!(
            "{{\"name\":{},\"text_utf8\":{},\"supplied_advances_bits\":{:?},\"supplied_kinds\":{:?},\"width\":{},\"margin_bits\":{:?},\"alignment\":{},\"single_line\":{},\"word_wrap\":{},\"natural_fold_bits\":{},\"natural_width_bits\":{},\"layout_width_integer\":{},\"paragraph_height_bits\":{},\"direction\":{},\"logical_to_visual_before_layout_utf16\":{:?},\"entry_annotations_before_layout\":{:?},\"logical_to_visual_after_layout_utf16\":{:?},\"native_call_counts\":{:?},\"icu_calls\":{:?},\"lines\":[{}],\"entries\":[{}]}}",
            json_string(self.name),
            json_string(self.text),
            self.advances
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            source
                .iter()
                .map(|character| match character {
                    32 => 1,
                    9 => 2,
                    10 => 3,
                    _ => 0,
                })
                .collect::<Vec<_>>(),
            self.width,
            self.margins.map(f32::to_bits),
            self.alignment,
            self.single_line,
            self.word_wrap,
            trace
                .state
                .natural_fold_bits
                .map_or_else(|| "null".into(), |bits| bits.to_string()),
            read_u32(machine.engine, RICH_PARAGRAPH + 20),
            read_u32(machine.engine, LAYOUT + 128),
            read_u32(machine.engine, PARAGRAPH + 16),
            bytes(machine.engine, PARAGRAPH + 24, 1)[0],
            trace.state.pre_layout_map,
            trace.state.pre_layout_annotations,
            logical_map,
            trace.state.calls,
            host.calls,
            lines,
            entries
        )
    }
}

fn cases() -> Vec<Case> {
    let mut cases = vec![
        Case::ordinary("ordinary_exact", "ABC", &[10.0, 12.0, 8.0], 30),
        Case::ordinary("ordinary_wrap", "ABC", &[10.0, 12.0, 8.0], 23),
        Case::ordinary("automatic_width", "A B", &[10.0, 2.0, 12.0], 0),
        Case::ordinary("spaces_wrap", "AA BB", &[10.0, 10.0, 2.0, 10.0, 10.0], 25),
        Case::ordinary("newline_kind3_control", "A\nB", &[10.0, 0.0, 12.0], 30),
        Case::ordinary("tab_wrap", "A\tBC", &[10.0, 8.0, 12.0, 8.0], 30),
        Case::ordinary("rtl", "אבג", &[10.0, 12.0, 8.0], 23),
        Case::ordinary(
            "mixed_bidi",
            "A אב B",
            &[10.0, 2.0, 12.0, 8.0, 2.0, 10.0],
            25,
        ),
        Case::ordinary("oversized_first", "ABC", &[10.0, 12.0, 8.0], 5),
        Case::ordinary("zero_continuations", "e\u{301}B", &[10.0, 0.0, 12.0], 10),
        Case::ordinary("surrogate_continuation", "😀B", &[10.0, 0.0, 12.0], 10),
        Case::ordinary("surrogate_oversize", "😀B", &[10.0, 0.0, 12.0], 5),
        Case::ordinary("cjk_breaks", "漢字漢字", &[10.0, 10.0, 10.0, 10.0], 20),
        Case::ordinary("whitespace_only", "   ", &[2.0, 2.0, 2.0], 5),
        Case::ordinary("empty_forced", "", &[], 30),
    ];
    for (name, alignment) in [("margins", 0), ("right", 1), ("center", 2)] {
        let mut case = Case::ordinary(name, "ABC", &[10.0, 12.0, 8.0], 50);
        case.margins = [3.25, 4.5];
        case.alignment = alignment;
        cases.push(case);
    }
    let mut justify = Case::ordinary("justify", "A B C", &[10.0, 2.0, 12.0, 2.0, 8.0], 50);
    justify.alignment = 3;
    cases.push(justify);
    for (name, text, advances, width, margins) in [
        (
            "nowrap_exact_ceiling",
            "ABC",
            &[10.0, 12.0, 8.0][..],
            30,
            [0.0, 0.0],
        ),
        (
            "nowrap_margins_ceiling",
            "ABC",
            &[10.0, 12.0, 8.0][..],
            50,
            [3.25, 4.5],
        ),
        (
            "nowrap_epsilon_above",
            "ABC",
            &[10.0, 12.0, 7.9995][..],
            30,
            [0.0, 0.0],
        ),
        (
            "nowrap_epsilon_below",
            "ABC",
            &[10.0, 12.0, 7.998][..],
            30,
            [0.0, 0.0],
        ),
        (
            "nowrap_large_f32_fold",
            "ABC",
            &[16_777_216.0, 1.0, 1.0][..],
            16_777_220,
            [0.0, 0.0],
        ),
    ] {
        let mut case = Case::ordinary(name, text, advances, width);
        case.word_wrap = false;
        case.margins = margins;
        cases.push(case);
    }
    let mut single = Case::ordinary("single_line_expands_width", "A B", &[10.0, 2.0, 12.0], 5);
    single.single_line = true;
    cases.push(single);
    cases
}

pub(super) fn capture(
    machine: &mut Machine,
    base: &Path,
    text: &Path,
    skia: &Path,
    font: &Path,
    cpp: &Path,
) {
    for (path, expected) in [
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
    ] {
        let output = Command::new("sha256sum").arg(path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            std::str::from_utf8(&output.stdout)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap(),
            expected
        );
    }
    machine.call_timeout_micros = 0;
    machine.call_instruction_limit = 10_000_000;
    let mut environment = NativeFontEnvironment::with_libraries(
        machine,
        base,
        text,
        skia,
        font,
        &[(
            cpp,
            0x0b00_0000,
            "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
        )],
    );
    let mut host = ParagraphIcu::new(machine);
    assert_eq!(host.version("u_getVersion"), [76, 1, 0, 0]);
    assert_eq!(host.version("u_getUnicodeVersion"), [16, 0, 0, 0]);
    let mut trace = TraceRecorder::new(machine);
    environment.set_host_import_handler(host_import, ptr::from_mut(host.as_mut()).cast());
    let cases = cases();
    let mut outputs = Vec::new();
    for case in cases {
        let mut expected = None;
        for fill in [0, 165, 255, 0] {
            environment.reset(machine, fill, 0);
            let output = case.execute(machine, &mut host, &mut trace);
            if let Some(expected) = &expected {
                assert_eq!(&output, expected);
            } else {
                expected = Some(output);
            }
        }
        outputs.push(expected.unwrap());
    }
    println!(
        concat!(
            "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",",
            "\"model_library_sha256\":\"{}\",\"base_library_sha256\":\"{}\",\"text_library_sha256\":\"{}\",\"skia_library_sha256\":\"{}\",",
            "\"cpp_library_sha256\":\"4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4\",",
            "\"host_icu_uc_sha256\":\"a8e433e81075732faf255b17d4a25ce28632e41fef1a75e727ee7f4ed73ab151\",\"host_icu_data_sha256\":\"a04b2b906193fa1e40f968a3d16d7d6c844a1fafbdd5bce6e9f67b01c124ff24\",",
            "\"host_libc_sha256\":\"fa430b8f298f817a266046af84a77533185ad6fc4406c7d3787b5a0a0c207826\",\"host_libm_sha256\":\"6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3\",",
            "\"host_icu_version\":[76,1,0,0],\"host_unicode_version\":[16,0,0,0],\"memory_fills\":[0,165,255],\"repeat_zero_fill\":true,\"instruction_limit\":10000000,",
            "\"native_addresses\":{{\"rich_text_shell_constructor\":\"0x61c50\",\"rich_text_construct\":\"0x61d7c\",\"default_paragraph\":\"0x62544\",\"layout_paragraph_constructor\":\"0x72720\",\"set_single_line\":\"0x628c0\",\"set_word_wrap\":\"0x628f4\",\"do_paragraph_layout\":\"0x7278c\",\"calculate_paragraph_layout\":\"0x73cd0\",\"do_lay_text_out\":\"0x6a5c0\",\"get_block_info\":\"0x6ab9c\",\"set_layout\":\"0x6b4a4\"}},",
            "\"native_call_count_order\":[\"calculate_paragraph_layout\",\"do_lay_text_out\",\"get_block_info\",\"set_layout\"],\"constructor_default_single_line\":false,\"constructor_default_word_wrap\":true,",
            "\"entry_annotation_order\":[\"direction\",\"break_end_utf16\"],\"native_break_iterator_type\":2,\"native_break_iterator_locale\":\"\",\"supplied_object_type\":0,",
            "\"supplied_paragraph_index\":0,\"supplied_source_start_utf16\":0,\"supplied_force_layout\":true,\"supplied_cursor_bits\":0,\"supplied_top_bits\":0,\"supplied_max_height_bits\":1232348160,\"supplied_entry_size_bits\":1099497472,\"supplied_entry_metric_bits\":1099497472,\"supplied_spacing_bits\":1068289229,\"supplied_old_point_bits\":[0,0],\"supplied_ink_bits\":[1077936128,3242196992,1093664768,1086324736],",
            "\"resolved_paragraph_icu_symbols\":{:?},\"capture_boundary\":{},\"cases\":[{}]}}"
        ),
        LIBRARY_SHA256,
        frames::BASE_SHA256,
        geometry::TEXT_SHA256,
        text_font_source::SKIA_SHA256,
        host.names.values().collect::<Vec<_>>(),
        json_string(
            "Actual RichText constructor establishes single-line false and word-wrap true; actual setters select the recorded controls, actual GetParagraph obtains defaults and the native layout-paragraph constructor copies the caller-adjusted count, alignment and spacing. Complete DoParagraphLayout, CalculateParagraphLayout, DoLayTextOut, GetBlockInfo, SetLayout, native alignment/baseline/rectangle helpers and bundled libc++ vector/function algorithms execute. Source UTF16, dense 80-byte measured advances, kinds, size/metric17.125, oldpoint0 and ink rectangles are supplied. Kinds1(space),2(tab),3(newline control),0(other) are harness inputs, not native classification. Native ICU loader resolves suffix76 and pinned host ICU76.1/Unicode16 executes actual break/bidi algorithms on retained source buffers. Native dir/break annotations, pre/post logical maps, selected line ranges, placed entry logical/ink rectangles, line metrics and conditional widthfold/ceiling are observed. Width0 and single-line controls write integer natural width into layout member128; explicit word-wrap disabled controls write rounded paragraph member20; default wrapping does not execute either widthfold. Empty source is forced-layout control, not the unforced empty-font-measurement branch. Newline inside the supplied single paragraph does not establish native paragraph splitting. Oversized surrogate control selects inclusive UTF16 ranges[0,0],[1,1],[2,2]; scalar/grapheme-safe emergency splitting is not established. Entry/font measurement, source classification, font selection, spans/objects/bullets, obstacles/padding, pagination, draw clips, vector export and Android device ICU are excluded. Host bounded allocation/byte operations, single-thread mutex/gettid and zero clock service are boundaries. Line record padding bytes49..51 are uninitialized and excluded."
        ),
        outputs.join(",\n")
    );
}
