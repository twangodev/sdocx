//! Optional native fixed-axis normalization for the bounded PDF-paper route.

use crate::{BoundingBox, Document, LayoutDocument, Page, Stroke};

#[derive(Clone, Copy)]
pub(super) struct FixedAxisProfile {
    pub(super) source_dimensions: [u32; 2],
    pub(super) scale: f32,
}

pub(super) struct NormalizedPaperInputs {
    pub(super) document: Document,
    pub(super) layout: LayoutDocument,
    pub(super) profile: FixedAxisProfile,
}

pub(super) fn normalize_for_fixed_axis(
    document: &Document,
    layout: &LayoutDocument,
    page_indices: &[usize],
    fixed_axis_size: u32,
) -> Result<NormalizedPaperInputs, super::PdfError> {
    if fixed_axis_size <= 31 || fixed_axis_size > i32::MAX as u32 {
        return Err(super::PdfError::InvalidPdfPaperFixedAxisSize);
    }
    let selected = page_indices
        .first()
        .and_then(|index| layout.pages.get(*index))
        .ok_or(super::PdfError::EmptyDocument)?;
    let record_count = selected
        .page
        .background
        .pdf_paper
        .as_ref()
        .map_or(0, Vec::len);
    let unsupported = || super::PdfError::UnsupportedPdfPaper {
        output_page_index: 0,
        source_page_index: selected.source_page_index,
        record_count,
    };
    if document.metadata.page_mode != Some(1)
        || document.metadata.orientation != Some(0)
        || document.pages.len() != 1
        || layout.pages.len() != 1
        || page_indices != [0]
        || selected.source_page_index != 0
    {
        return Err(unsupported());
    }
    let (source_width, source_height) =
        document.metadata.page_dimensions.ok_or_else(unsupported)?;
    if source_width <= 31
        || source_height == 0
        || source_width > i32::MAX as u32
        || source_height > i32::MAX as u32
        || document.pages[0].width != source_width
        || document.pages[0].height != source_height
        || selected.page.width != source_width
        || selected.page.height != source_height
    {
        return Err(unsupported());
    }

    let scale = fixed_axis_size as f32 / source_width as f32;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(unsupported());
    }
    let normalized_width = if scale == 1.0 {
        source_width
    } else {
        fixed_axis_size
    };
    let note_height = if scale == 1.0 {
        source_height
    } else {
        scaled_page_height(source_height, scale, fixed_axis_size).ok_or_else(unsupported)?
    };
    let profile = FixedAxisProfile {
        source_dimensions: [source_width, source_height],
        scale,
    };

    let mut normalized_document = document.clone();
    normalized_document.metadata.page_dimensions = Some((normalized_width, note_height));
    normalize_page(
        &mut normalized_document.pages[0],
        normalized_width,
        note_height,
        scale,
    )?;

    let mut normalized_layout = layout.clone();
    normalize_page(
        &mut normalized_layout.pages[0].page,
        normalized_width,
        note_height,
        scale,
    )?;

    Ok(NormalizedPaperInputs {
        document: normalized_document,
        layout: normalized_layout,
        profile,
    })
}

fn normalize_page(
    page: &mut Page,
    fixed_axis_size: u32,
    note_height: u32,
    scale: f32,
) -> Result<(), super::PdfError> {
    page.width = fixed_axis_size;
    page.height = note_height;
    if scale == 1.0 {
        return Ok(());
    }
    page.content_bbox = scale_bbox(page.content_bbox, scale)?;
    for stroke in page.strokes_mut() {
        normalize_stroke(stroke, scale)?;
    }
    Ok(())
}

