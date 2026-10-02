use super::*;

const DRAWING: u64 = MODEL + 0x12000;
const RICH_TEXT: u64 = MODEL + 0x12100;
const RICH_IMPL: u64 = MODEL + 0x12200;
const SOURCE: u64 = MODEL + 0x12400;
const SOURCE_IMPL: u64 = MODEL + 0x12440;
const CHARACTERS: u64 = MODEL + 0x12500;
const GLYPH_CACHE: u64 = MODEL + 0x12600;
const OUTPUT: u64 = MODEL + 0x12c00;
const FONTS: u64 = MODEL + 0x13000;
const FONT_IMPLS: u64 = MODEL + 0x13100;
const FONT_VTABLE: u64 = MODEL + 0x13200;
const LANGUAGE: u64 = MODEL + 0x14000;
const FUNCTOR: u64 = MODEL + 0x16000;
const DATA: u64 = FUNCTOR + 0x100;
const ENTRY_VECTOR: u64 = FUNCTOR + 0x200;
const CACHE_VECTOR: u64 = FUNCTOR + 0x220;
const SOURCE_VECTOR: u64 = FUNCTOR + 0x240;
const RECORDS: u64 = FUNCTOR + 0x300;
const ADVANCES: u64 = FUNCTOR + 0x600;
const HOLDER: u64 = FUNCTOR + 0x640;
const WRAPPER: u64 = FUNCTOR + 0x680;
const GET_ID: u64 = 0x0300_0800;
const GET_BITMAP: u64 = GET_ID + 32;
const GET_LANGUAGE: u64 = GET_ID + 64;
const NOOP: u64 = GET_ID + 96;
const PRODUCER_START: u64 = TEXT + 0x77324;
const PRODUCER_END: u64 = TEXT + 0x77894;
const REGISTER_X29: i32 = 1;
const REGISTER_PC: i32 = 260;

fn pointer(engine: Engine, address: u64, value: u64) {
    write(engine, address, &value.to_le_bytes());
}

fn word(engine: Engine, address: u64, value: u32) {
    write(engine, address, &value.to_le_bytes());
}

fn vector(machine: &Machine, address: u64) -> Vec<u32> {
    let begin = read_u64(machine.engine, address);
    let end = read_u64(machine.engine, address + 8);
    assert!(end >= begin && end - begin <= 96 && (end - begin).is_multiple_of(4));
    (begin..end)
        .step_by(4)
        .map(|address| read_u32(machine.engine, address))
        .collect()
}

enum AdvanceProfile {
    PerCodeUnit,
    ZeroLeading,
    ZeroTrailing,
    ZeroAll,
}

impl AdvanceProfile {
    fn at(&self, index: usize, count: usize) -> f32 {
        let zero = match self {
            Self::PerCodeUnit => false,
            Self::ZeroLeading => index == 0,
            Self::ZeroTrailing => index + 1 == count,
            Self::ZeroAll => true,
        };
        if zero {
            0.0
        } else {
            1000.0 + index as f32 * 200.0
        }
    }
}

struct OwnershipCase {
    name: &'static str,
    source: &'static str,
    owners: &'static [u64],
    rtl: bool,
    continuation_style: bool,
    advances: AdvanceProfile,
}

