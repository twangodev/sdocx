use super::*;

const VECTOR: u64 = MODEL + 0x18000;
const PARAGRAPH: u64 = MODEL + 0x19000;
const TEXT_COMMON: u64 = MODEL + 0x1a000;
const STRING: u64 = MODEL + 0x1b000;
const NOOP: u64 = 0x0300_0000;
const MAP_INDEX: u64 = NOOP + 32;
const LENGTH: u64 = NOOP + 64;
const APPEND: u64 = NOOP + 96;
const PARAGRAPHS: u64 = NOOP + 128;
const INITIALIZER_START: u64 = 0x41a54c;
const INITIALIZER_END: u64 = 0x41a5ac;
const CREATE_SPANS: u64 = 0x419f3c;
const APPLY: u64 = 0x3efe7c;
const REGISTER_S0: i32 = 136;

#[derive(Clone, Debug, PartialEq)]
struct Span {
    kind: u32,
    range: [u32; 2],
    interval: u32,
    size: Option<f32>,
    bold: Option<bool>,
}

impl Span {
    fn read(engine: Engine, object: u64) -> Self {
        let base = read_u64(engine, object + 8);
        let property = read_u64(engine, object + 16);
        let kind = read_u32(engine, base);
        assert!(matches!(kind, 3 | 5));
        Self {
            kind,
            range: [read_u32(engine, base + 4), read_u32(engine, base + 8)],
            interval: read_u32(engine, base + 12),
            size: (kind == 3).then(|| read_float(engine, property)),
            bold: (kind == 5).then(|| read_u32(engine, property) as u8 != 0),
        }
    }

    fn verified(machine: &Machine, object: u64) -> Self {
        let span = Self::read(machine.engine, object);
        assert_eq!(machine.call(0x40c698, &[object]), u64::from(span.kind));
        assert_eq!(machine.call(0x40c754, &[object]), u64::from(span.range[0]));
        assert_eq!(machine.call(0x40c810, &[object]), u64::from(span.range[1]));
        assert_eq!(machine.call(0x40c8cc, &[object]), u64::from(span.interval));
        if let Some(size) = span.size {
            machine.call(0x409cf8, &[object]);
            assert_eq!(
                read_register(machine.engine, REGISTER_S0) as u32,
                size.to_bits()
            );
        }
        if let Some(bold) = span.bold {
            assert_eq!(machine.call(0x408cd8, &[object]) != 0, bold);
        }
        span
    }

    fn json(&self) -> String {
        let property = match (self.size, self.bold) {
            (Some(size), None) => format!("\"font_size\":{size:?}"),
            (None, Some(bold)) => format!("\"bold\":{bold}"),
            _ => unreachable!(),
        };
        format!(
            "{{\"kind\":{},\"range\":{:?},\"interval\":{},{} }}",
            self.kind, self.range, self.interval, property
        )
    }
}

#[derive(Default)]
struct Observation {
    mapped: [u32; 2],
    length: u32,
    append_order: Vec<Span>,
    paragraph_requests: Vec<[u32; 3]>,
    index_requests: Vec<[u32; 2]>,
}

