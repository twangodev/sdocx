use super::*;
use frames::{BASE, BASE_SHA256};

const BODY: u64 = 0x0800_0000;
const BODY_SHA256: &str = "27324ca3807f07e0c1d0647b23eb9af1296762a8c9d892ee486f37b1eb9543f0";
const LAYOUT: u64 = MODEL + 0x10000;
const LAYOUT_VTABLE: u64 = MODEL + 0x11000;
const DOCUMENT: u64 = MODEL + 0x12000;
const DOCUMENT_VTABLE: u64 = MODEL + 0x13000;
const PAGES: u64 = MODEL + 0x14000;
const LINE_VECTOR: u64 = MODEL + 0x15000;
const TEXT_VECTOR: u64 = MODEL + 0x15100;
const LINE_RANGES: u64 = MODEL + 0x16000;
const TEXT_RANGES: u64 = MODEL + 0x17000;
const GETTERS: u64 = MODEL + 0x18000;
const PAGE_COUNT: u64 = GETTERS;
const PAGE: u64 = GETTERS + 32;
const TEXT_LENGTH: u64 = GETTERS + 64;
const LINE_COUNT: u64 = GETTERS + 96;
const LINE_START: u64 = GETTERS + 128;
const LINE_END: u64 = GETTERS + 160;
const LINE_TOP: u64 = GETTERS + 192;
const WIDTH: u64 = GETTERS + 224;
const FIRST_EMPTY: u64 = GETTERS + 256;
const BACKGROUND: u64 = GETTERS + 288;
const CURSOR: u64 = GETTERS + 320;

#[derive(Clone)]
struct Line {
    background: [f32; 4],
    top: f32,
    source: [i32; 2],
}

#[derive(Clone)]
struct Case {
    name: String,
    pages: Vec<[i32; 5]>,
    lines: Vec<Line>,
    text_length: i32,
    source_text: Option<String>,
    first_empty: [f32; 4],
    cursor: [f32; 4],
    invalid_background: [f32; 4],
    first_page: i32,
    initial_lines: Vec<[i32; 2]>,
    initial_text: Vec<[i32; 2]>,
    null_document: bool,
    missing_page: Option<usize>,
}

impl Case {
    fn new(name: &str, pages: Vec<[i32; 5]>, lines: Vec<Line>) -> Self {
        let text_length = lines.last().map_or(0, |line| line.source[1] + 1);
        Self {
            name: name.into(),
            pages,
            lines,
            text_length,
            source_text: None,
            first_empty: [0.0, 2.0, 90.0, 12.0],
            cursor: [0.0, 3.0, 1.0, 13.0],
            invalid_background: [0.0, 0.0, 0.0, 0.0],
            first_page: 0,
            initial_lines: Vec::new(),
            initial_text: Vec::new(),
            null_document: false,
            missing_page: None,
        }
    }
}

struct Inputs {
    case: Case,
    calls: Vec<String>,
}

fn scalar(engine: Engine, slot: usize, value: f32) {
    assert!(value.is_finite());
    register(engine, 136 + slot as i32, u64::from(value.to_bits()));
}

fn rectangle(engine: Engine, bounds: [f32; 4]) {
    for (slot, value) in bounds.into_iter().enumerate() {
        scalar(engine, slot, value);
    }
}

