use super::super::native_wrap::{
    NativeBlock, NativeEntryCursor, NativeHorizontalAlignment, NativeWrapEntry, NativeWrapError,
    NativeWrapKind, NativeWrapMetrics, NativeWrapWidths, select_native_block,
    select_native_block_with_objects,
};
use super::*;

pub(super) struct NativeMixedSlots {
    entries: Vec<NativeWrapEntry>,
    objects: Vec<bool>,
    block_objects: Vec<Option<NativeBlockObjectGeometry>>,
    item_slots: Vec<Range<usize>>,
    origin_utf16: u32,
}

#[derive(Clone, Copy)]
struct NativeBlockObjectGeometry {
    visible_width: f32,
}

pub(super) struct NativeMixedLine {
    pub slots: NativeMixedSlots,
    pub visual_scalars: Vec<usize>,
}

impl NativeMixedLine {
    pub fn new(
        slots: NativeMixedSlots,
        styled: &StyledText<'_>,
        source: Range<usize>,
        visual_scalars: Vec<usize>,
    ) -> Result<Self, MeasurementError> {
        Ok(Self {
            slots: slots.retain_line(styled, source)?,
            visual_scalars,
        })
    }

    pub fn aligned_positions(
        &self,
        styled: &StyledText<'_>,
        line: &WrappedLine,
        left: f32,
        width: f32,
        alignment: Option<crate::ParagraphAlignment>,
    ) -> Result<(f32, LineGeometry), MeasurementError> {
        let alignment = match alignment {
            Some(crate::ParagraphAlignment::Right) => NativeHorizontalAlignment::Right,
            Some(crate::ParagraphAlignment::Center) => NativeHorizontalAlignment::Center,
            _ => NativeHorizontalAlignment::Start,
        };
        let origin = self
            .slots
            .block(styled, line.source.clone())?
            .aligned_origin(left, width, alignment)
            .map_err(|_| MeasurementError::InvalidCluster)?;
        self.slots
            .positions_at(
                styled,
                line,
                &self.visual_scalars,
                &line.visual_order,
                origin,
            )
            .map(|geometry| (origin, geometry))
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct NativeMixedSelection {
    pub end: usize,
    pub encountered_object_metric: bool,
}

impl NativeMixedSlots {
    fn retain_line(
        self,
        styled: &StyledText<'_>,
        source: Range<usize>,
    ) -> Result<Self, MeasurementError> {
        let range = self.range(styled, source)?;
        if range.is_empty() || range.end > self.entries.len() {
            return Err(MeasurementError::InvalidRange);
        }
        let origin_utf16 = u32::try_from(range.start)
            .ok()
            .and_then(|start| self.origin_utf16.checked_add(start))
            .ok_or(MeasurementError::InvalidRange)?;
        let entries = self.entries[range.clone()]
            .iter()
            .copied()
            .map(|mut entry| {
                entry.break_end_utf16 = entry
                    .break_end_utf16
                    .filter(|end| *end <= range.end)
                    .and_then(|end| end.checked_sub(range.start));
                entry
            })
            .collect();
        let item_slots = self
            .item_slots
            .into_iter()
            .filter(|slots| slots.start < range.end && slots.end > range.start)
            .map(|slots| {
                Ok(slots
                    .start
                    .checked_sub(range.start)
                    .ok_or(MeasurementError::InvalidRange)?
                    ..slots
                        .end
                        .checked_sub(range.start)
                        .filter(|end| *end <= range.len())
                        .ok_or(MeasurementError::InvalidRange)?)
            })
            .collect::<Result<_, MeasurementError>>()?;
        Ok(Self {
            entries,
            objects: self.objects[range.clone()].to_vec(),
            block_objects: self.block_objects[range].to_vec(),
            item_slots,
            origin_utf16,
        })
    }

    pub fn new(
        styled: &StyledText<'_>,
        source: Range<usize>,
        items: &[MeasuredItem],
        allowed: &[bool],
    ) -> Result<Option<Self>, MeasurementError> {
        if !items.iter().any(|item| matches!(item, MeasuredItem::Object { .. }))
            || items.iter().any(|item| matches!(item, MeasuredItem::TextCluster(cluster) if cluster.run.native_entries.is_none()))
        {
            return Ok(None);
        }
        let origin_utf16 = styled
            .index
            .char_to_utf16(source.start)
            .ok_or(MeasurementError::InvalidRange)?;
        let mut entries = Vec::new();
        let mut objects = Vec::new();
        let mut block_objects = Vec::new();
        let mut item_slots = Vec::with_capacity(items.len());
        for item in items {
            let start = entries.len();
            match item {
                MeasuredItem::TextCluster(cluster) => {
                    let local_allowed = &allowed
                        [cluster.source.start - source.start..=cluster.source.end - source.start];
                    let slots = NativeParagraphSlots::new(
                        styled,
                        cluster.source.clone(),
                        std::slice::from_ref(item),
                        local_allowed,
                    )?
                    .ok_or(MeasurementError::InvalidCluster)?;
                    entries.extend_from_slice(slots.entries());
                    objects.resize(entries.len(), false);
                    block_objects.resize(entries.len(), None);
                }
                MeasuredItem::Object {
                    placement,
                    font_size,
                } => {
                    if styled.index.slice(placement.object.source.clone()) != Some("\u{fffc}") {
                        return Err(MeasurementError::InvalidCluster);
                    }
                    native_geometry(placement.object.left_margin)?;
                    let size = native_geometry(*font_size)?;
                    entries.push(NativeWrapEntry {
                        advance: native_geometry(placement.object.advance())?,
                        kind: NativeWrapKind::Ordinary,
                        break_end_utf16: None,
                        metrics: NativeWrapMetrics {
                            font_size: size,
                            height: size,
                        },
                    });
                    objects.push(true);
                    block_objects.push((!placement.object.inline).then(|| {
                        NativeBlockObjectGeometry {
                            visible_width: placement.object.width() as f32,
                        }
                    }));
                }
            }
            item_slots.push(start..entries.len());
        }
        let mut ends = allowed
            .iter()
            .enumerate()
            .filter(|(_, allowed)| **allowed)
            .map(|(end, _)| {
                styled
                    .index
                    .char_to_utf16(source.start + end)
                    .and_then(|end| end.checked_sub(origin_utf16))
                    .map(|end| end as usize)
                    .ok_or(MeasurementError::InvalidRange)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .peekable();
        for (index, entry) in entries.iter_mut().enumerate() {
            while ends.peek().is_some_and(|&end| end <= index) {
                ends.next();
            }
            entry.break_end_utf16 = ends.peek().copied();
        }
        Ok(Some(Self {
            entries,
            objects,
            block_objects,
            item_slots,
            origin_utf16,
        }))
    }

    fn range(
        &self,
        styled: &StyledText<'_>,
        source: Range<usize>,
    ) -> Result<Range<usize>, MeasurementError> {
        let range = styled
            .index
            .source(source)
            .ok_or(MeasurementError::InvalidRange)?;
        let local = |offset: u32| {
            offset
                .checked_sub(self.origin_utf16)
                .map(|offset| offset as usize)
                .ok_or(MeasurementError::InvalidRange)
        };
        Ok(local(range.utf16().start)?..local(range.utf16().end)?)
    }

    pub fn select(
        &self,
        styled: &StyledText<'_>,
        source: Range<usize>,
        widths: [f64; 2],
        items: &mut [MeasuredItem],
        prepare: &mut impl FnMut(&mut PositionedObject),
    ) -> Result<Option<NativeMixedSelection>, MeasurementError> {
        let range = self.range(styled, source)?;
        let width = |value| {
            if value == f64::INFINITY {
                Ok(f32::MAX)
            } else {
                native_geometry(value)
            }
        };
        let block = select_native_block_with_objects(
            &self.entries,
            range.clone(),
            NativeWrapWidths {
                available: width(widths[0])?,
                full: width(widths[1])?,
            },
            &self.objects,
            |slot, _| {
                let mut cursor = NativeEntryCursor::new(0.0)?;
                for previous in range.start..slot {
                    let advance = if self.objects[previous] {
                        let item = self
                            .item_slots
                            .partition_point(|range| range.end <= previous);
                        native_geometry(items[item].advance())
                            .map_err(|_| NativeWrapError::InvalidAdvance)?
                    } else {
                        self.entries[previous].advance
                    };
                    cursor.advance(advance)?;
                }
                let item = self.item_slots.partition_point(|range| range.end <= slot);
                let MeasuredItem::Object { placement, .. } = &mut items[item] else {
                    return Err(NativeWrapError::InvalidRange);
                };
                placement.x = f64::from(
                    cursor.advance(0.0)?.left
                        + native_geometry(placement.object.left_margin)
                            .map_err(|_| NativeWrapError::InvalidGeometry)?,
                );
                if !placement.x.is_finite() {
                    return Err(NativeWrapError::NumericOverflow);
                }
                prepare(placement);
                native_geometry(placement.object.advance())
                    .map_err(|_| NativeWrapError::InvalidAdvance)
            },
        )
        .map_err(|_| MeasurementError::InvalidCluster)?;
        block
            .map(|block| {
                let end = u32::try_from(*block.range_utf16_inclusive.end() + 1)
                    .ok()
                    .and_then(|end| end.checked_add(self.origin_utf16))
                    .and_then(|end| styled.index.utf16_to_char(end))
                    .ok_or(MeasurementError::InvalidCluster)?;
                Ok(NativeMixedSelection {
                    end,
                    encountered_object_metric: block.encountered_object_metric,
                })
            })
            .transpose()
    }

    pub fn block_width(
        &self,
        styled: &StyledText<'_>,
        source: Range<usize>,
    ) -> Result<f64, MeasurementError> {
        self.block(styled, source)
            .map(|block| f64::from(block.width))
    }

    fn block(
        &self,
        styled: &StyledText<'_>,
        source: Range<usize>,
    ) -> Result<NativeBlock, MeasurementError> {
        let range = self.range(styled, source)?;
        let object = self.block_objects[range.start];
        if let Some(object) = object
            && (range.len() != 1 || !object.visible_width.is_finite())
        {
            return Err(MeasurementError::InvalidCluster);
        }
        let mut block = select_native_block(
            &self.entries,
            range,
            NativeWrapWidths {
                available: f32::MAX,
                full: f32::MAX,
            },
        )
        .map_err(|_| MeasurementError::InvalidCluster)?
        .ok_or(MeasurementError::InvalidCluster)?;
        if let Some(object) = object {
            block.width = object.visible_width;
        }
        Ok(block)
    }

    pub fn positions(
        &self,
        styled: &StyledText<'_>,
        line: &WrappedLine,
        visual_scalars: &[usize],
        visual_entries: &[LineEntry],
    ) -> Result<LineGeometry, MeasurementError> {
        self.positions_at(styled, line, visual_scalars, visual_entries, 0.0)
    }

    fn positions_at(
        &self,
        styled: &StyledText<'_>,
        line: &WrappedLine,
        visual_scalars: &[usize],
        visual_entries: &[LineEntry],
        origin: f32,
    ) -> Result<LineGeometry, MeasurementError> {
        let range = self.range(styled, line.source.clone())?;
        let mut positions = vec![None; self.entries.len()];
        let mut cursor =
            NativeEntryCursor::new(origin).map_err(|_| MeasurementError::InvalidCluster)?;
        let mut advance = 0.0;
        for &scalar in visual_scalars {
            let scalar = line.source.start + scalar;
            for slot in self.range(styled, scalar..scalar + 1)? {
                if !range.contains(&slot) || positions[slot].is_some() {
                    return Err(MeasurementError::InvalidCluster);
                }
                let placed = cursor
                    .advance(
                        self.block_objects[slot]
                            .map_or(self.entries[slot].advance, |object| object.visible_width),
                    )
                    .map_err(|_| MeasurementError::InvalidCluster)?;
                positions[slot] = Some(placed);
                advance = placed.right;
            }
        }
        if positions[range].iter().any(Option::is_none) {
            return Err(MeasurementError::InvalidCluster);
        }
        let position =
            |source: Range<usize>, margin: f64, rank| -> Result<LinePosition, MeasurementError> {
                let owner = self.range(styled, source)?.start;
                let placed = positions[owner].ok_or(MeasurementError::InvalidCluster)?;
                let x = placed.left + native_geometry(margin)?;
                if !x.is_finite() {
                    return Err(MeasurementError::InvalidCluster);
                }
                Ok(LinePosition {
                    x: f64::from(x),
                    visual_rank: rank,
                    ..Default::default()
                })
            };
        let mut text = vec![LinePosition::default(); line.placements.len()];
        let mut objects = vec![LinePosition::default(); line.objects.len()];
        for (rank, entry) in visual_entries.iter().enumerate() {
            match *entry {
                LineEntry::Text(index) => {
                    text[index] =
                        position(line.placements[index].cluster.source.clone(), 0.0, rank)?
                }
                LineEntry::Object(index) => {
                    objects[index] = position(
                        line.objects[index].object.source.clone(),
                        line.objects[index].object.left_margin,
                        rank,
                    )?
                }
            }
        }
        Ok(LineGeometry::Positioned {
            advance: f64::from(advance),
            text,
            objects,
        })
    }
}

#[cfg(test)]
mod tests;
