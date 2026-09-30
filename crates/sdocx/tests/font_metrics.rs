#![cfg(feature = "render")]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sdocx::fonts::{FontBook, FontError, UnicodeBuffer, fontdb};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use svgtypes::Transform;

#[derive(Deserialize)]
struct Fixture {
    version: u32,
    common_font_metrics: Metrics,
    fonts: Vec<Font>,
    font_samples: Vec<Sample>,
    native_reference: NativeReference,
}

#[derive(Deserialize)]
struct Metrics {
    units_per_em: u16,
    ascender: i16,
    descender: i16,
    line_gap: i16,
    covered_characters: Vec<String>,
    missing_characters: Vec<String>,
}

#[derive(Deserialize)]
struct Font {
    id: String,
    family: String,
    path: String,
    sha256: String,
    weight: u16,
    italic: bool,
    abc_glyph_ids: Vec<u32>,
    abc_hmtx_advances: Vec<i32>,
}

#[derive(Deserialize)]
struct Sample {
    font: String,
    text: String,
    font_size: f64,
    advance_units: i64,
    advance: f64,
    glyph_ids: Vec<u32>,
    clusters_utf8: Vec<u32>,
}

#[derive(Deserialize)]
struct NativeReference {
    fixture: String,
    sdocx_sha256: String,
    pdf_sha256: String,
    page_index: usize,
    pdf_to_svg_scale: f64,
    logical_pdf_page_height: f64,
    pdf_viewport: [f64; 4],
    coordinate_tolerance: f64,
    font_size: f64,
    lines: Vec<NativeLine>,
    additional_pages: Vec<NativePage>,
}

#[derive(Deserialize)]
struct NativePage {
    page_index: usize,
    lines: Vec<NativeLine>,
}

#[derive(Deserialize)]
struct NativeLine {
    text: String,
    x: f64,
    baseline: f64,
    font_size: Option<f64>,
    visible: Option<bool>,
    viewport_ink_bounds: Option<[f64; 4]>,
}

fn fixture() -> Fixture {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../../../conformance/text-metrics.json")).unwrap();
    assert_eq!(fixture.version, 1);
    assert_eq!(fixture.fonts.len(), 4);
    assert!(!fixture.font_samples.is_empty());
    fixture
}

