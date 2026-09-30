use std::collections::BTreeMap;
use std::ops::Range;

use super::TextSettings;
use crate::text_index::TextIndex;
use crate::{
    BoundingBox, ObjectSpanLayoutOption, RichTextBox, RichTextObjectContent, RichTextObjectSpan,
};

pub(in crate::render) struct TextObjectIndex<'a> {
    objects: Vec<TextObject<'a>>,
    issues: Vec<ObjectDiagnostic>,
}

pub(in crate::render) struct TextObject<'a> {
    pub source: Range<usize>,
    pub span_index: usize,
    pub span: &'a RichTextObjectSpan,
    pub bounds: BoundingBox,
}

#[derive(Debug, Clone)]
pub(in crate::render) struct MeasuredObject {
    pub source: Range<usize>,
    pub span_index: usize,
    pub bounds: BoundingBox,
    pub height: f64,
    pub inline: bool,
    pub top_margin: f64,
    pub bottom_margin: f64,
}

impl TextObject<'_> {
    pub fn measured(&self, settings: TextSettings) -> MeasuredObject {
        let margin = match self.span.layout_option {
            ObjectSpanLayoutOption::BlockWithSmallMargin => settings.pixels(10.0),
            ObjectSpanLayoutOption::BlockWithMediumMargin => settings.pixels(20.0),
            _ => 0.0,
        };
        MeasuredObject {
            source: self.source.clone(),
            span_index: self.span_index,
            bounds: self.bounds,
            height: self.bounds.y_max - self.bounds.y_min,
            inline: self.span.layout_option == ObjectSpanLayoutOption::Inline,
            top_margin: margin,
            bottom_margin: margin,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ObjectDiagnosticKind {
    InvalidAnchor,
    NonReplacementAnchor,
    UnsupportedContent,
    InvalidBounds,
    MixedParagraphLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ObjectDiagnostic {
    pub anchor_utf16: i32,
    pub kind: ObjectDiagnosticKind,
}

impl<'a> TextObjectIndex<'a> {
    pub fn new(text: &'a RichTextBox, index: &TextIndex<'_>) -> Self {
        let mut selected = BTreeMap::new();
        let mut issues = Vec::new();
        for (span_index, span) in text.object_spans.iter().enumerate() {
            let source = u32::try_from(span.text_index_utf16)
                .ok()
                .and_then(|anchor| index.utf16_to_char(anchor))
                .filter(|&anchor| anchor < index.len());
            let Some(source) = source else {
                issues.push(ObjectDiagnostic {
                    anchor_utf16: span.text_index_utf16,
                    kind: ObjectDiagnosticKind::InvalidAnchor,
                });
                continue;
            };
            if index.slice(source..source + 1) != Some("\u{fffc}") {
                issues.push(ObjectDiagnostic {
                    anchor_utf16: span.text_index_utf16,
                    kind: ObjectDiagnosticKind::NonReplacementAnchor,
                });
                continue;
            }
            let validated = object_bounds(span)
                .and_then(|bounds| {
                    valid_bounds(bounds)
                        .then_some(bounds)
                        .ok_or(ObjectDiagnosticKind::InvalidBounds)
                })
                .map(|bounds| TextObject {
                    source: source..source + 1,
                    span_index,
                    span,
                    bounds,
                });
            let object = match validated {
                Ok(object) => Some(object),
                Err(kind) => {
                    issues.push(ObjectDiagnostic {
                        anchor_utf16: span.text_index_utf16,
                        kind,
                    });
                    None
                }
            };
            selected.insert(source, object);
        }
        Self {
            objects: selected.into_values().flatten().collect(),
            issues,
        }
    }

    pub fn in_range(&self, source: Range<usize>) -> &[TextObject<'a>] {
        let start = self
            .objects
            .partition_point(|object| object.source.start < source.start);
        let end = self
            .objects
            .partition_point(|object| object.source.start < source.end);
        &self.objects[start..end.max(start)]
    }

    pub fn issues(&self) -> &[ObjectDiagnostic] {
        &self.issues
    }
}

fn object_bounds(span: &RichTextObjectSpan) -> Result<BoundingBox, ObjectDiagnosticKind> {
    match span.content.as_ref() {
        Some(RichTextObjectContent::Image(image)) => {
            if !valid_bounds(image.bbox) {
                return Err(ObjectDiagnosticKind::InvalidBounds);
            }
            Ok(crate::render::image_drawn_bbox(image))
        }
        Some(RichTextObjectContent::Table(table)) => {
            if !valid_bounds(table.bbox) {
                return Err(ObjectDiagnosticKind::InvalidBounds);
            }
            Ok(crate::render::table::table_drawn_bounds(table))
        }
        Some(RichTextObjectContent::CodeBlock(code)) => Ok(code.bbox),
        None => Err(ObjectDiagnosticKind::UnsupportedContent),
    }
}

fn valid_bounds(bounds: BoundingBox) -> bool {
    let width = bounds.x_max - bounds.x_min;
    let height = bounds.y_max - bounds.y_min;
    [
        bounds.x_min,
        bounds.y_min,
        bounds.x_max,
        bounds.y_max,
        width,
        height,
    ]
    .into_iter()
    .all(|value| (value as f32).is_finite())
        && width > 0.0
        && height > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType, PlacedImage};

    fn text(value: &str, object_spans: Vec<RichTextObjectSpan>) -> RichTextBox {
        RichTextBox {
            text_area_type: None,
            bbox: BoundingBox::default(),
            rotation_degrees: None,
            text: value.into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: None,
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: Vec::new(),
            object_spans,
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        }
    }

    fn image(anchor: i32, width: f64) -> RichTextObjectSpan {
        RichTextObjectSpan {
            object_type: ObjectType::Image,
            object_data: Vec::new(),
            content: Some(RichTextObjectContent::Image(Box::new(PlacedImage {
                bbox: BoundingBox {
                    x_min: -10.0,
                    y_min: -20.0,
                    x_max: -10.0 + width,
                    y_max: 40.0,
                },
                rotation_degrees: None,
                media_id: None,
                media_index: None,
                crop_rect: None,
                original_bbox: None,
                border_media_id: None,
                original_media_id: None,
            }))),
            text_index_utf16: anchor,
            layout_option: ObjectSpanLayoutOption::Inline,
            layout_constraint: ObjectSpanLayoutConstraint::Normal,
        }
    }

    #[test]
    fn anchors_resolve_utf16_to_scalars_without_consuming_adjacent_unicode() {
        let text = text("A😀\u{fffc}e\u{301}", vec![image(3, 30.0)]);
        let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
        let objects = index.in_range(0..5);
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].source, 2..3);
        assert!(std::ptr::eq(objects[0].span, &text.object_spans[0]));
        let bounds = object_bounds(objects[0].span).unwrap();
        assert_eq!(bounds.x_max - bounds.x_min, 30.0);
        assert!(index.issues().is_empty());
        assert!(index.in_range(0..2).is_empty());
        assert_eq!(index.in_range(2..3).len(), 1);
        assert!(index.in_range(3..5).is_empty());
        assert!(index.in_range(2..2).is_empty());
        assert!(
            index
                .in_range(std::ops::Range { start: 4, end: 1 })
                .is_empty()
        );
    }

    #[test]
    fn invalid_and_nonreplacement_anchors_are_reported_without_objects() {
        let text = text(
            "A😀\u{fffc}B",
            [-1, 2, 5, 99, 0, 1, 4]
                .into_iter()
                .map(|anchor| image(anchor, 30.0))
                .collect(),
        );
        let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
        assert!(index.in_range(0..4).is_empty());
        assert_eq!(
            index
                .issues()
                .iter()
                .map(|issue| (issue.anchor_utf16, issue.kind))
                .collect::<Vec<_>>(),
            vec![
                (-1, ObjectDiagnosticKind::InvalidAnchor),
                (2, ObjectDiagnosticKind::InvalidAnchor),
                (5, ObjectDiagnosticKind::InvalidAnchor),
                (99, ObjectDiagnosticKind::InvalidAnchor),
                (0, ObjectDiagnosticKind::NonReplacementAnchor),
                (1, ObjectDiagnosticKind::NonReplacementAnchor),
                (4, ObjectDiagnosticKind::NonReplacementAnchor),
            ]
        );
    }

    #[test]
    fn source_order_and_last_stored_duplicate_determine_selected_geometry() {
        let text = text(
            "\u{fffc}x\u{fffc}",
            vec![image(2, 20.0), image(0, 10.0), image(2, 50.0)],
        );
        let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
        let objects = index.in_range(0..3);
        assert_eq!(
            objects
                .iter()
                .map(|object| object.source.clone())
                .collect::<Vec<_>>(),
            vec![0..1, 2..3]
        );
        assert!(std::ptr::eq(objects[0].span, &text.object_spans[1]));
        assert!(std::ptr::eq(objects[1].span, &text.object_spans[2]));
        assert_eq!(objects[0].span_index, 1);
        assert_eq!(objects[1].span_index, 2);
        assert_eq!(objects[1].bounds, object_bounds(objects[1].span).unwrap());
        let bounds = object_bounds(objects[1].span).unwrap();
        assert_eq!(bounds.x_max - bounds.x_min, 50.0);
    }

    #[test]
    fn measured_objects_keep_source_identity_and_only_explicit_inline_flag() {
        for (option, inline) in [
            (ObjectSpanLayoutOption::Inline, true),
            (ObjectSpanLayoutOption::Block, false),
            (ObjectSpanLayoutOption::BlockWithSmallMargin, false),
            (ObjectSpanLayoutOption::BlockWithMediumMargin, false),
            (ObjectSpanLayoutOption::Other(99), false),
        ] {
            let mut span = image(3, 30.0);
            span.layout_option = option;
            let text = text("A😀\u{fffc}B", vec![span]);
            let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
            let object = &index.in_range(0..4)[0];
            let measured = object.measured(TextSettings {
                scale: 1.0,
                font_size_delta: 0.0,
                ..Default::default()
            });
            assert_eq!(measured.source, 2..3);
            assert_eq!(measured.span_index, 0);
            assert_eq!(measured.bounds, object.bounds);
            assert_eq!(measured.height, object.bounds.y_max - object.bounds.y_min);
            assert_eq!(measured.inline, inline);
        }
    }

    #[test]
    fn object_margins_use_document_density_without_font_delta_or_rescaling_bounds() {
        for (option, expected) in [
            (ObjectSpanLayoutOption::Block, 0.0),
            (ObjectSpanLayoutOption::Inline, 0.0),
            (ObjectSpanLayoutOption::BlockWithSmallMargin, 30.0),
            (ObjectSpanLayoutOption::BlockWithMediumMargin, 60.0),
            (ObjectSpanLayoutOption::Other(99), 0.0),
        ] {
            let mut span = image(0, 30.0);
            span.layout_option = option;
            let text = text("\u{fffc}", vec![span]);
            let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
            let object = &index.in_range(0..1)[0];
            for font_size_delta in [-100.0, 0.0, 500.0] {
                let measured = object.measured(TextSettings {
                    scale: 3.0,
                    font_size_delta,
                    ..Default::default()
                });
                assert_eq!(measured.top_margin, expected);
                assert_eq!(measured.bottom_margin, expected);
                assert_eq!(measured.bounds, object.bounds);
                assert_eq!(measured.bounds.x_max - measured.bounds.x_min, 30.0);
                assert_eq!(measured.bounds.y_max - measured.bounds.y_min, 60.0);
                assert_eq!(measured.height, 60.0);
            }
        }
    }

    #[test]
    fn rotated_image_measurement_uses_drawn_bounds_around_the_stored_center() {
        let mut span = image(0, 20.0);
        if let Some(RichTextObjectContent::Image(image)) = span.content.as_mut() {
            image.rotation_degrees = Some(90.0);
        }
        let text = text("\u{fffc}", vec![span]);
        let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
        let measured = index.in_range(0..1)[0].measured(TextSettings {
            scale: 3.0,
            font_size_delta: 0.0,
            ..Default::default()
        });
        assert!((measured.bounds.x_max - measured.bounds.x_min - 60.0).abs() < 1e-10);
        assert!((measured.bounds.y_max - measured.bounds.y_min - 20.0).abs() < 1e-10);
        assert!((measured.height - 20.0).abs() < 1e-10);
        assert!(((measured.bounds.x_min + measured.bounds.x_max) / 2.0).abs() < 1e-10);
        assert!(((measured.bounds.y_min + measured.bounds.y_max) / 2.0 - 10.0).abs() < 1e-10);
    }

    #[test]
    fn image_rotation_cannot_promote_invalid_stored_bounds_or_angles() {
        for (width, angle) in [
            (-20.0, 90.0),
            (0.0, 45.0),
            (20.0, f64::NAN),
            (20.0, f64::INFINITY),
            (20.0, f64::NEG_INFINITY),
        ] {
            let mut span = image(0, width);
            if let Some(RichTextObjectContent::Image(image)) = span.content.as_mut() {
                image.rotation_degrees = Some(angle);
            }
            let text = text("\u{fffc}", vec![span]);
            let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
            assert!(index.in_range(0..1).is_empty());
            assert_eq!(index.issues()[0].kind, ObjectDiagnosticKind::InvalidBounds);
        }
    }

    #[test]
    fn native_float_overflow_rejects_finite_coordinates_and_extents() {
        let maximum = f64::from(f32::MAX);
        for bounds in [
            BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 1e308,
                y_max: 60.0,
            },
            BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 30.0,
                y_max: 1e308,
            },
            BoundingBox {
                x_min: 1e100,
                y_min: 0.0,
                x_max: 2e100,
                y_max: 60.0,
            },
            BoundingBox {
                x_min: -maximum,
                y_min: 0.0,
                x_max: maximum,
                y_max: 60.0,
            },
            BoundingBox {
                x_min: 0.0,
                y_min: -maximum,
                x_max: 30.0,
                y_max: maximum,
            },
        ] {
            assert!(
                [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max]
                    .into_iter()
                    .all(f64::is_finite)
            );
            let mut span = image(0, 30.0);
            if let Some(RichTextObjectContent::Image(image)) = span.content.as_mut() {
                image.bbox = bounds;
            }
            let text = text(
                "A\u{fffc}B",
                vec![RichTextObjectSpan {
                    text_index_utf16: 1,
                    ..span
                }],
            );
            let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
            assert!(index.in_range(0..3).is_empty());
            assert_eq!(
                index.issues(),
                &[ObjectDiagnostic {
                    anchor_utf16: 1,
                    kind: ObjectDiagnosticKind::InvalidBounds,
                }]
            );
            assert_eq!(text.text, "A\u{fffc}B");
        }
    }

    #[test]
    fn rotated_float_overflow_is_rejected_after_validating_the_original_box() {
        let maximum = f64::from(f32::MAX);
        let mut span = image(0, 30.0);
        if let Some(RichTextObjectContent::Image(image)) = span.content.as_mut() {
            image.bbox = BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: maximum,
                y_max: maximum,
            };
            assert!(valid_bounds(image.bbox));
            image.rotation_degrees = Some(45.0);
        }
        let text = text("\u{fffc}", vec![span]);
        let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
        assert!(index.in_range(0..1).is_empty());
        assert_eq!(index.issues()[0].kind, ObjectDiagnosticKind::InvalidBounds);
    }

    #[test]
    fn float_representability_validation_preserves_original_double_precision() {
        let bounds = BoundingBox {
            x_min: 0.1234567890123456,
            y_min: 0.9876543210987654,
            x_max: 100.98765432109876,
            y_max: 60.123456789012344,
        };
        let mut code = image(0, 30.0);
        code.object_type = ObjectType::CodeBlock;
        code.content = Some(RichTextObjectContent::CodeBlock(Box::new(
            crate::RichTextCodeBlock {
                bbox: bounds,
                rotation_degrees: None,
                title: None,
                body: None,
            },
        )));
        let text = text("\u{fffc}", vec![code]);
        let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
        let measured = index.in_range(0..1)[0].measured(TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
        });
        assert_eq!(measured.bounds, bounds);
        assert_ne!(measured.bounds.x_min, f64::from(bounds.x_min as f32));
        assert_ne!(measured.bounds.y_max, f64::from(bounds.y_max as f32));
        assert!(index.issues().is_empty());
    }

    #[test]
    fn unsupported_last_duplicate_does_not_resurrect_an_earlier_object() {
        let mut unsupported = image(0, 20.0);
        unsupported.content = None;
        let text = text("\u{fffc}", vec![image(0, 10.0), unsupported]);
        let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
        assert!(index.in_range(0..1).is_empty());
        assert_eq!(
            index.issues()[0].kind,
            ObjectDiagnosticKind::UnsupportedContent
        );
    }

    #[test]
    fn code_dimensions_do_not_depend_on_nested_text_content() {
        let bounds = BoundingBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 100.0,
            y_max: 200.0,
        };
        let mut code = image(0, 20.0);
        code.object_type = ObjectType::CodeBlock;
        code.content = Some(RichTextObjectContent::CodeBlock(Box::new(
            crate::RichTextCodeBlock {
                bbox: bounds,
                rotation_degrees: None,
                title: None,
                body: None,
            },
        )));
        let text = text("\u{fffc}", vec![code]);
        let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
        assert_eq!(object_bounds(index.in_range(0..1)[0].span).unwrap(), bounds);
        assert!(index.issues().is_empty());
    }

    fn table_object(anchor: i32, bounds: BoundingBox) -> RichTextObjectSpan {
        let mut table = image(anchor, 20.0);
        table.object_type = ObjectType::Table;
        table.content = Some(RichTextObjectContent::Table(Box::new(
            crate::RichTextTable {
                style: crate::TableStyle {
                    heading_column_enabled: false,
                    heading_row_enabled: false,
                    max_height_enabled: false,
                    vertical_cell_padding: None,
                    horizontal_cell_padding: None,
                    content_bbox: Some(BoundingBox::default()),
                    border: None,
                    auto_fit: None,
                    min_column_widths: None,
                    max_column_widths: None,
                    max_height: None,
                    max_width: None,
                    default_cell_border: None,
                    heading_background_color: None,
                    default_cell_background_color: None,
                    metadata: crate::TableRecordMetadata::default(),
                },
                bbox: bounds,
                rotation_degrees: None,
                column_widths: Vec::new(),
                rows: Vec::new(),
            },
        )));
        table
    }

    #[test]
    fn table_uses_object_bounds_instead_of_content_bounds() {
        let bounds = BoundingBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 100.0,
            y_max: 200.0,
        };
        let table = table_object(0, bounds);
        let text = text("\u{fffc}", vec![table]);
        let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
        assert_eq!(
            object_bounds(index.in_range(0..1)[0].span).unwrap(),
            BoundingBox {
                x_min: bounds.x_min - 0.5,
                y_min: bounds.y_min - 0.5,
                x_max: bounds.x_max + 0.5,
                y_max: bounds.y_max + 0.5
            }
        );
        assert!(index.issues().is_empty());
    }

    #[test]
    fn table_border_expansion_keeps_double_precision_and_invalid_originals_do_not_expand() {
        let bounds = BoundingBox {
            x_min: 0.1234567890123456,
            y_min: 0.9876543210987654,
            x_max: 984.1234567890123,
            y_max: 216.98765432109877,
        };
        let content = text("\u{fffc}", vec![table_object(0, bounds)]);
        let index = TextObjectIndex::new(&content, &TextIndex::new(&content.text));
        let selected = &index.in_range(0..1)[0];
        assert_eq!(selected.bounds.x_min, bounds.x_min - 0.5);
        assert_eq!(selected.bounds.y_max, bounds.y_max + 0.5);
        assert_ne!(
            selected.bounds.x_min,
            f64::from(selected.bounds.x_min as f32)
        );
        for invalid in [
            BoundingBox {
                x_max: bounds.x_min,
                ..bounds
            },
            BoundingBox {
                y_max: bounds.y_min - 0.25,
                ..bounds
            },
        ] {
            let text = text("\u{fffc}", vec![table_object(0, invalid)]);
            let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
            assert!(index.in_range(0..1).is_empty());
            assert_eq!(index.issues()[0].kind, ObjectDiagnosticKind::InvalidBounds);
        }
    }

    #[test]
    fn expanded_table_bounds_are_checked_before_the_object_consumes_source() {
        let bounds = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: f64::from(f32::MAX),
            y_max: 20.0,
        };
        let mut span = table_object(1, bounds);
        if let Some(RichTextObjectContent::Table(table)) = span.content.as_mut() {
            let edge = crate::TableEdgeStyle {
                color: 0,
                width: f32::MAX,
                start_radius: 0.0,
                end_radius: 0.0,
            };
            table.style.border = Some(crate::TableBorder {
                left: edge,
                top: edge,
                right: edge,
                bottom: edge,
                metadata: crate::TableRecordMetadata::default(),
            });
        }
        assert!(valid_bounds(bounds));
        let text = text("A\u{fffc}B", vec![span]);
        let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
        assert!(index.in_range(0..3).is_empty());
        assert_eq!(
            index.issues(),
            &[ObjectDiagnostic {
                anchor_utf16: 1,
                kind: ObjectDiagnosticKind::InvalidBounds
            }]
        );
    }

    #[test]
    fn dimensions_require_finite_positive_extents_but_accept_negative_origins() {
        for width in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let text = text("\u{fffc}", vec![image(0, width)]);
            let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
            assert!(index.in_range(0..1).is_empty());
            assert_eq!(index.issues()[0].kind, ObjectDiagnosticKind::InvalidBounds);
        }
        let mut overflow = image(0, 1.0);
        if let Some(RichTextObjectContent::Image(image)) = overflow.content.as_mut() {
            image.bbox.x_min = -f64::MAX;
            image.bbox.x_max = f64::MAX;
        }
        let text = text("\u{fffc}", vec![overflow]);
        let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
        assert_eq!(index.issues()[0].kind, ObjectDiagnosticKind::InvalidBounds);
        assert!(valid_bounds(BoundingBox {
            x_min: -40.0,
            y_min: -20.0,
            x_max: -10.0,
            y_max: -5.0
        }));
        for bounds in [
            BoundingBox {
                x_min: f64::NAN,
                y_min: 0.0,
                x_max: 10.0,
                y_max: 10.0,
            },
            BoundingBox {
                x_min: 0.0,
                y_min: f64::NEG_INFINITY,
                x_max: 10.0,
                y_max: 10.0,
            },
            BoundingBox {
                x_min: 0.0,
                y_min: 10.0,
                x_max: 10.0,
                y_max: 10.0,
            },
            BoundingBox {
                x_min: 0.0,
                y_min: 11.0,
                x_max: 10.0,
                y_max: 10.0,
            },
            BoundingBox {
                x_min: 0.0,
                y_min: -f64::MAX,
                x_max: 10.0,
                y_max: f64::MAX,
            },
        ] {
            assert!(!valid_bounds(bounds));
        }
    }
}
