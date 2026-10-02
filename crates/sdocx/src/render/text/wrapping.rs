use std::ops::Range;

use crate::PredefinedTextStyle;
use crate::render::RenderTheme;

use super::bidi::BidiError;
use super::breaks::{BreakCandidate, BreakKind, ParagraphBreaks, break_candidates};
use super::measurement::{MeasuredCluster, MeasurementError, ParagraphMeasurer};
use super::objects::{MeasuredObject, ObjectMeasurementContext};
use super::{StyledText, TextRenderer};

mod native_slots;
pub(in crate::render::text) use native_slots::NativePlacedLine;
pub(in crate::render) use native_slots::ParagraphMeasurementWidth;
use native_slots::{NativeLineSlots, NativeParagraphSlots};
mod native_mixed;
use native_mixed::NativeMixedSlots;

pub(in crate::render) struct WrappedLine {
    pub source: Range<usize>,
    pub font_size: f64,
    pub text_height: f64,
    pub advance: f64,
    pub placements: Vec<PositionedCluster>,
    pub objects: Vec<PositionedObject>,
    pub visual_order: Vec<LineEntry>,
    pub native_positioned: bool,
    pub geometry: LineGeometry,
    native_slots: Option<NativeLineSlots>,
    pub(super) native_placed: Option<NativePlacedLine>,
    native_mixed: bool,
    unsupported_native_wrapping: bool,
    fallback_measurement_issues: Vec<super::SourceTextDiagnostic>,
}

#[derive(Debug)]
pub(in crate::render) enum LineGeometry {
    Unmeasured,
    Positioned {
        advance: f64,
        text: Vec<LinePosition>,
        objects: Vec<LinePosition>,
    },
    Unavailable(BidiError),
}

impl std::fmt::Display for LineGeometry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unmeasured => formatter.write_str("text geometry was not measured"),
            Self::Positioned { .. } => formatter.write_str("positioned text geometry"),
            Self::Unavailable(error) => error.fmt(formatter),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::render) struct LinePosition {
    pub x: f64,
    pub glyph_offset_x: f64,
    pub extra_advance: f64,
    pub visual_rank: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::render) enum LineEntry {
    Text(usize),
    Object(usize),
}

pub(in crate::render) struct PositionedCluster {
    pub cluster: MeasuredCluster,
    pub x: f64,
    pub extra_advance: f64,
    pub visual_rank: usize,
}

pub(in crate::render) struct PositionedObject {
    pub object: MeasuredObject,
    pub x: f64,
    pub visual_rank: usize,
    pub prepared:
        Option<Result<crate::render::embedded::PreparedObject, super::ObjectDiagnosticKind>>,
}

enum MeasuredItem {
    TextCluster(MeasuredCluster),
    Object {
        placement: PositionedObject,
        font_size: f64,
    },
}

struct ParagraphItems {
    items: Vec<MeasuredItem>,
    font_size: f64,
}

impl MeasuredItem {
    fn source(&self) -> &Range<usize> {
        match self {
            Self::TextCluster(cluster) => &cluster.source,
            Self::Object { placement, .. } => &placement.object.source,
        }
    }

    fn advance(&self) -> f64 {
        match self {
            Self::TextCluster(cluster) => cluster.advance,
            Self::Object { placement, .. } => placement.object.advance(),
        }
    }
}

impl WrappedLine {
    pub fn unmeasured(source: Range<usize>, font_size: f64) -> Self {
        Self {
            source,
            font_size,
            text_height: font_size,
            advance: 0.0,
            placements: Vec::new(),
            objects: Vec::new(),
            visual_order: Vec::new(),
            native_positioned: false,
            geometry: LineGeometry::Unmeasured,
            native_slots: None,
            native_placed: None,
            native_mixed: false,
            unsupported_native_wrapping: false,
            fallback_measurement_issues: Vec::new(),
        }
    }

    pub fn unsupported_native_wrapping(&self) -> bool {
        self.unsupported_native_wrapping
    }

    pub fn fallback_measurement_issues(&self) -> &[super::SourceTextDiagnostic] {
        &self.fallback_measurement_issues
    }

    pub fn advance_for_paint(&self, canonical: bool) -> Option<f64> {
        if !canonical || self.native_slots.is_some() || self.native_mixed {
            return Some(self.advance);
        }
        match &self.geometry {
            LineGeometry::Positioned { advance, .. } => Some(*advance),
            _ => None,
        }
    }

    pub fn text_position(&self, index: usize, canonical: bool) -> Option<LinePosition> {
        if canonical {
            match &self.geometry {
                LineGeometry::Positioned { text, .. } => text.get(index).copied(),
                _ => None,
            }
        } else {
            self.placements.get(index).map(|placement| LinePosition {
                x: placement.x,
                glyph_offset_x: 0.0,
                extra_advance: placement.extra_advance,
                visual_rank: placement.visual_rank,
            })
        }
    }

    pub fn place_native_cell(
        &mut self,
        left: f64,
        width: f64,
        alignment: Option<crate::ParagraphAlignment>,
    ) -> Result<Option<f64>, MeasurementError> {
        let Some(slots) = &self.native_slots else {
            return Ok(None);
        };
        let native_left = left as f32;
        let native_width = width as f32;
        if !self.native_positioned
            || ![native_left, native_width].into_iter().all(f32::is_finite)
            || f64::from(native_left) != left
            || f64::from(native_width) != width
        {
            return Ok(None);
        }
        let aligned = slots.aligned_positions(native_left, native_width, alignment)?;
        let origin = f64::from(aligned.origin);
        self.native_placed = Some(aligned.dense);
        let mut text = aligned.positions;
        for position in &mut text {
            let relative = position.x - origin;
            if origin + relative != position.x {
                return Err(MeasurementError::InvalidCluster);
            }
            position.x = relative;
        }
        for (placement, position) in self.placements.iter_mut().zip(&text) {
            placement.x = position.x;
            placement.extra_advance = position.extra_advance;
            placement.visual_rank = position.visual_rank;
        }
        self.geometry = LineGeometry::Positioned {
            advance: aligned.end - origin,
            text,
            objects: Vec::new(),
        };
        Ok(Some(origin))
    }

    pub fn object_position(&self, index: usize, canonical: bool) -> Option<LinePosition> {
        if canonical {
            match &self.geometry {
                LineGeometry::Positioned { objects, .. } => objects.get(index).copied(),
                _ => None,
            }
        } else {
            self.objects.get(index).map(|placement| LinePosition {
                x: placement.x,
                glyph_offset_x: 0.0,
                extra_advance: 0.0,
                visual_rank: placement.visual_rank,
            })
        }
    }

    fn canonical_geometry(&self) -> Result<LineGeometry, MeasurementError> {
        if self.visual_order.len() != self.placements.len() + self.objects.len() {
            return Err(MeasurementError::InvalidCluster);
        }
        let mut text = vec![LinePosition::default(); self.placements.len()];
        let mut objects = vec![LinePosition::default(); self.objects.len()];
        let mut seen_text = vec![false; text.len()];
        let mut seen_objects = vec![false; objects.len()];
        let mut x = 0.0;
        for (rank, &entry) in self.visual_order.iter().enumerate() {
            let (position, seen, advance, content_x) = match entry {
                LineEntry::Text(index) => (
                    text.get_mut(index),
                    seen_text.get_mut(index),
                    self.placements
                        .get(index)
                        .map(|placement| placement.cluster.advance),
                    x,
                ),
                LineEntry::Object(index) => (
                    objects.get_mut(index),
                    seen_objects.get_mut(index),
                    self.objects
                        .get(index)
                        .map(|placement| placement.object.advance()),
                    finite_advance(
                        x + self
                            .objects
                            .get(index)
                            .ok_or(MeasurementError::InvalidCluster)?
                            .object
                            .left_margin,
                    )?,
                ),
            };
            let position = position.ok_or(MeasurementError::InvalidCluster)?;
            let seen = seen.ok_or(MeasurementError::InvalidCluster)?;
            if *seen {
                return Err(MeasurementError::InvalidCluster);
            }
            *seen = true;
            *position = LinePosition {
                x: content_x,
                visual_rank: rank,
                ..Default::default()
            };
            x = finite_advance(x + advance.ok_or(MeasurementError::InvalidCluster)?)?;
        }
        Ok(LineGeometry::Positioned {
            advance: x,
            text,
            objects,
        })
    }

    pub fn object_height(&self) -> f64 {
        self.objects.iter().fold(0.0_f64, |height, positioned| {
            height.max(positioned.object.height)
        })
    }

    pub fn base_height(&self) -> f64 {
        self.text_height.max(self.object_height())
    }

    pub fn position_native(&mut self, styled: &StyledText<'_>) -> Result<(), MeasurementError> {
        if self.native_positioned
            || self.visual_order.is_empty()
            || self
                .objects
                .iter()
                .any(|object| matches!(object.prepared, Some(Err(_))))
            || self.placements.iter().any(|placement| {
                let source = &placement.cluster.source;
                styled.foreground_segments(source.clone()).count() != 1
                    || styled.index.slice(source.clone()).is_none_or(|text| {
                        !matches!(placement.cluster.native_paint_offset(text), Ok(Some(_)))
                    })
            })
        {
            return Ok(());
        }
        if matches!(self.geometry, LineGeometry::Unmeasured) {
            self.geometry = self.canonical_geometry()?;
        }
        let LineGeometry::Positioned { text, objects, .. } = &self.geometry else {
            return Ok(());
        };
        for (placement, position) in self.placements.iter_mut().zip(text) {
            placement.x = position.x;
            placement.visual_rank = position.visual_rank;
        }
        for (placement, position) in self.objects.iter_mut().zip(objects) {
            placement.x = position.x;
            placement.visual_rank = position.visual_rank;
        }
        self.native_positioned = true;
        Ok(())
    }

