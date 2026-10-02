use super::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    text_utf8: String,
    range_utf16: [u32; 2],
    hb_calls: Vec<Call>,
    #[serde(default)]
    script_itemization: Option<ScriptTrace>,
}

#[derive(Deserialize)]
struct ScriptTrace {
    queries: Vec<ScriptQuery>,
    chunks: Vec<ScriptChunk>,
}

#[derive(Deserialize)]
struct ScriptQuery {
    source_utf16: u32,
    piece_relative_utf16: u32,
    piece_start_utf16: u32,
    codepoint: u32,
    script: u32,
    first_in_chunk: bool,
}

#[derive(Deserialize)]
struct ScriptChunk {
    range_utf16: [u32; 2],
    piece_relative_range_utf16: [u32; 2],
    piece_start_utf16: u32,
    font_run_end_utf16: u32,
    script: u32,
}

#[derive(Deserialize)]
struct Call {
    input: Input,
}

#[derive(Deserialize)]
struct Input {
    infos: Vec<Info>,
    script: u32,
    pre_context: Vec<u32>,
    post_context: Vec<u32>,
}

#[derive(Deserialize)]
struct Info {
    codepoint: u32,
    cluster: u32,
}

fn compare_capture(bytes: &[u8], digest: &str) -> (usize, usize) {
    assert_eq!(format!("{:x}", Sha256::digest(bytes)), digest);
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    let mut calls = 0;
    for case in &capture.cases {
        let itemization =
            PaintItemization::new(&case.text_utf8, case.range_utf16[0]..case.range_utf16[1])
                .unwrap();
        assert_eq!(itemization.source(), case.text_utf8);
        assert_eq!(
            itemization.chunks().len(),
            case.hb_calls.len(),
            "{}",
            case.name
        );
        for (chunk, call) in itemization.chunks().iter().zip(&case.hb_calls) {
            assert_eq!(
                chunk.script(),
                call.input.script.to_be_bytes(),
                "{}",
                case.name
            );
            assert_eq!(
                chunk
                    .infos()
                    .iter()
                    .map(|info| (info.character as u32, info.owner_utf16))
                    .collect::<Vec<_>>(),
                call.input
                    .infos
                    .iter()
                    .map(|info| (info.codepoint, info.cluster))
                    .collect::<Vec<_>>(),
                "{}",
                case.name,
            );
            assert_eq!(
                chunk
                    .pre_context()
                    .chars()
                    .rev()
                    .map(|character| character as u32)
                    .collect::<Vec<_>>(),
                call.input.pre_context,
                "{}",
                case.name,
            );
            assert_eq!(
                chunk
                    .post_context()
                    .chars()
                    .map(|character| character as u32)
                    .collect::<Vec<_>>(),
                call.input.post_context,
                "{}",
                case.name,
            );
            assert_eq!(
                &case.text_utf8[chunk.byte_range()],
                chunk
                    .infos()
                    .iter()
                    .map(|info| info.character)
                    .collect::<String>(),
            );
            let first = call.input.infos.first().unwrap();
            let last = call.input.infos.last().unwrap();
            assert_eq!(
                chunk.source_range_utf16(),
                first.cluster
                    ..last.cluster + char::from_u32(last.codepoint).unwrap().len_utf16() as u32
            );
            assert_eq!(chunk.scalar_range().len(), chunk.infos().len());
            calls += 1;
        }
    }
    (capture.cases.len(), calls)
}

