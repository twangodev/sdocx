use super::*;
use crate::fonts::FontBook;
use krilla::Document;
use krilla::geom::Size;
use krilla::page::PageSettings;
use krilla::paint::FillRule;
use krilla::tagging::TagTree;
use lopdf::content::Content;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    page_height: f32,
    rotation: f32,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    world_clip: Option<[f32; 4]>,
    pdf_scale: f32,
    backend_clip: bool,
    path_point_bits: Vec<[u32; 2]>,
}

fn clip_path([left, top, right, bottom]: [f32; 4]) -> Path {
    let mut builder = PathBuilder::new();
    builder.move_to(left, top);
    builder.line_to(right, top);
    builder.line_to(right, bottom);
    builder.line_to(left, bottom);
    builder.close();
    builder.finish().unwrap()
}

fn text(face: &ResolvedFace, source: &str, origin: [f64; 2]) -> NativeTextBlock {
    let font_data = face.shared_data();
    let parsed = ttf_parser::Face::parse(font_data.as_ref().as_ref(), face.index).unwrap();
    NativeTextBlock {
        source: Arc::from(source),
        runs: vec![NativeGlyphRun {
            face: face.clone(),
            font_size: 12.0,
            paint: NativeTextPaint {
                color: Color { r: 0, g: 0, b: 0 },
                bold: false,
                skew_x: 0.0,
            },
            glyphs: vec![NativeGlyph {
                glyph_id: u32::from(
                    parsed
                        .glyph_index(source.chars().next().unwrap())
                        .unwrap()
                        .0,
                ),
                origin,
                advance: [8.0, 0.0],
                source: 0..source.len(),
            }],
            variable: false,
        }],
    }
}

#[derive(Clone, Copy)]
struct GraphicsState {
    matrix: [f64; 6],
    clipped: bool,
}

impl GraphicsState {
    fn transform(self, [x, y]: [f64; 2]) -> [f64; 2] {
        let [a, b, c, d, e, f] = self.matrix;
        [a * x + c * y + e, b * x + d * y + f]
    }

    fn concat(&mut self, next: [f64; 6]) {
        let [a, b, c, d, e, f] = self.matrix;
        let [na, nb, nc, nd, ne, nf] = next;
        self.matrix = [
            a * na + c * nb,
            b * na + d * nb,
            a * nc + c * nd,
            b * nc + d * nd,
            a * ne + c * nf + e,
            b * ne + d * nf + f,
        ];
    }
}

fn bounds(points: &[[f64; 2]]) -> [f64; 4] {
    assert!(!points.is_empty());
    points.iter().fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |[left, top, right, bottom], [x, y]| {
            [left.min(*x), top.min(*y), right.max(*x), bottom.max(*y)]
        },
    )
}