    pub fn justify(&mut self, styled: &StyledText<'_>, width: f64) -> Result<(), MeasurementError> {
        if let Some(slots) = &self.native_slots
            && self.native_positioned
        {
            let share = slots.justification_share(width)?;
            let (text, advance) = slots.positions(share)?;
            for (placement, position) in self.placements.iter_mut().zip(&text) {
                placement.x = position.x;
                placement.extra_advance = position.extra_advance;
                placement.visual_rank = position.visual_rank;
            }
            self.advance = advance;
            self.geometry = LineGeometry::Positioned {
                advance,
                text,
                objects: Vec::new(),
            };
            return Ok(());
        }
        let width = native_geometry(width)?;
        let advance = native_geometry(self.advance)?;
        let mut weights = Vec::with_capacity(self.placements.len());
        let mut total_weight = 0_u32;
        for placement in &self.placements {
            let text = styled
                .index
                .slice(placement.cluster.source.clone())
                .ok_or(MeasurementError::InvalidRange)?;
            let mut weight = 0_u32;
            let mut leading_weight = 0_u32;
            for (index, scalar) in text.chars().enumerate() {
                let scalar_weight = justification_weight(scalar);
                weight = weight
                    .checked_add(scalar_weight)
                    .ok_or(MeasurementError::InvalidCluster)?;
                if index > 0
                    && placement.cluster.run.direction == crate::fonts::Direction::RightToLeft
                {
                    leading_weight = leading_weight
                        .checked_add(scalar_weight)
                        .ok_or(MeasurementError::InvalidCluster)?;
                }
            }
            total_weight = total_weight
                .checked_add(weight)
                .ok_or(MeasurementError::InvalidCluster)?;
            weights.push((weight, leading_weight));
        }
        if total_weight == 0 {
            return Ok(());
        }
        let extra = native_geometry(f64::from((width - advance) / total_weight as f32))?;
        let compatibility = self.justification_entries(&weights, false)?;
        let (compatibility, advance) = justify_entries(compatibility, extra, advance, false)?;
        let canonical = if let Some(slots) = &self.native_slots {
            let share = slots.justification_share(width.into())?;
            let (positions, advance) = slots.positions(share)?;
            Some((
                positions
                    .into_iter()
                    .enumerate()
                    .map(|(index, position)| (LineEntry::Text(index), position))
                    .collect(),
                native_geometry(advance)?,
            ))
        } else if let LineGeometry::Positioned { advance, .. } = &self.geometry {
            let entries = self.justification_entries(&weights, true)?;
            Some(justify_entries(
                entries,
                extra,
                native_geometry(*advance)?,
                true,
            )?)
        } else {
            None
        };
        for (entry, position) in compatibility {
            match entry {
                LineEntry::Text(index) => {
                    self.placements[index].x = position.x;
                    self.placements[index].extra_advance = position.extra_advance;
                }
                LineEntry::Object(index) => self.objects[index].x = position.x,
            }
        }
        if let Some((positions, advance)) = canonical
            && let LineGeometry::Positioned {
                advance: stored_advance,
                text,
                objects,
            } = &mut self.geometry
        {
            for (entry, position) in positions {
                match entry {
                    LineEntry::Text(index) => text[index] = position,
                    LineEntry::Object(index) => objects[index] = position,
                }
            }
            *stored_advance = f64::from(advance);
        }
        self.advance = f64::from(advance);
        Ok(())
    }

    fn justification_entries(
        &self,
        weights: &[(u32, u32)],
        canonical: bool,
    ) -> Result<Vec<JustificationEntry>, MeasurementError> {
        let mut entries = Vec::with_capacity(self.placements.len() + self.objects.len());
        for (index, placement) in self.placements.iter().enumerate() {
            let position = self
                .text_position(index, canonical)
                .ok_or(MeasurementError::InvalidCluster)?;
            entries.push(JustificationEntry {
                entry: LineEntry::Text(index),
                position,
                weight: weights[index].0,
                leading_weight: if canonical { weights[index].1 } else { 0 },
                rank: if canonical || self.native_positioned {
                    position.visual_rank
                } else {
                    placement.cluster.source.start
                },
            });
        }
        for (index, placement) in self.objects.iter().enumerate() {
            let position = self
                .object_position(index, canonical)
                .ok_or(MeasurementError::InvalidCluster)?;
            entries.push(JustificationEntry {
                entry: LineEntry::Object(index),
                position,
                weight: 0,
                leading_weight: 0,
                rank: if canonical || self.native_positioned {
                    position.visual_rank
                } else {
                    placement.object.source.start
                },
            });
        }
        Ok(entries)
    }

    pub fn has_block_margins(&self) -> bool {
        let [top, bottom] = self.object_margins();
        self.placements.is_empty()
            && self.objects.iter().any(|object| !object.object.inline)
            && top > 0.0
            && bottom > 0.0
    }

    pub fn object_margins(&self) -> [f64; 2] {
        self.objects
            .iter()
            .fold([0.0_f64; 2], |margins, positioned| {
                [
                    margins[0].max(positioned.object.top_margin),
                    margins[1].max(positioned.object.bottom_margin),
                ]
            })
    }
}

struct JustificationEntry {
    entry: LineEntry,
    position: LinePosition,
    weight: u32,
    leading_weight: u32,
    rank: usize,
}

fn native_geometry(value: f64) -> Result<f32, MeasurementError> {
    super::finite_native_geometry(value)
        .map(|value| value as f32)
        .ok_or(MeasurementError::InvalidCluster)
}

fn justification_weight(scalar: char) -> u32 {
    match scalar {
        ' ' => 1,
        '\t' => 4,
        _ => 0,
    }
}

fn justify_entries(
    mut entries: Vec<JustificationEntry>,
    extra: f32,
    advance: f32,
    canonical: bool,
) -> Result<(Vec<(LineEntry, LinePosition)>, f32), MeasurementError> {
    if canonical {
        entries.sort_unstable_by_key(|entry| entry.rank);
    } else {
        entries.sort_by(|left, right| {
            left.position
                .x
                .total_cmp(&right.position.x)
                .then(left.rank.cmp(&right.rank))
        });
    }
    let mut shift = 0.0_f32;
    let mut positions = Vec::with_capacity(entries.len());
    for entry in entries {
        let x = native_geometry(f64::from(native_geometry(entry.position.x)? + shift))?;
        let extra_advance = native_geometry(f64::from(extra * entry.weight as f32))?;
        let glyph_offset_x = native_geometry(f64::from(
            native_geometry(entry.position.glyph_offset_x)? + extra * entry.leading_weight as f32,
        ))?;
        shift = native_geometry(f64::from(shift + extra_advance))?;
        positions.push((
            entry.entry,
            LinePosition {
                x: f64::from(x),
                glyph_offset_x: f64::from(glyph_offset_x),
                extra_advance: f64::from(extra_advance),
                visual_rank: entry.position.visual_rank,
            },
        ));
    }
    Ok((positions, native_geometry(f64::from(advance + shift))?))
}

fn measured_items(
    styled: &StyledText<'_>,
    range: Range<usize>,
    measurer: &ParagraphMeasurer<'_, '_, '_>,
    renderer: &TextRenderer<'_>,
    breaks: &mut ParagraphBreaks,
    object_context: ObjectMeasurementContext,
) -> Result<ParagraphItems, MeasurementError> {
    let paragraph_objects = styled.objects.in_range(range.clone());
    let mut items = Vec::new();
    let mut start = range.start;
    let mut font_size = 0.0_f64;
    for object in paragraph_objects {
        if start < object.source.start {
            let measured = measurer.measure_line(start..object.source.start)?;
            finite_advance(measured.advance)?;
            font_size = font_size.max(measured.font_size);
            items.extend(measured.clusters.into_iter().map(MeasuredItem::TextCluster));
        }
        let object = object.measured(renderer.settings, object_context);
        let object_font_size = measurer.font_size(object.source.clone())?;
        font_size = font_size.max(object_font_size);
        let kind = if object.inline {
            BreakKind::Allowed
        } else {
            BreakKind::Mandatory
        };
        for end in [object.source.start, object.source.end] {
            let end = end - range.start;
            if end != 0 {
                breaks.candidates.push(BreakCandidate { end, kind });
                breaks.emergency.push(end);
            }
        }
        start = object.source.end;
        items.push(MeasuredItem::Object {
            placement: PositionedObject {
                object,
                x: 0.0,
                visual_rank: 0,
                prepared: None,
            },
            font_size: object_font_size,
        });
    }
    if start < range.end {
        let measured = measurer.measure_line(start..range.end)?;
        finite_advance(measured.advance)?;
        font_size = font_size.max(measured.font_size);
        items.extend(measured.clusters.into_iter().map(MeasuredItem::TextCluster));
    }
    if !paragraph_objects.is_empty() {
        breaks.candidates.sort_unstable_by_key(|candidate| {
            (candidate.end, candidate.kind != BreakKind::Mandatory)
        });
        breaks.candidates.dedup_by_key(|candidate| candidate.end);
        breaks.emergency.sort_unstable();
        breaks.emergency.dedup();
    }
    Ok(ParagraphItems { items, font_size })
}

fn paragraph_prefix_font_size(
    styled: &StyledText<'_>,
    source: &Range<usize>,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
) -> f64 {
    if source.start > 0
        && matches!(
            styled.index.slice(source.start - 1..source.start),
            Some("\r" | "\n")
        )
    {
        styled.style_at(source.start, theme, predefined).font_size
    } else {
        0.0
    }
}

fn finite_advance(advance: f64) -> Result<f64, MeasurementError> {
    if advance.is_finite() {
        Ok(advance)
    } else {
        Err(MeasurementError::InvalidCluster)
    }
}