fn buffer(text: &str) -> UnicodeBuffer {
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
#[ignore = "requires the external Hugging Face compatibility corpus"]
fn native_first_four_page_body_and_saved_code_match_independent_pdf_layout() {
    let reference = fixture().native_reference;
    let root = std::env::var_os("SDOCX_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../hf"))
        .canonicalize()
        .expect("corpus directory; see conformance/README.md");
    let source = std::fs::read(root.join(format!("{}.sdocx", reference.fixture))).unwrap();
    let pdf = std::fs::read(root.join(format!("{}.pdf", reference.fixture))).unwrap();
    assert_eq!(sha256(&source), reference.sdocx_sha256, "source archive");
    assert_eq!(sha256(&pdf), reference.pdf_sha256, "native reference PDF");

    let native_pdf = lopdf::Document::load_mem(&pdf).unwrap();
    let pages = native_pdf.get_pages();
    let document = sdocx::parse_bytes(&source).unwrap();
    assert_eq!(
        reference
            .additional_pages
            .iter()
            .map(|page| page.page_index)
            .collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    for (page_index, lines) in std::iter::once((reference.page_index, &reference.lines)).chain(
        reference
            .additional_pages
            .iter()
            .map(|page| (page.page_index, &page.lines)),
    ) {
        let page_id = *pages.values().nth(page_index).unwrap();
        let media_box = native_pdf
            .get_dictionary(page_id)
            .unwrap()
            .get(b"MediaBox")
            .unwrap()
            .as_array()
            .unwrap();
        let media_box =
            std::array::from_fn::<_, 4, _>(|index| f64::from(media_box[index].as_float().unwrap()));
        assert_eq!(
            media_box, reference.pdf_viewport,
            "native page {page_index}"
        );
        let native_viewport_height = media_box[3];
        let viewport_offset = (reference.logical_pdf_page_height - native_viewport_height)
            * reference.pdf_to_svg_scale;
        let svg = sdocx::render_page_svg(&document, page_index, &Default::default())
            .unwrap()
            .svg;
        assert_native_lines(&svg, page_index, lines, &reference, viewport_offset);
    }
}

fn compose_transform(left: Transform, right: Transform) -> Transform {
    Transform::new(
        left.a * right.a + left.c * right.b,
        left.b * right.a + left.d * right.b,
        left.a * right.c + left.c * right.d,
        left.b * right.c + left.d * right.d,
        left.a * right.e + left.c * right.f + left.e,
        left.b * right.e + left.d * right.f + left.f,
    )
}

fn inherited_number(node: roxmltree::Node<'_, '_>, attribute: &str) -> f64 {
    node.ancestors()
        .find_map(|ancestor| ancestor.attribute(attribute))
        .unwrap()
        .split([' ', ','])
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn assert_native_lines(
    svg: &str,
    page_index: usize,
    expected_lines: &[NativeLine],
    reference: &NativeReference,
    viewport_offset: f64,
) {
    let xml = roxmltree::Document::parse(svg).unwrap();
    let lines = xml
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(|node| {
            let text = node
                .descendants()
                .filter(|child| child.is_text())
                .filter_map(|child| child.text())
                .collect::<String>();
            (node, text)
        })
        .collect::<Vec<_>>();
    for expected in expected_lines {
        let matches = lines
            .iter()
            .filter(|(_, text)| *text == expected.text)
            .collect::<Vec<_>>();
        if expected.visible == Some(false) {
            let [left, top, right, bottom] = expected.viewport_ink_bounds.unwrap();
            let viewport_width = reference.pdf_viewport[2] * reference.pdf_to_svg_scale;
            let viewport_height = reference.pdf_viewport[3] * reference.pdf_to_svg_scale;
            assert!(
                right <= 0.0 || bottom <= 0.0 || left >= viewport_width || top >= viewport_height,
                "native page {page_index} {:?} ink must lie outside the viewport",
                expected.text
            );
            assert!(
                matches.is_empty(),
                "native page {page_index} {:?} is clipped and must not emit SVG text",
                expected.text
            );
            continue;
        }
        assert_eq!(matches.len(), 1, "page {page_index} {:?}", expected.text);
        let actual = matches[0].0;
        let positioned = actual
            .descendants()
            .find(|node| node.has_tag_name("tspan"))
            .unwrap_or(actual);
        let mut ancestors = positioned
            .ancestors()
            .filter(|node| node.is_element())
            .collect::<Vec<_>>();
        ancestors.reverse();
        let transform = ancestors
            .iter()
            .filter_map(|node| node.attribute("transform"))
            .fold(Transform::default(), |all, value| {
                compose_transform(all, value.parse().unwrap())
            });
        let local_x = inherited_number(positioned, "x");
        let local_y = inherited_number(positioned, "y");
        let x = transform.a * local_x + transform.c * local_y + transform.e;
        let y = transform.b * local_x + transform.d * local_y + transform.f;
        for (attribute, actual, native) in [
            ("x", x, expected.x),
            ("y", y, expected.baseline - viewport_offset),
        ] {
            assert!(
                (actual - native).abs() <= reference.coordinate_tolerance,
                "page {page_index} {:?} {attribute}: {actual} vs native viewport {native}",
                expected.text
            );
        }
        let spans = actual
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .collect::<Vec<_>>();
        assert!(!spans.is_empty(), "page {page_index} {:?}", expected.text);
        let native_font_size = expected.font_size.unwrap_or(reference.font_size);
        for span in spans {
            let actual_font_size = inherited_number(span, "font-size");
            assert!(
                (actual_font_size - native_font_size).abs() < 0.0001,
                "page {page_index} {:?} font size: {actual_font_size} vs {native_font_size}",
                expected.text
            );
        }
    }
}

#[test]
fn resolved_faces_match_independent_font_tables_and_hashes() {
    let fixture = fixture();
    let book = FontBook::default();
    let metrics = fixture.common_font_metrics;
    for font in fixture.fonts {
        let face = book
            .resolve(&font.family, font.weight == 700, font.italic)
            .unwrap();
        assert_eq!(face.family, font.family, "{}", font.id);
        assert_eq!(face.weight.0, font.weight, "{}", font.id);
        assert_eq!(
            face.style,
            if font.italic {
                fontdb::Style::Italic
            } else {
                fontdb::Style::Normal
            },
            "{}",
            font.id
        );
        assert_eq!(face.index, 0);
        assert_eq!(sha256(face.bytes()), font.sha256, "{}", font.id);
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(&font.path);
        assert_eq!(sha256(&std::fs::read(path).unwrap()), font.sha256);
        assert_eq!(face.metrics.units_per_em, metrics.units_per_em);
        assert_eq!(face.metrics.ascent, metrics.ascender);
        assert_eq!(face.metrics.descent, metrics.descender);
        assert_eq!(face.metrics.line_gap, metrics.line_gap);

        let abc = face.shape(buffer("ABC"), &[]).unwrap();
        assert_eq!(
            abc.glyphs.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
            font.abc_glyph_ids,
            "{}",
            font.id
        );
        assert_eq!(
            abc.glyphs
                .iter()
                .map(|glyph| glyph.x_advance)
                .collect::<Vec<_>>(),
            font.abc_hmtx_advances,
            "{}",
            font.id
        );
        for text in &metrics.covered_characters {
            assert!(
                !face.shape(buffer(text), &[]).unwrap().has_missing_glyphs(),
                "{} must cover {text:?}",
                font.id
            );
        }
        for text in &metrics.missing_characters {
            assert!(
                face.shape(buffer(text), &[]).unwrap().has_missing_glyphs(),
                "{} must report missing {text:?}",
                font.id
            );
        }
    }
}

#[test]
fn shaped_runs_match_independent_glyph_clusters_and_advances() {
    let fixture = fixture();
    let book = FontBook::default();
    for sample in fixture.font_samples {
        let font = fixture
            .fonts
            .iter()
            .find(|font| font.id == sample.font)
            .unwrap();
        let face = book
            .resolve(&font.family, font.weight == 700, font.italic)
            .unwrap();
        let run = face.shape(buffer(&sample.text), &[]).unwrap();
        assert_eq!(run.metrics, face.metrics);
        assert!(!run.has_missing_glyphs());
        assert_eq!(
            run.glyphs.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
            sample.glyph_ids,
            "{} {:?}",
            sample.font,
            sample.text
        );
        assert_eq!(
            run.glyphs
                .iter()
                .map(|glyph| glyph.cluster)
                .collect::<Vec<_>>(),
            sample.clusters_utf8,
            "{} {:?}",
            sample.font,
            sample.text
        );
        assert_eq!(run.advance_x(), sample.advance_units);
        let advance =
            run.advance_x() as f64 * sample.font_size / f64::from(run.metrics.units_per_em);
        assert_eq!(advance, sample.advance, "{} {:?}", sample.font, sample.text);
    }
}

#[test]
fn font_books_share_defaults_and_preserve_caller_database_selection() {
    let first = FontBook::default();
    let second = FontBook::default();
    assert!(Arc::ptr_eq(&first.database(), &second.database()));
    assert_eq!(first.database().faces().count(), 8);
    let bold = first.resolve("Roboto", true, false).unwrap();
    let repeated = second.resolve("Roboto", true, false).unwrap();
    assert_eq!(bold.id, repeated.id);
    assert!(std::ptr::eq(bold.bytes(), repeated.bytes()));

    let mut database = fontdb::Database::new();
    database.load_font_data(bold.bytes().to_vec());
    let database = Arc::new(database);
    let custom = FontBook::new(database.clone());
    assert!(Arc::ptr_eq(&custom.database(), &database));
    assert_eq!(custom.database().faces().count(), 1);
    let nearest = custom.resolve("Roboto", false, false).unwrap();
    assert_eq!(nearest.weight, fontdb::Weight::BOLD);
    assert_eq!(nearest.style, fontdb::Style::Normal);
    assert_eq!(nearest.bytes(), bold.bytes());
    assert!(matches!(
        custom.resolve("monospace", false, false),
        Err(FontError::MissingFont { .. })
    ));
    assert!(matches!(
        FontBook::new(Arc::new(fontdb::Database::new())).resolve("Roboto", false, false),
        Err(FontError::MissingFont { .. })
    ));
}
