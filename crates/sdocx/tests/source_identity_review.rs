#![cfg(feature = "serde")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, Page, RichTextBox, RichTextObjectContent,
    RichTextObjectSpan, RichTextSpan, RichTextSpanType, SpanIntervalType, layout_document,
};
use serde_json::json;

fn text(source: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: source.into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(10.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn document() -> Document {
    let bounds = json!({"x_min": 1.0, "y_min": 2.0, "x_max": 3.0, "y_max": 4.0});
    let metadata = json!({"property_mask": [], "field_mask": [],
        "fixed_trailing_data": [], "flexible_trailing_data": []});
    let edge = json!({"color": 0, "width": 1.0, "start_radius": 2.0, "end_radius": 3.0});
    let border = json!({"left": edge, "top": edge, "right": edge, "bottom": edge,
        "metadata": metadata});
    let mut body = text("\u{fffc}\u{fffc}\u{fffc}body");
    let image = json!({"Image": {"bbox": bounds, "original_bbox": bounds,
        "rotation_degrees": 1.0, "media_id": null, "media_index": null,
        "crop_rect": null, "border_media_id": null, "original_media_id": null}});
    let table = json!({"Table": {
        "bbox": bounds, "rotation_degrees": 1.0, "column_widths": [1.0],
        "style": {"heading_column_enabled": false, "heading_row_enabled": false,
            "max_height_enabled": false, "min_column_width": 1.0, "min_row_height": 1.0,
            "content_bbox": bounds, "border": border, "default_cell_border": border,
            "min_column_widths": [1.0], "max_column_widths": [1.0],
            "max_height": 1.0, "max_width": 1.0, "metadata": metadata},
        "rows": [{"max_height": 1.0, "min_height": 1.0, "height": 1.0,
            "metadata": metadata, "index": 0,
            "cells": [{"border": border, "metadata": metadata, "column_index": 0,
                "row_span": 1, "column_span": 1, "background_color": 0,
                "has_own_background_color": false, "bbox": bounds,
                "editable": true, "content": text("cell")}]}]}});
    let code = json!({"CodeBlock": {"bbox": bounds, "rotation_degrees": 1.0,
        "title": text("title"), "body": text("code")}});
    for (index, (kind, content)) in [("Image", image), ("Table", table), ("CodeBlock", code)]
        .into_iter()
        .enumerate()
    {
        body.object_spans.push(
            serde_json::from_value::<RichTextObjectSpan>(json!({
                "object_type": kind, "object_data": [1, 2, 3], "content": content,
                "text_index_utf16": index, "layout_option": "Inline",
                "layout_constraint": "Normal"
            }))
            .unwrap(),
        );
    }
    Document {
        pages: (0..2)
            .map(|index| Page {
                uuid: format!("source-identity-{index}"),
                width: 360,
                height: 100,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: Vec::new(),
            })
            .collect(),
        metadata: DocumentMetadata {
            note_text: Some(body),
            ..Default::default()
        },
    }
}

fn set_nan(body: &mut RichTextBox, field: usize, payload: u64) {
    let nan32 = f32::from_bits(0x7fc0_0000 | payload as u32);
    let nan64 = f64::from_bits(0x7ff8_0000_0000_0000 | payload);
    match field {
        0 => body.rotation_degrees = Some(nan64),
        1 => body.font_size = Some(nan32),
        2 => body.margins = Some([nan32; 4]),
        3 | 4 => {
            let Some(RichTextObjectContent::Image(image)) = body.object_spans[0].content.as_mut()
            else {
                unreachable!()
            };
            if field == 3 {
                image.original_bbox.as_mut().unwrap().x_min = nan64;
            } else {
                image.rotation_degrees = Some(nan64);
            }
        }
        5..=9 => {
            let Some(RichTextObjectContent::Table(table)) = body.object_spans[1].content.as_mut()
            else {
                unreachable!()
            };
            match field {
                5 => {
                    table
                        .style
                        .default_cell_border
                        .as_mut()
                        .unwrap()
                        .bottom
                        .end_radius = nan32
                }
                6 => table.style.max_column_widths.as_mut().unwrap()[0] = nan32,
                7 => table.rows[0].min_height = Some(nan32),
                8 => {
                    table.rows[0].cells[0]
                        .border
                        .as_mut()
                        .unwrap()
                        .left
                        .start_radius = nan32
                }
                9 => table.rows[0].cells[0].content.font_size = Some(nan32),
                _ => unreachable!(),
            }
        }
        10 | 11 => {
            let Some(RichTextObjectContent::CodeBlock(code)) =
                body.object_spans[2].content.as_mut()
            else {
                unreachable!()
            };
            if field == 10 {
                code.title.as_mut().unwrap().rotation_degrees = Some(nan64);
            } else {
                code.body.as_mut().unwrap().margins = Some([nan32; 4]);
            }
        }
        _ => unreachable!(),
    }
}

#[test]
fn recursive_nan_fields_retain_reflow_but_changed_payloads_do_not() {
    for field in 0..12 {
        let mut document = document();
        let body = document.metadata.note_text.as_mut().unwrap();
        set_nan(body, field, 0x42);
        let layout = layout_document(&document);
        for page in &layout.pages {
            assert!(page.body_text_reflow(&document).is_some(), "field {field}");
        }
        set_nan(document.metadata.note_text.as_mut().unwrap(), field, 0x43);
        for page in &layout.pages {
            assert!(
                page.body_text_reflow(&document).is_none(),
                "changed NaN payload, field {field}"
            );
        }
        set_nan(document.metadata.note_text.as_mut().unwrap(), field, 0x42);
        document.metadata.note_text.as_mut().unwrap().object_spans[1].object_data[0] ^= 1;
        for page in &layout.pages {
            assert!(
                page.body_text_reflow(&document).is_none(),
                "edited raw source, field {field}"
            );
        }
    }
}

#[test]
fn raw_nan_style_payloads_retain_exact_identity_without_serde_normalization() {
    let mut document = document();
    let body = document.metadata.note_text.as_mut().unwrap();
    body.spans.push(RichTextSpan {
        kind: RichTextSpanType::FontSize,
        start_utf16: 0,
        end_utf16: 1,
        interval_type: SpanIntervalType::ClosedOpen,
        payload: f32::from_bits(0x7fc0_0042).to_le_bytes().to_vec(),
    });
    let layout = layout_document(&document);
    assert!(layout.pages[1].body_text_reflow(&document).is_some());
    document.metadata.note_text.as_mut().unwrap().spans[0].payload[0] ^= 1;
    assert!(layout.pages[1].body_text_reflow(&document).is_none());
}