fn normalize_stroke(stroke: &mut Stroke, scale: f32) -> Result<(), super::PdfError> {
    let scale_f64 = |value: f64| {
        let result = (value as f32) * scale;
        result.is_finite().then_some(f64::from(result))
    };
    stroke.bbox = scale_bbox(stroke.bbox, scale)?;
    for point in &mut stroke.points {
        point.x = scale_f64(point.x).ok_or(super::PdfError::InvalidPageSize { page_index: 0 })?;
        point.y = scale_f64(point.y).ok_or(super::PdfError::InvalidPageSize { page_index: 0 })?;
    }
    stroke.pen_width = scale_f32(stroke.pen_width, scale)?;
    if let Some(rendering) = &mut stroke.rendering {
        if let Some(metadata) = rendering.metadata.as_deref_mut() {
            metadata.bbox = scale_bbox(metadata.bbox, scale)?;
        }
        if let Some(pen_size) = &mut rendering.style.pen_size {
            *pen_size = scale_f32(*pen_size, scale)?;
        }
    }
    Ok(())
}

fn scale_bbox(bbox: BoundingBox, scale: f32) -> Result<BoundingBox, super::PdfError> {
    let scale_coordinate = |value: f64| {
        let result = (value as f32) * scale;
        result.is_finite().then_some(f64::from(result))
    };
    Ok(BoundingBox {
        x_min: scale_coordinate(bbox.x_min)
            .ok_or(super::PdfError::InvalidPageSize { page_index: 0 })?,
        y_min: scale_coordinate(bbox.y_min)
            .ok_or(super::PdfError::InvalidPageSize { page_index: 0 })?,
        x_max: scale_coordinate(bbox.x_max)
            .ok_or(super::PdfError::InvalidPageSize { page_index: 0 })?,
        y_max: scale_coordinate(bbox.y_max)
            .ok_or(super::PdfError::InvalidPageSize { page_index: 0 })?,
    })
}

fn scale_f32(value: f32, scale: f32) -> Result<f32, super::PdfError> {
    let result = value * scale;
    result
        .is_finite()
        .then_some(result)
        .ok_or(super::PdfError::InvalidPageSize { page_index: 0 })
}

