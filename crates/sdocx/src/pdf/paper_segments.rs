//! Pure planner for the recovered positive-geometry SingleX PDF-paper slicing path.
//!
//! This is deliberately test-only until the page-writer integration and its query adapters are
//! independently reviewed. It plans note slices; it does not load or render source PDF pages.

use crate::PdfPaperRectangle;

const PAGE_BUDGET: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RuntimeRect([f32; 4]);

impl RuntimeRect {
    pub(super) fn from_saved(rectangle: PdfPaperRectangle) -> Result<Self, PlanError> {
        let values = match rectangle {
            PdfPaperRectangle::Integer(values) => values.map(|value| value as f32),
            PdfPaperRectangle::LegacyFloatBits(values) => values.map(f32::from_bits),
            PdfPaperRectangle::Unspecified(_) => return Err(PlanError::UnspecifiedRectangle),
        };
        Self::new(values)
    }

    fn new(values: [f32; 4]) -> Result<Self, PlanError> {
        if !values.into_iter().all(f32::is_finite) {
            return Err(PlanError::NonFiniteGeometry);
        }
        let [left, top, right, bottom] = values;
        if left < 0.0 || top < 0.0 || right < 0.0 || bottom < 0.0 {
            return Err(PlanError::NegativeGeometry);
        }
        if right < left || bottom < top {
            return Err(PlanError::InvertedGeometry);
        }
        if right == left || bottom == top {
            return Err(PlanError::DegenerateGeometry);
        }
        Ok(Self(values))
    }

