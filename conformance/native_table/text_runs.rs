use super::*;

const DRAWING: u64 = MODEL + 0x12000;
const RICH_TEXT: u64 = MODEL + 0x12100;
const RICH_IMPL: u64 = MODEL + 0x12200;
const SOURCE: u64 = MODEL + 0x12400;
const SOURCE_IMPL: u64 = MODEL + 0x12440;
const CHARACTERS: u64 = MODEL + 0x12500;
const GLYPH_CACHE: u64 = MODEL + 0x12600;
const GLYPHS: u64 = MODEL + 0x12800;
const OUTPUT: u64 = MODEL + 0x12c00;
const OUTPUT_POINTERS: u64 = MODEL + 0x12d00;
const RUN_FONTS: u64 = MODEL + 0x13000;
const FONT_IMPLS: u64 = MODEL + 0x13100;
const FONT_VTABLE: u64 = MODEL + 0x13200;
const LANGUAGE: u64 = MODEL + 0x14000;
const RUN_PARAGRAPH: u64 = MODEL + 0x14200;
const GET_ID: u64 = 0x0300_0800;
const GET_BITMAP: u64 = GET_ID + 32;
const GET_LANGUAGE: u64 = GET_ID + 64;
const GET_RUNS: u64 = TEXT + 0x66c98;

fn pointer(engine: Engine, address: u64, value: u64) {
    write(engine, address, &value.to_le_bytes());
}

fn word(engine: Engine, address: u64, value: u32) {
    write(engine, address, &value.to_le_bytes());
}

fn byte(engine: Engine, address: u64) -> u8 {
    let mut value = 0;
    check(unsafe { uc_mem_read(engine, address, ptr::from_mut(&mut value).cast(), 1) });
    value
}

struct RunCase {
    placement: Case,
    gravity: f32,
    language: &'static str,
    long_language: bool,
    paragraph_flag65: bool,
    bitmap_font: bool,
    font_change: bool,
    same_font_id: bool,
    glyph_count: usize,
    span_flags: u8,
    rtl: bool,
    preallocated: bool,
    subrange: bool,
    perturbation: &'static str,
}

