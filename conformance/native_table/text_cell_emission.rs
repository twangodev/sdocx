use super::*;

pub(super) fn newline_cases() -> Vec<Case> {
    [
        ("leading-newline", "\nAV"),
        ("consecutive-newlines", "AV\n\nTo"),
        ("newline-only", "\n"),
    ]
    .into_iter()
    .map(|(name, text)| Case {
        name,
        text,
        width: 80.0,
        font_size: Some(17.0),
        direction: 0,
        margins: None,
    })
    .collect()
}

pub(super) struct Input {
    pub entries: Vec<String>,
    pub context: String,
}

impl Input {
    pub fn capture(machine: &Machine, rich: u64, paragraphs: &[u64]) -> Self {
        let engine = machine.engine;
        let count = read_u32(engine, rich + 104) as usize;
        assert!(count <= 64);
        let implementation = read_u64(engine, rich);
        let paragraph_begin = read_u64(engine, implementation + 32);
        let paragraph_end = read_u64(engine, implementation + 40);
        assert!(
            paragraph_end >= paragraph_begin
                && (paragraph_end - paragraph_begin).is_multiple_of(100)
        );
        let paragraph_count = machine.call(TEXT + 0x62518, &[rich]) as usize;
        assert_eq!(
            paragraph_count as u64,
            (paragraph_end - paragraph_begin) / 100
        );
        let mut entries = Vec::new();
        for index in 0..count {
            let span = MODEL + 0x74000;
            write(engine, span, &[0; 72]);
            register(engine, REGISTER_X0 + 8, span);
            machine.call(TEXT + 0x61f3c, &[rich, index as u64]);
            let paragraph_index = machine.call(TEXT + 0x62344, &[rich, index as u64]) as i32;
            let flag = if paragraph_index < 0 || paragraph_index as usize >= paragraph_count {
                false
            } else {
                bytes(
                    engine,
                    paragraph_begin + paragraph_index as u64 * 100 + 65,
                    1,
                )[0] != 0
            };
            entries.push(format!(
                ",\"span\":{},\"paragraph_index\":{paragraph_index},\"paragraph_override\":{flag}",
                span_json(machine, span)
            ));
        }
        let layout_paragraphs = paragraphs.iter().map(|&paragraph| {
            let start = read_u32(engine, paragraph + 4);
            let length = read_u32(engine, paragraph + 8);
            assert!(length <= 64);
            let map = read_u64(engine, paragraph + 112);
            assert!(length == 0 || map != 0);
            let inverse_logical_map: Vec<_> = (0..length).map(|index| read_u32(engine, map + u64::from(index) * 4)).collect();
            format!("{{\"paragraph_index\":{},\"source_start_utf16\":{start},\"source_length_utf16\":{length},\"inverse_logical_map_utf16\":{inverse_logical_map:?},\"flag65\":{}}}", read_u32(engine, paragraph), bytes(engine, paragraph + 65, 1)[0] != 0)
        }).collect::<Vec<_>>();
        Self {
            entries,
            context: format!(
                "{{\"offset_bits\":[0,0],\"gravity_bits\":{},\"range_inclusive\":[0,{}],\"layout_paragraphs\":[{}]}}",
                read_u32(engine, implementation + 212),
                (count as u32).wrapping_sub(1),
                layout_paragraphs.join(",")
            ),
        }
    }
}

fn span_json(machine: &Machine, span: u64) -> String {
    let engine = machine.engine;
    let family = read_spen_string(engine, read_u64(engine, span + 24));
    format!(
        "{{\"font_size_bits\":{},\"foreground\":{},\"background\":{},\"composing_background\":{},\"style_bits\":{},\"family\":{},\"underline\":{},\"correction_foreground\":{},\"flags\":{},\"correction_foreground_enabled\":{}}}",
        read_u32(engine, span),
        read_u32(engine, span + 4),
        read_u32(engine, span + 8),
        read_u32(engine, span + 12),
        bytes(engine, span + 16, 1)[0],
        optional_json(family.as_deref()),
        read_u32(engine, span + 32),
        read_u32(engine, span + 36),
        bytes(engine, span + 40, 1)[0],
        bytes(engine, span + 66, 1)[0] != 0
    )
}