#[test]
fn supplied_native_text_clips_survive_vector_pdf_transport() {
    let bytes = include_bytes!("../../../../../conformance/table-text-clip-paths.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "e5f6d442e39aedf995d807a276d86ab7ff877c31d044a5597b620adb4eaa6bdc"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 132);
    assert_eq!(capture.rotation, 0.0);
    assert_eq!(capture.page_height, 800.0);
    let native_coordinate_tolerance =
        f64::from(f32::from_bits(capture.page_height.to_bits() + 2) - capture.page_height);
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    let mut document = Document::new();
    let mut painter = NativePdfPainter::default();
    let mut tags = Vec::new();
    for case in &capture.cases {
        let mut page = document.start_page_with(PageSettings::new(
            Size::from_wh(600.0, capture.page_height).unwrap(),
        ));
        let mut surface = page.surface();
        surface.push_transform(&Transform::from_scale(case.pdf_scale, case.pdf_scale));
        let parent_transform = surface.ctm();
        if case.backend_clip {
            surface.push_clip_path(&clip_path(case.world_clip.unwrap()), &FillRule::NonZero);
        }
        tags.push(
            painter
                .paint(&text(&face, "A", [40.0, 80.0]), &mut surface)
                .unwrap()
                .into(),
        );
        assert_eq!(surface.ctm(), parent_transform);
        if case.backend_clip {
            surface.pop();
        }
        assert_eq!(surface.ctm(), parent_transform);
        tags.push(
            painter
                .paint(&text(&face, "B", [300.0, 400.0]), &mut surface)
                .unwrap()
                .into(),
        );
        assert_eq!(surface.ctm(), parent_transform);
        surface.pop();
        surface.finish();
        page.finish();
    }
    document.set_tag_tree(TagTree::from(tags));
    let pdf = lopdf::Document::load_mem(&document.finish().unwrap()).unwrap();
    assert!(pdf.objects.values().any(|object| {
        object
            .as_dict()
            .is_ok_and(|dictionary| dictionary.has(b"FontFile2"))
    }));
    assert!(!pdf.objects.values().any(|object| {
        object.as_dict().is_ok_and(|dictionary| {
            dictionary
                .get(b"Subtype")
                .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
        })
    }));
    let pages = pdf.get_pages();
    assert_eq!(pages.len(), capture.cases.len());
    let mut clip_count = 0;
    for ((page_number, page_id), case) in pages.into_iter().zip(&capture.cases) {
        let operations = Content::decode(&pdf.get_page_content(page_id).unwrap())
            .unwrap()
            .operations;
        let mut state = GraphicsState {
            matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            clipped: false,
        };
        let mut stack = Vec::new();
        let mut path = Vec::new();
        let mut path_closed = false;
        let mut clips = Vec::new();
        let mut text_clips = Vec::new();
        let mut logical_text = Vec::new();
        for operation in operations {
            let numeric = || {
                operation
                    .operands
                    .iter()
                    .map(|value| f64::from(value.as_float().unwrap()))
                    .collect::<Vec<_>>()
            };
            match operation.operator.as_str() {
                "q" => stack.push(state),
                "Q" => state = stack.pop().unwrap(),
                "cm" => state.concat(numeric().try_into().unwrap()),
                "m" => {
                    path_closed = false;
                    path.push(state.transform(numeric().try_into().unwrap()));
                }
                "l" => path.push(state.transform(numeric().try_into().unwrap())),
                "h" => path_closed = true,
                "re" => {
                    let [x, y, width, height]: [f64; 4] = numeric().try_into().unwrap();
                    path.extend(
                        [
                            [x, y],
                            [x + width, y],
                            [x + width, y + height],
                            [x, y + height],
                        ]
                        .map(|point| state.transform(point)),
                    );
                    path_closed = true;
                }
                "W" => {
                    assert!(path_closed, "{}: unclosed PDF clip path", case.name);
                    clips.push(bounds(&path));
                    state.clipped = true;
                }
                "n" => {
                    path.clear();
                    path_closed = false;
                }
                "BT" => text_clips.push(state.clipped),
                "BDC" => {
                    if let Ok(properties) = operation.operands[1].as_dict()
                        && let Ok(source) = properties.get(b"ActualText")
                    {
                        logical_text.push(source.as_str().unwrap().to_vec());
                    }
                }
                "Do" | "BI" | "W*" => {
                    panic!("unexpected vector transport operator in {}", case.name)
                }
                _ => {}
            }
        }
        assert!(stack.is_empty(), "{}", case.name);
        assert_eq!(text_clips, [case.backend_clip, false], "{}", case.name);
        assert_eq!(logical_text.first().unwrap(), b"A", "{}", case.name);
        assert_eq!(logical_text.last().unwrap(), b"B", "{}", case.name);
        assert_eq!(
            pdf.extract_text(&[page_number])
                .unwrap()
                .split_whitespace()
                .collect::<String>(),
            "AB",
            "{}",
            case.name
        );
        if case.backend_clip {
            assert_eq!(clips.len(), 1, "{}", case.name);
            let native_points = case
                .path_point_bits
                .iter()
                .map(|point| point.map(|bits| f64::from(f32::from_bits(bits))))
                .collect::<Vec<_>>();
            let expected = bounds(&native_points);
            for (actual, expected) in clips[0].into_iter().zip(expected) {
                let error = (actual - expected).abs();
                assert!(
                    error <= native_coordinate_tolerance,
                    "{}: PDF clip {actual} differs from native {expected} by {error}",
                    case.name
                );
            }
            clip_count += 1;
        } else {
            assert!(clips.is_empty(), "{}", case.name);
            assert!(case.path_point_bits.is_empty(), "{}", case.name);
        }
    }
    assert_eq!(clip_count, 69);
}