    fn intersects(self, other: [f32; 4]) -> bool {
        let [left, top, right, bottom] = self.0;
        let [other_left, other_top, other_right, other_bottom] = other;
        left < other_right && other_left < right && top < other_bottom && other_top < bottom
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ResolvedRecord<'a> {
    pub(super) source_key: &'a str,
    pub(super) page_index: i32,
    pub(super) rectangle: RuntimeRect,
    pub(super) source_record_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NoteSlice {
    pub(super) y: i32,
    pub(super) height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Segment {
    pub(super) record_indices: Vec<usize>,
    pub(super) slices: Vec<NoteSlice>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BodyCutObservation {
    pub(super) visible: bool,
    /// Native `GetLineIndex`; `None` represents its `-1` result.
    pub(super) line_index: Option<i32>,
    pub(super) line_count: i32,
    pub(super) line_rect: [f32; 4],
}

pub(super) trait SegmentQueries {
    fn body_cut(&self, band: [f32; 4]) -> Result<BodyCutObservation, PlanError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PlanError {
    InvalidDimensions,
    UnspecifiedRectangle,
    NonFiniteGeometry,
    NegativeGeometry,
    InvertedGeometry,
    DegenerateGeometry,
    NegativePageIndex,
    InvalidBodyObservation,
    ArithmeticOverflow,
    PageBudgetExceeded,
}

#[derive(Debug)]
struct Planner<'a, Q> {
    note_width: i32,
    note_height: i32,
    default_height: i32,
    records: &'a [ResolvedRecord<'a>],
    queries: &'a Q,
}

pub(super) fn plan_segments<'a, Q: SegmentQueries>(
    note_width: i32,
    note_height: i32,
    default_height: i32,
    records: &'a [ResolvedRecord<'a>],
    queries: &'a Q,
) -> Result<Vec<Segment>, PlanError> {
    if note_height < 1 {
        return Ok(Vec::new());
    }
    if note_width < 1 || default_height < 1 {
        return Err(PlanError::InvalidDimensions);
    }
    for record in records {
        if record.page_index < 0 {
            return Err(PlanError::NegativePageIndex);
        }
    }

    Planner {
        note_width,
        note_height,
        default_height,
        records,
        queries,
    }
    .run()
}

impl<Q: SegmentQueries> Planner<'_, Q> {
    fn run(self) -> Result<Vec<Segment>, PlanError> {
        let mut segments = Vec::<Segment>::new();
        let mut y = 0_i32;
        let mut cursor = 0_usize;
        let mut planned_slices = 0_usize;
        let mut previous_pdf_record = None;

        while y < self.note_height {
            if planned_slices >= PAGE_BUDGET {
                return Err(PlanError::PageBudgetExceeded);
            }
            let strip_bottom = y
                .checked_add(self.default_height)
                .ok_or(PlanError::ArithmeticOverflow)?;
            let record = self.records.get(cursor).copied().filter(|record| {
                record.rectangle.intersects([
                    0.0,
                    y as f32,
                    self.note_width as f32,
                    strip_bottom as f32,
                ])
            });

            let (height, attach_record, append_allowed) = if let Some(record) = record {
                let top_delta = (y as f32 - record.rectangle.0[1]).abs();
                let truncated_delta = trunc_i32(top_delta)?;
                if truncated_delta < 2 {
                    let selected = trunc_i32(record.rectangle.0[3] - y as f32)?;
                    (
                        stored_height(selected, self.default_height),
                        Some(record),
                        false,
                    )
                } else {
                    (
                        stored_height(truncated_delta, self.default_height),
                        None,
                        true,
                    )
                }
            } else {
                (
                    self.normal_height(y, strip_bottom, self.records.get(cursor).copied())?,
                    None,
                    true,
                )
            };

            if let Some(record) = attach_record {
                let can_group = segments
                    .last()
                    .is_some_and(|segment| !segment.record_indices.is_empty())
                    && previous_pdf_record.is_some_and(|previous: ResolvedRecord<'_>| {
                        previous.source_key == record.source_key
                            && previous.page_index.checked_add(1) == Some(record.page_index)
                    });
                if !can_group {
                    segments.push(Segment {
                        record_indices: Vec::new(),
                        slices: Vec::new(),
                    });
                }
                let segment = segments.last_mut().expect("PDF segment");
                segment.slices.push(NoteSlice { y, height });
                segment.record_indices.push(record.source_record_index);
                previous_pdf_record = Some(record);
            } else {
                let append_to_current = append_allowed
                    && segments
                        .last()
                        .is_some_and(|segment| segment.record_indices.is_empty());
                if !append_to_current {
                    segments.push(Segment {
                        record_indices: Vec::new(),
                        slices: Vec::new(),
                    });
                }
                segments
                    .last_mut()
                    .expect("ordinary segment")
                    .slices
                    .push(NoteSlice { y, height });
            }
            planned_slices += 1;

            // Native appends before isLastPageBlank. At the note end, that check either breaks
            // immediately or returns false before the outer y < noteHeight test exits. Its
            // content probes therefore cannot change this topology. Checked advance below
            // excludes the signed-overflow case where native wrapping could continue the loop.
            y = y.checked_add(height).ok_or(PlanError::ArithmeticOverflow)?;
            if y < self.note_height
                && segments
                    .last()
                    .is_some_and(|s| !s.record_indices.is_empty())
            {
                cursor = cursor.checked_add(1).ok_or(PlanError::ArithmeticOverflow)?;
            }
        }
        Ok(segments)
    }

    fn normal_height(
        &self,
        y: i32,
        nominal_bottom: i32,
        current_record: Option<ResolvedRecord<'_>>,
    ) -> Result<i32, PlanError> {
        let band_top = nominal_bottom
            .checked_sub(1)
            .ok_or(PlanError::ArithmeticOverflow)?;
        let observation = self.queries.body_cut([
            0.0,
            band_top as f32,
            self.note_width as f32,
            nominal_bottom as f32,
        ])?;
        let mut height = self.default_height;
        let mut adjusted_by_body = false;
        if observation.visible {
            validate_body(observation)?;
            let last_line = observation.line_count - 1;
            if let Some(index) = observation.line_index {
                if index == last_line {
                    let cut = nominal_bottom as f32;
                    if observation.line_rect[1] <= cut && observation.line_rect[3] >= cut {
                        let candidate = trunc_i32(observation.line_rect[3] - y as f32)?;
                        if candidate != self.default_height
                            && (candidate as f32) <= (self.default_height as f32) * 1.2_f32
                        {
                            height = candidate;
                            adjusted_by_body = true;
                        }
                    }
                } else {
                    let candidate = trunc_i32(observation.line_rect[3] - y as f32)?;
                    if candidate != self.default_height
                        && (candidate as f32) <= (self.default_height as f32) * 1.2_f32
                    {
                        height = candidate;
                        adjusted_by_body = true;
                    }
                }
            } else {
                // Native -1 falls back to the nominal slice height.
            }
        }

        if adjusted_by_body {
            let candidate_bottom = y.checked_add(height).ok_or(PlanError::ArithmeticOverflow)?;
            if let Some(record) = current_record
                && record.rectangle.intersects([
                    0.0,
                    y as f32,
                    self.note_width as f32,
                    candidate_bottom as f32,
                ])
            {
                height = trunc_i32(record.rectangle.0[1] - y as f32)?;
            }
        }
        Ok(stored_height(height, self.default_height))
    }
}