unsafe extern "C" fn imported(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observation = unsafe { &mut *data.cast::<Observation>() };
    match address {
        NOOP => register(engine, REGISTER_X0, 0),
        MAP_INDEX => {
            let request = read_register(engine, REGISTER_X0 + 1);
            observation
                .index_requests
                .push([request as u32, (request >> 32) as u32]);
            register(
                engine,
                REGISTER_X0,
                u64::from(observation.mapped[0]) | u64::from(observation.mapped[1]) << 32,
            );
        }
        LENGTH => register(engine, REGISTER_X0, u64::from(observation.length)),
        APPEND => {
            let object = read_register(engine, REGISTER_X0 + 1);
            observation.append_order.push(Span::read(engine, object));
            register(engine, REGISTER_X0, 1);
        }
        PARAGRAPHS => {
            observation
                .paragraph_requests
                .push([0, 1, 2].map(|offset| read_register(engine, REGISTER_X0 + offset) as u32));
            register(engine, REGISTER_X0, 1);
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
        for address in [NOOP, MAP_INDEX, LENGTH, APPEND, PARAGRAPHS] {
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
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

fn spans(machine: &Machine) -> Vec<Span> {
    let begin = read_u64(machine.engine, VECTOR);
    let end = read_u64(machine.engine, VECTOR + 8);
    assert!(end >= begin && end - begin <= 128);
    (begin..end)
        .step_by(8)
        .map(|address| Span::verified(machine, read_u64(machine.engine, address)))
        .collect()
}

fn reset(machine: &mut Machine, fill: u8) {
    write(machine.engine, MODEL, &vec![fill; 0x100000]);
    machine.heap.cursor = HEAP;
    machine.heap.allocation_fill = fill;
    for address in [VECTOR, PARAGRAPH, TEXT_COMMON, STRING] {
        write(machine.engine, address, &[0; 512]);
    }
    write(machine.engine, 0x4b8fe1, &[fill; 4]);
    register(machine.engine, REGISTER_SP, STACK);
    check(unsafe {
        uc_emu_start(
            machine.engine,
            INITIALIZER_START,
            INITIALIZER_END,
            1_000_000,
            1000,
        )
    });
    assert_eq!(read_register(machine.engine, 260), INITIALIZER_END);
}

fn list_json(spans: &[Span]) -> String {
    spans.iter().map(Span::json).collect::<Vec<_>>().join(",")
}

fn factory(machine: &mut Machine, style: u32, seeded: bool, fill: u8) -> String {
    reset(machine, fill);
    if seeded {
        assert_eq!(machine.call(CREATE_SPANS, &[3, 7, 11, VECTOR]), 1);
    }
    let before = spans(machine);
    let success = machine.call(CREATE_SPANS, &[u64::from(style), 70_000, 70_012, VECTOR]) != 0;
    let after = spans(machine);
    assert_eq!(&after[..before.len()], before);
    format!(
        "{{\"style\":{style},\"range\":[70000,70012],\"seeded\":{seeded},\"success\":{success},\"before\":[{}],\"after\":[{}]}}",
        list_json(&before),
        list_json(&after)
    )
}

fn application(
    machine: &mut Machine,
    style: u32,
    paragraph_range: [u32; 2],
    mapped: [u32; 2],
    length: u32,
    fill: u8,
) -> String {
    reset(machine, fill);
    let mut recorder = Recorder::new(machine);
    recorder.observation.mapped = mapped;
    recorder.observation.length = length;
    assert_eq!(
        machine.call(
            0x413234,
            &[
                PARAGRAPH,
                u64::from(paragraph_range[0]),
                u64::from(paragraph_range[1]),
                u64::from(style),
                3
            ]
        ),
        1
    );
    write(machine.engine, TEXT_COMMON + 8, &STRING.to_le_bytes());
    let success = machine.call(APPLY, &[TEXT_COMMON, PARAGRAPH]) != 0;
    assert_eq!(recorder.observation.index_requests, [paragraph_range]);
    format!(
        "{{\"style\":{style},\"following_style\":3,\"paragraph_range\":{paragraph_range:?},\"supplied_text_index_map\":{mapped:?},\"supplied_utf16_length\":{length},\"success\":{success},\"index_requests\":{:?},\"append_calls\":[{}],\"paragraph_factory_requests\":{:?}}}",
        recorder.observation.index_requests,
        list_json(&recorder.observation.append_order),
        recorder.observation.paragraph_requests
    )
}

pub(super) fn capture(machine: &mut Machine) {
    check(unsafe { uc_mem_map(machine.engine, NOOP, 0x1000, 7) });
    for (plt, target) in [
        (0x47ac30, NEW),
        (0x480d80, 0x409b18),
        (0x480da0, 0x408b50),
        (0x488870, 0x40c574),
        (0x486530, 0x409c14),
        (0x486550, 0x408c44),
        (0x488980, 0x413c78),
        (0x4810a0, 0x413e48),
        (0x4810b0, 0x413f04),
        (0x4811f0, 0x413420),
        (0x481200, 0x4134dc),
        (0x487fc0, CREATE_SPANS),
        (0x487fb0, MAP_INDEX),
        (0x47b020, LENGTH),
        (0x4878d0, APPEND),
        (0x488160, PARAGRAPHS),
        (0x47ac10, NOOP),
    ] {
        bind_native(machine.engine, plt, target);
    }
    let mut factory_cases = Vec::new();
    for style in 0..=4 {
        for seeded in [false, true] {
            let expected = factory(machine, style, seeded, 0);
            for fill in [0xa5, 0xff] {
                assert_eq!(factory(machine, style, seeded, fill), expected);
            }
            factory_cases.push(expected);
        }
    }
    let mut application_cases = Vec::new();
    for (style, range, mapped, length) in [
        (0, [0, 0], [0, 4], 8),
        (1, [1, 1], [5, 9], 10),
        (2, [2, 3], [8, 15], 12),
        (3, [0, 0], [0, 4], 0),
        (4, [0, 0], [0, 4], 8),
    ] {
        let expected = application(machine, style, range, mapped, length, 0);
        for fill in [0xa5, 0xff] {
            assert_eq!(
                application(machine, style, range, mapped, length, fill),
                expected
            );
        }
        application_cases.push(expected);
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"memory_fills\":[0,165,255],\"global_style_initializer_window\":[\"0x41a54c\",\"0x41a5ac\"],\"create_span_list\":\"0x419f3c\",\"apply_predefined_style\":\"0x3efe7c\",\"capture_boundary\":\"The actual native global style assignment window, complete CreateSpanList, FontSizeSpan/BoldSpan/TextSpanBase constructors and property/range/interval getters execute unchanged. Complete m_ApplyPredefinedStyle executes with actual PredefinedStyleParagraph construction/getters, but paragraph-to-text index mapping and string length are supplied. AppendSpan is intercepted to record generated ordinary span inputs before native destruction; model editing/history/storage does not execute. CreateParagraphList is intercepted to record request order and supplies an empty successful result; paragraph generation, Widget conversion, measurement and rendering do not execute. Allocation/deletion and diagnostics are host supplied. No heading size or bold values are supplied; initializer reads the hash-pinned library globals. Factory seeded cases retain previously generated spans and capture append order, not final effective style precedence.\",\"factory_cases\":[\n{}\n],\"application_cases\":[\n{}\n]}}",
        factory_cases.join(",\n"),
        application_cases.join(",\n")
    );
}