unsafe extern "C" fn get(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let inputs = unsafe { &mut *data.cast::<Inputs>() };
    let index = read_register(engine, REGISTER_X0 + 1) as i32;
    let case = &inputs.case;
    match address {
        PAGE_COUNT => {
            assert_eq!(read_register(engine, REGISTER_X0), DOCUMENT);
            inputs.calls.push("page_count".into());
            register(engine, REGISTER_X0, case.pages.len() as u64);
        }
        PAGE => {
            assert_eq!(read_register(engine, REGISTER_X0), DOCUMENT);
            let index = usize::try_from(index).unwrap();
            assert!(index < case.pages.len());
            inputs.calls.push(format!("page:{index}"));
            register(
                engine,
                REGISTER_X0,
                if case.missing_page == Some(index) {
                    0
                } else {
                    PAGES + index as u64 * 32
                },
            );
        }
        TEXT_LENGTH | LINE_COUNT | WIDTH => {
            assert_eq!(read_register(engine, REGISTER_X0), LAYOUT);
            let (name, value) = match address {
                TEXT_LENGTH => ("text_length", case.text_length),
                LINE_COUNT => ("line_count", case.lines.len() as i32),
                WIDTH => ("layout_width", 90),
                _ => unreachable!(),
            };
            inputs.calls.push(name.into());
            register(engine, REGISTER_X0, value as u32 as u64);
        }
        LINE_START | LINE_END | LINE_TOP => {
            assert_eq!(read_register(engine, REGISTER_X0), LAYOUT);
            let name = match address {
                LINE_START => "line_start",
                LINE_END => "line_end",
                _ => "line_top",
            };
            inputs.calls.push(format!("{name}:{index}"));
            if case.lines.is_empty() {
                assert_eq!((address, index, case.text_length), (LINE_START, 0, 0));
                register(engine, REGISTER_X0, 0);
            } else {
                let line = &case.lines[usize::try_from(index).unwrap()];
                if address == LINE_TOP {
                    scalar(engine, 0, line.top);
                } else {
                    register(
                        engine,
                        REGISTER_X0,
                        line.source[usize::from(address == LINE_END)] as u32 as u64,
                    );
                }
            }
        }
        FIRST_EMPTY | CURSOR | BACKGROUND => {
            assert_eq!(read_register(engine, REGISTER_X0), LAYOUT);
            let bounds = if address == FIRST_EMPTY {
                assert_eq!(index, 90);
                inputs.calls.push("first_empty:90".into());
                case.first_empty
            } else if address == CURSOR {
                inputs.calls.push("default_cursor".into());
                case.cursor
            } else {
                inputs.calls.push(format!("background:{index}"));
                if index == -1 || case.lines.is_empty() {
                    case.invalid_background
                } else {
                    case.lines[usize::try_from(index).unwrap()].background
                }
            };
            rectangle(engine, bounds);
        }
        _ => unreachable!(),
    }
}

struct Interfaces {
    engine: Engine,
    hooks: Vec<usize>,
    inputs: Box<Inputs>,
}

impl Interfaces {
    fn new(machine: &Machine, case: Case) -> Self {
        let mut interfaces = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            inputs: Box::new(Inputs {
                case,
                calls: Vec::new(),
            }),
        };
        for address in [
            PAGE_COUNT,
            PAGE,
            TEXT_LENGTH,
            LINE_COUNT,
            LINE_START,
            LINE_END,
            LINE_TOP,
            WIDTH,
            FIRST_EMPTY,
            BACKGROUND,
            CURSOR,
        ] {
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    get as *mut c_void,
                    ptr::from_mut(interfaces.inputs.as_mut()).cast(),
                    address,
                    address,
                )
            });
            interfaces.hooks.push(hook);
        }
        interfaces
    }
}

impl Drop for Interfaces {
    fn drop(&mut self) {
        for hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, *hook) });
        }
    }
}

fn initialize(machine: &Machine, case: &Case, fill: u8) {
    assert!(case.pages.len() <= 16 && case.lines.len() <= 64);
    assert!(case.initial_lines.len() <= 16 && case.initial_text.len() <= 16);
    write(machine.engine, MODEL, &vec![fill; 0x100000]);
    for (object, vtable, members) in [
        (
            LAYOUT,
            LAYOUT_VTABLE,
            vec![
                (24, WIDTH),
                (80, TEXT_LENGTH),
                (176, LINE_COUNT),
                (208, LINE_START),
                (216, LINE_END),
                (248, LINE_TOP),
                (264, FIRST_EMPTY),
            ],
        ),
        (
            DOCUMENT,
            DOCUMENT_VTABLE,
            vec![(104, PAGE_COUNT), (120, PAGE)],
        ),
    ] {
        write(machine.engine, object, &vtable.to_le_bytes());
        for (offset, function) in members {
            write(machine.engine, vtable + offset, &function.to_le_bytes());
        }
    }
    for (index, page) in case.pages.iter().enumerate() {
        for (member, value) in page.iter().enumerate() {
            write(
                machine.engine,
                PAGES + index as u64 * 32 + member as u64 * 4,
                &value.to_le_bytes(),
            );
        }
    }
    for (vector, storage, initial) in [
        (LINE_VECTOR, LINE_RANGES, &case.initial_lines),
        (TEXT_VECTOR, TEXT_RANGES, &case.initial_text),
    ] {
        for (offset, pointer) in [
            (0, storage),
            (8, storage + initial.len() as u64 * 8),
            (16, storage + 128),
        ] {
            write(machine.engine, vector + offset, &pointer.to_le_bytes());
        }
        for (index, range) in initial.iter().enumerate() {
            for (member, value) in range.iter().enumerate() {
                write(
                    machine.engine,
                    storage + index as u64 * 8 + member as u64 * 4,
                    &value.to_le_bytes(),
                );
            }
        }
    }
}