fn scaled_page_height(source_height: u32, scale: f32, fixed_axis_size: u32) -> Option<u32> {
    let scaled = source_height as f32 * scale;
    let scaled = if fixed_axis_size == 2256 {
        scaled.round()
    } else {
        scaled.trunc()
    };
    (scaled.is_finite() && scaled >= 1.0 && scaled < i32::MAX as f32).then_some(scaled as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DocumentMetadata, ObjectMetadata, PageObject, PdfPaperRecord, PdfPaperRectangle,
        StrokeProperties, StrokeRendering, StrokeStyle,
    };

    fn bbox(x_min: f64, y_min: f64, x_max: f64, y_max: f64) -> BoundingBox {
        BoundingBox {
            x_min,
            y_min,
            x_max,
            y_max,
        }
    }

    fn object_metadata(bbox: BoundingBox) -> ObjectMetadata {
        ObjectMetadata {
            format_version: 0,
            uuid: "stroke".to_owned(),
            modified_time_raw: 0,
            bbox,
            replay_timestamp_raw: 0,
            resize_mode_raw: 0,
            rotatable: false,
            selectable: true,
            movable: true,
            visible: true,
            replayable: true,
            out_of_canvas_enabled: false,
            template: false,
            flip_enabled: false,
            float_drawn_rect: false,
            locked: false,
            removable: true,
            rotation_degrees: None,
            property_mask: Vec::new(),
            field_mask: Vec::new(),
            fixed_trailing_data: Vec::new(),
            flexible_trailing_data: Vec::new(),
        }
    }

    fn scaled_fixture() -> (Document, LayoutDocument) {
        let source_bbox = bbox(100.0, 200.0, 300.0, 400.0);
        let stroke = Stroke {
            rendering: Some(StrokeRendering {
                metadata: Some(Box::new(object_metadata(source_bbox))),
                pen_name: None,
                advanced_settings: None,
                tool_type_raw: 0,
                properties: StrokeProperties {
                    compressed: false,
                    replay_only: false,
                    stylus_channels: false,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: false,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    pen_size: Some(8.0),
                    ..StrokeStyle::default()
                },
            }),
            bbox: source_bbox,
            points: vec![crate::Point {
                x: 101.25,
                y: 202.5,
            }],
            pressures: Vec::new(),
            timestamps: Vec::new(),
            tilts: Vec::new(),
            orientations: Vec::new(),
            color: None,
            pen_width: 12.0,
        };
        let mut page = Page {
            uuid: "page".to_owned(),
            width: 1_812,
            height: 15_372,
            content_bbox: source_bbox,
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageObject::from(stroke)],
        };
        page.background.pdf_paper = Some(vec![PdfPaperRecord {
            media_id: 7,
            page_index: 0,
            rectangle: PdfPaperRectangle::Integer([0, 0, 1_812, 1_485]),
        }]);
        let document = Document {
            pages: vec![page.clone()],
            metadata: DocumentMetadata {
                page_mode: Some(1),
                orientation: Some(0),
                page_dimensions: Some((1_812, 15_372)),
                default_page_dimensions: Some((1_080, 1_527)),
                ..DocumentMetadata::default()
            },
        };
        let layout = LayoutDocument {
            pages: vec![crate::LayoutPage {
                source_page_index: 0,
                page,
                body_text: None,
            }],
            stored_page_count: 1,
            omitted_trailing_blank_page: false,
        };
        (document, layout)
    }

    #[test]
    fn fixed_axis_profile_matches_native_f32_scale_and_preserves_source_records() {
        let (document, layout) = scaled_fixture();
        let normalized = normalize_for_fixed_axis(&document, &layout, &[0], 1_080).unwrap();
        let scale = 1_080.0_f32 / 1_812.0_f32;
        assert_eq!(scale.to_bits(), 0x3f18_9531);
        assert_eq!(normalized.profile.scale.to_bits(), scale.to_bits());
        assert_eq!(normalized.profile.source_dimensions, [1_812, 15_372]);
        assert_eq!(
            normalized.document.metadata.page_dimensions.unwrap().0,
            1_080
        );
        assert_eq!(
            normalized.document.metadata.page_dimensions.unwrap().1,
            (15_372.0_f32 * scale).trunc() as u32
        );
        assert_eq!(
            normalized.document.metadata.default_page_dimensions,
            Some((1_080, 1_527))
        );
        assert_eq!(
            normalized.layout.pages[0].page.background.pdf_paper,
            layout.pages[0].page.background.pdf_paper
        );
        let crate::PageObjectContent::Stroke(stroke) =
            &normalized.layout.pages[0].page.objects[0].content
        else {
            panic!("fixture stroke")
        };
        assert_eq!(
            stroke.points[0].x.to_bits(),
            f64::from(101.25_f32 * scale).to_bits()
        );
        assert_eq!(
            stroke.points[0].y.to_bits(),
            f64::from(202.5_f32 * scale).to_bits()
        );
        assert_eq!(stroke.pen_width.to_bits(), (12.0_f32 * scale).to_bits());
        assert_eq!(
            stroke
                .rendering
                .as_ref()
                .unwrap()
                .style
                .pen_size
                .unwrap()
                .to_bits(),
            (8.0_f32 * scale).to_bits()
        );
        assert_eq!(
            stroke.bbox.x_min.to_bits(),
            f64::from(100.0_f32 * scale).to_bits()
        );
        assert_eq!(
            stroke
                .rendering
                .as_ref()
                .unwrap()
                .metadata
                .as_ref()
                .unwrap()
                .bbox
                .x_min
                .to_bits(),
            f64::from(100.0_f32 * scale).to_bits()
        );
        assert_eq!(document.metadata.page_dimensions, Some((1_812, 15_372)));
        let crate::PageObjectContent::Stroke(source_stroke) =
            &layout.pages[0].page.objects[0].content
        else {
            panic!("fixture stroke")
        };
        assert_eq!(source_stroke.points[0].x, 101.25);
        assert_eq!(source_stroke.pen_width, 12.0);
    }

    #[test]
    fn fixed_axis_profile_uses_native_2256_height_rounding_and_rejects_small_axes() {
        assert_eq!(scaled_page_height(4_509, 0.5, 2_256), Some(2_255));
        assert_eq!(scaled_page_height(4_509, 0.5, 2_255), Some(2_254));
        assert_eq!(scaled_page_height(2_147_483_648, 1.0, 2_256), None);
        let (document, layout) = scaled_fixture();
        assert!(matches!(
            normalize_for_fixed_axis(&document, &layout, &[0], 31),
            Err(super::super::PdfError::InvalidPdfPaperFixedAxisSize)
        ));
    }

    #[test]
    fn exact_scale_preserves_f64_source_geometry_and_header_mismatch_is_rejected() {
        let (mut document, mut layout) = scaled_fixture();
        for page in [&mut document.pages[0], &mut layout.pages[0].page] {
            let crate::PageObjectContent::Stroke(stroke) = &mut page.objects[0].content else {
                panic!("fixture stroke")
            };
            stroke.points[0].x = 101.250_000_000_1;
            stroke.bbox.x_min = 100.000_000_000_1;
            stroke
                .rendering
                .as_mut()
                .unwrap()
                .metadata
                .as_deref_mut()
                .unwrap()
                .bbox
                .x_min = stroke.bbox.x_min;
        }
        let normalized = normalize_for_fixed_axis(&document, &layout, &[0], 1_812).unwrap();
        let crate::PageObjectContent::Stroke(stroke) =
            &normalized.layout.pages[0].page.objects[0].content
        else {
            panic!("fixture stroke")
        };
        assert_eq!(
            stroke.points[0].x.to_bits(),
            101.250_000_000_1_f64.to_bits()
        );
        assert_eq!(stroke.bbox.x_min.to_bits(), 100.000_000_000_1_f64.to_bits());

        let mut mismatched = document;
        mismatched.pages[0].height -= 1;
        assert!(matches!(
            normalize_for_fixed_axis(&mismatched, &layout, &[0], 1_080),
            Err(super::super::PdfError::UnsupportedPdfPaper { .. })
        ));
    }

    #[test]
    fn fixed_axis_scale_matches_captured_native_stroke_geometry() {
        let (mut document, mut layout) = scaled_fixture();
        let source_bbox = bbox(
            224.239_273_071_289_06,
            1_788.361_694_335_937_5,
            237.750_137_329_101_56,
            1_789.617_553_710_937_5,
        );
        for page in [&mut document.pages[0], &mut layout.pages[0].page] {
            let crate::PageObjectContent::Stroke(stroke) = &mut page.objects[0].content else {
                panic!("fixture stroke")
            };
            stroke.bbox = source_bbox;
            stroke
                .rendering
                .as_mut()
                .unwrap()
                .metadata
                .as_deref_mut()
                .unwrap()
                .bbox = source_bbox;
            stroke
                .rendering
                .as_mut()
                .unwrap()
                .metadata
                .as_deref_mut()
                .unwrap()
                .uuid = "7496fa0a".to_owned();
            stroke.points[0] = crate::Point {
                x: 227.512_313_842_773_44,
                y: 1_789.198_974_609_375,
            };
            stroke.pen_width = f32::from_bits(0x40ec302c);
            stroke.rendering.as_mut().unwrap().style.pen_size = Some(f32::from_bits(0x40ec302c));
        }
        let normalized = normalize_for_fixed_axis(&document, &layout, &[0], 1_080).unwrap();
        let crate::PageObjectContent::Stroke(stroke) =
            &normalized.layout.pages[0].page.objects[0].content
        else {
            panic!("fixture stroke")
        };
        let actual_bbox = [
            stroke.bbox.x_min as f32,
            stroke.bbox.y_min as f32,
            stroke.bbox.x_max as f32,
            stroke.bbox.y_max as f32,
        ];
        let expected_bbox = [
            f32::from_bits(0x4305a70d),
            f32::from_bits(0x44853d26),
            f32::from_bits(0x430db494),
            f32::from_bits(0x4485551a),
        ];
        assert_eq!(
            actual_bbox.map(f32::to_bits),
            expected_bbox.map(f32::to_bits)
        );
        let metadata_bbox = stroke
            .rendering
            .as_ref()
            .unwrap()
            .metadata
            .as_ref()
            .unwrap()
            .bbox;
        assert_eq!(
            [
                metadata_bbox.x_min as f32,
                metadata_bbox.y_min as f32,
                metadata_bbox.x_max as f32,
                metadata_bbox.y_max as f32,
            ]
            .map(f32::to_bits),
            expected_bbox.map(f32::to_bits)
        );
        assert_eq!(
            (stroke.points[0].x as f32).to_bits(),
            f32::from_bits(0x43079a76).to_bits()
        );
        assert_eq!(
            (stroke.points[0].y as f32).to_bits(),
            f32::from_bits(0x44854d1e).to_bits()
        );
        assert_eq!(
            stroke.pen_width.to_bits(),
            f32::from_bits(0x408cc63f).to_bits()
        );
        assert_eq!(
            stroke
                .rendering
                .as_ref()
                .unwrap()
                .style
                .pen_size
                .unwrap()
                .to_bits(),
            f32::from_bits(0x408cc63f).to_bits()
        );
    }

    #[test]
    fn identity_scale_keeps_large_saved_height_exact() {
        let (mut document, mut layout) = scaled_fixture();
        for page in [&mut document.pages[0], &mut layout.pages[0].page] {
            page.width = 1_812;
            page.height = 16_777_217;
        }
        document.metadata.page_dimensions = Some((1_812, 16_777_217));
        let normalized = normalize_for_fixed_axis(&document, &layout, &[0], 1_812).unwrap();
        assert_eq!(
            normalized.document.metadata.page_dimensions,
            Some((1_812, 16_777_217))
        );
    }

    #[test]
    #[ignore = "offline retained native/body geometry comparison; requires pinned artifact paths"]
    fn retained_quiz_1080_prepared_body_geometry_diagnostic() {
        use std::{env, fs, io::Write, path::PathBuf};

        const ARCHIVE_SHA256: &str =
            "38fd0ef0729d3a113e1c14bcc10557dcc263e5a3582fd80a3cf99c8c2c4ad40a";
        const GEOMETRY_RECEIPT_SHA256: &str =
            "3e3042bbf9cdbd203f214c413d68b927a3c21aa60311006cf5071569f214f03a";
        let archive_path = PathBuf::from(
            env::var_os("SDOCX_BODY_GEOMETRY_ARCHIVE")
                .expect("set SDOCX_BODY_GEOMETRY_ARCHIVE to the pinned quiz.sdocx"),
        );
        let receipt_path = PathBuf::from(
            env::var_os("SDOCX_BODY_GEOMETRY_RECEIPT")
                .expect("set SDOCX_BODY_GEOMETRY_RECEIPT to the pinned native geometry receipt"),
        );
        let output_dir = PathBuf::from(
            env::var_os("SDOCX_BODY_GEOMETRY_OUTPUT_DIR")
                .expect("set SDOCX_BODY_GEOMETRY_OUTPUT_DIR to a fresh output directory"),
        );

        fn sha256(bytes: &[u8]) -> String {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(bytes))
        }
        fn rect_intersects_or_contains(query: [f32; 4], line: [f32; 4]) -> bool {
            let contains = query[0] <= line[0]
                && query[1] <= line[1]
                && query[2] >= line[2]
                && query[3] >= line[3];
            let intersects = query[0] < query[2]
                && query[1] < query[3]
                && query[0] < line[2]
                && query[2] > line[0]
                && query[1] < line[3]
                && query[3] > line[1];
            contains || intersects
        }

        let archive_bytes = fs::read(&archive_path).expect("read pinned quiz archive");
        assert_eq!(
            sha256(&archive_bytes),
            ARCHIVE_SHA256,
            "archive pin mismatch"
        );
        let receipt_bytes = fs::read(&receipt_path).expect("read pinned native geometry receipt");
        assert_eq!(
            sha256(&receipt_bytes),
            GEOMETRY_RECEIPT_SHA256,
            "native geometry receipt pin mismatch"
        );
        let receipt: serde_json::Value =
            serde_json::from_slice(&receipt_bytes).expect("native geometry receipt JSON");
        assert_eq!(
            receipt["source_event_sha256"]["1080"].as_str(),
            Some("59db2ca7194960b3a51289aca7cfb3b56e67ae86ce1600a877573da2d220486d")
        );
        assert_eq!(
            receipt["source_event_sha256"]["1812"].as_str(),
            Some("610949b314f910a710cb7a19f9e4762262a1497d090dec89049de8c25e24ff36")
        );
        assert_eq!(receipt["line_rect_pairs"].as_array().unwrap().len(), 116);

        let document = crate::parse_bytes(&archive_bytes).expect("parse pinned quiz archive");
        assert_eq!(document.metadata.page_mode, Some(1));
        assert_eq!(
            document.metadata.flow_dimensions.map(|dims| dims.0),
            Some(1_080)
        );
        let layout = crate::layout_document(&document);
        let normalized = normalize_for_fixed_axis(&document, &layout, &[0], 1_080)
            .expect("apply the public PDF route's explicit 1080 fixed-axis normalization");
        assert_eq!(normalized.profile.source_dimensions, [1_812, 15_372]);
        assert_eq!(
            normalized.profile.scale.to_bits(),
            (1_080.0_f32 / 1_812.0_f32).to_bits()
        );
        assert_eq!(normalized.document.pages[0].width, 1_080);
        assert_eq!(normalized.layout.pages[0].page.width, 1_080);
        assert_eq!(
            normalized
                .document
                .metadata
                .flow_dimensions
                .map(|dims| dims.0),
            Some(1_080)
        );

        let fonts = crate::fonts::FontBook::default();
        let options = crate::render::RenderOptions::default();
        let mut cache = crate::render::DocumentTextCache::default();
        let snapshot = cache
            .continuous_body_query_snapshot(
                &normalized.document,
                &normalized.layout.pages[0],
                &options,
                &fonts,
            )
            .expect("1080 flow-domain body query snapshot");
        let mut rust_lines = Vec::new();
        for index in 0.. {
            let Some(rect) = snapshot.line_rect(index) else {
                break;
            };
            rust_lines.push(rect);
        }
        assert!(
            !rust_lines.is_empty(),
            "prepared body has no measured lines"
        );

        let native_lines = receipt["line_rect_pairs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                row["1080_rect_f32"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_f64().unwrap() as f32)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(native_lines.len(), 116);
        let paired_count = rust_lines.len().min(native_lines.len());
        let mut paired_rows = Vec::with_capacity(paired_count);
        let mut exact_components = [0_usize; 4];
        let mut max_abs_delta = [0.0_f64; 4];
        for index in 0..paired_count {
            assert_eq!(native_lines[index].len(), 4);
            let native = [
                native_lines[index][0],
                native_lines[index][1],
                native_lines[index][2],
                native_lines[index][3],
            ];
            let rust = rust_lines[index];
            for axis in 0..4 {
                if rust[axis].to_bits() == native[axis].to_bits() {
                    exact_components[axis] += 1;
                }
                max_abs_delta[axis] = max_abs_delta[axis]
                    .max((f64::from(rust[axis]) - f64::from(native[axis])).abs());
            }
            paired_rows.push(serde_json::json!({
                "line": index,
                "rust_prepared_rect_f32": rust,
                "native_1080_rect_f32": native,
                "rust_minus_native_f32": [
                    rust[0] - native[0], rust[1] - native[1],
                    rust[2] - native[2], rust[3] - native[3]
                ]
            }));
        }

        let native_queries = receipt["queries"]["1080"]["rows"].as_array().unwrap();
        let mut query_rows = Vec::with_capacity(native_queries.len());
        for row in native_queries {
            let rect = row["query_rect_f32"].as_array().unwrap();
            let query = [
                rect[0].as_f64().unwrap() as f32,
                rect[1].as_f64().unwrap() as f32,
                rect[2].as_f64().unwrap() as f32,
                rect[3].as_f64().unwrap() as f32,
            ];
            let observed = snapshot.query(query);
            query_rows.push(serde_json::json!({
                "query_rect_f32": query,
                "native_visible": row["native_visible"],
                "rust_prepared_visible": observed.visible,
                "rust_line_index": observed.line_index,
                "rust_line_count": observed.line_count,
                "matches": observed.visible == row["native_visible"].as_bool().unwrap()
            }));
        }

        let scale = 1_812.0_f32 / 1_080.0_f32;
        let rust_image_line_indices = rust_lines
            .iter()
            .enumerate()
            .filter_map(|(index, rect)| snapshot.query(*rect).visible.then_some(index))
            .collect::<Vec<_>>();
        let mapped_lines = rust_lines
            .iter()
            .map(|line| (*line).map(|coordinate| coordinate * scale))
            .collect::<Vec<_>>();
        let native_1812_queries = receipt["queries"]["1812"]["rows"].as_array().unwrap();
        let mut projected_1812_query_rows = Vec::with_capacity(native_1812_queries.len());
        for row in native_1812_queries {
            let rect = row["query_rect_f32"].as_array().unwrap();
            let query = [
                rect[0].as_f64().unwrap() as f32,
                rect[1].as_f64().unwrap() as f32,
                rect[2].as_f64().unwrap() as f32,
                rect[3].as_f64().unwrap() as f32,
            ];
            let predicted = rust_image_line_indices.iter().any(|&index| {
                mapped_lines
                    .get(index)
                    .is_some_and(|line| rect_intersects_or_contains(query, *line))
            });
            projected_1812_query_rows.push(serde_json::json!({
                "query_rect_f32": query,
                "native_visible": row["native_visible"],
                "rust_1080_plan_projected_visible": predicted,
                "matches": predicted == row["native_visible"].as_bool().unwrap()
            }));
        }

        let output = serde_json::json!({
            "schema": "sdocx.prepared_body_native_geometry_diagnostic.v1",
            "diagnostic_only": true,
            "parity_claim": false,
            "native_1080_geometry_and_queries_asserted": true,
            "archive_sha256": sha256(&archive_bytes),
            "native_receipt_sha256": sha256(&receipt_bytes),
            "explicit_fixed_axis_size": 1080,
            "source_page_dimensions": normalized.profile.source_dimensions,
            "public_fixed_axis_scale_f32": normalized.profile.scale,
            "public_fixed_axis_scale_bits": format!("0x{:08x}", normalized.profile.scale.to_bits()),
            "normalized_page_dimensions": [normalized.document.pages[0].width, normalized.document.pages[0].height],
            "flow_dimensions": normalized.document.metadata.flow_dimensions,
            "rust_prepared_line_count": rust_lines.len(),
            "native_retained_line_count": native_lines.len(),
            "paired_line_count": paired_count,
            "exact_component_counts_left_top_right_bottom": exact_components,
            "max_abs_delta_f32_left_top_right_bottom": max_abs_delta,
            "line_rect_pairs": paired_rows,
            "native_1080_query_count": query_rows.len(),
            "native_1080_query_rows": query_rows,
            "native_1812_projected_query_count": projected_1812_query_rows.len(),
            "native_1812_projected_query_rows": projected_1812_query_rows,
            "rust_image_owner_line_indices": rust_image_line_indices,
            "projection_scale_f32": scale,
            "projection_scale_bits": format!("0x{:08x}", scale.to_bits()),
            "projection_is_a_diagnostic_from_retained_native_axis_ratio": true,
            "limitations": [
                "The 116 native 1080 line rectangles and native 1080 visibility queries are asserted; this does not establish rendered-page appearance.",
                "The 1812 query projection is an offline hypothesis from the two retained native profiles, not an implemented rendering transform or PDF appearance claim."
            ]
        });
        fs::create_dir_all(&output_dir).expect("create diagnostic output directory");
        let output_path = output_dir.join("prepared-body-geometry.json");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output_path)
            .expect("diagnostic output must be fresh");
        file.write_all(&serde_json::to_vec_pretty(&output).unwrap())
            .unwrap();
        file.write_all(b"\n").unwrap();
        assert_eq!(rust_lines.len(), 116, "native 1080 prepared line count");
        assert_eq!(exact_components, [116; 4], "native 1080 line rectangles");
        assert!(
            output["native_1080_query_rows"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["matches"] == true)
        );
    }
}