pub(in crate::render) fn unmeasured_paragraph(
    styled: &StyledText<'_>,
    source: Range<usize>,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
    renderer: &TextRenderer<'_>,
    object_context: ObjectMeasurementContext,
) -> Vec<WrappedLine> {
    let objects = styled.objects.in_range(source.clone());
    if objects.is_empty() {
        let mut line = WrappedLine::unmeasured(
            source.clone(),
            styled.line_font_size(source.clone(), theme, predefined),
        );
        line.fallback_measurement_issues = renderer.local_measurement_issues(source);
        return vec![line];
    }
    let mut lines = Vec::with_capacity(2 * objects.len() + 1);
    let mut start = source.start;
    for object in objects {
        if start < object.source.start {
            let text = start..object.source.start;
            lines.push(WrappedLine::unmeasured(
                text.clone(),
                styled.line_font_size(text, theme, predefined),
            ));
        }
        let measured = object.measured(renderer.settings, object_context);
        let mut line = WrappedLine::unmeasured(measured.source.clone(), 0.0);
        line.font_size = styled
            .style_at(measured.source.start, theme, predefined)
            .font_size;
        line.advance = measured.advance();
        let x = measured.left_margin;
        line.objects.push(PositionedObject {
            object: measured,
            x,
            visual_rank: 0,
            prepared: None,
        });
        lines.push(line);
        renderer.object_layout_unsupported(object.span.text_index_utf16);
        start = object.source.end;
    }
    if start < source.end {
        let text = start..source.end;
        lines.push(WrappedLine::unmeasured(
            text.clone(),
            styled.line_font_size(text, theme, predefined),
        ));
    }
    if let Some(line) = lines.first_mut() {
        let prefix_font_size = paragraph_prefix_font_size(styled, &source, theme, predefined);
        line.font_size = line.font_size.max(prefix_font_size);
    }
    for line in &mut lines {
        line.fallback_measurement_issues = renderer.local_measurement_issues(line.source.clone());
    }
    lines
}

pub(in crate::render) struct ParagraphWrapper<'a, 'text, 'fonts> {
    styled: &'a StyledText<'text>,
    measurer: ParagraphMeasurer<'a, 'text, 'fonts>,
    range: Range<usize>,
    items: Vec<MeasuredItem>,
    allowed: Vec<bool>,
    mandatory: Vec<bool>,
    emergency: Vec<bool>,
    first_font_size: f64,
    font_size: f64,
    full_width: f64,
    start_item: usize,
    native_slots: Option<NativeParagraphSlots>,
    renderer: &'a TextRenderer<'fonts>,
    native_slots_rejected: bool,
}

impl<'a, 'text, 'fonts> ParagraphWrapper<'a, 'text, 'fonts> {
    pub fn new(
        styled: &'a StyledText<'text>,
        range: Range<usize>,
        width: impl Into<ParagraphMeasurementWidth>,
        theme: RenderTheme,
        predefined: Option<PredefinedTextStyle>,
        renderer: &'a TextRenderer<'fonts>,
        object_context: ObjectMeasurementContext,
    ) -> Result<Self, MeasurementError> {
        let text = styled
            .index
            .slice(range.clone())
            .ok_or(MeasurementError::InvalidRange)?;
        let mut breaks = break_candidates(text);
        let measurer = ParagraphMeasurer::new(styled, range.clone(), theme, predefined, renderer)?;
        let measured = measured_items(
            styled,
            range.clone(),
            &measurer,
            renderer,
            &mut breaks,
            object_context,
        )?;
        let mut cluster_ends = vec![false; range.len() + 1];
        cluster_ends[0] = true;
        let mut source_end = range.start;
        let mut advance = 0.0;
        for item in &measured.items {
            let source = item.source();
            if source.start != source_end || source.end <= source_end || source.end > range.end {
                return Err(MeasurementError::InvalidCluster);
            }
            advance = finite_advance(advance + item.advance())?;
            cluster_ends[source.end - range.start] = true;
            source_end = source.end;
        }
        if source_end != range.end {
            return Err(MeasurementError::InvalidCluster);
        }
        let mut allowed = vec![false; range.len() + 1];
        let mut mandatory = vec![false; range.len() + 1];
        let mut emergency = vec![false; range.len() + 1];
        for candidate in breaks.candidates {
            if !cluster_ends[candidate.end] {
                if candidate.kind == BreakKind::Mandatory {
                    return Err(MeasurementError::InvalidCluster);
                }
                continue;
            }
            allowed[candidate.end] = true;
            mandatory[candidate.end] = candidate.kind == BreakKind::Mandatory;
        }
        for end in breaks.emergency {
            emergency[end] = cluster_ends[end];
        }
        let mut native_slots_rejected = false;
        let native_slots =
            match NativeParagraphSlots::new(styled, range.clone(), &measured.items, &allowed) {
                Ok(slots) => slots,
                Err(_) => {
                    native_slots_rejected = true;
                    renderer.for_source(range.clone()).measurement_unsupported(
                        super::TextDiagnosticKind::UnsupportedNativeWrapping,
                        "native",
                        text,
                    );
                    None
                }
            };
        let full_width = match width.into() {
            ParagraphMeasurementWidth::Constrained(width) => normalized_width(width),
            ParagraphMeasurementWidth::Automatic { insets } => native_slots
                .as_ref()
                .ok_or(MeasurementError::InvalidCluster)?
                .automatic_width(insets)?,
        };
        Ok(Self {
            styled,
            measurer,
            range: range.clone(),
            items: measured.items,
            allowed,
            mandatory,
            emergency,
            first_font_size: paragraph_prefix_font_size(styled, &range, theme, predefined),
            font_size: measured.font_size,
            full_width,
            start_item: 0,
            native_slots,
            renderer,
            native_slots_rejected,
        })
    }

    pub fn measurement_width(&self) -> f64 {
        self.full_width
    }

    pub fn remaining_source(&self) -> Range<usize> {
        self.items
            .get(self.start_item)
            .map_or(self.range.end, |item| item.source().start)..self.range.end
    }

    pub fn object_edge_margins(&self) -> [bool; 2] {
        let has_margins = |item: Option<&MeasuredItem>| matches!(item, Some(MeasuredItem::Object { placement, .. }) if placement.object.top_margin > 0.0 && placement.object.bottom_margin > 0.0);
        [
            has_margins(self.items.first()),
            has_margins(self.items.last()),
        ]
    }

    pub fn candidate(
        &mut self,
        width: f64,
        mut prepare: impl FnMut(&mut PositionedObject),
    ) -> Result<Option<WrappedLine>, MeasurementError> {
        if self.start_item == self.items.len() {
            return Ok(None);
        }
        let width = normalized_width(width);
        let mut unsupported_native_wrapping = self.native_slots_rejected;
        let mut prepared_objects = Vec::new();
        let mut prepare_once = |placement: &mut PositionedObject| {
            if !prepared_objects.contains(&placement.object.source.start) {
                prepared_objects.push(placement.object.source.start);
                prepare(placement);
            }
        };
        match NativeMixedSlots::new(self.styled, self.range.clone(), &self.items, &self.allowed) {
            Ok(Some(slots)) => {
                let start = self.items[self.start_item].source().start;
                let limit = self
                    .mandatory
                    .iter()
                    .enumerate()
                    .skip(start - self.range.start + 1)
                    .find(|(_, mandatory)| **mandatory)
                    .map_or(self.range.end, |(end, _)| self.range.start + end);
                if let Ok(Some(end)) = slots.select(
                    self.styled,
                    start..limit,
                    [width, self.full_width],
                    &mut self.items,
                    &mut prepare_once,
                ) {
                    let end_item = self.items.partition_point(|item| item.source().end <= end);
                    if end_item > self.start_item
                        && self.items[end_item - 1].source().end == end
                        && self.emergency[end - self.range.start]
                    {
                        return self
                            .make_line(end_item, unsupported_native_wrapping)
                            .map(Some);
                    }
                }
                self.report_native_wrap_policy(start..limit);
                unsupported_native_wrapping = true;
            }
            Err(_) => {
                self.report_native_wrap_policy(
                    self.items[self.start_item].source().start..self.range.end,
                );
                unsupported_native_wrapping = true;
            }
            Ok(None) => {}
        }
        if let Some(slots) = &self.native_slots {
            let start = self.items[self.start_item].source().start;
            let limit = self
                .mandatory
                .iter()
                .enumerate()
                .skip(start - self.range.start + 1)
                .find(|(_, mandatory)| **mandatory)
                .map_or(self.range.end, |(end, _)| self.range.start + end);
            if let Ok(Some(end)) = slots.select(self.styled, start..limit, width, self.full_width) {
                let end_item = self.items.partition_point(|item| item.source().end <= end);
                if end_item > self.start_item
                    && self.items[end_item - 1].source().end == end
                    && self.emergency[end - self.range.start]
                {
                    return self
                        .make_line(end_item, unsupported_native_wrapping)
                        .map(Some);
                }
            }
            self.report_native_wrap_policy(start..limit);
            unsupported_native_wrapping = true;
        }
        let mut advance = 0.0;
        let mut last_allowed = None;
        let mut last_emergency = None;
        let mut selected = None;
        for index in self.start_item..self.items.len() {
            let snapshot = self.items[index].advance();
            let old_sum = finite_advance(advance + snapshot)?;
            let first_object = index == self.start_item
                && width >= self.full_width
                && matches!(self.items[index], MeasuredItem::Object { .. });
            if old_sum > width
                && !first_object
                && (last_allowed.is_some() || last_emergency.is_some())
            {
                break;
            }
            if let MeasuredItem::Object { placement, .. } = &mut self.items[index]
                && (old_sum <= width || first_object)
            {
                placement.x = finite_advance(advance + placement.object.left_margin)?;
                prepare_once(placement);
            }
            advance = finite_advance(advance + self.items[index].advance())?;
            let end = self.items[index].source().end - self.range.start;
            if self.emergency[end] {
                last_emergency = Some(index + 1);
            }
            if self.allowed[end] {
                last_allowed = Some(index + 1);
            }
            if (self.mandatory[end] || old_sum > width) && self.emergency[end] {
                selected = Some(index + 1);
                break;
            }
        }
        let end_item = selected
            .or(last_allowed)
            .or(last_emergency)
            .ok_or(MeasurementError::InvalidCluster)?;
        self.make_line(end_item, unsupported_native_wrapping)
            .map(Some)
    }

