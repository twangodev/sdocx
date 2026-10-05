use std::sync::Arc;

use lopdf::{Object, content::Content};
use rustybuzz::ttf_parser::{Face, GlyphId};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use svgtypes::Transform;

use super::*;
use crate::fonts::{FontBook, ResolvedFace};
use crate::pdf::{PdfOptions, render_native_scene_pdf_for_test};
use crate::render::RenderedScene;
use crate::render::text::native::{NativeGlyph, NativeGlyphRun, NativeTextBlock, NativeTextPaint};
use crate::render::text::native_cell_clip::NativeCellTextClipContext;
use crate::render::vector::{FontFamily, Svg, TSpan, Text};
use crate::text_index::TextIndex;
use crate::{Color, RenderedPage};

#[derive(Deserialize)]
struct Capture {
    page_height: f32,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    source_bbox: [f32; 4],
    run_bbox: [f32; 4],
    cell_origin: [f32; 2],
    pdf_scale: f32,
    world_clip: Option<[f32; 4]>,
    backend_clip: bool,
}

fn carrier(
    scene: &mut Scene,
    source: &str,
    actual_glyph: char,
    face: &ResolvedFace,
    origin: [f64; 2],
    size: f64,
) -> u16 {
    let font = Face::parse(face.bytes(), face.index).unwrap();
    let id = font.glyph_index(actual_glyph).unwrap();
    let block = NativeTextBlock {
        source: Arc::from(source),
        runs: vec![NativeGlyphRun {
            face: face.clone(),
            font_size: size,
            paint: NativeTextPaint {
                color: Color {
                    r: 20,
                    g: 40,
                    b: 60,
                },
                alpha: 255,
                bold: false,
                skew_x: 0.0,
            },
            glyphs: vec![NativeGlyph {
                glyph_id: u32::from(id.0),
                origin,
                advance: [size * 0.6, 0.0],
                source: TextIndex::new(source).source(0..1).unwrap(),
            }],
            variable: false,
        }],
    };
    let retained = scene.native_text().register(0, block).unwrap();
    let family = face.svg_family();
    scene.scope(
        Text::new("")
            .native_id(retained)
            .x(origin[0])
            .y(origin[1])
            .family(FontFamily::Named(&family)),
        |scene| scene.push(TSpan::new(source).font_size(size)),
    );
    id.0
}

#[derive(Clone)]
struct State {
    matrix: Transform,
    clipped: bool,
    font: Vec<u8>,
}

impl State {
    fn point(&self, x: f64, y: f64) -> [f64; 2] {
        let t = self.matrix;
        [t.a * x + t.c * y + t.e, t.b * x + t.d * y + t.f]
    }

    fn concat(&mut self, right: [f64; 6]) {
        let left = self.matrix;
        let [a, b, c, d, e, f] = right;
        self.matrix = Transform::new(
            left.a * a + left.c * b,
            left.b * a + left.d * b,
            left.a * c + left.c * d,
            left.b * c + left.d * d,
            left.a * e + left.c * f + left.e,
            left.b * e + left.d * f + left.f,
        );
    }
}

fn embedded_glyph(pdf: &lopdf::Document, font_name: &[u8], cid: u16) -> (Vec<u8>, u16) {
    let fonts = pdf.get_page_fonts(pdf.get_pages()[&1]).unwrap();
    let descriptor = fonts[font_name];
    let descendant = pdf
        .dereference(
            &descriptor
                .get(b"DescendantFonts")
                .unwrap()
                .as_array()
                .unwrap()[0],
        )
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let glyph = match pdf
        .dereference(descendant.get(b"CIDToGIDMap").unwrap())
        .unwrap()
        .1
    {
        Object::Name(name) if name == b"Identity" => cid,
        Object::Stream(stream) => {
            let mapping = stream.decompressed_content().unwrap();
            let offset = usize::from(cid) * 2;
            u16::from_be_bytes(mapping[offset..offset + 2].try_into().unwrap())
        }
        other => panic!("unsupported retained font mapping {other:?}"),
    };
    let descriptor = pdf
        .dereference(descendant.get(b"FontDescriptor").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let program = pdf
        .dereference(descriptor.get(b"FontFile2").unwrap())
        .unwrap()
        .1
        .as_stream()
        .unwrap();
    (program.decompressed_content().unwrap(), glyph)
}

fn bounds(points: &[[f64; 2]]) -> [f64; 4] {
    points.iter().fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |b, p| {
            [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ]
        },
    )
}

