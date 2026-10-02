use super::*;
use frames::{BASE, BASE_SHA256};
use geometry::TEXT_SHA256;
use text_font_source::json_string;

const TEXT: u64 = 0x0500_0000;
const SOURCE: u64 = MODEL + 0x18000;
const VIEW: u64 = MODEL + 0x18400;
const ITERATOR: u64 = MODEL + 0x18440;

fn caller_view(machine: &Machine, context: [u32; 2], selected: [u32; 2], rtl: bool) -> String {
    register(machine.engine, REGISTER_SP, STACK);
    register(machine.engine, 1, STACK + 0x120);
    for (number, value) in [
        (19, u64::from(context[0]) | (u64::from(context[1]) << 32)),
        (20, u64::from(selected[0]) | (u64::from(selected[1]) << 32)),
        (23, u64::from(selected[1])),
        (24, u64::from(rtl)),
        (27, VIEW),
    ] {
        register(machine.engine, REGISTER_X0 + number, value);
    }
    check(unsafe { uc_emu_start(machine.engine, TEXT + 0x96f94, TEXT + 0x96fd8, 0, 100) });
    assert_eq!(read_register(machine.engine, 260), TEXT + 0x96fd8);
    let view = STACK + 0xf0;
    let pointer = read_u64(machine.engine, view);
    let origin = (pointer - SOURCE) / 2;
    let length = read_u64(machine.engine, view + 8);
    let start = read_u32(machine.engine, STACK + 104);
    let end = read_u32(machine.engine, STACK + 108);
    format!(
        "{{\"source_origin_utf16\":{origin},\"source_length_utf16\":{length},\"selected_range_in_view_utf16\":[{start},{end}]}}"
    )
}

fn predicate_ranges(machine: &Machine) -> Vec<[u32; 2]> {
    let mut ranges = Vec::new();
    let mut start = None;
    for unit in 0..=65_536 {
        let separator = unit < 65_536 && machine.call(TEXT + 0x9b050, &[unit]) != 0;
        match (start, separator) {
            (None, true) => start = Some(unit as u32),
            (Some(first), false) => {
                ranges.push([first, unit as u32]);
                start = None;
            }
            _ => {}
        }
    }
    ranges
}

fn helper_unit_ranges(machine: &Machine) -> Vec<[u32; 4]> {
    write(machine.engine, SOURCE, &65_u16.to_le_bytes());
    write(machine.engine, SOURCE + 4, &66_u16.to_le_bytes());
    write(machine.engine, VIEW, &SOURCE.to_le_bytes());
    write(machine.engine, VIEW + 8, &3_u32.to_le_bytes());
    let mut ranges: Vec<[u32; 4]> = Vec::new();
    for unit in 0..=u16::MAX {
        write(machine.engine, SOURCE + 2, &unit.to_le_bytes());
        let previous = machine.call(TEXT + 0x9af90, &[VIEW, 3]) as u32;
        let next = machine.call(TEXT + 0x9b0b0, &[VIEW, 0]) as u32;
        if let Some(last) = ranges.last_mut()
            && last[2..] == [previous, next]
        {
            last[1] = u32::from(unit) + 1;
        } else {
            ranges.push([u32::from(unit), u32::from(unit) + 1, previous, next]);
        }
    }
    ranges
}

fn case(machine: &Machine, name: &str, source: &str, fill: u8) -> String {
    write(machine.engine, SOURCE, &vec![fill; 0x500]);
    let units: Vec<_> = source.encode_utf16().collect();
    assert!(units.len() < 256);
    for (index, unit) in units.iter().enumerate() {
        write(
            machine.engine,
            SOURCE + index as u64 * 2,
            &unit.to_le_bytes(),
        );
    }
    write(machine.engine, VIEW, &SOURCE.to_le_bytes());
    write(
        machine.engine,
        VIEW + 8,
        &(units.len() as u32).to_le_bytes(),
    );
    let helper_queries: Vec<_> = (0..=units.len())
        .map(|offset| {
            let previous = machine.call(TEXT + 0x9af90, &[VIEW, offset as u64]);
            let next = machine.call(TEXT + 0x9b0b0, &[VIEW, offset as u64]);
            format!("[{offset},{previous},{next}]")
        })
        .collect();
    let mut boundaries = vec![0];
    let mut offset = 0;
    for character in source.chars() {
        offset += character.len_utf16() as u32;
        boundaries.push(offset);
    }
    let mut selections = Vec::new();
    for &start in &boundaries {
        for &end in boundaries.iter().filter(|&&end| end > start) {
            for rtl in [false, true] {
                write(machine.engine, VIEW + 16, &start.to_le_bytes());
                write(machine.engine, VIEW + 20, &end.to_le_bytes());
                write(machine.engine, VIEW + 24, &[u8::from(rtl)]);
                let mut position = if rtl { end } else { start };
                let mut windows = Vec::new();
                while if rtl {
                    position > start
                } else {
                    position < end
                } {
                    write(machine.engine, ITERATOR, &[fill; 32]);
                    machine.call(TEXT + 0x98168, &[ITERATOR, VIEW, u64::from(position)]);
                    let context_start = read_u32(machine.engine, ITERATOR + 12);
                    let context_end = read_u32(machine.engine, ITERATOR + 16);
                    let selected_start = read_u32(machine.engine, ITERATOR + 20);
                    let selected_end = read_u32(machine.engine, ITERATOR + 24);
                    let view = caller_view(
                        machine,
                        [context_start, context_end],
                        [selected_start, selected_end],
                        rtl,
                    );
                    windows.push(format!("{{\"context_utf16\":[{context_start},{context_end}],\"selected_utf16\":[{selected_start},{selected_end}],\"caller_view\":{view}}}"));
                    let next = if rtl { selected_start } else { selected_end };
                    assert_ne!(position, next, "{name} iterator did not progress");
                    position = next;
                }
                selections.push(format!(
                    "{{\"range_utf16\":[{start},{end}],\"rtl\":{rtl},\"windows\":[{}]}}",
                    windows.join(",")
                ));
            }
        }
    }
    format!(
        "{{\"name\":{},\"source\":{},\"helper_queries\":[{}],\"selections\":[{}]}}",
        json_string(name),
        json_string(source),
        helper_queries.join(","),
        selections.join(",")
    )
}

