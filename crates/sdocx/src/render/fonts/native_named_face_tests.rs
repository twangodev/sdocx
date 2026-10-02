use super::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
struct Capture {
    font_manager_evidence_sha256: String,
    supplied_config_sha256: String,
    source_flags: Vec<u8>,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    source_profile: Profile,
    layout_source_id: u32,
    layout_uses_selected_physical_face: bool,
    shape: Shape,
}
#[derive(Deserialize)]
struct Profile {
    span_name: Option<String>,
    default_name: Option<String>,
    caller_direction: bool,
    source_size_bits: u32,
    source_flags: u8,
    physical_font_sha256: String,
    physical_font_style: u32,
    physical_source_id: u32,
    physical_face_index: u32,
    physical_data_size: usize,
    physical_mapping_is_pinned: bool,
    set_typeface_selects_same_physical_face: bool,
    physical_fakery: u32,
    paint_size_bits: u32,
    paint_scale_x_bits: u32,
    paint_skew_x_bits: u32,
    paint_flags: u32,
    paint_weight: u16,
    paint_italic: bool,
}
#[derive(Deserialize)]
struct Shape {
    name: String,
    text_utf8: String,
    range_utf16: [u32; 2],
    family_locale: Option<String>,
    paint: Paint,
    hb_calls: Vec<Call>,
    layout_piece: Layout,
    callbacks: Callbacks,
    entry_geometry: Entries,
}
#[derive(Deserialize)]
struct Paint {
    size_bits: u32,
    scale_x_bits: u32,
    skew_x_bits: u32,
    packed_flags: u32,
    weight: u16,
    italic: bool,
    variant: u8,
    locale_list_id: u32,
    letter_spacing_bits: u32,
    word_spacing_bits: u32,
    feature_settings: String,
}
#[derive(Deserialize)]
struct Call {
    nativefont_scale: [i32; 2],
    nativefont_ppem: [u32; 2],
    input: Input,
    output: Output,
}
#[derive(Deserialize)]
struct Input {
    infos: Vec<Info>,
    script: u32,
    direction: u32,
    language: Option<String>,
    flags: u32,
    cluster_level: u32,
    content_type: u32,
    pre_context: Vec<u32>,
    post_context: Vec<u32>,
    features: Vec<Feature>,
}
#[derive(Deserialize)]
struct Info {
    codepoint: u32,
    cluster: u32,
}
#[derive(Deserialize)]
struct Feature {
    tag: u32,
    value: u32,
    start: u32,
    end: u32,
}
#[derive(Deserialize)]
struct Output {
    infos: Vec<GlyphInfo>,
    positions: Vec<[i32; 4]>,
}
#[derive(Deserialize)]
struct GlyphInfo {
    glyph_id: u32,
    cluster: u32,
}
#[derive(Deserialize)]
struct Layout {
    font_indices: Vec<u8>,
    font_fakery_bits: Vec<u32>,
    glyph_ids: Vec<u32>,
    owners_utf16: Vec<u32>,
    full_positions_bits: Vec<[u32; 2]>,
    owner_positions_bits: Vec<[u32; 2]>,
    ink_bounds_bits: Vec<[u32; 4]>,
    character_advances_bits: Vec<u32>,
    total_advance_bits: u32,
}
#[derive(Deserialize)]
struct Callbacks {
    vector_glyphs: Vec<Vec<u32>>,
    vector_raw_bits: Vec<u32>,
    vector_quantized: Vec<i32>,
    scalar_glyphs: Vec<u32>,
    scalar_raw_bits: Vec<u32>,
    scalar_quantized: Vec<i32>,
    raw_skia_bounds_bits: Vec<[u32; 4]>,
}
#[derive(Deserialize)]
struct Entries {
    entry_widths_bits: Vec<u32>,
    entry_ink_bits: Vec<[u32; 4]>,
    glyphs: Vec<EntryGlyph>,
}
#[derive(Deserialize)]
struct EntryGlyph {
    glyph_id: u32,
    owner_utf16: u32,
    entry_position_bits: [u32; 2],
    entry_ink_bits: [u32; 4],
}

fn capture() -> Capture {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-text-shaping-named-faces.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "52bc6161befde8fe518148ca65631be25ae0712a9d07170caae61820ae211c30"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 110);
    assert_eq!(capture.source_flags, [0, 1, 4]);
    assert_eq!(
        capture.font_manager_evidence_sha256,
        "20ca51223fa1b5a010786586a12ab4d4c5937cd5ba4e1362ff661d6fab069754"
    );
    assert_eq!(
        capture.supplied_config_sha256,
        "9864ad4db5012ad4b63f82fcd0375b4f6a02a92d762147805ebdc02de0f4ed22"
    );
    capture
}