fn validate_body(observation: BodyCutObservation) -> Result<(), PlanError> {
    if observation.line_count < 0
        || observation
            .line_index
            .is_some_and(|index| index < 0 || index >= observation.line_count)
        || !observation.line_rect.into_iter().all(f32::is_finite)
    {
        return Err(PlanError::InvalidBodyObservation);
    }
    let [left, top, right, bottom] = observation.line_rect;
    if left < 0.0 || top < 0.0 || right < left || bottom < top {
        return Err(PlanError::InvalidBodyObservation);
    }
    Ok(())
}

fn trunc_i32(value: f32) -> Result<i32, PlanError> {
    if !value.is_finite() || value < i32::MIN as f32 || value >= i32::MAX as f32 {
        return Err(PlanError::ArithmeticOverflow);
    }
    Ok(value.trunc() as i32)
}

fn stored_height(height: i32, default_height: i32) -> i32 {
    if height < 1 { default_height } else { height }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct Queries {
        body: BodyCutObservation,
    }

    impl SegmentQueries for Queries {
        fn body_cut(&self, _band: [f32; 4]) -> Result<BodyCutObservation, PlanError> {
            Ok(self.body)
        }
    }

    fn queries() -> Queries {
        Queries {
            body: BodyCutObservation {
                visible: false,
                line_index: None,
                line_count: 0,
                line_rect: [0.0; 4],
            },
        }
    }

    fn record(
        source_key: &'static str,
        page_index: i32,
        rect: [f32; 4],
        source_record_index: usize,
    ) -> ResolvedRecord<'static> {
        ResolvedRecord {
            source_key,
            page_index,
            rectangle: RuntimeRect::new(rect).unwrap(),
            source_record_index,
        }
    }

    #[test]
    fn native_quiz_topology_retains_the_appended_blank_tail() {
        let record = record("quiz.pdf", 0, [0.0, 0.0, 1812.0, 1485.0], 0);
        let segments = plan_segments(1812, 15372, 1527, &[record], &queries()).unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].record_indices, [0]);
        assert_eq!(segments[0].slices, [NoteSlice { y: 0, height: 1485 }]);
        // Observed from SingleX on the ConstructLoad-loaded archive, not a
        // synthetic WNote (see the Single-mode findings).
        assert_eq!(
            segments[1].slices,
            [
                1485, 3012, 4539, 6066, 7593, 9120, 10647, 12174, 13701, 15228
            ]
            .map(|y| NoteSlice { y, height: 1527 })
        );
    }

    #[test]
    fn quiz_image_cut_uses_native_line_bottom_not_saved_image_bottom() {
        struct ImageCut;
        impl SegmentQueries for ImageCut {
            fn body_cut(&self, band: [f32; 4]) -> Result<BodyCutObservation, PlanError> {
                assert_eq!(band, [0.0, 6065.0, 1812.0, 6066.0]);
                Ok(BodyCutObservation {
                    visible: true,
                    line_index: Some(112),
                    line_count: 116,
                    line_rect: [1111490560, 1168837030, 1141759706, 1170258266].map(f32::from_bits),
                })
            }
        }

        // Native body layout of the quiz's LF/U+FFFC source and block image,
        // with resolved font 36, frame width 1812 and margins [48,30,48,30].
        // The saved image ends at 6154.564, but its native line ends at 6167.169.
        // This tests that one observed cut, not the complete note topology.
        let planner = Planner {
            note_width: 1812,
            note_height: 15372,
            default_height: 1527,
            records: &[],
            queries: &ImageCut,
        };
        assert_eq!(planner.normal_height(4539, 6066, None).unwrap(), 1628);
    }

    #[test]
    fn native_cs61_order_groups_consecutive_source_pages() {
        let spans = [
            (0, 1397),
            (1397, 1397),
            (2794, 1398),
            (4192, 1398),
            (5590, 1397),
            (6987, 1398),
            (8385, 1398),
            (9783, 1397),
            (11180, 1398),
            (12578, 1398),
            (13976, 1397),
            (15373, 1398),
            (16771, 1398),
            (18169, 1397),
            (19566, 1398),
            (20964, 1398),
            (22362, 1398),
            (23760, 1397),
            (25157, 1397),
            (26554, 1398),
        ];
        let records = spans
            .into_iter()
            .enumerate()
            .map(|(index, (y, height))| {
                record(
                    "cs61.pdf",
                    index as i32,
                    [0.0, y as f32, 1080.0, (y + height) as f32],
                    index,
                )
            })
            .collect::<Vec<_>>();
        let segments = plan_segments(1080, 27952, 1527, &records, &queries()).unwrap();
        assert_eq!(segments[0].record_indices.len(), 20);
        assert_eq!(segments[0].slices.len(), 20);
    }

    #[test]
    fn misaligned_positive_tops_emit_ordinary_prefix_then_pdf_slice() {
        for offset in [3.0, 7.0] {
            let record = record("paper.pdf", 0, [0.0, offset, 100.0, 100.0], 0);
            let segments = plan_segments(100, 200, 100, &[record], &queries()).unwrap();
            assert_eq!(
                segments[0].slices[0],
                NoteSlice {
                    y: 0,
                    height: offset as i32
                }
            );
            assert_eq!(segments[1].record_indices, [0]);
            assert_eq!(segments[1].slices[0].y, offset as i32);
        }
    }

    #[test]
    fn rectangle_edge_touch_does_not_intersect_but_one_unit_overlap_does() {
        let touching = record("paper.pdf", 0, [100.0, 0.0, 120.0, 100.0], 0);
        let overlap = record("paper.pdf", 0, [99.0, 0.0, 120.0, 100.0], 0);
        let no_hit = plan_segments(100, 100, 100, &[touching], &queries()).unwrap();
        let hit = plan_segments(100, 100, 100, &[overlap], &queries()).unwrap();
        assert!(
            no_hit
                .iter()
                .all(|segment| segment.record_indices.is_empty())
        );
        assert_eq!(hit[0].record_indices, [0]);
    }

    #[test]
    fn body_candidate_1832_is_accepted_and_clipped_but_1833_is_rejected() {
        for (line_bottom, expected) in [(1832.0, 1600), (1833.0, 1527)] {
            let qs = Queries {
                body: BodyCutObservation {
                    visible: true,
                    line_index: Some(0),
                    line_count: 1,
                    line_rect: [0.0, 1527.0, 1812.0, line_bottom],
                },
            };
            let record = record("paper.pdf", 0, [0.0, 1600.0, 1812.0, 1700.0], 0);
            let segments = plan_segments(1812, 2000, 1527, &[record], &qs).unwrap();
            assert_eq!(segments[0].slices[0].height, expected);
        }
    }

    #[test]
    fn body_candidate_1000_without_record_intersection_is_retained() {
        let qs = Queries {
            body: BodyCutObservation {
                visible: true,
                line_index: Some(0),
                line_count: 2,
                line_rect: [0.0, 990.0, 100.0, 1000.0],
            },
        };
        let record = record("paper.pdf", 0, [0.0, 2000.0, 100.0, 2500.0], 0);
        let segments = plan_segments(100, 3000, 1527, &[record], &qs).unwrap();
        assert_eq!(segments[0].slices[0], NoteSlice { y: 0, height: 1000 });
    }

    #[test]
    fn grouping_requires_resolved_source_identity_and_consecutive_indices() {
        for (second_key, second_index, expected_segments) in
            [("other.pdf", 1, 2), ("same.pdf", 3, 2), ("same.pdf", 2, 1)]
        {
            let records = [
                record("same.pdf", 1, [0.0, 0.0, 100.0, 100.0], 4),
                record(second_key, second_index, [0.0, 100.0, 100.0, 200.0], 5),
            ];
            let segments = plan_segments(100, 200, 100, &records, &queries()).unwrap();
            assert_eq!(segments.len(), expected_segments);
        }
    }

    #[test]
    fn saved_rectangle_conversion_is_explicit_and_preserves_legacy_bits() {
        assert_eq!(
            RuntimeRect::from_saved(PdfPaperRectangle::Integer([1, 2, 3, 4])).unwrap(),
            RuntimeRect([1.0, 2.0, 3.0, 4.0])
        );
        let values = [1.25_f32, 2.5, 3.75, 4.0];
        let bits = values.map(f32::to_bits);
        assert_eq!(
            RuntimeRect::from_saved(PdfPaperRectangle::LegacyFloatBits(bits)).unwrap(),
            RuntimeRect(values)
        );
        assert_eq!(
            RuntimeRect::from_saved(PdfPaperRectangle::Unspecified([0; 16])),
            Err(PlanError::UnspecifiedRectangle)
        );
        assert_eq!(
            RuntimeRect::from_saved(PdfPaperRectangle::Integer([0, 0, 0, 1])),
            Err(PlanError::DegenerateGeometry)
        );
    }
}
