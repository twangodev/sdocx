use super::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    font_manager_evidence_sha256: String,
    supplied_config_sha256: String,
    memory_fills: Vec<u8>,
    repeat_zero_fill: bool,
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
    span_name: String,
    caller_direction: bool,
    source_size_bits: u32,
    source_flags: u8,
    physical_font_sha256: String,
    physical_font_style: u32,
    physical_source_id: u32,
    physical_fakery: u32,
    paint_size_bits: u32,
}

#[derive(Deserialize)]
struct Shape {
    name: String,
    text_utf8: String,
    range_utf16: [u32; 2],
    layout_piece: Layout,
    callbacks: Callbacks,
    entry_geometry: Entries,
}

#[derive(Deserialize)]
struct Layout {
    glyph_ids: Vec<u32>,
    owners_utf16: Vec<u32>,
    full_positions_bits: Vec<[u32; 2]>,
    character_advances_bits: Vec<u32>,
    total_advance_bits: u32,
}

#[derive(Deserialize)]
struct Callbacks {
    vector_glyphs: Vec<Vec<u32>>,
    vector_raw_bits: Vec<u32>,
    vector_quantized: Vec<i32>,
    scalar_glyphs: Vec<u32>,
}

#[derive(Deserialize)]
struct Entries {
    source_utf16: Vec<u16>,
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
        "/../../conformance/table-text-shaping-consumer-metrics.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "a5ce8f4ce74f205300f6fdb6e7ac7fa0a5023c377e659a2527760582feca8041"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 52);
    assert_eq!(capture.memory_fills, [0, 165, 255]);
    assert!(capture.repeat_zero_fill);
    assert_eq!(capture.source_flags, [0]);
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

#[test]
fn native_consumer_profiles_preserve_source_advances_and_logical_entry_geometry() {
    let capture = capture();
    let book = FontBook::default();
    let face = book
        .resolve_native_name(
            &NativeFontNameRequest::new(
                Some("Roboto-Regular"),
                Some("Roboto-Regular"),
                PaintShapeDirection::LeftToRight,
            )
            .unwrap(),
        )
        .unwrap()
        .face()
        .clone();
    let face_hash = format!("{:x}", Sha256::digest(face.bytes()));
    let mut glyph_count = 0;
    for (index, case) in capture.cases.iter().enumerate() {
        let profile = &case.source_profile;
        let size = f32::from_bits(profile.source_size_bits);
        let (expected_name, expected_text, expected_size) = if index < 50 {
            let (name, text) = [
                ("marker1", "1."),
                ("marker9", "9."),
                ("marker10", "10."),
                ("markeraa", "aa."),
                ("bodyABC", "ABC"),
            ][index % 5];
            let size = [
                10.0_f32, 12.0, 15.0, 17.0, 20.0, 24.0, 30.0, 36.0, 45.0, 100.0,
            ][index / 5];
            (name, text, size)
        } else if index == 50 {
            ("native_AB_F10", "AB", 10.0)
        } else {
            ("native_AB_F30", "AB", 30.0)
        };
        assert_eq!(case.shape.name, expected_name);
        assert_eq!(case.shape.text_utf8, expected_text);
        assert_eq!(size.to_bits(), expected_size.to_bits());
        assert_eq!(profile.span_name, "Roboto-Regular");
        assert!(!profile.caller_direction);
        assert_eq!(profile.source_flags, 0);
        assert_eq!(profile.physical_font_sha256, face_hash);
        assert_eq!(
            (profile.physical_font_style, profile.physical_fakery),
            (400, 0)
        );
        assert_eq!(profile.physical_source_id, case.layout_source_id);
        assert!(case.layout_uses_selected_physical_face);
        let paint = PaintSpanProfile::new(size, 0)
            .unwrap()
            .metric_input()
            .unwrap();
        assert_eq!(paint.size.to_bits(), profile.paint_size_bits);
        let source = expected_text.encode_utf16().collect::<Vec<_>>();
        assert_eq!(case.shape.range_utf16, [0, source.len() as u32]);
        assert_eq!(case.shape.entry_geometry.source_utf16, source);
        let itemization = PaintItemization::new(expected_text, 0..source.len() as u32).unwrap();
        let mut shaper = face.paint_shaper(paint).unwrap();
        let piece = shaper
            .shape_text(PaintTextRequest::new(
                &itemization,
                PaintShapeDirection::LeftToRight,
            ))
            .unwrap();
        let layout = piece.layout();
        assert_eq!(
            layout
                .glyphs()
                .iter()
                .map(|glyph| glyph.id)
                .collect::<Vec<_>>(),
            case.shape.layout_piece.glyph_ids
        );
        assert_eq!(
            layout
                .glyphs()
                .iter()
                .map(|glyph| glyph.owner_utf16)
                .collect::<Vec<_>>(),
            case.shape.layout_piece.owners_utf16
        );
        assert_eq!(
            layout
                .glyphs()
                .iter()
                .map(|glyph| glyph.full_position.map(f32::to_bits))
                .collect::<Vec<_>>(),
            case.shape.layout_piece.full_positions_bits
        );
        assert_eq!(
            layout
                .character_advances()
                .iter()
                .map(|advance| advance.to_bits())
                .collect::<Vec<_>>(),
            case.shape.layout_piece.character_advances_bits
        );
        assert_eq!(
            layout.total_advance().to_bits(),
            case.shape.layout_piece.total_advance_bits
        );
        assert_eq!(layout.character_advances().len(), source.len());
        let entries = piece.entry_geometry();
        assert_eq!(entries.entries().len(), source.len());
        assert_eq!(entries.glyphs().len(), layout.glyphs().len());
        assert_eq!(
            entries
                .entries()
                .iter()
                .map(|entry| entry.advance().to_bits())
                .collect::<Vec<_>>(),
            case.shape.entry_geometry.entry_widths_bits
        );
        assert_eq!(
            entries
                .entries()
                .iter()
                .map(|entry| entry.ink_bounds().map(f32::to_bits))
                .collect::<Vec<_>>(),
            case.shape.entry_geometry.entry_ink_bits
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
            case.shape
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
        let callbacks = &case.shape.callbacks;
        assert!(callbacks.scalar_glyphs.is_empty());
        assert_eq!(callbacks.vector_raw_bits.len(), layout.glyphs().len());
        assert_eq!(callbacks.vector_quantized.len(), layout.glyphs().len());
        assert_eq!(
            callbacks
                .vector_glyphs
                .iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>(),
            case.shape.layout_piece.glyph_ids
        );
        assert_eq!(
            piece
                .shaped_runs()
                .iter()
                .flat_map(|run| run.glyphs().iter().map(|glyph| glyph.x_advance))
                .collect::<Vec<_>>(),
            callbacks.vector_quantized
        );
        let provider = face.paint_metrics(paint).unwrap();
        for (glyph, &expected_bits) in layout.glyphs().iter().zip(&callbacks.vector_raw_bits) {
            assert_eq!(
                provider.glyph(glyph.id).unwrap().advance.to_bits(),
                expected_bits
            );
        }
        if index == 50 {
            assert_eq!(
                callbacks.vector_raw_bits,
                [652.0_f32.to_bits(), 623.0_f32.to_bits()]
            );
            assert_eq!(callbacks.vector_quantized, [166912, 159488]);
        } else if index == 51 {
            assert_eq!(callbacks.vector_quantized, [501000, 478125]);
        }
        glyph_count += layout.glyphs().len();
    }
    assert_eq!(glyph_count, 134);
}