fn ranges(engine: Engine, vector: u64) -> Vec<[i32; 2]> {
    let start = read_u64(engine, vector);
    let end = read_u64(engine, vector + 8);
    assert!(end >= start && end - start <= 128 && (end - start) % 8 == 0);
    (0..(end - start) / 8)
        .map(|index| {
            [
                read_u32(engine, start + index * 8) as i32,
                read_u32(engine, start + index * 8 + 4) as i32,
            ]
        })
        .collect()
}

fn fixture(machine: &Machine, case: &Case, fill: u8) -> String {
    initialize(machine, case, fill);
    let mut interfaces = Interfaces::new(machine, case.clone());
    let document = if case.null_document { 0 } else { DOCUMENT };
    let mut boundaries = Vec::new();
    let tested_lines = if case.text_length == 0 {
        1
    } else {
        case.lines.len()
    };
    for page in 0..=case.pages.len() {
        for line in 0..tested_lines {
            let down = machine.call(
                BODY + 0xb7d90,
                &[LAYOUT, document, page as u64, line as u64],
            );
            let down_calls = std::mem::take(&mut interfaces.inputs.calls);
            let up = machine.call(
                BODY + 0xb7e88,
                &[LAYOUT, document, page as u64, line as u64],
            );
            let up_calls = std::mem::take(&mut interfaces.inputs.calls);
            assert!(down <= 1 && up <= 1);
            boundaries.push(format!("{{\"page\":{page},\"line\":{line},\"down\":{},\"up\":{},\"down_calls\":{down_calls:?},\"up_calls\":{up_calls:?}}}", down != 0, up != 0));
        }
    }
    let indexer_executed = case.missing_page.is_none();
    if indexer_executed {
        machine.call(
            BODY + 0xb77a4,
            &[
                LAYOUT,
                document,
                case.first_page as u32 as u64,
                LINE_VECTOR,
                TEXT_VECTOR,
            ],
        );
    }
    let lines: Vec<_> = case
        .lines
        .iter()
        .map(|line| {
            format!(
                "{{\"background\":{:?},\"top\":{:?},\"source_inclusive\":{:?}}}",
                line.background, line.top, line.source
            )
        })
        .collect();
    format!(
        "{{\"name\":{:?},\"pages\":{:?},\"lines\":[{}],\"text_length_utf16\":{},\"source_text\":{},\"first_empty_rect\":{:?},\"default_cursor_rect\":{:?},\"invalid_background_rect\":{:?},\"first_page\":{},\"initial_line_sections\":{:?},\"initial_text_sections\":{:?},\"null_document\":{},\"missing_page\":{},\"boundaries\":[{}],\"indexer_executed\":{indexer_executed},\"line_sections\":{:?},\"text_sections\":{:?},\"indexer_calls\":{:?}}}",
        case.name,
        case.pages,
        lines.join(","),
        case.text_length,
        case.source_text
            .as_ref()
            .map_or("null".into(), |text| format!("{text:?}")),
        case.first_empty,
        case.cursor,
        case.invalid_background,
        case.first_page,
        case.initial_lines,
        case.initial_text,
        case.null_document,
        case.missing_page
            .map_or("null".into(), |index| index.to_string()),
        boundaries.join(","),
        ranges(machine.engine, LINE_VECTOR),
        ranges(machine.engine, TEXT_VECTOR),
        interfaces.inputs.calls
    )
}

fn line(top: f32, bottom: f32, start: i32, end: i32) -> Line {
    Line {
        background: [0.0, top, 90.0, bottom],
        top,
        source: [start, end],
    }
}

