use super::*;

const HOOKS: [u64; 5] = [0x9bfd0, 0x9bfd4, 0x9c048, 0x9c04c, 0x9c0c4];

struct ScriptQuery {
    source_utf16: u32,
    piece_relative_utf16: u32,
    piece_start_utf16: u32,
    codepoint: u32,
    script: u32,
    first_in_chunk: bool,
}

struct ScriptChunk {
    range_utf16: [u32; 2],
    piece_relative_range_utf16: [u32; 2],
    piece_start_utf16: u32,
    font_run_end_utf16: u32,
    script: u32,
}

#[derive(Default)]
pub(super) struct ItemizationTrace {
    pending: Option<ScriptQuery>,
    queries: Vec<ScriptQuery>,
    chunks: Vec<ScriptChunk>,
}

impl ItemizationTrace {
    pub(super) fn json(&self) -> String {
        assert!(self.pending.is_none());
        let queries = self
            .queries
            .iter()
            .map(|query| {
                format!(
            "{{\"source_utf16\":{},\"piece_relative_utf16\":{},\"piece_start_utf16\":{},\"codepoint\":{},\"script\":{},\"first_in_chunk\":{}}}",
            query.source_utf16, query.piece_relative_utf16, query.piece_start_utf16, query.codepoint, query.script, query.first_in_chunk,
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let chunks = self
            .chunks
            .iter()
            .map(|chunk| {
                format!(
            "{{\"range_utf16\":{:?},\"piece_relative_range_utf16\":{:?},\"piece_start_utf16\":{},\"font_run_end_utf16\":{},\"script\":{}}}",
            chunk.range_utf16, chunk.piece_relative_range_utf16, chunk.piece_start_utf16, chunk.font_run_end_utf16, chunk.script,
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("{{\"queries\":[{queries}],\"chunks\":[{chunks}]}}")
    }
}

unsafe extern "C" fn capture_loop(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let state = unsafe { &mut *data.cast::<ShapeTrace>() };
    let trace = state.itemization.as_mut().unwrap();
    match address - TEXT {
        0x9bfd0 | 0x9c048 => {
            assert!(trace.pending.is_none() && trace.queries.len() < 8192);
            let first = address - TEXT == 0x9bfd0;
            let piece_relative_utf16 = if first {
                read_register(engine, REGISTER_X0 + 26)
            } else {
                read_u64(engine, read_register(engine, REGISTER_SP) + 248)
            };
            let piece_start_utf16 = read_u64(engine, read_register(engine, REGISTER_SP) + 216);
            let source_utf16 = piece_start_utf16.checked_add(piece_relative_utf16).unwrap();
            assert!(source_utf16 < 4096);
            let unicode = read_register(engine, REGISTER_X0);
            let callback = read_register(engine, REGISTER_X0 + if first { 8 } else { 9 });
            assert_eq!(callback, read_u64(engine, unicode + 56));
            assert_eq!(
                read_register(engine, REGISTER_X0 + 2),
                read_u64(engine, unicode + 120)
            );
            assert_eq!(callback, TEXT + 0xed9b8);
            trace.pending = Some(ScriptQuery {
                source_utf16: source_utf16 as u32,
                piece_relative_utf16: piece_relative_utf16 as u32,
                piece_start_utf16: piece_start_utf16 as u32,
                codepoint: read_register(engine, REGISTER_X0 + 1) as u32,
                script: 0,
                first_in_chunk: first,
            });
        }
        0x9bfd4 | 0x9c04c => {
            let Some(mut query) = trace.pending.take() else {
                assert_eq!(address - TEXT, 0x9bfd4);
                return;
            };
            query.script = read_register(engine, REGISTER_X0) as u32;
            trace.queries.push(query);
        }
        0x9c0c4 => {
            let start = read_register(engine, REGISTER_X0 + 26);
            let end = read_u64(engine, read_register(engine, REGISTER_SP) + 248);
            let font_end = read_register(engine, REGISTER_X0 + 23);
            let origin = read_u64(engine, read_register(engine, REGISTER_SP) + 216);
            assert!(start < end && end <= font_end && font_end + origin <= 4096);
            assert!(trace.chunks.len() < 4096);
            trace.chunks.push(ScriptChunk {
                range_utf16: [(start + origin) as u32, (end + origin) as u32],
                piece_relative_range_utf16: [start as u32, end as u32],
                piece_start_utf16: origin as u32,
                font_run_end_utf16: (font_end + origin) as u32,
                script: read_register(engine, REGISTER_X0 + 22) as u32,
            });
        }
        _ => unreachable!(),
    }
}

pub(super) fn add_hooks(recorder: &mut TraceRecorder) {
    for offset in HOOKS {
        let address = TEXT + offset;
        let mut hook = 0;
        check(unsafe {
            uc_hook_add(
                recorder.engine,
                &mut hook,
                4,
                capture_loop as *mut c_void,
                ptr::from_mut(recorder.state.as_mut()).cast(),
                address,
                address,
            )
        });
        recorder.hooks.push(hook);
    }
}

pub(super) fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (ltr, rtl, text) in [
        ("plain_mixed", "plain_mixed_rtl", "AVΑΒАВ"),
        ("neutral_between", "neutral_between_rtl", "A(12).Α[34]А"),
        ("spaces_between", "spaces_between_rtl", "A 12 Α 34 А"),
        ("leading_common", "leading_common_rtl", "(12)ΑA"),
        (
            "leading_inherited",
            "leading_inherited_rtl",
            "\u{301}\u{327}ΑA",
        ),
        ("only_inherited", "only_inherited_rtl", "\u{301}\u{327}"),
        ("only_common", "only_common_rtl", "(12) ."),
        ("common_inherited", "common_inherited_rtl", "(12)😀\u{301}"),
        ("paired_punctuation", "paired_punctuation_rtl", "A(Α)А"),
        (
            "latin_extension_mark_greek",
            "latin_extension_mark_greek_rtl",
            "Α\u{363}A",
        ),
        (
            "latin_extension_mark_cyrillic",
            "latin_extension_mark_cyrillic_rtl",
            "А\u{363}A",
        ),
        (
            "cyrillic_extension_mark_greek",
            "cyrillic_extension_mark_greek_rtl",
            "Α\u{483}A",
        ),
        (
            "inherited_reentry",
            "inherited_reentry_rtl",
            "A\u{301}Α\u{327}А\u{301}A",
        ),
        ("emoji_joiner", "emoji_joiner_rtl", "A😀\u{200d}🙂Α"),
        (
            "emoji_variation",
            "emoji_variation_rtl",
            "A\u{fe0f}Α\u{fe0e}А",
        ),
        ("leading_emoji", "leading_emoji_rtl", "😀ΑA"),
        (
            "neutral_script_reentry",
            "neutral_script_reentry_rtl",
            "A(Α)12A.А",
        ),
        ("context_cap", "context_cap_rtl", "123456AΑА987654"),
    ] {
        cases.push(Case::regular(ltr, text, 17.125));
        let mut case = Case::regular(rtl, text, 17.125);
        case.rtl = true;
        cases.push(case);
    }
    for (ltr, rtl, text, range) in [
        (
            "partial_common_context",
            "partial_common_context_rtl",
            "A(12)ΑА",
            [1, 5],
        ),
        (
            "partial_inherited_context",
            "partial_inherited_context_rtl",
            "A\u{301}\u{327}Α",
            [1, 3],
        ),
        (
            "partial_leading_inherited",
            "partial_leading_inherited_rtl",
            "A\u{301}\u{327}ΑА",
            [1, 5],
        ),
        (
            "partial_supplementary",
            "partial_supplementary_rtl",
            "😀A(12)ΑА🙂",
            [2, 9],
        ),
    ] {
        let mut case = Case::regular(ltr, text, 17.125);
        case.range = Some(range);
        cases.push(case);
        let mut case = Case::regular(rtl, text, 17.125);
        case.range = Some(range);
        case.rtl = true;
        cases.push(case);
    }
    cases
}

pub(super) fn append_metadata(output: &mut String) {
    output.pop();
    output.push_str(",\"script_itemization_hook_addresses\":{\"first_query\":\"0x9bfd0\",\"first_result\":\"0x9bfd4\",\"next_query\":\"0x9c048\",\"next_result\":\"0x9c04c\",\"chunk_ready\":\"0x9c0c4\"},\"script_itemization_capture_boundary\":\"Actual single-face LayoutPiece inline script loop decodes source UTF16 within supplied font run, calls bundled HarfBuzz Unicode script callback and records actual scalar-query results and finalized chunk UTF16 ranges/tags before shaping. Captured Latin/Greek/Cyrillic, Common/Inherited, ASCII punctuation/digits/spaces, combining extension marks, emoji/variation/joiner, script reentry and scalar-aligned partial ranges execute supplied LTR/RTL direction with pinned Roboto. Common/Inherited resolution is actual native loop behavior, not ICU Script_Extensions or full bidi resolution. Actual HB buffers retain capped five-scalar context from the complete supplied source, including outside requested range. All-neutral missing glyph output is retained as observed; this suite does not establish fallback selection or general glyph coverage. Font selection, malformed UTF16/surrogate-split ranges, other scripts, Unicode-version-independent properties, full SpanRunFunctor and wrapping/composition are not established.\"}");
}