impl RunCase {
    fn fixture(&self, machine: &mut Machine, fill: u8, cached: bool) -> String {
        let placements = self.placement.place_entries(machine, fill);
        machine.heap.cursor = HEAP;
        machine.heap.allocation_fill = fill;
        for (address, size) in [
            (DRAWING, 160),
            (RICH_TEXT, 16),
            (RICH_IMPL, 256),
            (SOURCE, 24),
            (SOURCE_IMPL, 24),
            (GLYPH_CACHE, 240),
            (OUTPUT, 24),
            (RUN_FONTS, 32),
            (FONT_IMPLS, 64),
            (FONT_VTABLE, 88),
            (LANGUAGE, 24),
            (RUN_PARAGRAPH, 100),
            (SPANS, 432),
        ] {
            write(machine.engine, address, &vec![0; size]);
        }
        let count = self.placement.line_count * 3;
        for (address, value) in [
            (DRAWING + 64, RICH_TEXT),
            (DRAWING + 96, ENTRIES),
            (DRAWING + 120, GLYPH_CACHE),
            (RICH_TEXT, RICH_IMPL),
            (RICH_IMPL, SOURCE),
            (RICH_IMPL + 8, SPANS),
            (RICH_IMPL + 16, SPANS + count as u64 * 72),
            (RICH_IMPL + 24, SPANS + count as u64 * 72),
            (SOURCE + 8, SOURCE_IMPL),
            (SOURCE_IMPL + 16, CHARACTERS),
            (FONT_VTABLE + 48, GET_ID),
            (FONT_VTABLE + 56, GET_BITMAP),
            (FONT_VTABLE + 80, GET_LANGUAGE),
        ] {
            pointer(machine.engine, address, value);
        }
        word(machine.engine, DRAWING + 88, count as u32);
        float(machine.engine, RICH_IMPL + 212, self.gravity);
        assert!(self.language.len() < 23);
        if self.long_language {
            pointer(machine.engine, LANGUAGE, 33);
            pointer(machine.engine, LANGUAGE + 8, self.language.len() as u64);
            pointer(machine.engine, LANGUAGE + 16, LANGUAGE + 32);
            write(machine.engine, LANGUAGE + 32, self.language.as_bytes());
            write(
                machine.engine,
                LANGUAGE + 32 + self.language.len() as u64,
                &[0],
            );
        } else {
            write(machine.engine, LANGUAGE, &[(self.language.len() * 2) as u8]);
            write(machine.engine, LANGUAGE + 1, self.language.as_bytes());
        }
        if self.paragraph_flag65 {
            pointer(machine.engine, RICH_IMPL + 32, RUN_PARAGRAPH);
            pointer(machine.engine, RICH_IMPL + 40, RUN_PARAGRAPH + 100);
            pointer(machine.engine, RICH_IMPL + 48, RUN_PARAGRAPH + 100);
            write(machine.engine, RUN_PARAGRAPH + 65, &[1]);
        }
        for index in 0..2 {
            let font = RUN_FONTS + index * 16;
            let implementation = FONT_IMPLS + index * 32;
            pointer(machine.engine, font + 8, implementation);
            pointer(machine.engine, implementation, FONT_VTABLE);
            word(
                machine.engine,
                implementation + 8,
                7 + if self.same_font_id { 0 } else { index as u32 },
            );
            write(
                machine.engine,
                implementation + 12,
                &[u8::from(self.bitmap_font)],
            );
            pointer(machine.engine, implementation + 16, LANGUAGE);
        }
        let mut inputs = Vec::new();
        for index in 0..count {
            let entry = ENTRIES + index as u64 * 80;
            let span = SPANS + index as u64 * 72;
            float(
                machine.engine,
                span,
                if self.placement.style_change && index == 1 {
                    24.0
                } else {
                    20.0
                },
            );
            word(machine.engine, span + 4, 0xff12_3456);
            word(machine.engine, span + 8, 0x8065_4321);
            write(machine.engine, span + 16, &[3]);
            write(machine.engine, span + 40, &[self.span_flags]);
            write(
                machine.engine,
                CHARACTERS + index as u64 * 2,
                &(65 + index as u16).to_le_bytes(),
            );
            let font = RUN_FONTS
                + if self.font_change && index == 1 {
                    16
                } else {
                    0
                };
            let info = GLYPH_CACHE + index as u64 * 40;
            let glyph = GLYPHS + index as u64 * 36;
            for (address, value) in [
                (info, glyph),
                (info + 8, glyph + self.glyph_count as u64 * 12),
                (info + 16, glyph + self.glyph_count as u64 * 12),
                (info + 24, font),
            ] {
                pointer(machine.engine, address, value);
            }
            write(machine.engine, info + 34, &[1]);
            let mut glyph_inputs = Vec::new();
            for within in 0..self.glyph_count {
                let address = glyph + within as u64 * 12;
                let codeword = ((65 + index as u32) << 8) | (within as u32 + 1);
                let offset = within as f32 * 2.25;
                word(machine.engine, address, codeword);
                float(machine.engine, address + 4, offset);
                float(machine.engine, address + 8, -0.75);
                glyph_inputs.push(format!("[{codeword},{offset:?},-0.75]"));
            }
            if self.rtl {
                let origin = 84.25 - (index % 3) as f32 * 10.0;
                float(machine.engine, entry + 8, origin);
                float(machine.engine, entry, 10.0);
                word(machine.engine, entry + 52, 1);
            }
            if index == 1 {
                match self.perturbation {
                    "x-gap" => {
                        let x = read_float(machine.engine, entry + 8);
                        float(machine.engine, entry + 8, f32::from_bits(x.to_bits() + 1));
                    }
                    "baseline" => {
                        let y = read_float(machine.engine, entry + 12);
                        float(machine.engine, entry + 12, y + 3.5);
                    }
                    "none" => {}
                    _ => unreachable!(),
                }
            }
            inputs.push(format!(
                "{{\"position\":{:?},\"layout_rect\":{:?},\"glyph_rect\":{:?},\"font_wrapper\":{},\"font_id\":{},\"font_size\":{:?},\"glyphs\":[{}]}}",
                [read_float(machine.engine, entry + 8), read_float(machine.engine, entry + 12)],
                rectangle(machine.engine, entry + 16),
                rectangle(machine.engine, entry + 32),
                usize::from(self.font_change && index == 1),
                if self.font_change && !self.same_font_id && index == 1 { 8 } else { 7 },
                read_float(machine.engine, span),
                glyph_inputs.join(",")
            ));
        }
        if self.preallocated {
            pointer(machine.engine, OUTPUT, OUTPUT_POINTERS);
            pointer(machine.engine, OUTPUT + 8, OUTPUT_POINTERS);
            pointer(
                machine.engine,
                OUTPUT + 16,
                OUTPUT_POINTERS + count as u64 * 8,
            );
        }
        let snapshot = cached.then(|| {
            cached_snapshot::CachedInput {
                entries: ENTRIES,
                caches: GLYPH_CACHE,
                spans: SPANS,
                source: CHARACTERS,
                logical_map: LOGICAL_MAP,
                count,
                gravity: self.gravity,
                paragraph_flag65: self.paragraph_flag65,
            }
            .snapshot(machine)
        });
        scalar(machine.engine, 0, self.placement.offset[0]);
        scalar(machine.engine, 1, self.placement.offset[1]);
        let range = if self.subrange {
            [1, count - 2]
        } else {
            [0, count - 1]
        };
        assert_eq!(
            machine.call(
                GET_RUNS,
                &[DRAWING, range[0] as u64, range[1] as u64, OUTPUT, 0]
            ) & 1,
            1
        );
        let begin = read_u64(machine.engine, OUTPUT);
        let end = read_u64(machine.engine, OUTPUT + 8);
        assert!(end >= begin && end - begin <= count as u64 * 8);
        let runs: Vec<_> = (begin..end)
            .step_by(8)
            .map(|address| record(machine, read_u64(machine.engine, address)))
            .collect();
        let mut fixture = format!(
            "{{\"name\":{:?},\"cursor\":{:?},\"margin\":{:?},\"font_size\":{:?},\"base_height\":{:?},\"pixels\":{:?},\"multiplier\":{:?},\"object_metric\":{},\"line_count\":{},\"offset\":{:?},\"gravity\":{:?},\"language\":{:?},\"long_language\":{},\"paragraph_flag65\":{},\"bitmap_font\":{},\"rtl\":{},\"span_flags\":{},\"preallocated_output\":{},\"range_inclusive\":{range:?},\"perturbation\":{:?},\"post_cursors\":{placements:?},\"entries\":[{}],\"runs\":[{}]}}",
            self.placement.name,
            self.placement.cursor,
            self.placement.margin,
            self.placement.font_size,
            self.placement.height,
            self.placement.pixels,
            self.placement.multiplier,
            self.placement.object_metric,
            self.placement.line_count,
            self.placement.offset,
            self.gravity,
            self.language,
            self.long_language,
            self.paragraph_flag65,
            self.bitmap_font,
            self.rtl,
            self.span_flags,
            self.preallocated,
            self.perturbation,
            inputs.join(","),
            runs.join(",")
        );
        if let Some(snapshot) = snapshot {
            assert_eq!(fixture.pop(), Some('}'));
            fixture.push_str(&format!(",\"cached_input\":{snapshot}}}"));
        }
        fixture
    }
}