fn face_and_paint(book: &FontBook, case: &Case) -> (ResolvedFace, PaintMetricInput) {
    let profile = &case.source_profile;
    let direction = direction(profile.caller_direction);
    let name = NativeFontNameRequest::new(
        profile.span_name.as_deref(),
        profile.default_name.as_deref(),
        direction,
    )
    .unwrap();
    let resolution = book.resolve_native_name(&name).unwrap();
    let face = resolution.face().clone();
    assert_eq!(
        format!("{:x}", Sha256::digest(face.bytes())),
        profile.physical_font_sha256
    );
    assert_eq!(face.bytes().len(), profile.physical_data_size);
    assert_eq!(face.index, profile.physical_face_index);
    assert_eq!(
        u32::from(resolution.typeface_style().weight())
            | (u32::from(resolution.typeface_style().italic()) << 16),
        profile.physical_font_style
    );
    assert_eq!(resolution.typeface_style().weight(), profile.paint_weight);
    assert_eq!(resolution.typeface_style().italic(), profile.paint_italic);
    assert_eq!(profile.physical_source_id, case.layout_source_id);
    assert!(
        profile.physical_mapping_is_pinned
            && profile.set_typeface_selects_same_physical_face
            && case.layout_uses_selected_physical_face
    );
    assert_eq!(profile.physical_fakery, 0);
    let span = PaintSpanProfile::new(
        f32::from_bits(profile.source_size_bits),
        profile.source_flags,
    )
    .unwrap();
    let paint = span.metric_input().unwrap();
    assert_eq!(paint.size.to_bits(), profile.paint_size_bits);
    assert_eq!(paint.scale_x.to_bits(), profile.paint_scale_x_bits);
    assert_eq!(paint.skew_x.to_bits(), profile.paint_skew_x_bits);
    assert_eq!(span.packed_flags(), profile.paint_flags);
    let actual = &case.shape.paint;
    assert_eq!(
        [actual.size_bits, actual.scale_x_bits, actual.skew_x_bits],
        [
            profile.paint_size_bits,
            profile.paint_scale_x_bits,
            profile.paint_skew_x_bits
        ]
    );
    assert_eq!(actual.packed_flags, profile.paint_flags);
    assert_eq!(
        (actual.weight, actual.italic),
        (profile.paint_weight, profile.paint_italic)
    );
    assert_eq!((actual.variant, actual.locale_list_id), (0, 0));
    assert_eq!(
        (actual.letter_spacing_bits, actual.word_spacing_bits),
        (0, 0)
    );
    assert!(actual.feature_settings.is_empty());
    assert!(case.shape.family_locale.is_none());
    (face, paint)
}

fn direction(rtl: bool) -> PaintShapeDirection {
    if rtl {
        PaintShapeDirection::RightToLeft
    } else {
        PaintShapeDirection::LeftToRight
    }
}

fn compare_input(itemization: &PaintItemization, shape: &Shape, rtl: bool) {
    assert_eq!(itemization.chunks().len(), shape.hb_calls.len());
    for (chunk, call) in itemization.chunks().iter().zip(&shape.hb_calls) {
        let input = &call.input;
        assert_eq!(chunk.script(), input.script.to_be_bytes());
        assert_eq!(
            chunk
                .infos()
                .iter()
                .map(|info| (u32::from(info.character), info.owner_utf16))
                .collect::<Vec<_>>(),
            input
                .infos
                .iter()
                .map(|info| (info.codepoint, info.cluster))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            chunk
                .pre_context()
                .chars()
                .rev()
                .map(u32::from)
                .collect::<Vec<_>>(),
            input.pre_context
        );
        assert_eq!(
            chunk
                .post_context()
                .chars()
                .map(u32::from)
                .collect::<Vec<_>>(),
            input.post_context
        );
        assert_eq!(input.direction, if rtl { 5 } else { 4 });
        assert!(input.language.is_none());
        assert_eq!(
            (input.flags, input.cluster_level, input.content_type),
            (0, 0, 1)
        );
        let features = PaintFeatureProfile::for_script(chunk.script(), 0.0).unwrap();
        assert_eq!(
            features
                .features()
                .iter()
                .map(|feature| (
                    u32::from_be_bytes(feature.tag),
                    feature.value,
                    feature.start,
                    feature.end
                ))
                .collect::<Vec<_>>(),
            input
                .features
                .iter()
                .map(|feature| (feature.tag, feature.value, feature.start, feature.end))
                .collect::<Vec<_>>()
        );
    }
}