impl OwnershipCase {
    fn fixture(&self, machine: &mut Machine, fill: u8, offset: [f32; 2], cached: bool) -> String {
        let placement = Case {
            name: self.name.into(),
            cursor: 3.25,
            margin: 4.75,
            font_size: 20.0,
            height: 20.0,
            pixels: 0.0,
            multiplier: 1.35,
            alignment: 0,
            object_metric: false,
            offset,
            line_count: 1,
            style_change: false,
        };
        placement.place_entries(machine, fill);
        machine.heap.cursor = HEAP;
        machine.heap.allocation_fill = fill;
        let source: Vec<_> = self.source.encode_utf16().collect();
        let count = source.len();
        assert!(count <= 12 && self.owners.len() <= 6);
        for (address, size) in [
            (DRAWING, 160),
            (RICH_TEXT, 16),
            (RICH_IMPL, 256),
            (SOURCE, 24),
            (SOURCE_IMPL, 24),
            (GLYPH_CACHE, count * 40),
            (ENTRIES, count * 80),
            (OUTPUT, 24),
            (FONTS, 32),
            (FONT_IMPLS, 64),
            (FONT_VTABLE, 88),
            (LANGUAGE, 24),
            (SPANS, count * 72),
            (FUNCTOR, 0x700),
            (STACK, 0x600),
        ] {
            write(machine.engine, address, &vec![0; size]);
        }
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
            (FUNCTOR, SOURCE_VECTOR),
            (FUNCTOR + 16, DATA),
            (DATA, ENTRY_VECTOR),
            (DATA + 8, CACHE_VECTOR),
            (ENTRY_VECTOR, ENTRIES),
            (ENTRY_VECTOR + 8, ENTRIES + count as u64 * 80),
            (CACHE_VECTOR, GLYPH_CACHE),
            (CACHE_VECTOR + 8, GLYPH_CACHE + count as u64 * 40),
            (SOURCE_VECTOR, CHARACTERS),
            (STACK + 168, RECORDS),
            (STACK + 176, RECORDS + self.owners.len() as u64 * 64),
            (STACK + 192, ADVANCES),
            (HOLDER, WRAPPER),
            (WRAPPER, 1),
        ] {
            pointer(machine.engine, address, value);
        }
        word(machine.engine, DRAWING + 88, count as u32);
        word(machine.engine, SOURCE_VECTOR + 8, count as u32);
        write(machine.engine, LANGUAGE, &[4, b'e', b'n']);
        for index in 0..2 {
            let font = FONTS + index * 16;
            let implementation = FONT_IMPLS + index * 32;
            pointer(machine.engine, font + 8, implementation);
            pointer(machine.engine, implementation, FONT_VTABLE);
            word(machine.engine, implementation + 8, 7 + index as u32);
            pointer(machine.engine, implementation + 16, LANGUAGE);
        }
        let advances: Vec<_> = (0..count)
            .map(|index| self.advances.at(index, count))
            .collect();
        let logical_map: Vec<_> = (0..count)
            .map(|index| if self.rtl { count - 1 - index } else { index })
            .collect();
        let mut initial_spans = Vec::new();
        for index in 0..count {
            let owned = self.owners.contains(&(index as u64));
            let entry = ENTRIES + index as u64 * 80;
            let info = GLYPH_CACHE + index as u64 * 40;
            let span = SPANS + index as u64 * 72;
            let changed = !owned && self.continuation_style;
            machine.call(TEXT + 0x65920, &[entry]);
            pointer(machine.engine, info + 24, FONTS + u64::from(changed) * 16);
            write(machine.engine, info + 34, &[u8::from(owned)]);
            float(machine.engine, span, if changed { 24.0 } else { 20.0 });
            word(
                machine.engine,
                span + 4,
                if changed { 0xffab_cdef } else { 0xff12_3456 },
            );
            word(machine.engine, span + 8, 0x8065_4321);
            write(machine.engine, span + 16, &[3]);
            write(
                machine.engine,
                CHARACTERS + index as u64 * 2,
                &source[index].to_le_bytes(),
            );
            float(machine.engine, ADVANCES + index as u64 * 4, advances[index]);
            initial_spans.push(format!(
                "{{\"font_id\":{},\"font_size\":{},\"foreground\":{},\"cached_drawable\":{owned}}}",
                if changed { 8 } else { 7 },
                if changed { 24 } else { 20 },
                if changed {
                    0xffab_cdef_u32
                } else {
                    0xff12_3456
                }
            ));
        }
        let mut shaped = Vec::new();
        for (index, &owner) in self.owners.iter().enumerate() {
            assert!((owner as usize) < count);
            let record = RECORDS + index as u64 * 64;
            let codeword = 0x4101 + index as u32;
            let position = [index as f32 * 225.0, -75.0];
            let bounds = if advances[owner as usize] == 0.0 {
                [0.0; 4]
            } else {
                [0.0, -1200.0, 800.0, 600.0]
            };
            pointer(machine.engine, record, HOLDER);
            word(machine.engine, record + 16, codeword);
            pointer(machine.engine, record + 40, owner);
            float(machine.engine, record + 28, position[0]);
            float(machine.engine, record + 32, position[1]);
            set_rectangle(machine.engine, record + 48, bounds);
            shaped.push(format!("{{\"owner_utf16\":{owner},\"codeword\":{codeword},\"position\":{position:?},\"ink_rect\":{bounds:?}}}"));
        }
        register(machine.engine, REGISTER_SP, STACK);
        register(machine.engine, REGISTER_X29, STACK + 0x400);
        register(machine.engine, REGISTER_X0 + 20, STACK + 168);
        register(machine.engine, REGISTER_X0 + 21, SPANS);
        register(machine.engine, REGISTER_X0 + 23, FUNCTOR);
        register(machine.engine, REGISTER_X0 + 24, 0);
        register(machine.engine, REGISTER_X0 + 28, 0);
        check(unsafe {
            uc_emu_start(
                machine.engine,
                PRODUCER_START,
                PRODUCER_END,
                1_000_000,
                1_000_000,
            )
        });
        assert_eq!(read_register(machine.engine, REGISTER_PC), PRODUCER_END);
        let produced = self.entries(machine, count);
        for (index, &logical) in logical_map.iter().enumerate() {
            word(
                machine.engine,
                LOGICAL_MAP + index as u64 * 4,
                logical as u32,
            );
            word(
                machine.engine,
                ENTRIES + index as u64 * 80 + 52,
                u32::from(self.rtl),
            );
        }
        word(machine.engine, BLOCKS + 4, count as u32 - 1);
        pointer(machine.engine, PARAGRAPH + 112, PLACED_LINES);
        scalar(machine.engine, 0, placement.cursor);
        machine.call(SET_LAYOUT, &[PARAGRAPH, LINES]);
        let post_cursor = f32::from_bits(read_register(machine.engine, 136) as u32);
        let placed = self.entries(machine, count);
        let snapshot = cached.then(|| {
            cached_snapshot::CachedInput {
                entries: ENTRIES,
                caches: GLYPH_CACHE,
                spans: SPANS,
                source: CHARACTERS,
                logical_map: LOGICAL_MAP,
                count,
                gravity: 0.0,
                paragraph_flag65: false,
            }
            .snapshot(machine)
        });
        scalar(machine.engine, 0, offset[0]);
        scalar(machine.engine, 1, offset[1]);
        assert_eq!(
            machine.call(TEXT + 0x66c98, &[DRAWING, 0, count as u64 - 1, OUTPUT, 0]) & 1,
            1
        );
        let begin = read_u64(machine.engine, OUTPUT);
        let end = read_u64(machine.engine, OUTPUT + 8);
        assert!(end >= begin && end - begin <= count as u64 * 8);
        let runs: Vec<_> = (begin..end)
            .step_by(8)
            .map(|address| self.run(machine, read_u64(machine.engine, address)))
            .collect();
        let mut fixture = format!(
            "{{\"name\":{:?},\"source_utf16\":{source:?},\"shaped_glyphs\":[{}],\"shaped_advances\":{advances:?},\"span_inputs\":[{}],\"range_inclusive\":[0,{}],\"logical_map\":{logical_map:?},\"direction\":{},\"cursor\":{},\"margin\":{},\"line_height\":{},\"offset\":{offset:?},\"produced_entries\":[{produced}],\"placed_entries\":[{placed}],\"post_cursor\":{post_cursor:?},\"runs\":[{}]}}",
            self.name,
            shaped.join(","),
            initial_spans.join(","),
            count - 1,
            u32::from(self.rtl),
            placement.cursor,
            placement.margin,
            placement.height,
            runs.join(",")
        );
        if let Some(snapshot) = snapshot {
            assert_eq!(fixture.pop(), Some('}'));
            fixture.push_str(&format!(",\"cached_input\":{snapshot}}}"));
        }
        fixture
    }

    fn entries(&self, machine: &Machine, count: usize) -> String {
        (0..count).map(|index| {
            let entry = ENTRIES + index as u64 * 80;
            let glyphs = vector(machine, GLYPH_CACHE + index as u64 * 40);
            let glyphs:Vec<_>=glyphs.chunks_exact(3).map(|glyph| format!("[{}, {:?}, {:?}]",glyph[0],f32::from_bits(glyph[1]),f32::from_bits(glyph[2]))).collect();
            format!("{{\"kind\":{},\"advance\":{:?},\"position\":{:?},\"layout_rect\":{:?},\"ink_rect\":{:?},\"glyphs\":[{}]}}",read_u32(machine.engine,entry+48),read_float(machine.engine,entry),[read_float(machine.engine,entry+8),read_float(machine.engine,entry+12)],rectangle(machine.engine,entry+16),rectangle(machine.engine,entry+32),glyphs.join(","))
        }).collect::<Vec<_>>().join(",")
    }

    fn run(&self, machine: &Machine, address: u64) -> String {
        let codes = vector(machine, address + 8);
        let positions: Vec<_> = vector(machine, address + 32)
            .into_iter()
            .map(f32::from_bits)
            .collect();
        assert_eq!(codes.len(), positions.len());
        format!(
            "{{\"range_inclusive\":[{},{}],\"codewords\":{codes:?},\"positions\":{positions:?},\"origin\":{:?},\"layout_rect\":{:?},\"ink_rect\":{:?},\"font_id\":{},\"font_size\":{:?},\"foreground\":{}}}",
            read_u32(machine.engine, address),
            read_u32(machine.engine, address + 4),
            [
                read_float(machine.engine, address + 80),
                read_float(machine.engine, address + 84)
            ],
            rectangle(machine.engine, address + 104),
            rectangle(machine.engine, address + 88),
            read_u32(machine.engine, address + 124),
            read_float(machine.engine, address + 132),
            read_u32(machine.engine, address + 136)
        )
    }
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
        (0xf0560, NOOP),
        (0xf0570, NOOP),
        (0xef900, BASE + 0xb10ec),
        (0xefc60, BASE + 0xb1510),
        (0xefc40, TEXT + 0x779d0),
    ] {
        bind_native(machine.engine, TEXT + plt, target);
    }
    for (address, instruction) in [
        (GET_ID, 0xb9400800_u32),
        (GET_BITMAP, 0x39403000),
        (GET_LANGUAGE, 0xf9400800),
        (NOOP, 0xd65f03c0),
    ] {
        word(machine.engine, address, instruction);
        word(machine.engine, address + 4, 0xd65f03c0);
    }
    let _copy = CopyHook::new(machine);
    let mut captures = Vec::new();
    for (name, source, owners, rtl, continuation_style) in [
        ("ordinary", "ABZ", &[0, 1, 2][..], false, false),
        ("ligature-owner", "fiZ", &[0, 2][..], false, false),
        ("ffi-owner", "ffiZ", &[0, 3][..], false, false),
        (
            "ligature-continuation-style",
            "fiZ",
            &[0, 2][..],
            false,
            true,
        ),
        ("surrogate-owner", "😀Z", &[0, 2][..], false, false),
        (
            "surrogate-continuation-style",
            "😀Z",
            &[0, 2][..],
            false,
            true,
        ),
        (
            "combining-shared-owner",
            "a\u{301}Z",
            &[0, 0, 2][..],
            false,
            false,
        ),
        (
            "combining-separate-owner",
            "a\u{301}Z",
            &[0, 1, 2][..],
            false,
            false,
        ),
        (
            "combining-continuation-style",
            "a\u{301}Z",
            &[0, 0, 2][..],
            false,
            true,
        ),
        ("owner-gap", "ABCDZ", &[0, 4][..], false, false),
        ("multiple-last-owner", "ABZ", &[0, 2, 2][..], false, false),
        ("leading-unowned", "ABZ", &[1, 2][..], false, false),
        ("trailing-unowned", "ABZ", &[0, 1][..], false, false),
        ("rtl-shared-owner", "א\u{5b7}ב", &[0, 0, 2][..], true, false),
        (
            "rtl-continuation-style",
            "א\u{5b7}ב",
            &[0, 0, 2][..],
            true,
            true,
        ),
        ("emoji-zwj-owner", "👩‍👩Z", &[0, 5][..], false, false),
    ] {
        let case = OwnershipCase {
            name,
            source,
            owners,
            rtl,
            continuation_style,
            advances: AdvanceProfile::PerCodeUnit,
        };
        capture_case(machine, &mut captures, &case, cached);
    }
    for (name, source, owners, advances) in [
        (
            "zero-leading-owner",
            "\u{2066}AZ",
            &[0, 1, 2][..],
            AdvanceProfile::ZeroLeading,
        ),
        (
            "zero-trailing-owner",
            "ZA\u{2069}",
            &[0, 1, 2][..],
            AdvanceProfile::ZeroTrailing,
        ),
        (
            "zero-only-owners",
            "\u{2066}\u{2069}",
            &[0, 1][..],
            AdvanceProfile::ZeroAll,
        ),
    ] {
        let case = OwnershipCase {
            name,
            source,
            owners,
            rtl: false,
            continuation_style: false,
            advances,
        };
        capture_case(machine, &mut captures, &case, cached);
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"memory_fills\":[0,165,255],\"entry_constructor\":\"0x65920\",\"producer_span_index\":0,\"producer_window\":[\"0x77324\",\"0x77894\"],\"set_layout\":\"0x6b4a4\",\"get_drawn_text_run\":\"0x66c98\",\"append_text_block\":\"0x67ebc\",\"measurement_inputs\":\"UTF-16 source, shaped glyph codewords/cluster owners/positions/ink rectangles and per-code-unit advances are supplied. Owner GlyphInfo caches initially have drawable=true, an empty vector and supplied Font wrapper to bypass font creation. Unowned caches have drawable=false. Native entry constructor initializes each slot with kind=3 and empty metrics. Native entry initialization and SpanRunFunctor producer window, glyph-vector allocation, ownership indexing, advance/ink production, SetLayout, cached getters, complete retained-run grouping/union/emission execute unchanged. UTF-16 logical order, direction, spans, Font ID/bitmap/language getters and line/block metrics are supplied. Allocation/deletion/memory copy and mutex lock/unlock are host supplied. Native font selection, Minikin/HarfBuzz shaping, source-to-owner choice, bidi ordering, wrapping, paragraph direction, real embedded objects, emojis, Composer clipping and final PDF painting do not execute. Scenario labels describe supplied owner maps and source, not native shaping outputs.\",\"cases\":[\n{}\n]}}",
        captures.join(",\n")
    );
}

fn capture_case(
    machine: &mut Machine,
    captures: &mut Vec<String>,
    case: &OwnershipCase,
    cached: bool,
) {
    for offset in [[0.0, 0.0], [100.25, 200.75]] {
        let expected = case.fixture(machine, 0, offset, cached);
        for fill in [0xa5, 0xff] {
            assert_eq!(
                case.fixture(machine, fill, offset, cached),
                expected,
                "{} memory fill",
                case.name
            );
        }
        captures.push(expected);
    }
}
