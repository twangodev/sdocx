use super::{
    ObjectDiagnosticKind, PreparedTable, RenderTheme, RichTextTable, TextRenderer, native_add,
    native_measured_height, native_sub,
};
use crate::BoundingBox;

#[cfg(test)]
mod native_frame_tests;

#[cfg(test)]
mod native_row_tests;

const CHANGE_EPSILON: f64 = 0.001_f32 as f64;
const FLOAT_EPSILON: f64 = f32::EPSILON as f64;

#[derive(Clone, Default)]
pub(super) struct BandList {
    pub rectangles: Vec<BoundingBox>,
}

impl BandList {
    pub fn new(rectangles: Vec<BoundingBox>) -> Result<Self, ObjectDiagnosticKind> {
        let rectangles = rectangles
            .into_iter()
            .map(|rect| {
                Ok(BoundingBox {
                    x_min: native_measured_height(rect.x_min)?,
                    y_min: native_measured_height(rect.y_min)?,
                    x_max: native_measured_height(rect.x_max)?,
                    y_max: native_measured_height(rect.y_max)?,
                })
            })
            .collect::<Result<Vec<_>, ObjectDiagnosticKind>>()?;
        if rectangles
            .iter()
            .any(|rect| rect.x_max < rect.x_min || rect.y_max < rect.y_min)
        {
            return Err(ObjectDiagnosticKind::InvalidBounds);
        }
        Ok(Self { rectangles })
    }

    fn for_row(&self, top: f64) -> Result<Self, ObjectDiagnosticKind> {
        Self::new(
            self.rectangles
                .iter()
                .filter(|rect| rect.y_min >= top)
                .map(|rect| {
                    Ok(BoundingBox {
                        y_min: native_sub(rect.y_min, top)?,
                        y_max: native_sub(rect.y_max, top)?,
                        ..*rect
                    })
                })
                .collect::<Result<_, ObjectDiagnosticKind>>()?,
        )
    }

    fn same_native(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        if self.rectangles.len() != other.rectangles.len() {
            return false;
        }
        let (Some(left), Some(right)) = (self.rectangles.first(), other.rectangles.first()) else {
            return false;
        };
        [
            left.x_min as f32 - right.x_min as f32,
            left.y_min as f32 - right.y_min as f32,
            left.x_max as f32 - right.x_max as f32,
            left.y_max as f32 - right.y_max as f32,
        ]
        .into_iter()
        .all(|delta| delta.abs() <= CHANGE_EPSILON as f32)
    }
}