fn cases() -> Vec<Case> {
    let pages = vec![[0, 0, 0, 90, 20], [20, 0, 0, 90, 20], [40, 0, 0, 90, 20]];
    let mut cases = Vec::new();
    for (name, top, bottom) in [
        ("before-start", 1.0, 19.0),
        ("ends-at-start", 1.0, 20.0),
        ("crosses-start", 1.0, 21.0),
        ("starts-at-start", 20.0, 30.0),
        ("before-end", 39.0, 45.0),
        ("starts-at-end", 40.0, 50.0),
        ("after-end", 41.0, 50.0),
        ("zero-at-start", 20.0, 20.0),
        ("zero-at-end", 40.0, 40.0),
        ("zero-before-start", 19.0, 19.0),
        ("covers-three-pages", 1.0, 59.0),
        ("inverted-background", 25.0, 15.0),
        (
            "one-ulp-before-start",
            f32::from_bits(20_f32.to_bits() - 1),
            25.0,
        ),
        (
            "one-ulp-after-start",
            f32::from_bits(20_f32.to_bits() + 1),
            25.0,
        ),
        (
            "bottom-one-ulp-before-start",
            1.0,
            f32::from_bits(20_f32.to_bits() - 1),
        ),
        (
            "bottom-one-ulp-after-start",
            1.0,
            f32::from_bits(20_f32.to_bits() + 1),
        ),
        (
            "top-one-ulp-before-end",
            f32::from_bits(40_f32.to_bits() - 1),
            45.0,
        ),
        (
            "top-one-ulp-after-end",
            f32::from_bits(40_f32.to_bits() + 1),
            45.0,
        ),
    ] {
        cases.push(Case::new(
            name,
            pages.clone(),
            vec![line(top, bottom, 0, 5)],
        ));
    }
    cases.push(Case::new(
        "adjoining-lines-disjoint-text",
        pages.clone(),
        vec![
            line(1.0, 20.0, 0, 1),
            line(20.0, 40.0, 2, 5),
            line(40.0, 60.0, 6, 8),
        ],
    ));
    cases.push(Case::new(
        "crossing-line-shared-utf16-range",
        pages.clone(),
        vec![
            line(1.0, 25.0, 0, 1),
            line(25.0, 45.0, 2, 5),
            line(45.0, 59.0, 6, 8),
        ],
    ));
    cases.push(Case::new(
        "empty-gap-before-later-line",
        pages.clone(),
        vec![line(41.0, 55.0, 4, 8)],
    ));
    let source = "ab😀cdef";
    let mut surrogate = Case::new(
        "surrogate-source-crossing-pages",
        pages.clone(),
        vec![
            line(1.0, 25.0, 0, 1),
            line(25.0, 45.0, 2, 4),
            line(45.0, 59.0, 5, 7),
        ],
    );
    surrogate.text_length = source.encode_utf16().count() as i32;
    surrogate.source_text = Some(source.into());
    cases.push(surrogate);
    cases.push(Case::new(
        "local-rect-top-uses-native-height",
        vec![[100, 0, 5, 90, 25]],
        vec![line(119.0, 125.0, 0, 2)],
    ));
    cases.push(Case::new(
        "page-end-integer-add-before-f32",
        vec![[16_777_217, 0, 0, 90, 1]],
        vec![line(16_777_216.0, 16_777_220.0, 0, 2)],
    ));
    cases.push(Case::new(
        "only-first-page-has-lines",
        pages.clone(),
        vec![line(1.0, 10.0, 0, 2)],
    ));
    cases.push(Case::new(
        "only-last-page-has-lines",
        pages.clone(),
        vec![line(45.0, 55.0, 0, 2)],
    ));
    cases.push(Case::new(
        "single-page-inclusive-end",
        vec![[0, 0, 0, 90, 20]],
        vec![line(1.0, 10.0, 0, 1), line(10.0, 19.0, 2, 5)],
    ));
    cases.push(Case::new(
        "no-pages",
        Vec::new(),
        vec![line(1.0, 10.0, 0, 2)],
    ));
    cases.push(Case::new(
        "nonempty-text-zero-lines",
        pages.clone(),
        Vec::new(),
    ));
    cases.last_mut().unwrap().text_length = 3;
    for top in [-1.0, 0.0, f32::from_bits(1), 1.0] {
        let mut case = Case::new(
            &format!("first-line-fallback-{top:?}"),
            pages.clone(),
            vec![line(top, 25.0, 0, 2)],
        );
        case.first_empty = [0.0, 25.0, 90.0, 45.0];
        cases.push(case);
    }
    let mut distinct = Case::new(
        "line-top-independent-of-background-top",
        pages.clone(),
        vec![line(1.0, 25.0, 0, 1), line(20.0, 45.0, 2, 5)],
    );
    distinct.lines[0].top = 40.0;
    distinct.lines[1].top = 41.0;
    cases.push(distinct);
    for cursor in [
        [0.0, 3.0, 1.0, 13.0],
        [0.0, 15.0, 1.0, 25.0],
        [0.0, 20.0, 1.0, 20.0],
        [0.0, 40.0, 1.0, 50.0],
    ] {
        let mut case = Case::new(
            &format!("empty-text-cursor-{cursor:?}"),
            pages.clone(),
            Vec::new(),
        );
        case.cursor = cursor;
        cases.push(case);
    }
    for first_page in [1, 2, 3] {
        let mut case = Case::new(
            &format!("rescan-from-page-{first_page}"),
            pages.clone(),
            vec![
                line(1.0, 25.0, 0, 1),
                line(25.0, 45.0, 2, 5),
                line(45.0, 59.0, 6, 8),
            ],
        );
        case.first_page = first_page;
        case.initial_lines = vec![[0, 1], [0, 2], [1, 2]];
        case.initial_text = vec![[0, 2], [0, 6], [2, 7]];
        cases.push(case);
    }
    let mut shrink = Case::new(
        "native-vector-shrink",
        vec![[0, 0, 0, 90, 20]],
        vec![line(1.0, 10.0, 0, 2)],
    );
    shrink.initial_lines = vec![[5, 5], [6, 6], [7, 7]];
    shrink.initial_text = vec![[8, 8], [9, 9], [10, 10]];
    cases.push(shrink);
    for prior in [[-1, 0], [1, 0], [1, 1]] {
        let mut case = Case::new(
            &format!("rescan-prior-section-{prior:?}"),
            pages.clone(),
            vec![
                line(1.0, 25.0, 0, 1),
                line(25.0, 45.0, 2, 5),
                line(45.0, 59.0, 6, 8),
            ],
        );
        case.first_page = 1;
        case.initial_lines = vec![prior];
        case.initial_text = vec![[5, 5]];
        cases.push(case);
    }
    let mut null = Case::new(
        "null-document-no-mutation",
        pages.clone(),
        vec![line(1.0, 10.0, 0, 2)],
    );
    null.null_document = true;
    null.initial_lines = vec![[5, 5]];
    null.initial_text = vec![[8, 8]];
    cases.push(null);
    let mut missing = Case::new("missing-page-record", pages, vec![line(1.0, 10.0, 0, 2)]);
    missing.missing_page = Some(0);
    cases.push(missing);
    cases
}