#[test]
fn native_harf_buzz_inputs_pin_chunks_owners_and_full_source_context() {
    let mut count = (0, 0);
    for (bytes, digest) in [
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../conformance/table-text-shaping.json"
            ))
            .as_slice(),
            "a4c58481cb1e36bb7de8783bea49a800b10b4227177684c48f5b5c356d7a425b",
        ),
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../conformance/table-text-shaping-numeric.json"
            ))
            .as_slice(),
            "1e476f7fc8254b2a6c7316c6ff18b9c436f8ab57707d536daa07e11eca2af455",
        ),
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../conformance/table-text-shaping-gpos.json"
            ))
            .as_slice(),
            "9280953ac5de0b6b745baedcda73cf4d88a95a7b52cbfbc99188e0b66a71efb2",
        ),
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../conformance/table-text-shaping-skia-metrics.json"
            ))
            .as_slice(),
            "c8aa3d08839e9f7e7ddff6d9200616faa11adb67f8a7124f7fd91d5088c9eec0",
        ),
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../conformance/table-text-shaping-mixed-scripts.json"
            ))
            .as_slice(),
            "00d1634d29634fac146b42baa1e1a349bd66a2a74724ca7e7d5c539f116703ad",
        ),
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../conformance/table-text-entry-geometry.json"
            ))
            .as_slice(),
            "4615a8778c0a7b6301bd9efddbd591da2fd00278d4a3f0db06b04e4b1c923dc4",
        ),
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../conformance/table-text-shaping-entry-skia-metrics.json"
            ))
            .as_slice(),
            "c7216fb3148f524a8e7cde5c0bb83a616c60ae81c3cedb7c7f2d266f1bd3800a",
        ),
    ] {
        let actual = compare_capture(bytes, digest);
        count.0 += actual.0;
        count.1 += actual.1;
    }
    assert_eq!(count, (107, 456));
}

#[test]
fn neutral_scripts_adopt_inside_the_requested_range_without_extension_intersection() {
    let chunks = |source: &str| {
        PaintItemization::new(source, 0..source.encode_utf16().count() as u32)
            .unwrap()
            .chunks()
            .iter()
            .map(|chunk| (chunk.source_range_utf16(), chunk.script()))
            .collect::<Vec<_>>()
    };
    assert_eq!(chunks("Α\u{363}A"), [(0..2, *b"Grek"), (2..3, *b"Latn")]);
    assert_eq!(
        chunks("Α\u{483}A"),
        [(0..1, *b"Grek"), (1..2, *b"Cyrl"), (2..3, *b"Latn")]
    );
    assert_eq!(
        chunks("A(Α)А"),
        [(0..2, *b"Latn"), (2..4, *b"Grek"), (4..5, *b"Cyrl")]
    );
    assert_eq!(chunks("(12)ΑA"), [(0..5, *b"Grek"), (5..6, *b"Latn")]);
    assert_eq!(chunks("\u{301}\u{327}"), [(0..2, COMMON)]);
    let partial = PaintItemization::new("A\u{301}\u{327}Α", 1..3).unwrap();
    assert_eq!(partial.chunks()[0].script(), COMMON);
    assert_eq!(partial.chunks()[0].pre_context(), "A");
    assert_eq!(partial.chunks()[0].post_context(), "Α");
}

#[test]
fn supplementary_scalars_keep_absolute_coordinates_and_capped_context() {
    let source = "123456😀AΑ🙂987654";
    let itemization = PaintItemization::new(source, 6..12).unwrap();
    let first = &itemization.chunks()[0];
    assert_eq!(first.source_range_utf16(), 6..9);
    assert_eq!(first.scalar_range(), 6..8);
    assert_eq!(first.byte_range(), 6..11);
    assert_eq!(first.pre_context(), "23456");
    assert_eq!(first.post_context(), "Α🙂987");
    let last = &itemization.chunks()[1];
    assert_eq!(last.source_range_utf16(), 9..12);
    assert_eq!(last.scalar_range(), 8..10);
    assert_eq!(last.byte_range(), 11..17);
    assert_eq!(last.pre_context(), "456😀A");
    assert_eq!(last.post_context(), "98765");
}