pub(super) fn cold(
    plan: &mut PreparedTable,
    table: &RichTextTable,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> Result<(), ObjectDiagnosticKind> {
    for row_index in 0..plan.rows.len() {
        for column_index in 0..plan.rows[row_index].cells.len() {
            let top = plan.rows[row_index].cells[column_index].frame.y_min;
            plan.rows[row_index].cells[column_index].bands = plan.bands.for_row(top)?;
            plan.layout_cell(row_index, column_index, table, theme, renderer)?;
        }
        let height = plan.row_height(row_index)?;
        let mut growth = 0.0_f64;
        for cell in &plan.rows[row_index].cells {
            growth = growth.max(native_sub(cell.metrics.measured_height, height)?);
        }
        if growth > 0.0 {
            extend_row(plan, row_index, growth)?;
            offset_from_row(plan, row_index + 1, growth)?;
        }
    }
    Ok(())
}

pub(super) fn warm(
    plan: &mut PreparedTable,
    table: &RichTextTable,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> Result<(), ObjectDiagnosticKind> {
    for row_index in 0..plan.rows.len() {
        layout_row(plan, row_index, table, theme, renderer)?;
        update_positions(plan, row_index, table)?;
        compress_row(plan, row_index)?;
    }
    Ok(())
}

fn update_split(plan: &mut PreparedTable, row_index: usize) -> Result<bool, ObjectDiagnosticKind> {
    let row = &mut plan.rows[row_index];
    let bands = plan.bands.for_row(row.cells[0].frame.y_min)?;
    if row.cells[0].bands.same_native(&bands) {
        return Ok(false);
    }
    for cell in &mut row.cells {
        cell.bands = bands.clone();
    }
    Ok(true)
}

fn layout_row(
    plan: &mut PreparedTable,
    row_index: usize,
    table: &RichTextTable,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> Result<(), ObjectDiagnosticKind> {
    if update_split(plan, row_index)? {
        adjust_first_line(plan, row_index, table, renderer)?;
    } else {
        if row_index == 0 {
            return Ok(());
        }
        let minimum = plan.first_line_minimum(row_index, table, renderer)?;
        let pending = plan.pending_gaps[row_index];
        if pending > CHANGE_EPSILON {
            let Some(previous) = plan.rows[row_index - 1].cells[0].bands.rectangles.first() else {
                return Ok(());
            };
            let previous_height = native_sub(previous.y_max, previous.y_min)?;
            if minimum > native_sub(pending, previous_height)? {
                return Ok(());
            }
            plan.pending_gaps[row_index] = 0.0;
            offset_from_row(plan, row_index, -pending)?;
            return Ok(());
        }
        let Some(first) = plan.rows[row_index].cells[0].bands.rectangles.first() else {
            return Ok(());
        };
        if native_sub(minimum, first.y_min)? <= FLOAT_EPSILON {
            return Ok(());
        }
        let displacement = first.y_max;
        offset_from_row(plan, row_index, displacement)?;
        plan.pending_gaps[row_index] = displacement;
        update_split(plan, row_index)?;
    }
    for column_index in 0..plan.rows[row_index].cells.len() {
        plan.layout_cell(row_index, column_index, table, theme, renderer)?;
    }
    Ok(())
}

fn adjust_first_line(
    plan: &mut PreparedTable,
    row_index: usize,
    table: &RichTextTable,
    renderer: &TextRenderer<'_>,
) -> Result<(), ObjectDiagnosticKind> {
    if row_index == 0 || plan.rows[row_index].cells[0].bands.rectangles.is_empty() {
        return Ok(());
    }
    let pending = plan.pending_gaps[row_index];
    if pending > CHANGE_EPSILON {
        plan.pending_gaps[row_index] = 0.0;
        offset_from_row(plan, row_index, -pending)?;
        update_split(plan, row_index)?;
    }
    let Some(first) = plan.rows[row_index].cells[0].bands.rectangles.first() else {
        return Ok(());
    };
    if native_sub(
        plan.first_line_minimum(row_index, table, renderer)?,
        first.y_min,
    )? > FLOAT_EPSILON
    {
        let displacement = first.y_max;
        offset_from_row(plan, row_index, displacement)?;
        plan.pending_gaps[row_index] = displacement;
        update_split(plan, row_index)?;
    }
    Ok(())
}

fn update_positions(
    plan: &mut PreparedTable,
    start: usize,
    table: &RichTextTable,
) -> Result<(), ObjectDiagnosticKind> {
    for row_index in start..plan.rows.len() {
        let mut height = 0.0_f64;
        for column in 0..plan.rows[row_index].cells.len() {
            let owner = plan
                .topology
                .frame_owner(super::CellPosition {
                    row: row_index,
                    column,
                })
                .ok_or(ObjectDiagnosticKind::UnsupportedContent)?;
            height = height.max(
                plan.rows[owner.row].cells[owner.column]
                    .metrics
                    .measured_height,
            );
        }
        if height < CHANGE_EPSILON {
            height = f64::from(table.rows[row_index].min_height.unwrap_or(0.0));
        }
        let difference = native_sub(height, plan.row_height(row_index)?)?;
        if difference.abs() > CHANGE_EPSILON {
            extend_row(plan, row_index, difference)?;
            offset_from_row(plan, row_index + 1, difference)?;
        }
    }
    Ok(())
}

fn extend_row(
    plan: &mut PreparedTable,
    row_index: usize,
    displacement: f64,
) -> Result<(), ObjectDiagnosticKind> {
    for cell in &mut plan.rows[row_index].cells {
        cell.frame.y_max = native_add(cell.frame.y_max, displacement)?;
    }
    Ok(())
}

fn offset_from_row(
    plan: &mut PreparedTable,
    start: usize,
    displacement: f64,
) -> Result<(), ObjectDiagnosticKind> {
    let mut pending_actions = vec![(start, displacement)];
    while let Some((start, mut displacement)) = pending_actions.pop() {
        for row_index in start..plan.rows.len() {
            let pending = plan.pending_gaps[row_index];
            if pending >= FLOAT_EPSILON {
                if displacement < 0.0 {
                    plan.pending_gaps[row_index] = 0.0;
                    pending_actions.push((row_index, displacement));
                    pending_actions.push((row_index, -pending));
                    break;
                }
                let remaining = native_sub(pending, displacement)?;
                plan.pending_gaps[row_index] = remaining;
                if remaining > FLOAT_EPSILON {
                    break;
                }
                displacement = -remaining;
                plan.pending_gaps[row_index] = 0.0;
            }
            for cell in &mut plan.rows[row_index].cells {
                let old_top = cell.frame.y_min;
                cell.frame.y_min = native_add(old_top, displacement)?;
                cell.frame.y_max = native_add(cell.frame.y_max, displacement)?;
                cell.layout.translate(0.0, cell.frame.y_min - old_top);
            }
        }
    }
    Ok(())
}

fn compress_row(plan: &mut PreparedTable, row_index: usize) -> Result<(), ObjectDiagnosticKind> {
    let height = plan.row_height(row_index)?;
    let mut selected = None;
    for band in &plan.rows[row_index].cells[0].bands.rectangles {
        if band.y_min > height {
            break;
        }
        selected = Some(*band);
    }
    let Some(band) = selected.filter(|band| band.x_max > band.x_min && band.y_max > band.y_min)
    else {
        return Ok(());
    };
    let last_bottom = plan.rows[row_index]
        .cells
        .iter()
        .map(|cell| cell.metrics.last_line_bottom)
        .fold(0.0_f64, f64::max);
    if last_bottom < band.y_min {
        let displacement = native_sub(native_sub(band.y_min, height)?, plan.half_border)?;
        if displacement < 0.0 {
            extend_row(plan, row_index, displacement)?;
            offset_from_row(plan, row_index + 1, displacement)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::{grid, prepared};
    use super::*;

    fn bands(values: &[(f64, f64)]) -> BandList {
        BandList::new(
            values
                .iter()
                .map(|&(top, bottom)| BoundingBox {
                    x_min: 0.0,
                    x_max: 1080.0,
                    y_min: top,
                    y_max: bottom,
                })
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn native_cache_compares_only_count_and_first_complete_rectangle() {
        let old = bands(&[(10.0, 30.0), (50.0, 70.0)]);
        assert!(old.same_native(&old));
        assert!(old.same_native(&bands(&[(10.0, 30.0), (60.0, 90.0)])));
        assert!(!old.same_native(&bands(&[(10.0, 30.0)])));
        assert!(!BandList::default().same_native(&BandList::default()));
        let mut wider = old.clone();
        wider.rectangles[0].x_max = 1081.0;
        assert!(!old.same_native(&wider));
        let zero = bands(&[(0.0, 30.0)]);
        let mut boundary = zero.clone();
        boundary.rectangles[0].y_min = CHANGE_EPSILON;
        assert!(zero.same_native(&boundary));
        boundary.rectangles[0].y_min = f64::from(f32::from_bits(0.001_f32.to_bits() + 1));
        assert!(!zero.same_native(&boundary));
    }

    #[test]
    fn unchanged_first_band_preserves_stale_later_bands() {
        let source = grid(&[100.0], &[200.0]);
        let mut plan = prepared(&source);
        plan.rows[0].cells[0].bands = bands(&[(10.0, 30.0), (50.0, 70.0)]);
        plan.bands = bands(&[(10.5, 30.5), (60.5, 90.5)]);
        assert!(!update_split(&mut plan, 0).unwrap());
        assert_eq!(plan.rows[0].cells[0].bands.rectangles[1].y_min, 50.0);
    }

    #[test]
    fn positive_offsets_consume_pending_gap_before_moving_frames() {
        for (offset, pending, shift) in [(20.0, 40.0, 0.0), (80.0, 0.0, 20.0)] {
            let mut plan = prepared(&grid(&[100.0; 2], &[200.0]));
            let top = plan.rows[0].cells[0].frame.y_min;
            plan.pending_gaps[0] = 60.0;
            offset_from_row(&mut plan, 0, offset).unwrap();
            assert_eq!(plan.pending_gaps[0], pending);
            assert_eq!(plan.rows[0].cells[0].frame.y_min, top + shift);
            assert_eq!(plan.rows[1].cells[0].frame.y_min, top + 100.0 + shift);
        }
        let mut plan = prepared(&grid(&[100.0; 2], &[200.0]));
        plan.pending_gaps = vec![60.0, 30.0];
        offset_from_row(&mut plan, 0, 80.0).unwrap();
        assert_eq!(plan.rows[0].cells[0].frame.y_min, 20.5);
        assert_eq!(plan.rows[1].cells[0].frame.y_min, 100.5);
        assert_eq!(plan.pending_gaps[1], 10.0);
    }

    #[test]
    fn negative_offsets_remove_pending_gap_then_apply_original_shift() {
        let mut source = grid(&[100.0; 2], &[200.0]);
        source.rows[1].cells[0].content.text = "A".into();
        let mut plan = prepared(&source);
        let old_baseline = plan.rows[1].cells[0].layout.lines[0].baseline;
        plan.pending_gaps[0] = 60.0;
        offset_from_row(&mut plan, 0, -10.0).unwrap();
        assert_eq!(plan.pending_gaps[0], 0.0);
        assert_eq!(plan.rows[0].cells[0].frame.y_min, -69.5);
        assert_eq!(plan.rows[1].cells[0].frame.y_min, 30.5);
        assert_eq!(
            plan.rows[1].cells[0].layout.lines[0].baseline,
            old_baseline - 70.0
        );
    }

    #[test]
    fn warm_rows_shrink_from_cached_children_before_later_rows_relayout() {
        let source = grid(&[100.0; 2], &[200.0]);
        let mut plan = prepared(&source);
        plan.rows[0].cells[0].metrics.measured_height = 40.0;
        plan.rows[1].cells[0].metrics.measured_height = 30.0;
        update_positions(&mut plan, 0, &source).unwrap();
        assert_eq!(plan.row_height(0).unwrap(), 40.0);
        assert_eq!(plan.row_height(1).unwrap(), 30.0);
        assert_eq!(plan.rows[1].cells[0].frame.y_min, 40.5);
        let mut minimum = source;
        minimum.rows[1].min_height = Some(12.0);
        plan.rows[1].cells[0].metrics.measured_height = 0.0;
        update_positions(&mut plan, 1, &minimum).unwrap();
        assert_eq!(plan.row_height(1).unwrap(), 12.0);
    }

    #[test]
    fn compression_uses_last_intersecting_band_and_excludes_bottom_margin() {
        for (last_bottom, expected_height) in [(50.0, 79.5), (80.0, 100.0)] {
            let mut plan = prepared(&grid(&[100.0; 2], &[200.0]));
            plan.rows[0].cells[0].bands = bands(&[(80.0, 100.0), (120.0, 140.0)]);
            plan.rows[0].cells[0].metrics.last_line_bottom = last_bottom;
            plan.rows[0].cells[0].metrics.measured_height = 150.0;
            compress_row(&mut plan, 0).unwrap();
            assert_eq!(plan.row_height(0).unwrap(), expected_height);
            assert_eq!(plan.rows[1].cells[0].frame.y_min, expected_height + 0.5);
        }
    }

    #[test]
    fn first_line_relocation_applies_to_later_rows_only() {
        let source = grid(&[100.0; 2], &[200.0]);
        let mut plan = prepared(&source);
        for row in &mut plan.rows {
            row.cells[0].bands = bands(&[(10.0, 30.0)]);
            row.cells[0].metrics.measured_height = 20.0;
        }
        plan.bands = bands(&[(110.5, 130.5)]);
        let fonts = crate::fonts::FontBook::default();
        let renderer =
            TextRenderer::new(super::super::super::text::TextSettings::default(), &fonts);
        adjust_first_line(&mut plan, 0, &source, &renderer).unwrap();
        assert_eq!(plan.rows[0].cells[0].frame.y_min, 0.5);
        adjust_first_line(&mut plan, 1, &source, &renderer).unwrap();
        assert_eq!(plan.rows[1].cells[0].frame.y_min, 130.5);
        assert_eq!(plan.pending_gaps[1], 30.0);
    }

    #[test]
    fn unchanged_row_removes_gap_only_when_minimum_fits_previous_band() {
        for (minimum, expected_top, expected_pending) in [(20.0, 60.5, 0.0), (21.0, 100.5, 40.0)] {
            let source = grid(&[100.0; 2], &[200.0]);
            let mut plan = prepared(&source);
            plan.rows[0].cells[0].bands = bands(&[(10.0, 30.0)]);
            plan.rows[1].cells[0].bands = bands(&[(10.0, 30.0)]);
            plan.rows[1].cells[0].metrics.measured_height = minimum;
            plan.pending_gaps[1] = 40.0;
            plan.bands = bands(&[(110.5, 130.5)]);
            let fonts = crate::fonts::FontBook::default();
            let renderer =
                TextRenderer::new(super::super::super::text::TextSettings::default(), &fonts);
            layout_row(
                &mut plan,
                1,
                &source,
                RenderTheme::for_canvas(false),
                &renderer,
            )
            .unwrap();
            assert_eq!(plan.rows[1].cells[0].frame.y_min, expected_top);
            assert_eq!(plan.pending_gaps[1], expected_pending);
            assert_eq!(plan.rows[1].cells[0].bands.rectangles[0].y_min, 10.0);
        }
    }

    #[test]
    fn zero_minimum_on_later_row_uses_first_table_row_height() {
        let source = grid(&[100.0, 80.0], &[200.0]);
        let mut plan = prepared(&source);
        plan.rows[0].cells[0].frame.y_max = 10.5;
        plan.rows[1].cells[0].metrics.measured_height = 0.0;
        let fonts = crate::fonts::FontBook::default();
        let renderer =
            TextRenderer::new(super::super::super::text::TextSettings::default(), &fonts);
        assert_eq!(
            plan.first_line_minimum(1, &source, &renderer).unwrap(),
            10.0
        );
    }
}
