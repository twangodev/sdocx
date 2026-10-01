#![cfg(all(feature = "render", feature = "serde"))]

#[path = "support/shape_text.rs"]
mod shape_text;

#[cfg(feature = "pdf")]
#[path = "support/pdf_geometry.rs"]
mod pdf_geometry;

use sdocx::{RichTextParagraph, RichTextParagraphType, TextDiagnosticKind};
use shape_text::{assert_lines, bounds, document, render, shape, text};

#[test]
fn known_shape_geometry_overrides_the_asymmetric_saved_child_frame() {
    for (kind, x, y) in [(1, 30.0, 25.0), (4, 0.0, 10.0), (8, 52.0, 36.0)] {
        let doc = document(shape(kind, text("A")), 1);
        let source = serde_json::to_value(&doc).unwrap();
        for replay in [false, true] {
            let page = render(&doc, replay);
            assert_lines(&page, &[("A", x, y)]);
            assert!(page.text_diagnostics.is_empty());
            assert_eq!(render(&doc, replay), page);
        }
        assert_eq!(serde_json::to_value(&doc).unwrap(), source);
    }
}

#[cfg(feature = "pdf")]
#[test]
fn synthesized_shape_text_keeps_its_frame_and_selectable_vector_pdf() {
    use sdocx::{RichTextSpan, RichTextSpanType, SpanIntervalType};
    use std::sync::Arc;

    let mut database = sdocx::fonts::fontdb::Database::new();
    database.load_font_data(include_bytes!("../assets/fonts/Roboto-Regular.ttf").to_vec());
    database.set_sans_serif_family("Roboto");
    let fonts = sdocx::fonts::FontBook::new(Arc::new(database));
    let options = sdocx::PdfOptions::new(fonts.database());
    for (kind, x, y) in [(1, 30.0, 25.0), (4, 0.0, 10.0), (8, 52.0, 36.0)] {
        let mut content = text("A");
        content.spans = [RichTextSpanType::Bold, RichTextSpanType::Italic]
            .into_iter()
            .map(|kind| RichTextSpan {
                kind,
                start_utf16: 0,
                end_utf16: 1,
                interval_type: SpanIntervalType::from(1),
                payload: vec![1, 0],
            })
            .collect();
        let doc = document(shape(kind, content), 1);
        let layout = sdocx::layout_document(&doc);
        for replay in [false, true] {
            let render = if replay {
                sdocx::render_layout_page_replay_svg_with_fonts
            } else {
                sdocx::render_layout_page_svg_with_fonts
            };
            let page = render(&doc, &layout, 0, &Default::default(), &fonts).unwrap();
            assert_lines(&page, &[("A", x, y)]);
            assert!(page.text_diagnostics.is_empty());
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            let span = xml
                .descendants()
                .find(|node| node.has_tag_name("tspan"))
                .unwrap();
            assert_eq!(span.attribute("font-style"), Some("italic"));
            assert_eq!(span.attribute("font-weight"), Some("bold"));
        }
        let bytes = sdocx::render_layout_pages_pdf_with_fonts(
            &doc,
            &layout,
            &[0],
            &Default::default(),
            &options,
            &fonts,
        )
        .unwrap();
        let geometry = pdf_geometry::read(&bytes, f64::from(options.dpi));
        assert_eq!(geometry.source, "A");
        assert_eq!(geometry.extracted_text.trim(), "A");
        assert_eq!(pdf_geometry::tagged_source(&bytes), "A");
        assert_eq!(geometry.image_resources, 0);
        assert_eq!(geometry.text.len(), 1);
        assert!(
            (geometry.text[0].1 - x).abs() < 0.0001,
            "{kind}: {:?}",
            geometry.text
        );
        assert!(
            (geometry.text[0].2 - y).abs() < 0.0001,
            "{kind}: {:?}",
            geometry.text
        );
    }
}

#[test]
fn margins_scale_with_density_and_gravity_uses_the_inset_height() {
    // Density 3: F30, line height 40.5, margins top 9/bottom 15.
    // Inset heights 70/100/48 give center offsets 2.75/17.75/0.
    for (kind, x, top, center, bottom) in [
        (1, 36.0, 54.0, 56.75, 59.5),
        (4, 6.0, 39.0, 56.75, 74.5),
        (8, 58.0, 65.0, 65.0, 65.0),
    ] {
        for (gravity, y) in [(0, top), (1, center), (2, bottom)] {
            let mut content = text("A");
            content.margins = Some([2.0, 3.0, 4.0, 5.0]);
            content.gravity = Some(gravity);
            let doc = document(shape(kind, content), 3);
            for replay in [false, true] {
                let page = render(&doc, replay);
                assert_lines(&page, &[("A", x, y)]);
                assert!(page.text_diagnostics.is_empty());
            }
        }
    }
}

#[test]
fn alignment_uses_the_pinned_advance_and_the_native_inset_width() {
    // Canonical Roboto ABC advance is 3944/2048 * F10 = 19.2578125.
    for (kind, baseline, right) in [
        (1, 25.0, 150.7421875),
        (4, 10.0, 180.7421875),
        (8, 36.0, 128.7421875),
    ] {
        for (alignment, x) in [(2_u32, 90.37109375), (1, right)] {
            let mut content = text("ABC");
            content.paragraphs.push(RichTextParagraph {
                kind: RichTextParagraphType::Alignment,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: alignment.to_le_bytes().to_vec(),
            });
            let doc = document(shape(kind, content), 1);
            for replay in [false, true] {
                let page = render(&doc, replay);
                assert_lines(&page, &[("ABC", x, baseline)]);
                assert!(page.text_diagnostics.is_empty());
            }
        }
    }
}