pub(super) fn capture(machine: &mut Machine, base_path: &Path, bodytext_path: &Path) {
    frames::load_base(machine, base_path);
    map_library(machine.engine, bodytext_path, BODY, BODY_SHA256);
    for (plt, target) in [
        (0x1051b0, BODY + 0xb7d90),
        (0x1051c0, BODY + 0xb7e88),
        (0x105240, BACKGROUND),
        (0x105260, CURSOR),
        (0x1043b0, BASE + 0xb08bc),
    ] {
        bind_native(machine.engine, BODY + plt, target);
    }
    let mut captures = Vec::new();
    for case in cases() {
        let expected = fixture(machine, &case, 0);
        for fill in [165, 255] {
            assert_eq!(
                fixture(machine, &case, fill),
                expected,
                "allocation-fill dependence in {}",
                case.name
            );
        }
        captures.push(expected);
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"bodytext_library_sha256\":\"{BODY_SHA256}\",\"allocation_fills\":[0,165,255],\"is_down_line_address\":\"0xb7d90\",\"is_up_line_address\":\"0xb7e88\",\"update_text_range_address\":\"0xb77a4\",\"native_rect_height_address\":\"0xb08bc\",\"adapter\":\"Supplied page records and measured-line getter values; complete native boundary predicates, range scan, inclusive UTF16 conversion, vector resize and Rect::Height execute. Native paragraph shaping, default-cursor construction, first-empty construction and document repagination do not execute.\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}