pub(super) fn capture(machine: &mut Machine, base: &Path, text: &Path) {
    map_library(machine.engine, base, BASE, BASE_SHA256);
    map_library(machine.engine, text, TEXT, TEXT_SHA256);
    let classification = predicate_ranges(machine);
    let helper_classification = helper_unit_ranges(machine);
    for _ in 0..3 {
        assert_eq!(predicate_ranges(machine), classification);
        assert_eq!(helper_unit_ranges(machine), helper_classification);
    }
    let mut cases = vec![
        ("empty".to_owned(), String::new()),
        ("words".to_owned(), "Alpha beta gamma".to_owned()),
        ("spaces".to_owned(), " A  B ".to_owned()),
        ("supplementaries".to_owned(), "A😀B 𝄞C".to_owned()),
        (
            "marks-punctuation".to_owned(),
            "A\u{301},.(Β\u{363})Ж!".to_owned(),
        ),
        (
            "control-nonseparators".to_owned(),
            "A\tB\nC\rD\u{a0}E\u{202f}F".to_owned(),
        ),
        (
            "cjk-boundaries".to_owned(),
            "A\u{33ff}\u{3400}\u{9fff}\u{a000}B".to_owned(),
        ),
        (
            "supplementary-cjk".to_owned(),
            "A\u{20000}B\u{fe0f}C".to_owned(),
        ),
    ];
    for unit in [0x20]
        .into_iter()
        .chain(0x2000..=0x200a)
        .chain(0x200e..=0x200f)
        .chain(0x202a..=0x202e)
        .chain(0x2066..=0x2069)
        .chain([0x3000])
    {
        cases.push((
            format!("separator-{unit:04x}"),
            format!("AB{}CD", char::from_u32(unit).unwrap()),
        ));
    }
    let captures: Vec<_> = cases
        .into_iter()
        .map(|(name, source)| {
            let expected = case(machine, &name, &source, 0);
            for fill in [0xa5, 0xff, 0] {
                assert_eq!(
                    case(machine, &name, &source, fill),
                    expected,
                    "{name} fill/repeat"
                );
            }
            expected
        })
        .collect();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"base_library_sha256\":\"{BASE_SHA256}\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"memory_fills\":[0,165,255],\"repeat\":true,\"predicate\":\"0x9b050\",\"previous\":\"0x9af90\",\"next\":\"0x9b0b0\",\"iterator\":\"0x98168\",\"separator_ranges_utf16\":{classification:?},\"helper_unit_ranges_utf16\":{helper_classification:?},\"capture_boundary\":\"Actual complete native UTF16 separator predicate and backward/forward scans of an A-unit-B stencil are exhaustively queried for all 65536 raw UTF16 units, including surrogate units. Stencil ranges store first/end unit and actual previous-at-3/next-at-0 outputs. Complete backward/forward context scans and directional iterator constructor execute on supplied valid UTF16 paragraph views and every nonempty scalar-aligned selected subrange. Output context/selected endpoints are read from native iterator storage. The actual bounded caller TextView window 0x96f94..0x96fd8 consumes those endpoints; source pointer/length and local selected range are read from its emitted view. Source pointer, length, selected endpoints and direction are inputs. Full measureParagraph, cache lookup, LayoutPiece and HB are not executed; paragraph slicing is established separately by disassembly.\",\"cases\":[{}]}}",
        captures.join(",\n")
    );
}