fn compare_piece(piece: &PaintMeasuredPiece, shape: &Shape) {
    assert_eq!(piece.shaped_runs().len(), shape.hb_calls.len());
    for (run, call) in piece.shaped_runs().iter().zip(&shape.hb_calls) {
        assert_eq!([run.scale().x, run.scale().y], call.nativefont_scale);
        assert_eq!(run.scale().ppem, call.nativefont_ppem);
        assert_eq!(
            run.glyphs()
                .iter()
                .map(|glyph| (glyph.id, glyph.owner_utf16))
                .collect::<Vec<_>>(),
            call.output
                .infos
                .iter()
                .map(|glyph| (glyph.glyph_id, glyph.cluster))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            run.glyphs()
                .iter()
                .map(|glyph| [
                    glyph.x_advance,
                    glyph.y_advance,
                    glyph.x_offset,
                    glyph.y_offset
                ])
                .collect::<Vec<_>>(),
            call.output.positions
        );
    }
    let layout = piece.layout();
    let expected = &shape.layout_piece;
    assert!(expected.font_indices.iter().all(|&slot| slot == 0));
    assert_eq!(expected.font_fakery_bits, [0]);
    assert_eq!(
        layout
            .glyphs()
            .iter()
            .map(|glyph| glyph.id)
            .collect::<Vec<_>>(),
        expected.glyph_ids
    );
    assert_eq!(
        layout
            .glyphs()
            .iter()
            .map(|glyph| glyph.owner_utf16)
            .collect::<Vec<_>>(),
        expected.owners_utf16
    );
    assert_eq!(
        layout
            .glyphs()
            .iter()
            .map(|glyph| glyph.full_position.map(f32::to_bits))
            .collect::<Vec<_>>(),
        expected.full_positions_bits
    );
    assert_eq!(
        layout
            .glyphs()
            .iter()
            .map(|glyph| glyph.owner_position.map(f32::to_bits))
            .collect::<Vec<_>>(),
        expected.owner_positions_bits
    );
    assert_eq!(
        layout
            .glyphs()
            .iter()
            .map(|glyph| glyph.ink_bounds.map(f32::to_bits))
            .collect::<Vec<_>>(),
        expected.ink_bounds_bits
    );
    assert_eq!(
        layout
            .character_advances()
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        expected.character_advances_bits
    );
    assert_eq!(
        layout.total_advance().to_bits(),
        expected.total_advance_bits
    );
    let entries = piece.entry_geometry();
    let [start, end] = shape.range_utf16;
    assert_eq!(
        entries
            .entries()
            .iter()
            .map(|entry| entry.advance().to_bits())
            .collect::<Vec<_>>(),
        shape.entry_geometry.entry_widths_bits[start as usize..end as usize]
    );
    assert_eq!(
        entries
            .entries()
            .iter()
            .map(|entry| entry.ink_bounds().map(f32::to_bits))
            .collect::<Vec<_>>(),
        shape.entry_geometry.entry_ink_bits[start as usize..end as usize]
    );
    assert_eq!(
        entries
            .glyphs()
            .iter()
            .map(|glyph| (
                glyph.id(),
                glyph.owner_utf16(),
                glyph.position().map(f32::to_bits),
                glyph.ink_bounds().map(f32::to_bits)
            ))
            .collect::<Vec<_>>(),
        shape
            .entry_geometry
            .glyphs
            .iter()
            .map(|glyph| (
                glyph.glyph_id,
                glyph.owner_utf16,
                glyph.entry_position_bits,
                glyph.entry_ink_bits
            ))
            .collect::<Vec<_>>()
    );
}

fn ink_bits(ink: PaintInkBounds) -> [u32; 4] {
    [ink.left, ink.top, ink.right, ink.bottom].map(f32::to_bits)
}