#[test]
fn invalid_ranges_and_budgets_fail_before_owned_source_or_index_allocation() {
    for range in [Range { start: 1, end: 0 }, 1..2, 0..1, 0..4] {
        assert_eq!(
            PaintItemization::new("😀A", range),
            Err(PaintItemizationError::InvalidRange)
        );
    }
    assert!(PaintItemization::new("", 0..0).unwrap().chunks().is_empty());
    assert!(
        PaintItemization::new("😀A", 2..2)
            .unwrap()
            .chunks()
            .is_empty()
    );
    for source in [
        "a".repeat(MAX_SOURCE_BYTES + 1),
        "a".repeat(MAX_SOURCE_UTF16 + 1),
        "😀".repeat(MAX_SOURCE_UTF16 / 2 + 1),
    ] {
        assert_eq!(
            PaintItemization::new(&source, 0..0),
            Err(PaintItemizationError::InputBudget)
        );
    }
    let source = "a".repeat(MAX_SOURCE_UTF16);
    let bounded = PaintItemization::new(&source, 0..MAX_SELECTED_SCALARS as u32).unwrap();
    assert_eq!(bounded.chunks()[0].infos().len(), MAX_SELECTED_SCALARS);
    assert_eq!(
        PaintItemization::new(&source, 0..MAX_SELECTED_SCALARS as u32 + 1),
        Err(PaintItemizationError::InputBudget)
    );
}

#[test]
fn boundary_scalar_is_queried_again_as_the_first_scalar_of_the_next_chunk() {
    let infos = [
        PaintSourceInfo {
            character: 'A',
            owner_utf16: 0,
        },
        PaintSourceInfo {
            character: 'Α',
            owner_utf16: 1,
        },
        PaintSourceInfo {
            character: 'А',
            owner_utf16: 2,
        },
    ];
    let mut queries = Vec::new();
    let chunks = script_ranges(&infos, |info, first_in_chunk| {
        let script = harfrust::script_for(info.character);
        queries.push((info.owner_utf16, script, first_in_chunk));
        script
    });
    assert_eq!(chunks.len(), 3);
    assert_eq!(
        queries,
        [
            (0, *b"Latn", true),
            (1, *b"Grek", false),
            (1, *b"Grek", true),
            (2, *b"Cyrl", false),
            (2, *b"Cyrl", true)
        ]
    );
}

#[test]
fn native_plain_script_queries_and_boundary_lookahead_match_exactly() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-text-shaping-itemization.json"
    ));
    assert_eq!(
        compare_capture(
            bytes,
            "eddd4290eb75ab124b82c07814d4354debe5af8a167cb0535a83541238dff710"
        ),
        (44, 102)
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    let mut query_count = 0;
    let mut common_count = 0;
    for case in &capture.cases {
        let trace = case.script_itemization.as_ref().unwrap();
        let itemization =
            PaintItemization::new(&case.text_utf8, case.range_utf16[0]..case.range_utf16[1])
                .unwrap();
        let infos: Vec<_> = itemization
            .chunks()
            .iter()
            .flat_map(|chunk| chunk.infos().iter().copied())
            .collect();
        let mut queries = Vec::new();
        script_ranges(&infos, |info, first_in_chunk| {
            let script = harfrust::script_for(info.character);
            queries.push((
                info.owner_utf16,
                info.character as u32,
                u32::from_be_bytes(script),
                first_in_chunk,
            ));
            script
        });
        assert_eq!(
            queries,
            trace
                .queries
                .iter()
                .map(|query| (
                    query.source_utf16,
                    query.codepoint,
                    query.script,
                    query.first_in_chunk
                ))
                .collect::<Vec<_>>(),
            "{}",
            case.name
        );
        for query in &trace.queries {
            assert_eq!(query.piece_start_utf16, case.range_utf16[0]);
            assert_eq!(
                query.source_utf16,
                query.piece_start_utf16 + query.piece_relative_utf16
            );
        }
        assert_eq!(itemization.chunks().len(), trace.chunks.len());
        for (chunk, native) in itemization.chunks().iter().zip(&trace.chunks) {
            assert_eq!(chunk.script(), native.script.to_be_bytes());
            assert_eq!(
                chunk.source_range_utf16(),
                native.range_utf16[0]..native.range_utf16[1]
            );
            assert_eq!(native.piece_start_utf16, case.range_utf16[0]);
            assert_eq!(native.font_run_end_utf16, case.range_utf16[1]);
            assert_eq!(
                native.range_utf16,
                native
                    .piece_relative_range_utf16
                    .map(|offset| offset + native.piece_start_utf16)
            );
            common_count += usize::from(chunk.script() == COMMON);
        }
        query_count += queries.len();
    }
    assert_eq!((query_count, common_count), (314, 10));
}