#[test]
fn scoped_native_svg_clips_reach_retained_pdf_glyphs_and_restore_neighbors() {
    let bytes = include_bytes!("../../../../../../../../conformance/table-text-clip-paths.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "e5f6d442e39aedf995d807a276d86ab7ff877c31d044a5597b620adb4eaa6bdc"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    let fonts = FontBook::default();
    let faces = [
        fonts.resolve("Roboto", false, false).unwrap(),
        fonts.resolve("Roboto Mono", false, false).unwrap(),
    ];
    let offset = [11.125, 23.375];
    let controls = capture
        .cases
        .iter()
        .filter(|case| {
            case.cell_origin == [0.0, 0.0]
                && ["inside-", "overflow-", "outside-horizontal-", "zero-width-"]
                    .iter()
                    .any(|prefix| case.name.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    assert_eq!(controls.len(), 12);
    for case in controls {
        let decision = NativeCellTextClipContext::new(case.source_bbox, case.cell_origin)
            .unwrap()
            .decide(case.run_bbox)
            .unwrap();
        let world = match decision {
            NativeCellRunClip::Unclipped => None,
            NativeCellRunClip::Clipped { world_rect, .. } => Some(world_rect),
        };
        assert_eq!(world, case.world_clip, "{}", case.name);
        let mut scene = Scene::new(Svg::new().width(300).height(capture.page_height));
        scene.retain_text();
        let mut first = None;
        NativeRunClip::new(decision, offset)
            .unwrap()
            .paint(&mut scene, |scene| {
                first = Some(carrier(scene, "A", 'V', &faces[0], [13.125, 43.375], 17.0));
            });
        let second = carrier(&mut scene, "B", 'M', &faces[1], [90.0, 100.0], 19.0);
        let registry = scene.take_native_text();
        let scene = RenderedScene {
            page: RenderedPage {
                source_page_index: 0,
                width: 300,
                height: capture.page_height as u32,
                svg: scene.finish(),
                text_diagnostics: Vec::new(),
                object_diagnostics: Vec::new(),
                geometry_diagnostics: Vec::new(),
            },
            text: registry,
            text_error: None,
        };
        let mut options = PdfOptions::from_font_book(&fonts);
        options.dpi = 72.0 / case.pdf_scale;
        let bytes = render_native_scene_pdf_for_test(&scene, &options)
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let page = pdf.get_dictionary(pdf.get_pages()[&1]).unwrap();
        let page_height = f64::from(
            page.get(b"MediaBox").unwrap().as_array().unwrap()[3]
                .as_float()
                .unwrap(),
        );
        let scale = 72.0 / f64::from(options.dpi);
        let page_bounds = [0.0, 0.0, 300.0 * scale, page_height];
        let tolerance =
            f64::from(f32::from_bits(capture.page_height.to_bits() + 4) - capture.page_height);
        let mut state = State {
            matrix: Transform::default(),
            clipped: false,
            font: Vec::new(),
        };
        let mut stack = Vec::new();
        let mut path = Vec::new();
        let mut clips = Vec::new();
        let mut page_clips = 0;
        let mut glyphs = Vec::new();
        let mut actual_text = Vec::new();
        for operation in Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
            .unwrap()
            .operations
        {
            let numbers = || {
                operation
                    .operands
                    .iter()
                    .map(|value| f64::from(value.as_float().unwrap()))
                    .collect::<Vec<_>>()
            };
            match operation.operator.as_str() {
                "q" => stack.push(state.clone()),
                "Q" => state = stack.pop().unwrap(),
                "cm" => state.concat(numbers().try_into().unwrap()),
                "m" | "l" => {
                    let [x, y]: [f64; 2] = numbers().try_into().unwrap();
                    path.push(state.point(x, y));
                }
                "re" => {
                    let [x, y, w, h]: [f64; 4] = numbers().try_into().unwrap();
                    path.extend(
                        [[x, y], [x + w, y], [x + w, y + h], [x, y + h]]
                            .map(|[x, y]| state.point(x, y)),
                    );
                }
                "W" | "W*" => {
                    let rectangle = bounds(&path);
                    if rectangle
                        .into_iter()
                        .zip(page_bounds)
                        .all(|(actual, expected)| (actual - expected).abs() <= tolerance)
                    {
                        page_clips += 1;
                    } else {
                        assert!(
                            glyphs.is_empty(),
                            "{}: native clip must precede text",
                            case.name
                        );
                        clips.push(rectangle);
                        state.clipped = true;
                    }
                }
                "n" => path.clear(),
                "Tf" => state.font = operation.operands[0].as_name().unwrap().to_vec(),
                "Tj" => {
                    let code = operation.operands[0].as_str().unwrap();
                    assert_eq!(code.len(), 2);
                    glyphs.push((
                        state.font.clone(),
                        u16::from_be_bytes(code.try_into().unwrap()),
                        state.clipped,
                    ));
                }
                "BDC" => {
                    if let Ok(properties) = operation.operands[1].as_dict()
                        && let Ok(text) = properties.get(b"ActualText")
                    {
                        actual_text.push(lopdf::decode_text_string(text).unwrap());
                    }
                }
                "Do" | "BI" => panic!("{}: unexpected nontext transport", case.name),
                _ => {}
            }
        }
        assert!(stack.is_empty(), "{}", case.name);
        assert!(page_clips > 0, "{}", case.name);
        assert_eq!(glyphs.len(), 2, "{}", case.name);
        assert_eq!(
            glyphs.iter().map(|glyph| glyph.2).collect::<Vec<_>>(),
            [case.backend_clip, false],
            "{}",
            case.name
        );
        assert_ne!(glyphs[0].0, glyphs[1].0);
        assert_eq!(actual_text, ["A", "B"], "{}", case.name);
        assert_eq!(
            pdf.extract_text(&[1])
                .unwrap()
                .split_whitespace()
                .collect::<String>(),
            "AB",
            "{}",
            case.name
        );
        assert_eq!(clips.len(), usize::from(case.backend_clip), "{}", case.name);
        if case.backend_clip {
            let expected = case.world_clip.unwrap().map(f64::from);
            let [left, bottom, right, top] = clips[0];
            let actual = [
                left / scale,
                (page_height - top) / scale,
                right / scale,
                (page_height - bottom) / scale,
            ];
            let expected = [
                expected[0] + offset[0],
                expected[1] + offset[1],
                expected[2] + offset[0],
                expected[3] + offset[1],
            ];
            for (actual, expected) in actual.into_iter().zip(expected) {
                assert!(
                    (actual - expected).abs() <= tolerance,
                    "{}: {actual} != {expected}",
                    case.name
                );
            }
        }
        for ((font_name, cid, _), (face, original)) in glyphs
            .iter()
            .zip(faces.iter().zip([first.unwrap(), second]))
        {
            let (program, glyph) = embedded_glyph(&pdf, font_name, *cid);
            let embedded = Face::parse(&program, 0).unwrap();
            let original_face = Face::parse(face.bytes(), face.index).unwrap();
            assert_eq!(
                embedded.glyph_bounding_box(GlyphId(glyph)),
                original_face.glyph_bounding_box(GlyphId(original)),
                "{}",
                case.name
            );
            assert_eq!(
                embedded.glyph_hor_advance(GlyphId(glyph)),
                original_face.glyph_hor_advance(GlyphId(original)),
                "{}",
                case.name
            );
        }
        assert!(
            !pdf.objects
                .values()
                .any(|object| object.as_stream().is_ok_and(|stream| stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|kind| kind.as_name().is_ok_and(|name| name == b"Image")))),
            "{}",
            case.name
        );
    }
}