#[test]
fn wrapping_ceils_the_inset_width_instead_of_the_outer_shape_width() {
    // Ellipse outer widths 28/27 yield inset widths 19.6/18.9, ceiled to 20/19.
    // ABC's 19.2578125px fits only the first; each default F10 line adds 13.5.
    for (width, expected) in [
        (28.0, vec![("ABC", 4.2, 25.0)]),
        (27.0, vec![("AB", 4.05, 25.0), ("C", 4.05, 38.5)]),
    ] {
        let mut shape = shape(1, text("ABC"));
        shape.geometry_bbox = bounds(0.0, 0.0, width, 100.0);
        let doc = document(shape, 1);
        for replay in [false, true] {
            let page = render(&doc, replay);
            assert_lines(&page, &expected);
            assert!(page.text_diagnostics.is_empty());
        }
    }
}

#[test]
fn shape_rotation_uses_the_original_geometry_pivot_and_ignores_stale_child_rotation() {
    let mut content = text("A");
    content.rotation_degrees = Some(-73.0);
    let mut shape = shape(1, content);
    shape.geometry_bbox = bounds(20.0, -10.0, 200.0, 100.0);
    shape.rotation_degrees = 30.0;
    let doc = document(shape, 1);
    for replay in [false, true] {
        let page = render(&doc, replay);
        assert_lines(&page, &[("A", 50.0, 15.0)]);
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert!(
            xml.descendants()
                .any(|node| node.attribute("transform") == Some("rotate(30.00 120.00 40.00)"))
        );
        assert!(
            !xml.descendants()
                .filter_map(|node| node.attribute("transform"))
                .any(|transform| transform.contains("-73"))
        );
        assert!(page.text_diagnostics.is_empty());
    }
}

#[test]
fn unsupported_shapes_preserve_the_saved_frame_and_empty_text_has_no_diagnostic() {
    for kind in [2, 3, 5, 999] {
        for replay in [false, true] {
            let page = render(&document(shape(kind, text("A")), 1), replay);
            assert_lines(&page, &[("A", 71.0, 83.0)]);
            assert_eq!(page.text_diagnostics.len(), 1);
            assert_eq!(
                page.text_diagnostics[0].kind,
                TextDiagnosticKind::UnsupportedTextFrame
            );
            let empty = render(&document(shape(kind, text("")), 1), replay);
            assert_lines(&empty, &[]);
            assert!(empty.text_diagnostics.is_empty());
        }
    }
}

#[test]
fn invalid_known_shape_geometry_uses_placed_recovery_and_never_document_flow() {
    let mut content = text("A");
    content.bbox = bounds(71.0, 73.0, 0.0, 0.0);
    let mut shape = shape(4, content);
    shape.geometry_bbox = bounds(0.0, 0.0, 200.0, 0.0);
    let doc = document(shape, 1);
    for replay in [false, true] {
        let page = render(&doc, replay);
        assert_lines(&page, &[("A", 71.0, 83.0)]);
        assert!(
            page.text_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == TextDiagnosticKind::InvalidGeometry)
        );
        assert!(
            !page
                .text_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == TextDiagnosticKind::UnsupportedTextFrame)
        );
    }
}

#[test]
fn native_integer_width_survives_when_large_origin_absorbs_the_measured_endpoint() {
    let mut shape = shape(4, text("A A"));
    shape.geometry_bbox = bounds(1e30, 0.0, 1e30, 100.0);
    let doc = document(shape, 1);
    let source = serde_json::to_value(&doc).unwrap();
    // Widget's finite native width saturates to INTMAX. Adding that width to
    // this origin cannot retain the increment in f64; wrapping needs the width.
    for replay in [false, true] {
        let page = render(&doc, replay);
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let lines: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .collect();
        assert_eq!(lines.len(), 1);
        let actual: String = lines[0]
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .filter_map(|node| node.text())
            .collect();
        assert_eq!(actual, "A A");
        assert!(
            page.text_diagnostics.is_empty(),
            "{:?}",
            page.text_diagnostics
        );
        assert!(page.object_diagnostics.is_empty());
        assert_eq!(render(&doc, replay), page);
    }
    assert_eq!(serde_json::to_value(&doc).unwrap(), source);
}

#[cfg(feature = "pdf")]
#[test]
fn native_shape_frames_export_selectable_vector_text_with_embedded_fonts() {
    for kind in [1, 4, 8] {
        let doc = document(shape(kind, text("ABC")), 1);
        for replay in [false, true] {
            let page = render(&doc, replay);
            let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            assert_eq!(
                pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
                "ABC"
            );
            assert!(
                !pdf.objects
                    .values()
                    .any(|object| object
                        .as_stream()
                        .is_ok_and(|stream| stream.dict.get(b"Subtype").is_ok_and(|value| value
                            .as_name()
                            .is_ok_and(|name| name == b"Image"))))
            );
            assert!(pdf.objects.values().any(|object| {
                object
                    .as_dict()
                    .is_ok_and(|dict| dict.has(b"FontFile2") || dict.has(b"FontFile3"))
            }));
        }
    }
}