fn vector(machine: &Machine, address: u64) -> Vec<u32> {
    let begin = read_u64(machine.engine, address);
    let end = read_u64(machine.engine, address + 8);
    assert!(end >= begin && end - begin <= 72 && (end - begin).is_multiple_of(4));
    (begin..end)
        .step_by(4)
        .map(|address| read_u32(machine.engine, address))
        .collect()
}

fn record(machine: &Machine, address: u64) -> String {
    let codes = vector(machine, address + 8);
    let positions: Vec<_> = vector(machine, address + 32)
        .into_iter()
        .map(f32::from_bits)
        .collect();
    assert_eq!(codes.len(), positions.len());
    assert!(positions.iter().all(|value| value.is_finite()));
    format!(
        "{{\"range_inclusive\":[{},{}],\"codewords\":{codes:?},\"positions\":{positions:?},\"origin\":{:?},\"glyph_rect\":{:?},\"layout_rect\":{:?},\"span_bit_1\":{},\"font_id\":{},\"first_codeword_high_bits\":{},\"font_size\":{:?},\"foreground\":{},\"style\":{},\"background\":{}}}",
        read_u32(machine.engine, address),
        read_u32(machine.engine, address + 4),
        [
            read_float(machine.engine, address + 80),
            read_float(machine.engine, address + 84)
        ],
        rectangle(machine.engine, address + 88),
        rectangle(machine.engine, address + 104),
        byte(machine.engine, address + 120),
        read_u32(machine.engine, address + 124) as i32,
        read_u32(machine.engine, address + 128),
        read_float(machine.engine, address + 132),
        read_u32(machine.engine, address + 136),
        byte(machine.engine, address + 140),
        read_u32(machine.engine, address + 144)
    )
}

