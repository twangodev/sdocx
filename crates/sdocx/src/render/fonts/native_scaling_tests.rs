use std::collections::HashMap;

use crate::render::harfrust;

use harfrust::font::{BuiltinFontFuncs, FontFuncs};
use harfrust::{
    BufferClusterLevel, BufferFlags, Direction, Feature, FontRef, GlyphId, IntegerScalingRounding,
    Script, ShapeOptions, Shaper, ShaperData, Tag, UnicodeBuffer,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{FontBook, ResolvedFace};

#[derive(Deserialize)]
struct Capture {
    font_sha256: String,
    font_face_index: u32,
    memory_fills: [u8; 3],
    repeat_zero_fill: bool,
    context_order: ContextOrder,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct ContextOrder {
    pre: String,
    post: String,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    callbacks: Callbacks,
    hb_calls: Vec<ShapeCall>,
}

#[derive(Deserialize)]
struct Callbacks {
    vector_glyphs: Vec<Vec<u32>>,
    vector_quantized: Vec<i32>,
    scalar_glyphs: Vec<u32>,
}

#[derive(Deserialize)]
struct ShapeCall {
    nativefont_scale: [i32; 2],
    input: SourceInput,
    output: ShapeOutput,
}

#[derive(Deserialize)]
struct SourceInput {
    infos: Vec<UnicodeInfo>,
    direction: u32,
    script: u32,
    language: Option<String>,
    flags: u32,
    cluster_level: u32,
    content_type: u32,
    pre_context: Vec<u32>,
    post_context: Vec<u32>,
    features: Vec<SourceFeature>,
}

#[derive(Deserialize)]
struct UnicodeInfo {
    codepoint: u32,
    cluster: u32,
}

#[derive(Deserialize)]
struct SourceFeature {
    tag: u32,
    value: u32,
    start: u32,
    end: u32,
}

#[derive(Deserialize)]
struct ShapeOutput {
    infos: Vec<GlyphInfo>,
    positions: Vec<[i32; 4]>,
    content_type: u32,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct GlyphInfo {
    glyph_id: u32,
    cluster: u32,
}

impl SourceInput {
    fn buffer(&self) -> UnicodeBuffer {
        assert_eq!(self.content_type, 1);
        let mut buffer = UnicodeBuffer::new();
        for info in &self.infos {
            buffer.add(char::from_u32(info.codepoint).unwrap(), info.cluster);
        }
        buffer.set_direction(match self.direction {
            4 => Direction::LeftToRight,
            5 => Direction::RightToLeft,
            other => panic!("unsupported captured horizontal direction {other}"),
        });
        buffer.set_script(Script::from_iso15924_tag(tag(self.script)).unwrap());
        if let Some(language) = &self.language {
            buffer.set_language(language.parse().unwrap());
        }
        buffer.set_flags(BufferFlags::from_bits(self.flags).unwrap());
        buffer.set_cluster_level(match self.cluster_level {
            0 => BufferClusterLevel::MonotoneGraphemes,
            1 => BufferClusterLevel::MonotoneCharacters,
            2 => BufferClusterLevel::Characters,
            other => panic!("unsupported captured cluster level {other}"),
        });
        assert!(self.pre_context.len() <= 5 && self.post_context.len() <= 5);
        buffer.set_pre_context_codepoints(&self.pre_context);
        buffer.set_post_context_codepoints(&self.post_context);
        buffer
    }

    fn features(&self) -> Vec<Feature> {
        self.features
            .iter()
            .map(|feature| Feature {
                tag: tag(feature.tag),
                value: feature.value,
                start: feature.start,
                end: feature.end,
            })
            .collect()
    }
}

fn tag(value: u32) -> Tag {
    Tag::new(&value.to_be_bytes())
}

struct CapturedAdvances(HashMap<u32, i32>);

impl CapturedAdvances {
    fn new(callbacks: &Callbacks) -> Self {
        assert!(callbacks.scalar_glyphs.is_empty());
        let glyphs: Vec<_> = callbacks.vector_glyphs.iter().flatten().copied().collect();
        assert_eq!(glyphs.len(), callbacks.vector_quantized.len());
        let mut advances = HashMap::new();
        for (glyph, &advance) in glyphs.into_iter().zip(&callbacks.vector_quantized) {
            if let Some(previous) = advances.insert(glyph, advance) {
                assert_eq!(advance, previous, "consistent per-case native glyph metric");
            }
        }
        Self(advances)
    }
}

impl FontFuncs for CapturedAdvances {
    fn advance_width(&mut self, _: &BuiltinFontFuncs, glyph: GlyphId) -> i32 {
        *self
            .0
            .get(&glyph.to_u32())
            .expect("shaping requested a captured native glyph advance")
    }

    fn advance_height(&mut self, _: &BuiltinFontFuncs, _: GlyphId) -> i32 {
        panic!("horizontal fixtures require no vertical metric callback")
    }

    fn vertical_origin(&mut self, _: &BuiltinFontFuncs, _: GlyphId) -> (i32, i32) {
        panic!("horizontal fixtures require no vertical origin callback")
    }

    fn extents(&mut self, _: &BuiltinFontFuncs, _: GlyphId) -> Option<harfrust::GlyphExtents> {
        panic!("these fixtures require no uncaptured glyph extents callback")
    }
}

fn captures() -> [(Capture, [usize; 3]); 3] {
    [
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../conformance/table-text-shaping.json"
            ))
            .as_slice(),
            "a4c58481cb1e36bb7de8783bea49a800b10b4227177684c48f5b5c356d7a425b",
            [16, 16, 48],
        ),
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../conformance/table-text-shaping-numeric.json"
            ))
            .as_slice(),
            "1e476f7fc8254b2a6c7316c6ff18b9c436f8ab57707d536daa07e11eca2af455",
            [17, 19, 182],
        ),
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../conformance/table-text-shaping-gpos.json"
            ))
            .as_slice(),
            "9280953ac5de0b6b745baedcda73cf4d88a95a7b52cbfbc99188e0b66a71efb2",
            [4, 4, 12],
        ),
    ]
    .map(|(bytes, hash, counts)| {
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), hash);
        (serde_json::from_slice(bytes).unwrap(), counts)
    })
}

