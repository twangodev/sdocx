use super::{LayoutContext, TextFrame, TextLine};
use crate::{ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, RichTextObjectContent};

use super::super::{StyledText, TextRenderer};

mod page_obstacles;
pub(in crate::render) use page_obstacles::{NativeCellPageObstacles, NativeObjectPageObstacles};

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(not(all(test, feature = "serde")), allow(dead_code))]
pub(in crate::render) struct NativeObjectEntryBounds {
    pub advance: f32,
    pub height: f32,
    pub font_size: f32,
    pub position: [f32; 2],
    pub layout: [f32; 4],
    pub ink: [f32; 4],
    page_obstacles: Option<NativeObjectPageObstacles>,
}

impl NativeObjectEntryBounds {
    pub fn text_bound(self) -> [f32; 4] {
        self.ink
    }

    pub fn supports_callback_bands(self, callback_top: f32, bands: &[&crate::BoundingBox]) -> bool {
        self.page_obstacles.map_or_else(
            || bands.is_empty(),
            |page| page.matches_callback_bands(callback_top, bands),
        )
    }
}

pub(super) fn native_object_entry_bounds(
    styled: &StyledText<'_>,
    frame: &TextFrame<'_>,
    lines: &[TextLine],
    renderer: &TextRenderer<'_>,
    context: LayoutContext,
) -> Option<NativeObjectEntryBounds> {
    let source = styled.text_box;

    let page_obstacles = if frame.exclusions.is_empty() {
        None
    } else {
        let page = renderer.native_object_page_obstacles?;
        if !page.supports_parent_frame(frame) {
            return None;
        }
        Some(page)
    };
    if !matches!(context, LayoutContext::Flow | LayoutContext::Capture)
        || source.text != "\u{fffc}"
        || source.object_spans.len() != 1
        || !source.spans.is_empty()
        || !source.runs.is_empty()
        || !source.paragraphs.is_empty()
        || !source.text_sections.is_empty()
        || source.color.is_some()
        || source.highlight_color.is_some()
        || source.underline
        || source.font_size.is_some_and(|size| size != 17.0)
        || source.margins.is_some_and(|margins| margins != [0.0; 4])
        || source.gravity.is_some_and(|gravity| gravity != 0)
        || source
            .rotation_degrees
            .is_some_and(|rotation| rotation != 0.0)
        || renderer.settings.scale != 1.0
        || renderer.settings.font_size_delta != 0.0
        || frame.bbox.x_min != 0.0
        || frame.bbox.y_min != 0.0
        || frame.gravity.is_some_and(|gravity| gravity != 0)
        || !frame.bbox.y_max.is_finite()
        || frame.bbox.y_max < 0.0
        || f64::from(frame.bbox.y_max as f32) != frame.bbox.y_max
    {
        return None;
    }
    let span = &source.object_spans[0];
    if span.text_index_utf16 != 0
        || span.layout_option != ObjectSpanLayoutOption::Block
        || !matches!(
            span.layout_constraint,
            ObjectSpanLayoutConstraint::OverPages
                | ObjectSpanLayoutConstraint::OverPagesOverlapPadding
        )
    {
        return None;
    }
    let callback_bands = renderer.table_split_rects(span.layout_constraint, 0.0);
    if page_obstacles.map_or_else(
        || !callback_bands.is_empty(),
        |page| {
            span.layout_constraint != ObjectSpanLayoutConstraint::OverPages
                || !page.matches_callback_bands(0.0, &callback_bands.iter().collect::<Vec<_>>())
        },
    ) {
        return None;
    }
    let Some(RichTextObjectContent::Table(table)) = span.content.as_ref() else {
        return None;
    };
    if table.bbox.x_min != 0.0
        || table.bbox.y_min != 0.0
        || table.style.max_width.is_some_and(|width| width != 0.0)
    {
        return None;
    }
    let [line] = lines else {
        return None;
    };
    let [object] = line.line.objects.as_slice() else {
        return None;
    };
    if line.line.source != (0..1)
        || !line.line.placements.is_empty()
        || line.marker.is_some()
        || line.predefined.is_some()
        || line.alignment.is_some()
        || line.line.font_size != 17.0
        || object.object.inline
        || object.object.source != (0..1)
        || object.object.left_margin != 0.0
        || object.object.top_margin != 0.0
        || object.object.bottom_margin != 0.0
    {
        return None;
    }
    let Some(Ok(crate::render::embedded::PreparedObject::Table(prepared))) =
        object.prepared.as_ref()
    else {
        return None;
    };
    if prepared.rows.iter().flat_map(|row| &row.cells).any(|cell| {
        cell.layout.lines.is_empty()
            || cell.layout.lines.iter().any(|line| {
                line.native_bands.is_none()
                    || line.line.native_placed.is_none()
                    || line.line.source.is_empty()
                    || line.line.placements.iter().any(|placement| {
                        let Some(entries) = &placement.cluster.run.native_entries else {
                            return true;
                        };
                        let owner = entries.geometry().source_range_utf16().start;
                        entries.entry_facts_at_utf16(owner).is_none()
                    })
            })
    }) {
        return None;
    }
    let bands = line.native_bands?;
    let [left, top, right, bottom] = [
        prepared.measured_bbox.x_min,
        prepared.measured_bbox.y_min,
        prepared.measured_bbox.x_max,
        prepared.measured_bbox.y_max,
    ]
    .map(|value| value as f32);
    let width = right - left;
    let height = bottom - top;
    let advance = frame.bbox.x_max as f32;
    let offset = line.line.object_position(0, true)?.x as f32;
    let x = line.x as f32 + offset;
    if ![left, top, right, bottom, width, height, advance, x]
        .into_iter()
        .all(f32::is_finite)
        || width <= 0.0
        || height <= 0.0
        || advance < width
        || f64::from(advance) != frame.bbox.x_max
        || f64::from(height) != object.object.height
        || f64::from(width) != object.object.width
        || bands.overflow
        || line.x != 0.0
        || offset != 0.0
        || line.baseline != f64::from(bands.baseline)
        || (frame.bbox.y_max != 0.0 && frame.bbox.y_max < f64::from(bands.bottom))
    {
        return None;
    }
    let layout = [x, bands.top, x + width, bands.bottom];
    let ink = [x, bands.baseline - height, x + width, bands.baseline];
    let entry = NativeObjectEntryBounds {
        advance,
        height,
        font_size: 17.0,
        position: [x, bands.baseline],
        layout,
        ink,
        page_obstacles,
    };
    entry
        .layout
        .into_iter()
        .chain(entry.text_bound())
        .all(f32::is_finite)
        .then_some(entry)
}

#[cfg(all(test, feature = "serde"))]
mod tests;

#[cfg(all(test, feature = "serde"))]
pub(in crate::render) use tests::{caller_capture_profile, caller_capture_source};