#[test]
fn native_named_faces_preserve_supported_whole_pieces_and_reject_unverified_composites() {
    let capture = capture();
    let book = FontBook::default();
    let mut supported = 0;
    let mut rejected = BTreeMap::new();
    let mut physical_faces = BTreeSet::new();
    for case in &capture.cases {
        let (face, paint) = face_and_paint(&book, case);
        physical_faces.insert(case.source_profile.physical_font_sha256.clone());
        let [start, end] = case.shape.range_utf16;
        let windows = PaintContextWindows::new(&case.shape.text_utf8, start..end).unwrap();
        assert_eq!(windows.windows().len(), 1);
        let window = &windows.windows()[0];
        assert_eq!(window.source(), case.shape.text_utf8);
        let itemization =
            PaintItemization::new(window.source(), window.selected_range_in_window_utf16())
                .unwrap();
        compare_input(
            &itemization,
            &case.shape,
            case.source_profile.caller_direction,
        );
        let mut shaper = face.paint_shaper(paint).unwrap();
        match shaper.shape_text(PaintTextRequest::new(
            &itemization,
            direction(case.source_profile.caller_direction),
        )) {
            Ok(piece) => {
                compare_piece(&piece, &case.shape);
                supported += 1;
            }
            Err(PaintPieceError::Shape(PaintShapeError::Metrics(
                PaintMetricError::UnsupportedComposite { glyph_id },
            ))) => {
                assert_eq!(case.source_profile.source_flags, 4);
                assert!(
                    case.shape
                        .callbacks
                        .vector_glyphs
                        .iter()
                        .flatten()
                        .chain(&case.shape.layout_piece.glyph_ids)
                        .any(|&glyph| glyph == glyph_id)
                );
                *rejected
                    .entry((case.source_profile.span_name.clone().unwrap(), glyph_id))
                    .or_insert(0_usize) += 1;
            }
            Err(error) => panic!(
                "{} {:?} flags{}: {error}",
                case.shape.name, case.source_profile.span_name, case.source_profile.source_flags
            ),
        }
    }
    assert_eq!(physical_faces.len(), 4);
    assert_eq!(supported, 106);
    assert_eq!(
        rejected,
        BTreeMap::from([
            (("Roboto-Regular".to_owned(), 2578), 1),
            (("Roboto-Bold".to_owned(), 2578), 1),
            (("Roboto-Italic".to_owned(), 2578), 1),
            (("Roboto-BoldItalic".to_owned(), 2578), 1),
        ])
    );
}

#[test]
fn native_named_faces_match_raw_skia_metrics_and_signed_callback_quantization() {
    let book = FontBook::default();
    let mut advances = 0;
    let mut bounds = 0;
    let mut rejected = 0;
    for case in capture().cases {
        let (face, paint) = face_and_paint(&book, &case);
        let metrics = face.paint_metrics(paint).unwrap();
        let callbacks = &case.shape.callbacks;
        let vector_count = callbacks.vector_glyphs.iter().map(Vec::len).sum::<usize>();
        assert_eq!(vector_count, callbacks.vector_raw_bits.len());
        assert_eq!(vector_count, callbacks.vector_quantized.len());
        assert_eq!(
            callbacks.scalar_glyphs.len(),
            callbacks.scalar_raw_bits.len()
        );
        assert_eq!(
            callbacks.scalar_glyphs.len(),
            callbacks.scalar_quantized.len()
        );
        assert_eq!(
            case.shape.layout_piece.glyph_ids.len(),
            callbacks.raw_skia_bounds_bits.len()
        );
        for ((&glyph, &raw), &quantized) in callbacks
            .vector_glyphs
            .iter()
            .flatten()
            .zip(&callbacks.vector_raw_bits)
            .zip(&callbacks.vector_quantized)
        {
            match metrics.glyph(glyph) {
                Ok(metric) => {
                    assert_eq!(
                        metric.advance.to_bits(),
                        raw,
                        "{} {} vector advance",
                        case.shape.name,
                        glyph
                    );
                    assert_eq!((metric.advance * 256.0) as i32, quantized);
                    advances += 1;
                }
                Err(PaintMetricError::UnsupportedComposite { .. }) => {
                    assert_eq!(case.source_profile.source_flags, 4);
                    rejected += 1;
                }
                Err(error) => panic!("{} {glyph}: {error}", case.shape.name),
            }
        }
        for ((&glyph, &raw), &quantized) in callbacks
            .scalar_glyphs
            .iter()
            .zip(&callbacks.scalar_raw_bits)
            .zip(&callbacks.scalar_quantized)
        {
            let metric = metrics.glyph(glyph).unwrap();
            assert_eq!(metric.advance.to_bits(), raw);
            assert_eq!((f64::from(metric.advance * 256.0) + 0.5) as i32, quantized);
        }
        for (&glyph, &ink) in case
            .shape
            .layout_piece
            .glyph_ids
            .iter()
            .zip(&callbacks.raw_skia_bounds_bits)
        {
            match metrics.glyph(glyph) {
                Ok(metric) => {
                    assert_eq!(
                        ink_bits(metric.ink),
                        ink,
                        "{} {} raw ink",
                        case.shape.name,
                        glyph
                    );
                    bounds += 1;
                }
                Err(PaintMetricError::UnsupportedComposite { .. }) => {
                    assert_eq!(case.source_profile.source_flags, 4)
                }
                Err(error) => panic!("{} {glyph}: {error}", case.shape.name),
            }
        }
    }
    assert_eq!((advances, bounds, rejected), (429, 429, 8));
}
