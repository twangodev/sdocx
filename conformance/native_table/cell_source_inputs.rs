use super::*;

const VECTOR: u64 = MODEL + 0x74000;

pub(super) fn cases() -> Vec<Case> {
    let mut cases: Vec<_> = [
        ("uncomposed-cedilla", "A\u{327}B", 0.0, Some(17.0), 0),
        (
            "stacked-overline-dot",
            "A\u{305}\u{307}B",
            0.0,
            Some(17.0),
            0,
        ),
        (
            "marks-across-wrap",
            "A\u{327}B A\u{305}C",
            20.0,
            Some(17.0),
            0,
        ),
        (
            "supplementary-mark-wrap",
            "A😀\u{327}B",
            20.0,
            Some(17.0),
            0,
        ),
        ("mixed-hebrew-wrap", "AV אב A\u{327}B", 25.0, Some(17.0), 1),
        ("mixed-arabic-wrap", "AV بَ A\u{327}B", 25.0, Some(17.0), 1),
        ("leading-mark", "\u{305}AB", 0.0, Some(17.0), 0),
        ("negative-first-ink", "j\u{305}f", 0.0, Some(17.0), 0),
        (
            "newline-mixed-maps",
            "\nAV אב\nA\u{327}B\n",
            25.0,
            Some(17.0),
            1,
        ),
        ("rtl-tab-newline", "אב\tA\u{327}B\nTo", 25.0, Some(17.0), 1),
        ("zero-advance-mark", "\u{200b}\u{305}A", 0.0, Some(17.0), 0),
        ("constructor-font50-marks", "A\u{327}B", 80.0, None, 0),
    ]
    .into_iter()
    .map(|(name, text, width, font_size, direction)| Case {
        name,
        text,
        width,
        font_size,
        direction,
        margins: None,
    })
    .collect();
    cases.push(Case {
        name: "margin-wrapped-marks",
        text: "A\u{327}B A\u{305}C",
        width: 25.0,
        font_size: Some(17.0),
        direction: 0,
        margins: Some([1.25, 2.5, 3.75, 4.5]),
    });
    cases
}

pub(super) fn snapshot(machine: &Machine, object: u64) -> String {
    let component = object + 40;
    let common = read_u64(machine.engine, read_u64(machine.engine, object + 56) + 8);
    assert_ne!(common, 0);
    let text = read_spen_string(machine.engine, machine.call(0x39c9e8, &[component])).unwrap();
    let positions = 0..=text.encode_utf16().count() as u64;
    let fonts: Vec<_> = positions
        .clone()
        .map(|position| scalar_bits(machine, 0x39f504, &[component, position]))
        .collect();
    let foregrounds: Vec<_> = positions
        .map(|position| machine.call(0x39f4ec, &[component, position]) as u32)
        .collect();
    let margins = [0x39e680, 0x39e6c4, 0x39e708, 0x39e74c]
        .map(|getter| scalar_bits(machine, getter, &[component]));
    let gravity = machine.call(0x39f824, &[component]);
    let font_spans = style_spans(machine, common, 3, 8);
    let foreground_spans = style_spans(machine, common, 1, 2);
    let paragraphs = native_list(machine, 0x3e380c, &[common], |paragraph| {
        let kind = machine.call(0x413d8c, &[paragraph]);
        let start = machine.call(0x413e48, &[paragraph]) as i32;
        let end = machine.call(0x413f04, &[paragraph]) as i32;
        let alignment = if kind == 3 {
            machine.call(0x40e5e4, &[paragraph]).to_string()
        } else {
            "null".into()
        };
        format!(
            "{{\"kind\":{kind},\"paragraph_index_range\":[{start},{end}],\"alignment\":{alignment}}}"
        )
    });
    format!(
        "{{\"text_utf8\":{},\"common_present\":true,\"font_size_at_utf16_including_end_bits\":{fonts:?},\"foreground_at_utf16_including_end_argb\":{foregrounds:?},\"gravity\":{gravity},\"margin_bits\":{margins:?},\"font_size_spans\":{font_spans},\"foreground_spans\":{foreground_spans},\"paragraphs\":{paragraphs},\"observation_boundary\":{}}}",
        json_string(&text),
        json_string(
            "Actual Model getters execute after the native Widget conversion, measurement, layout, bounds snapshots and cached emission have completed. Native GetParagraphList (0x3e380c) calls GetParaList (0x400f08), which normalizes owned paragraph start/end through 0x400f90/0x400f9c; this observer is not a pure source read and cannot influence the previously captured producer output. Captured paragraph ranges index paragraphs rather than UTF16 units. Temporary getter vector storage is allocated and deleted through native functions. Measured entries, glyph offsets and paragraph maps are not supplied from fixture expectations."
        )
    )
}

fn scalar_bits(machine: &Machine, address: u64, arguments: &[u64]) -> u32 {
    machine.call(address, arguments);
    read_register(machine.engine, 136) as u32
}

fn style_spans(machine: &Machine, common: u64, kind: u64, mask: u64) -> String {
    native_list(machine, 0x3e32f8, &[common, mask], |span| {
        assert_eq!(machine.call(0x40c698, &[span]), kind);
        let start = machine.call(0x40c754, &[span]) as i32;
        let end = machine.call(0x40c810, &[span]) as i32;
        let interval = machine.call(0x40c8cc, &[span]);
        let value = match kind {
            3 => format!(
                "\"font_size_bits\":{}",
                scalar_bits(machine, 0x409cf8, &[span])
            ),
            1 => format!(
                "\"color_argb\":{},\"color_type\":{}",
                machine.call(0x40a42c, &[span]),
                machine.call(0x40a484, &[span])
            ),
            _ => unreachable!(),
        };
        format!(
            "{{\"kind\":{kind},\"utf16_range\":[{start},{end}],\"interval\":{interval},{value}}}"
        )
    })
}

fn native_list(
    machine: &Machine,
    getter: u64,
    prefix: &[u64],
    observe: impl Fn(u64) -> String,
) -> String {
    write(machine.engine, VECTOR, &[0; 24]);
    let mut arguments = prefix.to_vec();
    arguments.push(VECTOR);
    machine.call(getter, &arguments);
    let begin = read_u64(machine.engine, VECTOR);
    let end = read_u64(machine.engine, VECTOR + 8);
    let capacity = read_u64(machine.engine, VECTOR + 16);
    assert!(end >= begin && capacity >= end && (end - begin) % 8 == 0 && end - begin <= 2048);
    let entries: Vec<_> = (begin..end)
        .step_by(8)
        .map(|pointer| observe(read_u64(machine.engine, pointer)))
        .collect();
    if begin != 0 {
        machine.call(0x47ac00, &[begin]);
    }
    write(machine.engine, VECTOR, &[0; 24]);
    format!("[{}]", entries.join(","))
}