pub(super) fn capture(machine: &mut Machine) {
    capture_mode(machine, false);
}

pub(super) fn capture_cached(machine: &mut Machine) {
    capture_mode(machine, true);
}

fn capture_mode(machine: &mut Machine, cached: bool) {
    for (plt, target) in [
        (0xef290, TEXT + 0x78274),
        (0xef2a0, TEXT + 0x78074),
        (0xeeef0, TEXT + 0x61f3c),
        (0xeef40, TEXT + 0x62344),
        (0xef380, BASE + 0xc49cc),
        (0xef140, SAME_DRAW),
        (0xef1b0, TEXT + 0x67ebc),
        (0xeec30, NEW),
        (0xeec40, DELETE),
        (0xf08e0, COPY),
    ] {
        bind_native(machine.engine, TEXT + plt, target);
    }
    for (address, instruction) in [
        (GET_ID, 0xb9400800_u32),
        (GET_BITMAP, 0x39403000),
        (GET_LANGUAGE, 0xf9400800),
    ] {
        write(machine.engine, address, &instruction.to_le_bytes());
        write(machine.engine, address + 4, &0xd65f03c0_u32.to_le_bytes());
    }
    let _copy = CopyHook::new(machine);
    let mut captures = Vec::new();
    for (name, font_size, height, pixels, multiplier, object_metric) in [
        ("ordinary", 20.0, 20.0, 0.0, 1.35, false),
        ("pixel-spacing", 20.0, 20.0, 7.0, 1.6, false),
        ("fractional", 17.25, 17.25, 0.0, 1.6, false),
        ("mixed-metrics", 44.0, 60.0, 0.0, 1.35, false),
        ("object-leading", 20.0, 100.0, 0.0, 1.35, true),
        ("large-object-font", 900.0, 100.0, 0.0, 1.35, true),
    ] {
        for alignment in 0..3 {
            for (cursor, margin, offset, gravity) in [
                (0.0, 0.0, [0.0, 0.0], 0.0),
                (3.25, 4.75, [100.25, 200.75], 7.25),
                (-20.5, 1.125, [-30.25, -40.5], -3.5),
            ] {
                for (line_count, style_change) in [(1, false), (2, false), (2, true)] {
                    let case = RunCase {
                        placement: Case {
                            name: format!("{name}-{}", captures.len()),
                            cursor,
                            margin,
                            font_size,
                            height,
                            pixels,
                            multiplier,
                            alignment,
                            object_metric,
                            offset,
                            line_count,
                            style_change,
                        },
                        gravity,
                        ..ordinary()
                    };
                    capture_case(machine, &mut captures, case, cached);
                }
            }
        }
    }
    for name in [
        "multiple-glyphs",
        "font-change",
        "font-wrapper",
        "bitmap-font",
        "und-Deva",
        "und-DevaX",
        "und-Deve",
        "und-Deva-long-string",
        "en-long-string",
        "link-and-bit-1",
        "paragraph-flag65",
        "paragraph-flag65-link",
        "rtl",
        "rtl-multiple-glyphs",
        "subrange",
        "x-gap",
        "baseline",
    ] {
        for line_count in 1..=2 {
            for (offset, gravity) in [([0.0, 0.0], 0.0), ([100.25, 200.75], 7.25)] {
                let mut case = ordinary();
                case.placement.name = format!("{name}-{}", captures.len());
                case.placement.line_count = line_count;
                case.placement.offset = offset;
                case.gravity = gravity;
                case.preallocated = false;
                match name {
                    "multiple-glyphs" => case.glyph_count = 3,
                    "font-change" => case.font_change = true,
                    "font-wrapper" => {
                        case.font_change = true;
                        case.same_font_id = true;
                    }
                    "bitmap-font" => case.bitmap_font = true,
                    "und-Deva" => case.language = "und-Deva",
                    "und-DevaX" => case.language = "und-DevaX",
                    "und-Deve" => case.language = "und-Deve",
                    "und-Deva-long-string" => {
                        case.language = "und-Deva";
                        case.long_language = true;
                    }
                    "en-long-string" => case.long_language = true,
                    "link-and-bit-1" => case.span_flags = 3,
                    "paragraph-flag65" => case.paragraph_flag65 = true,
                    "paragraph-flag65-link" => {
                        case.paragraph_flag65 = true;
                        case.span_flags = 3;
                    }
                    "rtl" => case.rtl = true,
                    "rtl-multiple-glyphs" => {
                        case.rtl = true;
                        case.glyph_count = 3;
                    }
                    "subrange" => case.subrange = true,
                    "x-gap" | "baseline" => case.perturbation = name,
                    _ => unreachable!(),
                }
                capture_case(machine, &mut captures, case, cached);
            }
        }
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"memory_fills\":[0,165,255],\"set_layout\":\"0x6b4a4\",\"get_drawn_text_run\":\"0x66c98\",\"append_text_block\":\"0x67ebc\",\"glyph_info\":\"0x78274\",\"span\":\"0x61f3c\",\"paragraph_index\":\"0x62344\",\"measurement_inputs\":\"Supplied entry/block/line metrics, logical maps, UTF-16 source, cached glyph codewords/offsets, spans, FontImpl ID/bitmap/language getters, caller offsets and vertical gravity. Native SetLayout/GetBaseline, cached GetGlyphInfo/GetSpan, complete getDrawnTextRun grouping/union/vector accumulation and appendTextBlock execute unchanged. Allocation, deletion and memory copy/move are host supplied. Native paragraph vectors are empty or contain one supplied record with flag 65; emoji slices are null. Glyph codewords and cached metrics are test inputs, not outputs of native shaping or font selection. No wrap selection, real embedded object layout, bullets, justification, emoji images, Composer clipping or final PDF paths/pixels execute.\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}

fn ordinary() -> RunCase {
    RunCase {
        placement: Case {
            name: String::new(),
            cursor: 3.25,
            margin: 4.75,
            font_size: 20.0,
            height: 20.0,
            pixels: 0.0,
            multiplier: 1.35,
            alignment: 0,
            object_metric: false,
            offset: [0.0, 0.0],
            line_count: 1,
            style_change: false,
        },
        gravity: 0.0,
        language: "en",
        long_language: false,
        paragraph_flag65: false,
        bitmap_font: false,
        font_change: false,
        same_font_id: false,
        glyph_count: 1,
        span_flags: 0,
        rtl: false,
        preallocated: true,
        subrange: false,
        perturbation: "none",
    }
}

fn capture_case(machine: &mut Machine, captures: &mut Vec<String>, case: RunCase, cached: bool) {
    let expected = case.fixture(machine, 0, cached);
    for fill in [0xa5, 0xff] {
        assert_eq!(
            case.fixture(machine, fill, cached),
            expected,
            "{} memory fill",
            case.placement.name
        );
    }
    captures.push(expected);
}
