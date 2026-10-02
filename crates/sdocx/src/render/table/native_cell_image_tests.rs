use super::*;
use crate::{
    ObjectSpanLayoutOption, ObjectType, PlacedImage, RichTextObjectContent, RichTextObjectSpan,
};

#[derive(Deserialize)]
struct ImageCapture {
    cases: Vec<ImageCase>,
}

#[derive(Deserialize)]
struct ImageCase {
    #[serde(flatten)]
    geometry: Case,
    source_setup: ImageSource,
    writer: ImageWriter,
}

#[derive(Deserialize)]
struct ImageSource {
    object_type: u32,
    source_bounds_bits: Option<[u32; 4]>,
    layout_option: u32,
    layout_constraint: u32,
    source_resize_bounds_bits: Option<[u32; 4]>,
    public_cell_append_accepted: bool,
}

#[derive(Deserialize)]
struct ImageWriter {
    after_actual_size_notification: Option<State>,
}

fn image_span(input: &ImageSource) -> RichTextObjectSpan {
    RichTextObjectSpan {
        object_type: ObjectType::Image,
        object_data: Vec::new(),
        content: Some(RichTextObjectContent::Image(Box::new(PlacedImage {
            bbox: rect(input.source_bounds_bits.unwrap()),
            rotation_degrees: None,
            media_id: None,
            media_index: None,
            crop_rect: None,
            original_bbox: None,
            border_media_id: None,
            original_media_id: None,
        }))),
        text_index_utf16: 2,
        layout_option: ObjectSpanLayoutOption::from(input.layout_option),
        layout_constraint: ObjectSpanLayoutConstraint::from(input.layout_constraint),
    }
}

fn assert_image(plan: &PreparedTable, state: &State, label: &str) {
    let native = state.cells[0]
        .entries
        .iter()
        .find(|entry| entry.kind == 5)
        .unwrap();
    let cell = &plan.rows[0].cells[0];
    let mut native_text = state.cells[0]
        .entries
        .iter()
        .filter(|entry| entry.kind == 0);
    for line in &cell.layout.lines {
        let advance = line
            .line
            .advance_for_paint(false)
            .unwrap_or(line.line.advance);
        let alignment =
            crate::render::text::line_alignment_offset(advance, line.width, line.alignment);
        for (index, _) in line.line.placements.iter().enumerate() {
            let position = line.line.text_position(index, false).unwrap();
            let left = ((line.x - cell.frame.x_min) + alignment + position.x) as f32;
            let native = native_text.next().unwrap();
            assert_eq!(left.to_bits(), native.position_bits[0], "{label} text x");
        }
    }
    assert!(native_text.next().is_none());
    let (line, placed) = cell
        .layout
        .lines
        .iter()
        .find_map(|line| line.line.objects.first().map(|placed| (line, placed)))
        .unwrap();
    let baseline = (line.baseline - cell.frame.y_min) as f32;
    let advance = line
        .line
        .advance_for_paint(false)
        .unwrap_or(line.line.advance);
    let alignment = crate::render::text::line_alignment_offset(advance, line.width, line.alignment);
    let position = line.line.object_position(0, false).unwrap();
    let left = ((line.x - cell.frame.x_min) + alignment + position.x) as f32;
    let top = baseline - placed.object.height as f32;
    assert_eq!(
        left.to_bits(),
        native.position_bits[0],
        "{label} image x: width {} advance {advance} alignment {alignment} position {}",
        line.width,
        position.x
    );
    assert_eq!(
        baseline.to_bits(),
        native.position_bits[1],
        "{label} image y"
    );
    assert_eq!(
        (placed.object.height as f32).to_bits(),
        native.font_height_bits,
        "{label} image height"
    );
    assert_eq!(
        [left, top, left + placed.object.width() as f32, baseline].map(f32::to_bits),
        native.ink_rect_bits,
        "{label} image bounds"
    );
}

