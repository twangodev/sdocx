//! Source ownership checks for layout inspection nodes and reflow snapshots.
//!
//! Ordinary equality accepts signed-zero differences. When NaNs prevent that
//! fast path, recursive float extraction retains their exact payload bits while
//! normalizing both signs of zero before comparing all remaining source data.

use crate::{RichTextBox, RichTextObjectContent};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct SourceSnapshot {
    source: RichTextBox,
}

impl SourceSnapshot {
    pub(super) fn capture(source: &RichTextBox) -> Self {
        Self {
            source: source.clone(),
        }
    }

    pub(super) fn matches(&self, source: &RichTextBox) -> bool {
        same_text_source(&self.source, source)
    }
}

pub(crate) fn same_text_source(left: &RichTextBox, right: &RichTextBox) -> bool {
    if left == right {
        return true;
    }
    let mut left = left.clone();
    let mut right = right.clone();
    let mut left_floats = SourceFloats::default();
    let mut right_floats = SourceFloats::default();
    left_floats.text(&mut left);
    right_floats.text(&mut right);
    left_floats.0 == right_floats.0 && left == right
}

#[derive(Default)]
struct SourceFloats(Vec<u64>);

impl SourceFloats {
    fn f32(&mut self, value: &mut f32) {
        self.0.push(if *value == 0.0 {
            0
        } else {
            u64::from(value.to_bits())
        });
        *value = 0.0;
    }

    fn f64(&mut self, value: &mut f64) {
        self.0.push(if *value == 0.0 { 0 } else { value.to_bits() });
        *value = 0.0;
    }

    fn bounds(&mut self, bounds: &mut crate::BoundingBox) {
        for value in [
            &mut bounds.x_min,
            &mut bounds.y_min,
            &mut bounds.x_max,
            &mut bounds.y_max,
        ] {
            self.f64(value);
        }
    }

    fn rotation(&mut self, rotation: &mut Option<f64>) {
        if let Some(rotation) = rotation {
            self.f64(rotation);
        }
    }

    fn size(&mut self, size: &mut Option<f32>) {
        if let Some(size) = size {
            self.f32(size);
        }
    }

    fn border(&mut self, border: &mut Option<crate::TableBorder>) {
        let Some(border) = border else { return };
        for edge in [
            &mut border.left,
            &mut border.top,
            &mut border.right,
            &mut border.bottom,
        ] {
            self.f32(&mut edge.width);
            self.f32(&mut edge.start_radius);
            self.f32(&mut edge.end_radius);
        }
    }

    fn text(&mut self, text: &mut RichTextBox) {
        self.bounds(&mut text.bbox);
        self.rotation(&mut text.rotation_degrees);
        self.size(&mut text.font_size);
        if let Some(margins) = &mut text.margins {
            for margin in margins {
                self.f32(margin);
            }
        }
        for object in &mut text.object_spans {
            match object.content.as_mut() {
                Some(RichTextObjectContent::Image(image)) => {
                    self.bounds(&mut image.bbox);
                    self.rotation(&mut image.rotation_degrees);
                    if let Some(bounds) = &mut image.original_bbox {
                        self.bounds(bounds);
                    }
                }
                Some(RichTextObjectContent::Table(table)) => self.table(table),
                Some(RichTextObjectContent::CodeBlock(code)) => {
                    self.bounds(&mut code.bbox);
                    self.rotation(&mut code.rotation_degrees);
                    if let Some(title) = &mut code.title {
                        self.text(title);
                    }
                    if let Some(body) = &mut code.body {
                        self.text(body);
                    }
                }
                None => {}
            }
        }
    }

    fn table(&mut self, table: &mut crate::RichTextTable) {
        self.bounds(&mut table.bbox);
        self.rotation(&mut table.rotation_degrees);
        for width in &mut table.column_widths {
            self.f32(width);
        }
        let style = &mut table.style;
        self.size(&mut style.min_column_width);
        self.size(&mut style.min_row_height);
        if let Some(bounds) = &mut style.content_bbox {
            self.bounds(bounds);
        }
        self.border(&mut style.border);
        self.border(&mut style.default_cell_border);
        for widths in [&mut style.min_column_widths, &mut style.max_column_widths]
            .into_iter()
            .flatten()
        {
            for width in widths {
                self.f32(width);
            }
        }
        self.size(&mut style.max_height);
        self.size(&mut style.max_width);
        for row in &mut table.rows {
            self.size(&mut row.max_height);
            self.size(&mut row.min_height);
            self.f32(&mut row.height);
            for cell in &mut row.cells {
                self.border(&mut cell.border);
                self.bounds(&mut cell.bbox);
                self.text(&mut cell.content);
            }
        }
    }
}
