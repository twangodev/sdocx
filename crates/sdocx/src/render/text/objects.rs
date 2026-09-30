use std::collections::BTreeMap;
use std::ops::Range;

use crate::text_index::TextIndex;
use crate::{BoundingBox, RichTextBox, RichTextObjectContent, RichTextObjectSpan};

pub(in crate::render) struct TextObjectIndex<'a> {
    objects: Vec<TextObject<'a>>,
    issues: Vec<ObjectDiagnostic>,
}

pub(in crate::render) struct TextObject<'a> {
    pub source: Range<usize>,
    pub span: &'a RichTextObjectSpan,
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
        for span in &text.object_spans {
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
                        .then_some(())
                        .ok_or(ObjectDiagnosticKind::InvalidBounds)
                })
                .map(|()| TextObject {
                    source: source..source + 1,
                    span,
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
        Some(RichTextObjectContent::Image(image)) => Ok(image.bbox),
        Some(RichTextObjectContent::Table(table)) => Ok(table.bbox),
        Some(RichTextObjectContent::CodeBlock(code)) => Ok(code.bbox),
        None => Err(ObjectDiagnosticKind::UnsupportedContent),
    }
}

fn valid_bounds(bounds: BoundingBox) -> bool {
    [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max]
        .into_iter()
        .all(f64::is_finite)
        && (bounds.x_max - bounds.x_min).is_finite()
        && (bounds.y_max - bounds.y_min).is_finite()
        && bounds.x_max > bounds.x_min
        && bounds.y_max > bounds.y_min
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
        let bounds = object_bounds(objects[1].span).unwrap();
        assert_eq!(bounds.x_max - bounds.x_min, 50.0);
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

    #[test]
    fn table_uses_object_bounds_instead_of_content_bounds() {
        let bounds = BoundingBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 100.0,
            y_max: 200.0,
        };
        let mut table = image(0, 20.0);
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
        let text = text("\u{fffc}", vec![table]);
        let index = TextObjectIndex::new(&text, &TextIndex::new(&text.text));
        assert_eq!(object_bounds(index.in_range(0..1)[0].span).unwrap(), bounds);
        assert!(index.issues().is_empty());
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