#[test]
fn native_cell_images_match_shared_cold_warm_and_fresh_resize_geometry() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-live-cell-images.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "88351ebe30102d23f31ec96a12a757d632068d00e5fba5d8b90d700e3d5bc00f"
    );
    let capture: ImageCapture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 8);
    let profile = constructor_profile();
    let fonts = crate::fonts::FontBook::default();
    let renderer = TextRenderer::new(
        super::super::super::text::TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
        },
        &fonts,
    );
    let theme = RenderTheme::for_canvas(false);
    let mut images = 0;
    for case in capture.cases {
        if !case.source_setup.public_cell_append_accepted {
            assert!(matches!(case.source_setup.object_type, 22 | 23));
            continue;
        }
        images += 1;
        assert_eq!(case.source_setup.object_type, 3);
        let mut table = source_table(&case.geometry, &profile);
        table.rows[0].cells[0].content.text.push('\u{fffc}');
        table.rows[0].cells[0]
            .content
            .object_spans
            .push(image_span(&case.source_setup));
        assert!(supports_prepared_content(&table));
        let source = table.clone();
        let mut plan = cold_plan(&case.geometry, &table, theme, &renderer);
        assert_geometry(&plan, &table, &case.geometry.states[0], &case.geometry.name);
        assert_image(&plan, &case.geometry.states[0], &case.geometry.name);
        pagination::warm(&mut plan, &table, theme, &renderer).unwrap();
        plan.update_geometry(&table, &renderer).unwrap();
        assert_geometry(&plan, &table, &case.geometry.states[1], &case.geometry.name);
        assert_image(&plan, &case.geometry.states[1], &case.geometry.name);
        let drawing = prepare_table_drawing(
            &table,
            ObjectSpanLayoutConstraint::Normal,
            [0.0; 2],
            theme,
            &renderer,
        )
        .unwrap()
        .unwrap();
        assert!(
            drawing.rows[0].cells[0]
                .layout
                .lines
                .iter()
                .any(|line| !line.line.objects.is_empty())
        );
        if let Some(bounds) = case.source_setup.source_resize_bounds_bits {
            let mut resized = table.clone();
            let Some(RichTextObjectContent::Image(image)) =
                &mut resized.rows[0].cells[0].content.object_spans[0].content
            else {
                panic!()
            };
            image.bbox = rect(bounds);
            let mut updated = cold_plan(&case.geometry, &resized, theme, &renderer);
            pagination::warm(&mut updated, &resized, theme, &renderer).unwrap();
            updated.update_geometry(&resized, &renderer).unwrap();
            let native = case.writer.after_actual_size_notification.as_ref().unwrap();
            assert_geometry(&updated, &resized, native, &case.geometry.name);
            assert_image(&updated, native, &case.geometry.name);
        }
        assert_eq!(table, source);
    }
    assert_eq!(images, 6);
}

#[test]
fn malformed_cell_images_preserve_the_preparation_boundary() {
    #[derive(Debug)]
    enum InvalidImage {
        MissingContent,
        WrongType,
        WrongAnchor,
        Rotated,
        EmptyBounds,
        OverflowBounds,
        NonfiniteBounds,
    }
    let capture: ImageCapture = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-live-cell-images.json"
    )))
    .unwrap();
    let case = capture
        .cases
        .iter()
        .find(|case| case.source_setup.public_cell_append_accepted)
        .unwrap();
    let mut source = source_table(&case.geometry, &constructor_profile());
    source.rows[0].cells[0].content.text.push('\u{fffc}');
    source.rows[0].cells[0]
        .content
        .object_spans
        .push(image_span(&case.source_setup));
    assert!(supports_prepared_content(&source));
    let fonts = crate::fonts::FontBook::default();
    let renderer = TextRenderer::new(Default::default(), &fonts);
    let theme = RenderTheme::for_canvas(false);
    for invalid in [
        InvalidImage::MissingContent,
        InvalidImage::WrongType,
        InvalidImage::WrongAnchor,
        InvalidImage::Rotated,
        InvalidImage::EmptyBounds,
        InvalidImage::OverflowBounds,
        InvalidImage::NonfiniteBounds,
    ] {
        let mut table = source.clone();
        let span = &mut table.rows[0].cells[0].content.object_spans[0];
        match invalid {
            InvalidImage::MissingContent => span.content = None,
            InvalidImage::WrongType => span.object_type = ObjectType::CodeBlock,
            InvalidImage::WrongAnchor => span.text_index_utf16 = 0,
            _ => {
                let Some(RichTextObjectContent::Image(image)) = &mut span.content else {
                    unreachable!()
                };
                match invalid {
                    InvalidImage::Rotated => image.rotation_degrees = Some(15.0),
                    InvalidImage::EmptyBounds => image.bbox.x_max = image.bbox.x_min,
                    InvalidImage::OverflowBounds => image.bbox.x_max = f64::MAX,
                    InvalidImage::NonfiniteBounds => image.bbox.y_min = f64::NAN,
                    _ => unreachable!(),
                }
            }
        }
        assert!(
            !supports_prepared_content(&table),
            "invalid image {invalid:?}"
        );
        assert!(
            prepare_table_drawing(
                &table,
                ObjectSpanLayoutConstraint::Normal,
                [0.0; 2],
                theme,
                &renderer,
            )
            .is_none(),
            "invalid image {invalid:?}",
        );
    }
    assert_eq!(
        source.rows[0].cells[0].content.object_spans[0].text_index_utf16,
        2
    );
    assert!(supports_prepared_content(&source));
}