fn captured_face(capture: &Capture) -> ResolvedFace {
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(face.bytes())),
        capture.font_sha256
    );
    assert_eq!(face.index, capture.font_face_index);
    assert_eq!(capture.memory_fills, [0, 0xa5, 0xff]);
    assert!(capture.repeat_zero_fill);
    assert_eq!(capture.context_order.pre, "nearest_first");
    assert_eq!(capture.context_order.post, "source_order");
    face
}

fn shape(
    shaper: &Shaper<'_>,
    call: &ShapeCall,
    advances: &mut CapturedAdvances,
    rounding: Option<IntegerScalingRounding>,
) -> (Vec<GlyphInfo>, Vec<[i32; 4]>) {
    let features = call.input.features();
    let mut options = ShapeOptions::new()
        .features(&features)
        .scale_separate(Some((call.nativefont_scale[0], call.nativefont_scale[1])))
        .font_funcs(Some(advances));
    if let Some(rounding) = rounding {
        options = options.integer_scaling_rounding(rounding);
    }
    let output = shaper.shape(call.input.buffer(), options);
    let infos = output
        .glyph_infos()
        .iter()
        .map(|glyph| GlyphInfo {
            glyph_id: glyph.glyph_id,
            cluster: glyph.cluster,
        })
        .collect();
    let positions = output
        .glyph_positions()
        .iter()
        .map(|glyph| {
            [
                glyph.x_advance,
                glyph.y_advance,
                glyph.x_offset,
                glyph.y_offset,
            ]
        })
        .collect();
    (infos, positions)
}

#[test]
fn explicit_floor_shaping_matches_all_242_captured_positions_with_supplied_metrics() {
    let mut total = 0;
    for (capture, counts) in captures() {
        let face = captured_face(&capture);
        let font = FontRef::from_index(face.bytes(), face.index).unwrap();
        let data = ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        assert_eq!(capture.cases.len(), counts[0]);
        let mut calls = 0;
        let mut glyphs = 0;
        for case in &capture.cases {
            let mut advances = CapturedAdvances::new(&case.callbacks);
            for call in &case.hb_calls {
                assert_eq!(call.output.content_type, 2);
                let (infos, positions) = shape(
                    &shaper,
                    call,
                    &mut advances,
                    Some(IntegerScalingRounding::Floor),
                );
                assert_eq!(
                    infos, call.output.infos,
                    "{} glyphs and UTF-16 owners",
                    case.name
                );
                assert_eq!(
                    positions, call.output.positions,
                    "{} native scaled positions",
                    case.name
                );
                calls += 1;
                glyphs += positions.len();
            }
        }
        assert_eq!([calls, glyphs], [counts[1], counts[2]]);
        total += glyphs;
    }
    assert_eq!(total, 242);
}

#[test]
fn default_rounding_is_preserved_and_differs_from_actual_native_pairs() {
    let (capture, _) = captures().into_iter().next().unwrap();
    let face = captured_face(&capture);
    let font = FontRef::from_index(face.bytes(), face.index).unwrap();
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let case = capture
        .cases
        .iter()
        .find(|case| case.name == "av_default")
        .unwrap();
    let call = &case.hb_calls[0];
    let mut advances = CapturedAdvances::new(&case.callbacks);
    let implicit = shape(&shaper, call, &mut advances, None);
    let nearest = shape(
        &shaper,
        call,
        &mut advances,
        Some(IntegerScalingRounding::Nearest),
    );
    let floor = shape(
        &shaper,
        call,
        &mut advances,
        Some(IntegerScalingRounding::Floor),
    );
    assert_eq!(implicit, nearest);
    assert_eq!(nearest.0, floor.0);
    assert_eq!(nearest.1[0][0], 265417);
    assert_eq!(floor.1, call.output.positions);
    assert_eq!(floor.1[0][0], 265416);
    assert_ne!(nearest.1, call.output.positions);
}