    pub fn restore(&mut self, line: WrappedLine) {
        for placement in line.objects {
            let source = placement.object.source.start;
            let index = self
                .items
                .partition_point(|item| item.source().start < source);
            if let Some(MeasuredItem::Object {
                placement: stored, ..
            }) = self.items.get_mut(index)
            {
                *stored = placement;
            }
        }
    }

    fn report_native_wrap_policy(&self, source: Range<usize>) {
        let text = self.styled.index.slice(source.clone()).unwrap_or_default();
        self.renderer.for_source(source).measurement_unsupported(
            super::TextDiagnosticKind::UnsupportedNativeWrapping,
            "native",
            text,
        );
    }

    pub fn commit(&mut self, source_end: usize) {
        self.start_item = self
            .items
            .partition_point(|item| item.source().end <= source_end);
    }

    fn make_line(
        &mut self,
        end_item: usize,
        unsupported_native_wrapping: bool,
    ) -> Result<WrappedLine, MeasurementError> {
        let source =
            self.items[self.start_item].source().start..self.items[end_item - 1].source().end;
        let mut placements = Vec::new();
        let mut objects = Vec::new();
        let mut font_size = if self.start_item == 0 {
            self.first_font_size
        } else {
            0.0
        };
        let mut text_height = 0.0_f64;
        let mut x = 0.0;
        let mut logical_sources = Vec::new();
        let mut logical_entries = Vec::new();
        for item in &mut self.items[self.start_item..end_item] {
            logical_sources.push(item.source().clone());
            let advance = item.advance();
            match item {
                MeasuredItem::TextCluster(cluster) => {
                    font_size = font_size.max(cluster.run.style.font_size);
                    text_height = text_height.max(cluster.run.style.font_size);
                    logical_entries.push(LineEntry::Text(placements.len()));
                    placements.push(PositionedCluster {
                        cluster: cluster.clone(),
                        x,
                        extra_advance: 0.0,
                        visual_rank: logical_entries.len() - 1,
                    });
                }
                MeasuredItem::Object {
                    placement,
                    font_size: object_font_size,
                } => {
                    font_size = font_size.max(*object_font_size);
                    logical_entries.push(LineEntry::Object(objects.len()));
                    objects.push(PositionedObject {
                        object: placement.object.clone(),
                        x: finite_advance(x + placement.object.left_margin)?,
                        visual_rank: logical_entries.len() - 1,
                        prepared: placement.prepared.take(),
                    });
                }
            }
            x = finite_advance(x + advance)?;
        }
        let visual_order = self.measurer.visual_order(source.clone(), &logical_sources);
        if source == self.range {
            font_size = font_size.max(self.font_size);
        }
        let mut line = WrappedLine {
            source,
            font_size,
            text_height,
            advance: x,
            placements,
            objects,
            visual_order: visual_order
                .as_ref()
                .map(|order| order.iter().map(|&index| logical_entries[index]).collect())
                .unwrap_or_default(),
            native_positioned: false,
            geometry: LineGeometry::Unmeasured,
            native_slots: None,
            native_placed: None,
            native_mixed: false,
            unsupported_native_wrapping,
            fallback_measurement_issues: Vec::new(),
        };
        line.geometry = match visual_order {
            Ok(_) => line.canonical_geometry()?,
            Err(error) => LineGeometry::Unavailable(error),
        };
        if let Some(slots) = &self.native_slots {
            let scalars = line
                .source
                .clone()
                .map(|scalar| scalar..scalar + 1)
                .collect::<Vec<_>>();
            let positioned = self
                .measurer
                .visual_order(line.source.clone(), &scalars)
                .map_err(|_| MeasurementError::InvalidCluster)
                .and_then(|order| {
                    slots.line(
                        self.styled,
                        line.source.clone(),
                        &line.placements,
                        &line.visual_order,
                        &order,
                    )
                })
                .and_then(|slots| slots.positions(0.0).map(|positions| (slots, positions)));
            match positioned {
                Ok((slots, (text, advance))) => {
                    line.advance = slots.block_width();
                    line.geometry = LineGeometry::Positioned {
                        advance,
                        text,
                        objects: Vec::new(),
                    };
                    line.native_slots = Some(slots);
                }
                Err(_) => {
                    line.unsupported_native_wrapping = true;
                    self.report_native_wrap_policy(line.source.clone());
                }
            }
        }
        if let Ok(Some(slots)) =
            NativeMixedSlots::new(self.styled, self.range.clone(), &self.items, &self.allowed)
        {
            let scalars = line
                .source
                .clone()
                .map(|scalar| scalar..scalar + 1)
                .collect::<Vec<_>>();
            let positioned = self
                .measurer
                .visual_order(line.source.clone(), &scalars)
                .map_err(|_| MeasurementError::InvalidCluster)
                .and_then(|order| slots.positions(self.styled, &line, &order, &line.visual_order));
            match positioned {
                Ok(geometry) => {
                    line.advance = slots.block_width(self.styled, line.source.clone())?;
                    let logical_scalars = (0..line.source.len()).collect::<Vec<_>>();
                    let logical =
                        slots.positions(self.styled, &line, &logical_scalars, &logical_entries)?;
                    if let LineGeometry::Positioned { text, objects, .. } = logical {
                        for (placement, position) in line.placements.iter_mut().zip(text) {
                            placement.x = position.x;
                        }
                        for (placement, position) in line.objects.iter_mut().zip(objects) {
                            placement.x = position.x;
                        }
                    }
                    line.geometry = geometry;
                    line.native_mixed = true;
                }
                Err(_) => {
                    line.unsupported_native_wrapping = true;
                    self.report_native_wrap_policy(line.source.clone());
                }
            }
        }
        if matches!(line.geometry, LineGeometry::Positioned { .. }) && line.objects.is_empty() {
            line.position_native(self.styled)?;
        }
        Ok(line)
    }
}

fn normalized_width(width: f64) -> f64 {
    if width.is_nan() { 0.0 } else { width.max(0.0) }
}

pub(in crate::render) fn wrap_paragraph(
    styled: &StyledText<'_>,
    range: Range<usize>,
    max_width: f64,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
    renderer: &TextRenderer<'_>,
    object_context: ObjectMeasurementContext,
) -> Result<Vec<WrappedLine>, MeasurementError> {
    let mut paragraph = ParagraphWrapper::new(
        styled,
        range,
        max_width,
        theme,
        predefined,
        renderer,
        object_context,
    )?;
    let mut lines = Vec::new();
    while let Some(line) = paragraph.candidate(max_width, |_| {})? {
        paragraph.commit(line.source.end);
        lines.push(line);
    }
    Ok(lines)
}
#[cfg(test)]
mod tests {
    use super::super::{TextContext, TextSettings};
    use super::*;
    use crate::fonts::{FontBook, fontdb};
    use crate::{
        BoundingBox, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType, PlacedImage,
        RichTextBox, RichTextObjectContent, RichTextObjectSpan, RichTextSpan, RichTextSpanType,
    };
    use std::sync::Arc;

    pub(super) fn text(value: &str) -> RichTextBox {
        RichTextBox {
            text_area_type: None,
            bbox: BoundingBox::default(),
            rotation_degrees: None,
            text: value.into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: Some(45.0),
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: Vec::new(),
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        }
    }

    fn span(kind: RichTextSpanType, start: u32, end: u32, payload: &[u8]) -> RichTextSpan {
        RichTextSpan {
            kind,
            start_utf16: start,
            end_utf16: end,
            interval_type: crate::SpanIntervalType::from(0),
            payload: payload.into(),
        }
    }

    pub(super) fn image(
        anchor: i32,
        width: f64,
        option: ObjectSpanLayoutOption,
    ) -> RichTextObjectSpan {
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
            layout_option: option,
            layout_constraint: ObjectSpanLayoutConstraint::Normal,
        }
    }

    fn wrap_with_fonts(
        content: &RichTextBox,
        range: Range<usize>,
        width: f64,
        fonts: &FontBook,
    ) -> Result<Vec<WrappedLine>, MeasurementError> {
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
        };
        let styled = StyledText::new(content, TextContext::Placed, settings);
        let renderer = TextRenderer::new(settings, fonts);
        wrap_paragraph(
            &styled,
            range,
            width,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
            ObjectMeasurementContext::Frame,
        )
    }

    fn wrap(content: &RichTextBox, width: f64) -> Vec<WrappedLine> {
        wrap_with_fonts(
            content,
            0..content.text.chars().count(),
            width,
            &FontBook::default(),
        )
        .unwrap()
    }

    fn legacy_fonts() -> FontBook {
        FontBook::new(FontBook::default().database())
    }

    fn wrap_legacy(content: &RichTextBox, width: f64) -> Vec<WrappedLine> {
        wrap_with_fonts(
            content,
            0..content.text.chars().count(),
            width,
            &legacy_fonts(),
        )
        .unwrap()
    }

    fn arabic_fonts() -> FontBook {
        let mut database = fontdb::Database::new();
        database
            .load_font_data(include_bytes!("../../../tests/assets/fonts/DejaVuSans.ttf").to_vec());
        database.set_sans_serif_family("DejaVu Sans");
        FontBook::new(Arc::new(database))
    }

    #[test]
    fn lam_alef_has_canonical_geometry_without_standalone_svg_admission() {
        let content = text("لا");
        let line = wrap_with_fonts(&content, 0..2, 1000.0, &arabic_fonts())
            .unwrap()
            .remove(0);
        assert_eq!(line.source, 0..2);
        assert_eq!(line.placements.len(), 1);
        assert_eq!(line.placements[0].cluster.source, 0..2);
        assert_eq!(line.placements[0].cluster.advance, 25.6640625);
        assert!(!line.native_positioned);
        assert_eq!(line.advance_for_paint(true), Some(25.6640625));
        assert_eq!(line.text_position(0, true).unwrap().x, 0.0);
        assert!(matches!(line.geometry, LineGeometry::Positioned { .. }));
        assert!(
            line.placements[0]
                .cluster
                .native_paint_offset("لا")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn rtl_ligature_geometry_keeps_logical_source_and_svg_compatibility() {
        let content = text("لا אב");
        let line = wrap_with_fonts(&content, 0..5, 1000.0, &arabic_fonts())
            .unwrap()
            .remove(0);
        assert!(!line.native_positioned);
        assert_eq!(
            line.placements
                .iter()
                .map(|placement| placement.cluster.source.clone())
                .collect::<Vec<_>>(),
            [0..2, 2..3, 3..4, 4..5]
        );
        assert_eq!(
            line.visual_order,
            [
                LineEntry::Text(3),
                LineEntry::Text(2),
                LineEntry::Text(1),
                LineEntry::Text(0)
            ]
        );
        assert_eq!(
            (0..4)
                .map(|index| line.text_position(index, true).unwrap().x)
                .collect::<Vec<_>>(),
            [70.400390625, 56.09619140625, 26.015625, 0.0]
        );
        assert_eq!(
            (0..4)
                .map(|index| line.text_position(index, false).unwrap().x)
                .collect::<Vec<_>>(),
            [0.0, 25.6640625, 39.96826171875, 70.048828125]
        );
        assert_eq!(line.advance_for_paint(true), Some(96.064453125));
    }

    #[test]
    fn justification_positions_rtl_ligatures_without_replacing_svg_geometry() {
        let content = text("لا  אב");
        let styled = StyledText::new(&content, TextContext::Placed, TextSettings::default());
        let mut line = wrap_with_fonts(&content, 0..6, 1000.0, &arabic_fonts())
            .unwrap()
            .remove(0);
        assert_eq!(line.advance_for_paint(true), Some(110.36865234375));
        line.justify(&styled, 200.0).unwrap();
        assert!(!line.native_positioned);
        assert_eq!(line.source, 0..6);
        assert_eq!(line.advance_for_paint(true), Some(200.0));
        assert_eq!(line.advance_for_paint(false), Some(200.0));
        assert_eq!(
            (0..5)
                .map(|index| line.text_position(index, true).unwrap().x)
                .collect::<Vec<_>>(),
            [
                174.3359375,
                115.216064453125,
                56.09619140625,
                26.015625,
                0.0
            ]
        );
        assert_eq!(
            (0..5)
                .map(|index| line.text_position(index, false).unwrap().x)
                .collect::<Vec<_>>(),
            [
                0.0,
                25.6640625,
                84.783935546875,
                143.90380859375,
                173.984375
            ]
        );
        for index in [1, 2] {
            assert_eq!(
                line.text_position(index, true).unwrap().extra_advance,
                44.815673828125
            );
        }
        assert_eq!(line.placements[0].cluster.source, 0..2);
    }

    #[test]
    fn unsupported_paragraph_bidi_retains_its_geometry_reason() {
        let lines = wrap(&text("A\u{2029}ב"), 1000.0);
        assert_eq!(ranges(&lines), [0..2, 2..3]);
        for line in lines {
            assert!(matches!(
                line.geometry,
                LineGeometry::Unavailable(BidiError::UnsupportedParagraphSeparator)
            ));
            assert_eq!(line.advance_for_paint(true), None);
            assert!(line.text_position(0, true).is_none());
            assert!(line.text_position(0, false).is_some());
        }
    }

    fn paragraph_with_object<'a, 'text, 'fonts>(
        styled: &'a StyledText<'text>,
        renderer: &'a TextRenderer<'fonts>,
        width: f64,
    ) -> ParagraphWrapper<'a, 'text, 'fonts> {
        ParagraphWrapper::new(
            styled,
            0..styled.index.len(),
            width,
            RenderTheme::for_canvas(false),
            None,
            renderer,
            ObjectMeasurementContext::Body,
        )
        .unwrap()
    }

    #[test]
    fn object_admission_uses_the_old_width_and_downstream_text_uses_the_new_width() {
        let mut content = text("AA\u{fffc}B");
        content.font_size = Some(10.0);
        content.object_spans = vec![image(2, 21.0, ObjectSpanLayoutOption::Inline)];
        let settings = TextSettings::default();
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        let fonts = legacy_fonts();
        let renderer = TextRenderer::new(settings, &fonts);
        let mut paragraph = paragraph_with_object(&styled, &renderer, 70.0);
        let mut calls = 0;
        let line = paragraph
            .candidate(70.0, |placement| {
                calls += 1;
                placement.object.width = 61.0;
                placement.object.advance = 61.0;
            })
            .unwrap()
            .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(line.source, 0..3);
        assert_eq!(line.advance, 74.046875);
        assert_eq!(line.objects[0].x, 17.046875);
        paragraph.commit(line.source.end);
        let next = paragraph
            .candidate(70.0, |_| panic!("no object remains"))
            .unwrap()
            .unwrap();
        assert_eq!(next.source, 3..4);
        assert_eq!(next.advance, 6.2255859375);
    }

    #[test]
    fn obstacle_retry_retains_the_mutated_object_width_before_readmission() {
        let mut content = text("AA\u{fffc}");
        content.font_size = Some(10.0);
        content.object_spans = vec![image(2, 21.0, ObjectSpanLayoutOption::Inline)];
        let settings = TextSettings::default();
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let mut paragraph = paragraph_with_object(&styled, &renderer, 70.0);
        let first = paragraph
            .candidate(70.0, |placement| {
                placement.object.width = 61.0;
                placement.object.advance = 61.0;
            })
            .unwrap()
            .unwrap();
        assert_eq!(first.source, 0..3);
        paragraph.restore(first);
        let retried = paragraph
            .candidate(70.0, |_| panic!("old overflow skips callback"))
            .unwrap()
            .unwrap();
        assert_eq!(retried.source, 0..2);
        paragraph.commit(retried.source.end);
        let object = paragraph.candidate(70.0, |_| {}).unwrap().unwrap();
        assert_eq!(object.source, 2..3);
        assert_eq!(object.advance, 61.0);
        assert_eq!(object.objects[0].x, 4.0);
    }

    #[test]
    fn ordinary_object_overflow_skips_callback_but_first_object_overflow_invokes_it() {
        let mut content = text("A\u{fffc}B");
        content.font_size = Some(10.0);
        content.object_spans = vec![image(1, 40.0, ObjectSpanLayoutOption::Inline)];
        let settings = TextSettings::default();
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let mut paragraph = paragraph_with_object(&styled, &renderer, 20.0);
        let first = paragraph
            .candidate(20.0, |_| panic!("ordinary old overflow skips callback"))
            .unwrap()
            .unwrap();
        assert_eq!(first.source, 0..1);
        paragraph.commit(first.source.end);
        let mut calls = 0;
        let object = paragraph
            .candidate(20.0, |placement| {
                calls += 1;
                placement.object.width = 61.0;
                placement.object.advance = 61.0;
            })
            .unwrap()
            .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(object.source, 1..2);
        assert_eq!(object.advance, 61.0);
    }

    #[test]
    fn shrinking_an_admitted_object_allows_following_text_on_the_same_line() {
        let mut content = text("A\u{fffc}B");
        content.font_size = Some(10.0);
        content.object_spans = vec![image(1, 40.0, ObjectSpanLayoutOption::Inline)];
        let settings = TextSettings::default();
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        let fonts = legacy_fonts();
        let renderer = TextRenderer::new(settings, &fonts);
        let mut paragraph = paragraph_with_object(&styled, &renderer, 55.0);
        let line = paragraph
            .candidate(55.0, |placement| {
                placement.object.width = 10.0;
                placement.object.advance = 10.0;
            })
            .unwrap()
            .unwrap();
        assert_eq!(line.source, 0..3);
        assert_eq!(line.advance, 22.7490234375);
        assert_eq!(line.placements[1].x, 16.5234375);
    }

    #[test]
    fn body_inline_margins_position_content_and_change_wrap_opportunities() {
        let mut content = text("A\u{fffc}B");
        content
            .object_spans
            .push(image(1, 40.0, ObjectSpanLayoutOption::Inline));
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
        };
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        let measure = |width, context| {
            wrap_paragraph(
                &styled,
                0..3,
                width,
                RenderTheme::for_canvas(false),
                None,
                &renderer,
                context,
            )
            .unwrap()
        };
        let frame = measure(f64::INFINITY, ObjectMeasurementContext::Frame);
        let body = measure(f64::INFINITY, ObjectMeasurementContext::Body);
        assert_eq!(frame[0].objects[0].x, 29.35546875);
        assert_eq!(body[0].objects[0].x, 33.35546875);
        assert_eq!(frame[0].placements[1].x, 69.35546875);
        assert_eq!(body[0].placements[1].x, 77.35546875);
        assert_eq!(body[0].advance - frame[0].advance, 8.0);
        assert_eq!(
            ranges(&measure(77.0, ObjectMeasurementContext::Frame)),
            [0..2, 2..3]
        );
        assert_eq!(
            ranges(&measure(77.0, ObjectMeasurementContext::Body)),
            [0..1, 1..3]
        );
        let fallback = unmeasured_paragraph(
            &styled,
            0..3,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
            ObjectMeasurementContext::Body,
        );
        assert_eq!(ranges(&fallback), [0..1, 1..2, 2..3]);
        assert_eq!(fallback[1].advance, 48.0);
        assert_eq!(fallback[1].objects[0].x, 4.0);
    }

    #[test]
    fn native_override_positions_keep_placements_in_logical_source_order() {
        let lines = wrap(&text("A\u{202e}BC\u{202c}D"), f64::INFINITY);
        let line = &lines[0];
        assert!(line.native_positioned);
        assert_eq!(
            line.placements
                .iter()
                .map(|placement| placement.cluster.source.clone())
                .collect::<Vec<_>>(),
            [0..1, 1..2, 2..3, 3..4, 4..5, 5..6]
        );
        assert_eq!(
            line.visual_order,
            [
                LineEntry::Text(0),
                LineEntry::Text(3),
                LineEntry::Text(2),
                LineEntry::Text(1),
                LineEntry::Text(4),
                LineEntry::Text(5)
            ]
        );
        assert_eq!(
            line.visual_order
                .iter()
                .filter_map(|entry| match entry {
                    LineEntry::Text(index) => Some(line.placements[*index].cluster.source.start),
                    LineEntry::Object(_) => None,
                })
                .collect::<Vec<_>>(),
            [0, 3, 2, 1, 4, 5]
        );
        assert_eq!(line.placements[3].x, 29.35546875);
        assert!(line.placements[2].x > line.placements[3].x);
        assert!(line.placements[5].x > line.placements[2].x);
    }

    #[test]
    fn unsafe_native_transport_keeps_legacy_geometry_and_the_visual_map() {
        for source in [
            "A\u{202e}(BC)\u{202c}D",
            "A\u{202e}e\u{301}\u{202c}D",
            "A\u{2029}BC",
        ] {
            let line = wrap(&text(source), f64::INFINITY).remove(0);
            assert!(!line.native_positioned, "{source:?}");
            assert!(
                line.placements
                    .windows(2)
                    .all(|pair| pair[0].x <= pair[1].x)
            );
            assert_eq!(line.visual_order.is_empty(), source.contains('\u{2029}'));
        }
    }

    #[test]
    fn native_visual_map_includes_objects_without_reordering_source_arrays() {
        let mut content = text("A\u{202e}B\u{fffc}C\u{202c}D");
        content
            .object_spans
            .push(image(3, 20.0, ObjectSpanLayoutOption::Inline));
        let mut line = wrap_legacy(&content, f64::INFINITY).remove(0);
        assert!(!line.native_positioned);
        assert_eq!(line.objects[0].object.source, 3..4);
        assert_eq!(
            line.visual_order,
            [
                LineEntry::Text(0),
                LineEntry::Text(3),
                LineEntry::Object(0),
                LineEntry::Text(2),
                LineEntry::Text(1),
                LineEntry::Text(4),
                LineEntry::Text(5)
            ]
        );
        assert_eq!(
            line.placements
                .iter()
                .map(|placement| placement.cluster.source.start)
                .collect::<Vec<_>>(),
            [0, 1, 2, 4, 5, 6]
        );
        let styled = StyledText::new(&content, TextContext::Placed, TextSettings::default());
        line.position_native(&styled).unwrap();
        assert!(line.native_positioned);
        assert_eq!(line.objects[0].x, 58.64501953125);
        assert_eq!(line.objects[0].visual_rank, 2);
        assert_eq!(line.placements[2].x, 78.64501953125);
        assert_eq!(line.placements[3].x, 29.35546875);
        assert_eq!(line.objects[0].object.source, 3..4);
    }

    #[test]
    fn failed_child_preparation_keeps_the_entire_line_in_logical_geometry() {
        let mut content = text("A\u{202e}B\u{fffc}C\u{202c}D");
        content
            .object_spans
            .push(image(3, 20.0, ObjectSpanLayoutOption::Inline));
        let mut line = wrap_legacy(&content, f64::INFINITY).remove(0);
        let positions = line.placements.iter().map(|p| p.x).collect::<Vec<_>>();
        let object_x = line.objects[0].x;
        line.objects[0].prepared = Some(Err(super::super::ObjectDiagnosticKind::InvalidBounds));
        let styled = StyledText::new(&content, TextContext::Placed, TextSettings::default());
        line.position_native(&styled).unwrap();
        assert!(!line.native_positioned);
        assert_eq!(line.objects[0].x, object_x);
        assert_eq!(line.object_position(0, true).unwrap().x, 58.64501953125);
        assert_eq!(line.text_position(3, true).unwrap().x, 29.35546875);
        assert_eq!(
            line.placements.iter().map(|p| p.x).collect::<Vec<_>>(),
            positions
        );
    }

    #[test]
    fn native_justification_uses_visual_rank_for_zero_advance_control_ties() {
        let content = text("A\u{202e}B\u{202a} \u{202c}D\u{202c}Z");
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
        };
        let styled = StyledText::new(&content, TextContext::Placed, settings);
        let mut line = wrap(&content, f64::INFINITY).remove(0);
        assert!(line.native_positioned);
        assert_eq!(line.placements[5].x, line.placements[3].x);
        assert_eq!(line.placements[3].x, line.placements[4].x);
        line.justify(&styled, line.advance + 20.0).unwrap();
        assert_eq!(line.placements[5].x, line.placements[3].x);
        assert_eq!(line.placements[3].x, line.placements[4].x);
        assert_eq!(line.placements[4].extra_advance, 20.0);
        assert!(line.placements[2].x > line.placements[4].x);
    }

    #[test]
    fn justification_compresses_negative_residuals_without_clamping() {
        let content = text("A\t B");
        let settings = TextSettings {
            scale: 1.0,
            ..Default::default()
        };
        let styled = StyledText::new(&content, TextContext::Placed, settings);
        let mut lines = wrap_legacy(&content, 1000.0);
        lines[0].justify(&styled, 50.0).unwrap();
        assert_eq!(lines[0].advance, 50.0);
        assert_eq!(lines[0].placements[1].extra_advance, -50.45703125);
        assert_eq!(lines[0].placements[2].extra_advance, -12.6142578125);
        assert_eq!(lines[0].placements[3].x, 21.98486328125);
    }

    #[test]
    fn justification_moves_inline_objects_with_the_surrounding_text() {
        let mut content = text("A \u{fffc} B");
        content.object_spans = vec![image(2, 40.0, ObjectSpanLayoutOption::Inline)];
        let settings = TextSettings {
            scale: 1.0,
            ..Default::default()
        };
        let styled = StyledText::new(&content, TextContext::Placed, settings);
        let mut lines = wrap_legacy(&content, 1000.0);
        lines[0].justify(&styled, 200.0).unwrap();
        assert_eq!(lines[0].advance, 200.0);
        assert_eq!(lines[0].objects[0].x, 80.670166015625);
        assert_eq!(lines[0].placements.last().unwrap().x, 171.98486328125);
        assert_eq!(lines[0].objects[0].object.source, 2..3);
        assert_eq!(lines[0].placements.last().unwrap().cluster.source, 4..5);
    }

    #[test]
    fn invalid_justification_geometry_does_not_mutate_retained_positions() {
        let content = text("A B");
        let settings = TextSettings {
            scale: 1.0,
            ..Default::default()
        };
        let styled = StyledText::new(&content, TextContext::Placed, settings);
        let mut lines = wrap(&content, 1000.0);
        let advance = lines[0].advance;
        let canonical_positions: Vec<_> = (0..lines[0].placements.len())
            .map(|index| lines[0].text_position(index, true))
            .collect();
        let positions: Vec<_> = lines[0]
            .placements
            .iter()
            .map(|placement| placement.x)
            .collect();
        for width in [f64::NAN, f64::INFINITY, f64::MAX] {
            assert!(lines[0].justify(&styled, width).is_err());
            assert_eq!(lines[0].advance, advance);
            assert_eq!(
                (0..lines[0].placements.len())
                    .map(|index| lines[0].text_position(index, true))
                    .collect::<Vec<_>>(),
                canonical_positions
            );
            assert_eq!(
                lines[0]
                    .placements
                    .iter()
                    .map(|placement| placement.x)
                    .collect::<Vec<_>>(),
                positions
            );
            assert!(
                lines[0]
                    .placements
                    .iter()
                    .all(|placement| placement.extra_advance == 0.0)
            );
        }
    }

    fn ranges(lines: &[WrappedLine]) -> Vec<Range<usize>> {
        lines.iter().map(|line| line.source.clone()).collect()
    }

    fn assert_single_line(lines: &[WrappedLine], expected: Range<usize>) {
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].source, expected);
    }

    #[test]
    fn inline_object_uses_actual_width_and_positions_surrounding_text() {
        let mut content = text("A\u{fffc}A");
        content.object_spans = vec![image(1, 40.0, ObjectSpanLayoutOption::Inline)];
        content.spans = vec![span(
            RichTextSpanType::FontSize,
            1,
            2,
            &200.0_f32.to_le_bytes(),
        )];
        let letter_width = wrap(&text("A"), 1000.0)[0].advance;
        let lines = wrap(&content, 2.0 * letter_width + 40.0);
        assert_single_line(&lines, 0..3);
        let line = &lines[0];
        assert_eq!(line.advance, 2.0 * letter_width + 40.0);
        assert_eq!(line.font_size, 200.0);
        assert_eq!(line.text_height, 45.0);
        assert_eq!(line.base_height(), 60.0);
        assert_eq!(line.object_height(), 60.0);
        assert_eq!(line.object_margins(), [0.0, 0.0]);
        assert_eq!(line.placements.len(), 2);
        assert_eq!(line.placements[0].cluster.source, 0..1);
        assert_eq!(line.placements[1].cluster.source, 2..3);
        assert_eq!(line.placements[1].x, letter_width + 40.0);
        assert_eq!(line.objects.len(), 1);
        assert_eq!(line.objects[0].object.source, 1..2);
        assert_eq!(line.objects[0].object.span_index, 0);
        assert_eq!(line.objects[0].x, letter_width);
        assert_single_line(&wrap(&content, line.advance - 0.000001), 0..3);
        assert_eq!(
            ranges(&wrap(
                &content,
                f64::from((line.advance as f32).next_down())
            )),
            [0..2, 2..3]
        );
    }

    #[test]
    fn block_objects_force_own_lines_with_actual_alignment_width() {
        for (option, margin) in [
            (ObjectSpanLayoutOption::Block, 0.0),
            (ObjectSpanLayoutOption::BlockWithSmallMargin, 10.0),
            (ObjectSpanLayoutOption::BlockWithMediumMargin, 20.0),
            (ObjectSpanLayoutOption::Other(9), 0.0),
        ] {
            let mut content = text("A\u{fffc}B");
            content.object_spans = vec![image(1, 20.0, option)];
            let lines = wrap(&content, 1000.0);
            assert_eq!(ranges(&lines), [0..1, 1..2, 2..3]);
            assert!(lines[0].objects.is_empty());
            assert!(lines[2].objects.is_empty());
            assert_eq!(lines[1].advance, 20.0);
            assert_eq!(lines[1].font_size, 45.0);
            assert_eq!(lines[1].text_height, 0.0);
            assert!(lines[1].placements.is_empty());
            assert_eq!(lines[1].objects[0].x, 0.0);
            assert_eq!(lines[1].object_margins(), [margin, margin]);
        }
    }

    #[test]
    fn object_only_paragraph_uses_source_font_metric_without_resolving_a_face() {
        let mut content = text("\u{fffc}");
        content.font_size = Some(500.0);
        content.object_spans = vec![image(0, 200.0, ObjectSpanLayoutOption::Inline)];
        let fonts = FontBook::new(Arc::new(fontdb::Database::new()));
        let lines = wrap_with_fonts(&content, 0..1, 0.0, &fonts).unwrap();
        assert_single_line(&lines, 0..1);
        assert_eq!(lines[0].font_size, 500.0);
        assert_eq!(lines[0].text_height, 0.0);
        assert_eq!(lines[0].base_height(), 60.0);
        assert_eq!(lines[0].advance, 200.0);
        assert_eq!(lines[0].object_height(), 60.0);
        assert!(lines[0].placements.is_empty());
    }

    #[test]
    fn preceding_native_separator_seeds_only_the_first_object_line_font_size() {
        for separator in ["\r", "\n"] {
            let mut content = text(&format!("A{separator}\u{fffc}\u{fffc}"));
            content.object_spans = vec![
                image(2, 200.0, ObjectSpanLayoutOption::Inline),
                image(3, 300.0, ObjectSpanLayoutOption::Inline),
            ];
            content.spans = vec![span(
                RichTextSpanType::FontSize,
                2,
                3,
                &90.0_f32.to_le_bytes(),
            )];
            let fonts = FontBook::new(Arc::new(fontdb::Database::new()));
            let lines = wrap_with_fonts(&content, 2..4, 0.0, &fonts).unwrap();
            assert_eq!(ranges(&lines), [2..3, 3..4]);
            assert_eq!(lines[0].font_size, 90.0);
            assert_eq!(lines[1].font_size, 45.0);
            assert!(lines.iter().all(|line| line.text_height == 0.0));
            assert!(lines.iter().all(|line| line.base_height() == 60.0));
            assert!(lines.iter().all(|line| line.placements.is_empty()));
        }
        let mut content = text("A\u{fffc}");
        content.object_spans = vec![image(1, 200.0, ObjectSpanLayoutOption::Inline)];
        let lines = wrap_with_fonts(&content, 1..2, 0.0, &FontBook::default()).unwrap();
        assert_eq!(lines[0].font_size, 45.0);
    }

    #[test]
    fn a_leading_separator_retains_font_metric_but_its_height_is_replaced_by_content() {
        let mut content = text("\n\u{fffc}");
        let mut object = image(1, 20.0, ObjectSpanLayoutOption::Inline);
        let Some(RichTextObjectContent::Image(image)) = &mut object.content else {
            unreachable!()
        };
        image.bbox.y_max = image.bbox.y_min + 20.0;
        content.object_spans.push(object);
        content.spans.push(span(
            RichTextSpanType::FontSize,
            1,
            2,
            &900.0_f32.to_le_bytes(),
        ));
        let lines = wrap_with_fonts(&content, 1..2, 100.0, &FontBook::default()).unwrap();
        assert_eq!(lines[0].font_size, 900.0);
        assert_eq!(lines[0].text_height, 0.0);
        assert_eq!(lines[0].base_height(), 20.0);
    }

    #[test]
    fn oversized_inline_objects_progress_without_losing_neighboring_source() {
        let mut content = text("A\u{fffc}\u{fffc}B");
        content.object_spans = vec![
            image(1, 200.0, ObjectSpanLayoutOption::Inline),
            image(2, 300.0, ObjectSpanLayoutOption::Inline),
        ];
        let lines = wrap(&content, 0.0);
        assert_eq!(ranges(&lines), [0..1, 1..2, 2..3, 3..4]);
        assert_eq!(lines[1].objects[0].object.span_index, 0);
        assert_eq!(lines[2].objects[0].object.span_index, 1);
        assert_eq!(lines[1].advance, 200.0);
        assert_eq!(lines[2].advance, 300.0);
    }

    #[test]
    fn objects_preserve_full_paragraph_direction_context() {
        let mut content = text("ب\u{fffc}.");
        content.object_spans = vec![image(1, 20.0, ObjectSpanLayoutOption::Inline)];
        let lines = wrap(&content, f64::INFINITY);
        assert_single_line(&lines, 0..3);
        assert_eq!(lines[0].placements.len(), 2);
        let punctuation = &lines[0].placements[1].cluster;
        assert_eq!(punctuation.source, 2..3);
        assert_eq!(
            punctuation.run.direction,
            crate::fonts::Direction::RightToLeft
        );
    }

    #[test]
    fn mixed_source_uses_utf16_anchors_and_grapheme_emergency_boundaries() {
        let mut content = text("😀\u{fffc}e\u{301}");
        content.object_spans = vec![image(2, 200.0, ObjectSpanLayoutOption::Inline)];
        let lines = wrap(&content, 0.0);
        assert_eq!(ranges(&lines), [0..1, 1..2, 2..4]);
        assert_eq!(lines[1].objects[0].object.source, 1..2);
        assert_eq!(lines[2].placements[0].cluster.source, 2..4);
    }

    #[test]
    fn text_on_each_side_of_objects_retains_cached_cross_line_kerning() {
        let mut content = text("AVA\u{fffc}AVA");
        content.object_spans = vec![image(3, 20.0, ObjectSpanLayoutOption::Block)];
        let lines = wrap_legacy(&content, 55.0);
        assert_eq!(ranges(&lines), [0..2, 2..3, 3..4, 4..6, 6..7]);
        for (first, second) in [(&lines[0], &lines[1]), (&lines[3], &lines[4])] {
            assert_eq!(first.advance, 54.42626953125);
            assert_eq!(second.advance, 29.35546875);
            assert!(Arc::ptr_eq(
                &first.placements[0].cluster.run,
                &second.placements[0].cluster.run,
            ));
        }
    }

    #[test]
    fn unavailable_font_fallback_retains_objects_and_source_without_width_estimates() {
        let mut content = text("A\u{fffc}V\u{fffc}B");
        content.object_spans = vec![
            image(1, 40.0, ObjectSpanLayoutOption::Inline),
            image(3, 50.0, ObjectSpanLayoutOption::BlockWithSmallMargin),
        ];
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
        };
        let styled = StyledText::new(&content, TextContext::Placed, settings);
        let fonts = FontBook::new(Arc::new(fontdb::Database::new()));
        let renderer = TextRenderer::new(settings, &fonts);
        let lines = unmeasured_paragraph(
            &styled,
            0..5,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
            ObjectMeasurementContext::Frame,
        );
        assert_eq!(ranges(&lines), [0..1, 1..2, 2..3, 3..4, 4..5]);
        assert_eq!(lines[1].advance, 40.0);
        assert_eq!(lines[3].advance, 50.0);
        assert_eq!(lines[3].object_margins(), [10.0, 10.0]);
        assert!(lines.iter().all(|line| line.placements.is_empty()));
        for line in [&lines[0], &lines[2], &lines[4]] {
            assert_eq!(line.advance, 0.0);
            assert_eq!(line.font_size, 45.0);
            assert!(line.objects.is_empty());
        }
        assert_eq!(lines[1].objects[0].object.source, 1..2);
        assert_eq!(lines[3].objects[0].object.source, 3..4);
        assert!(renderer.diagnostics().is_empty());
        let issues = renderer.object_diagnostics();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].anchor_utf16, 1);
        assert_eq!(issues[1].anchor_utf16, 3);
        assert!(issues.iter().all(|issue| {
            issue.kind == super::super::objects::ObjectDiagnosticKind::MixedParagraphLayout
        }));
    }

    #[test]
    fn advance_arithmetic_overflow_returns_error_without_clamping_finite_values() {
        let maximum = f64::MAX;
        let summed = maximum + maximum;
        let difference = maximum - -maximum;
        for overflow in [summed, difference, summed - difference] {
            assert!(matches!(
                finite_advance(overflow),
                Err(MeasurementError::InvalidCluster)
            ));
        }
        for advance in [-maximum, -45.0, 0.0, 45.0, maximum] {
            assert_eq!(finite_advance(advance).unwrap(), advance);
        }
    }

    #[test]
    fn wrapped_placements_keep_shared_paragraph_glyphs_and_kerning_positions() {
        let lines = wrap_legacy(&text("AVA"), 55.0);
        assert_eq!(ranges(&lines), [0..2, 2..3]);
        assert_eq!(lines[0].advance, 54.42626953125);
        assert_eq!(lines[1].advance, 29.35546875);
        assert_eq!(lines[0].placements.len(), 2);
        assert_eq!(lines[1].placements.len(), 1);
        assert_eq!(lines[0].placements[0].x, 0.0);
        assert_eq!(lines[0].placements[1].x, 27.44384765625);
        assert_eq!(lines[1].placements[0].x, 0.0);
        let run = &lines[0].placements[0].cluster.run;
        assert!(Arc::ptr_eq(run, &lines[0].placements[1].cluster.run));
        assert!(Arc::ptr_eq(run, &lines[1].placements[0].cluster.run));
        assert_eq!(run.glyphs[1].transport_advance[0], 26.982421875);
        for (placement, character) in lines
            .iter()
            .flat_map(|line| &line.placements)
            .zip("AVA".chars())
        {
            let offset = placement
                .cluster
                .paint_offset(&character.to_string())
                .unwrap()
                .unwrap();
            assert_eq!(offset.x, 0.0);
            assert_eq!(offset.y, 0.0);
        }
    }

    #[test]
    fn raw_font_abc_fits_exact_f64_advance_but_wraps_below_it() {
        let content = text("ABC");
        let exact = wrap_legacy(&content, 86.66015625);
        assert_single_line(&exact, 0..3);
        assert_eq!(exact[0].font_size, 45.0);
        assert!(!exact[0].unsupported_native_wrapping());
        assert_eq!(
            ranges(&wrap_legacy(&content, 86.66015625 - 0.00000001)),
            [0..2, 2..3]
        );
    }

    #[test]
    fn unmeasured_lines_keep_only_their_local_measurement_rejections() {
        let mut content = text("A\u{fffc}B");
        content.object_spans = vec![image(1, 20.0, ObjectSpanLayoutOption::Inline)];
        let settings = TextSettings::resolved();
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts).local_measurement_scope();
        renderer.for_source(0..1).measurement_unsupported(
            super::super::TextDiagnosticKind::UnsupportedMeasurementFont,
            "missing-A",
            "A",
        );
        renderer.for_source(2..3).measurement_unsupported(
            super::super::TextDiagnosticKind::UnsupportedMeasurementStyle,
            "missing-B",
            "B",
        );
        let lines = unmeasured_paragraph(
            &styled,
            0..3,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
            ObjectMeasurementContext::Frame,
        );
        assert_eq!(ranges(&lines), [0..1, 1..2, 2..3]);
        let first = lines[0].fallback_measurement_issues();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].owner, Some(super::super::SourceOwner::Text(0..1)));
        assert_eq!(first[0].diagnostic.family, "missing-A");
        assert!(lines[1].fallback_measurement_issues().is_empty());
        let last = lines[2].fallback_measurement_issues();
        assert_eq!(last.len(), 1);
        assert_eq!(last[0].owner, Some(super::super::SourceOwner::Text(2..3)));
        assert_eq!(last[0].diagnostic.family, "missing-B");
        renderer.for_source(0..3).measurement_unsupported(
            super::super::TextDiagnosticKind::UnsupportedMeasurementBudget,
            "later",
            "AB",
        );
        assert_eq!(lines[0].fallback_measurement_issues().len(), 1);
        assert_eq!(lines[2].fallback_measurement_issues().len(), 1);
    }

    #[test]
    fn paragraph_kerning_is_retained_across_emergency_line_breaks() {
        let face = rustybuzz::Face::from_slice(
            include_bytes!("../../../assets/fonts/Roboto-Regular.ttf"),
            0,
        )
        .unwrap();
        let upstream_advances = |value: &str| {
            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(value);
            buffer.guess_segment_properties();
            rustybuzz::shape(&face, &[], buffer)
                .glyph_positions()
                .iter()
                .map(|position| position.x_advance)
                .collect::<Vec<_>>()
        };
        let paragraph_advances = upstream_advances("AVA");
        let isolated_advances = upstream_advances("AV");
        assert_eq!(paragraph_advances, [1249, 1228, 1336]);
        assert_eq!(isolated_advances, [1249, 1303]);
        assert_eq!(
            f64::from(paragraph_advances[..2].iter().sum::<i32>()) / 2048.0 * 45.0,
            54.42626953125
        );
        assert_eq!(
            f64::from(isolated_advances.iter().sum::<i32>()) / 2048.0 * 45.0,
            56.07421875
        );
        assert_eq!(ranges(&wrap(&text("AVA"), 55.0)), [0..2, 2..3]);
        assert_eq!(ranges(&wrap(&text("AVAV"), 56.0)), [0..2, 2..3, 3..4]);
    }

    #[test]
    fn line_maximum_font_size_excludes_neighboring_style_ranges() {
        let mut content = text("AAA");
        content.spans = vec![span(
            RichTextSpanType::FontSize,
            1,
            2,
            &90.0_f32.to_le_bytes(),
        )];
        let lines = wrap(&content, 60.0);
        assert_eq!(ranges(&lines), [0..1, 1..2, 2..3]);
        assert_eq!(
            lines.iter().map(|line| line.font_size).collect::<Vec<_>>(),
            [45.0, 90.0, 45.0]
        );
    }

    #[test]
    fn arabic_combining_graphemes_preserve_contiguous_logical_source_ranges() {
        let content = text("Aب\u{64e}ت\u{64e}Z");
        let lines = wrap(&content, 0.0);
        assert_eq!(ranges(&lines), [0..1, 1..3, 3..5, 5..6]);
    }

    #[test]
    fn long_unbroken_token_retains_every_scalar_in_expected_line_ranges() {
        let content = text(&"A".repeat(4096));
        let lines = wrap(&content, 60.0);
        assert_eq!(lines.len(), 2048);
        for (index, line) in lines.iter().enumerate() {
            assert_eq!(line.source, index * 2..index * 2 + 2);
            assert_eq!(line.font_size, 45.0);
        }
    }

    #[test]
    fn local_font_size_and_selected_styles_change_breaks() {
        let content = text("ABC");
        assert_single_line(&wrap_legacy(&content, 87.0), 0..3);
        let mut larger = content.clone();
        larger.spans = vec![span(
            RichTextSpanType::FontSize,
            1,
            2,
            &90.0_f32.to_le_bytes(),
        )];
        let lines = wrap_legacy(&larger, 87.0);
        assert_eq!(ranges(&lines), [0..2, 2..3]);
        assert_eq!(
            lines.iter().map(|line| line.font_size).collect::<Vec<_>>(),
            [90.0, 45.0]
        );
        let mut bold = content.clone();
        bold.spans = vec![span(RichTextSpanType::Bold, 0, 3, &[1, 0])];
        assert_eq!(ranges(&wrap_legacy(&bold, 87.0)), [0..2, 2..3]);
        let mut italic = content.clone();
        italic.spans = vec![span(RichTextSpanType::Italic, 0, 3, &[1, 0])];
        assert_eq!(ranges(&wrap_legacy(&content, 85.0)), [0..2, 2..3]);
        assert_single_line(&wrap_legacy(&italic, 85.0), 0..3);
    }

    #[test]
    fn wrapped_ranges_preserve_all_leading_repeated_and_trailing_spaces() {
        let content = text(" A  B   C  ");
        let lines = wrap(&content, 50.0);
        assert!(lines.len() > 1);
        assert_eq!(lines.first().unwrap().source.start, 0);
        assert_eq!(lines.last().unwrap().source.end, 11);
        for pair in lines.windows(2) {
            assert_eq!(pair[0].source.end, pair[1].source.start);
        }
        let retained = lines
            .iter()
            .map(|line| {
                content
                    .text
                    .chars()
                    .skip(line.source.start)
                    .take(line.source.len())
                    .collect::<String>()
            })
            .collect::<String>();
        assert_eq!(retained, " A  B   C  ");
    }

    #[test]
    fn emergency_breaks_keep_combining_and_emoji_clusters_with_scalar_offsets() {
        let content = text("Ae\u{301}👩‍👩‍👧‍👦Z");
        let lines = wrap_with_fonts(&content, 1..10, 0.0, &FontBook::default()).unwrap();
        assert_eq!(ranges(&lines), [1..3, 3..10]);
    }

    #[test]
    fn nonbreaking_space_is_only_broken_when_the_whole_group_overflows() {
        let content = text("A\u{a0}B");
        assert_single_line(&wrap(&content, 80.0), 0..3);
        assert_eq!(ranges(&wrap(&content, 60.0)), [0..2, 2..3]);
    }

    #[test]
    fn crlf_mandatory_break_is_retained_as_one_source_boundary() {
        let content = text("A\r\nB");
        assert_eq!(ranges(&wrap(&content, f64::INFINITY)), [0..3, 3..4]);
    }

    #[test]
    fn empty_text_and_oversized_graphemes_terminate_without_dropping_source() {
        assert!(wrap(&text(""), 0.0).is_empty());
        assert_single_line(&wrap(&text("e\u{301}"), 0.0), 0..2);
        assert_single_line(&wrap(&text("👩‍👩‍👧‍👦"), 0.0), 0..7);
        assert_eq!(ranges(&wrap(&text("ABC"), f64::NAN)), [0..1, 1..2, 2..3]);
        assert_eq!(ranges(&wrap(&text("ABC"), -1.0)), [0..1, 1..2, 2..3]);
    }

    #[test]
    fn caller_monospace_font_changes_breaks_from_default_roboto() {
        let mut database = fontdb::Database::new();
        database.load_font_data(
            include_bytes!("../../../assets/fonts/RobotoMono-Regular.ttf").to_vec(),
        );
        database.set_sans_serif_family("Roboto Mono");
        let fonts = FontBook::new(Arc::new(database));
        let content = text("ABC");
        assert_eq!(ranges(&wrap(&content, 83.0)), [0..2, 2..3]);
        let caller = wrap_with_fonts(&content, 0..3, 83.0, &fonts).unwrap();
        assert_single_line(&caller, 0..3);
    }

    #[test]
    fn unavailable_fonts_return_typed_error_instead_of_estimated_widths() {
        let fonts = FontBook::new(Arc::new(fontdb::Database::new()));
        let content = text("ABC");
        let result = wrap_with_fonts(&content, 0..3, 1000.0, &fonts);
        assert!(
            matches!(result, Err(MeasurementError::UnavailableFace(family)) if family == "Roboto")
        );
    }
}
