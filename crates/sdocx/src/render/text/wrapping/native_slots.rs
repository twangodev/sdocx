use super::super::native_entry::NativeEntryKind;
use super::super::native_wrap::{
    NativeBlock, NativeEntryCursor, NativeHorizontalAlignment, NativeWrapEntry, NativeWrapKind,
    NativeWrapMetrics, NativeWrapWidths, select_native_block,
};
use super::*;

pub(in crate::render) enum ParagraphMeasurementWidth {
    Constrained(f64),
    Automatic { insets: [f32; 2] },
}

impl From<f64> for ParagraphMeasurementWidth {
    fn from(width: f64) -> Self {
        Self::Constrained(width)
    }
}

pub(super) struct NativeParagraphSlots {
    entries: Vec<NativeWrapEntry>,
    origin_utf16: u32,
}

pub(super) struct NativeLineSlots {
    entries: Vec<NativeWrapEntry>,
    visual_to_logical: Vec<usize>,
    cluster_slots: Vec<Range<usize>>,
    cluster_ranks: Vec<usize>,
    block: NativeBlock,
}

pub(super) struct NativeAlignedLine {
    pub origin: f32,
    pub positions: Vec<LinePosition>,
    pub end: f64,
}

fn width(value: f64) -> Result<f32, MeasurementError> {
    if value == f64::INFINITY {
        Ok(f32::MAX)
    } else {
        native_geometry(value)
    }
}

impl NativeParagraphSlots {
    pub fn automatic_width(&self, insets: [f32; 2]) -> Result<f64, MeasurementError> {
        let outer = self
            .entries
            .iter()
            .fold(insets[0] + insets[1], |width, entry| width + entry.advance);
        let rounded = (outer + 0.001_f32).ceil();
        if !rounded.is_finite() || rounded < 0.0 || rounded >= i32::MAX as f32 {
            return Err(MeasurementError::InvalidCluster);
        }
        let available = (rounded - insets[0]) - insets[1];
        if !available.is_finite() || available < 0.0 {
            return Err(MeasurementError::InvalidCluster);
        }
        Ok(f64::from(available))
    }

    pub fn entries(&self) -> &[NativeWrapEntry] {
        &self.entries
    }

    pub fn new(
        styled: &StyledText<'_>,
        source: Range<usize>,
        items: &[MeasuredItem],
        allowed: &[bool],
    ) -> Result<Option<Self>, MeasurementError> {
        if items.iter().any(|item| !matches!(item, MeasuredItem::TextCluster(cluster) if cluster.run.native_entries.is_some())) {
            return Ok(None);
        }
        let source_utf16 = styled
            .index
            .source(source.clone())
            .ok_or(MeasurementError::InvalidRange)?;
        let origin_utf16 = source_utf16.utf16().start;
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
        let mut entries = Vec::with_capacity((source_utf16.utf16().end - origin_utf16) as usize);
        for item in items {
            let MeasuredItem::TextCluster(cluster) = item else {
                unreachable!()
            };
            let native = cluster.run.native_entries.as_ref().unwrap();
            let text = styled
                .index
                .slice(cluster.source.clone())
                .ok_or(MeasurementError::InvalidRange)?;
            let start = styled
                .index
                .char_to_utf16(cluster.source.start)
                .ok_or(MeasurementError::InvalidRange)?;
            let mut offset = start;
            let size = native_geometry(cluster.run.style.font_size)?;
            for scalar in text.chars() {
                for unit in 0..scalar.len_utf16() {
                    let local = (offset - origin_utf16) as usize;
                    while ends.peek().is_some_and(|&end| end <= local) {
                        ends.next();
                    }
                    let producer = offset
                        .checked_sub(native.source().utf16().start)
                        .ok_or(MeasurementError::InvalidRange)?;
                    let entry = native
                        .geometry()
                        .entry_at_utf16(producer)
                        .ok_or(MeasurementError::InvalidCluster)?;
                    if local != entries.len() {
                        return Err(MeasurementError::InvalidCluster);
                    }
                    let facts = native.entry_facts_at_utf16(producer);
                    entries.push(NativeWrapEntry {
                        advance: entry.advance(),
                        kind: match facts.map(|facts| facts.kind) {
                            Some(NativeEntryKind::Space) => NativeWrapKind::Space,
                            Some(NativeEntryKind::Tab) => NativeWrapKind::Tab,
                            Some(_) => NativeWrapKind::Ordinary,
                            None => match (scalar, unit) {
                                (' ', 0) => NativeWrapKind::Space,
                                ('\t', 0) => NativeWrapKind::Tab,
                                _ => NativeWrapKind::Ordinary,
                            },
                        },
                        break_end_utf16: ends.peek().copied(),
                        metrics: facts.map_or(
                            NativeWrapMetrics {
                                font_size: size,
                                height: size,
                            },
                            |facts| NativeWrapMetrics {
                                font_size: facts.font_size,
                                height: facts.height,
                            },
                        ),
                    });
                    offset += 1;
                }
            }
        }
        if entries.len() != (source_utf16.utf16().end - origin_utf16) as usize {
            return Err(MeasurementError::InvalidCluster);
        }
        Ok(Some(Self {
            entries,
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
        let start = range
            .utf16()
            .start
            .checked_sub(self.origin_utf16)
            .ok_or(MeasurementError::InvalidRange)? as usize;
        let end = range
            .utf16()
            .end
            .checked_sub(self.origin_utf16)
            .ok_or(MeasurementError::InvalidRange)? as usize;
        Ok(start..end)
    }

    pub fn select(
        &self,
        styled: &StyledText<'_>,
        source: Range<usize>,
        available: f64,
        full: f64,
    ) -> Result<Option<usize>, MeasurementError> {
        let range = self.range(styled, source)?;
        let block = select_native_block(
            &self.entries,
            range,
            NativeWrapWidths {
                available: width(available)?,
                full: width(full)?,
            },
        )
        .map_err(|_| MeasurementError::InvalidCluster)?;
        block
            .map(|block| {
                let end = u32::try_from(*block.range_utf16_inclusive.end() + 1)
                    .ok()
                    .and_then(|end| end.checked_add(self.origin_utf16))
                    .ok_or(MeasurementError::InvalidRange)?;
                styled
                    .index
                    .utf16_to_char(end)
                    .ok_or(MeasurementError::InvalidCluster)
            })
            .transpose()
    }

    pub fn line(
        &self,
        styled: &StyledText<'_>,
        source: Range<usize>,
        clusters: &[PositionedCluster],
        visual_clusters: &[LineEntry],
        visual_scalars: &[usize],
    ) -> Result<NativeLineSlots, MeasurementError> {
        let range = self.range(styled, source.clone())?;
        let block = select_native_block(
            &self.entries,
            range.clone(),
            NativeWrapWidths {
                available: f32::MAX,
                full: f32::MAX,
            },
        )
        .map_err(|_| MeasurementError::InvalidCluster)?
        .ok_or(MeasurementError::InvalidCluster)?;
        let entries = self.entries[range.clone()].to_vec();
        let cluster_slots = clusters
            .iter()
            .map(|cluster| {
                self.range(styled, cluster.cluster.source.clone())
                    .map(|slots| slots.start - range.start..slots.end - range.start)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut visual_to_logical = Vec::with_capacity(entries.len());
        for &scalar in visual_scalars {
            let slots = self.range(styled, source.start + scalar..source.start + scalar + 1)?;
            visual_to_logical.extend(slots.map(|slot| slot - range.start));
        }
        if visual_to_logical.len() != entries.len() {
            return Err(MeasurementError::InvalidCluster);
        }
        let mut cluster_ranks = vec![0; clusters.len()];
        for (rank, entry) in visual_clusters.iter().enumerate() {
            let LineEntry::Text(index) = *entry else {
                return Err(MeasurementError::InvalidCluster);
            };
            cluster_ranks[index] = rank;
        }
        Ok(NativeLineSlots {
            entries,
            visual_to_logical,
            cluster_slots,
            cluster_ranks,
            block,
        })
    }
}

impl NativeLineSlots {
    pub fn block_width(&self) -> f64 {
        f64::from(self.block.width)
    }

    pub fn justification_share(&self, width: f64) -> Result<f32, MeasurementError> {
        if self.block.space_weight == 0 {
            return Ok(0.0);
        }
        self.block
            .justification_share(native_geometry(width)?)
            .map_err(|_| MeasurementError::InvalidCluster)
    }

    pub fn positions(&self, share: f32) -> Result<(Vec<LinePosition>, f64), MeasurementError> {
        self.positions_at(0.0, share)
    }

    pub fn aligned_positions(
        &self,
        left: f32,
        width: f32,
        alignment: Option<crate::ParagraphAlignment>,
    ) -> Result<NativeAlignedLine, MeasurementError> {
        let native_alignment = match alignment {
            Some(crate::ParagraphAlignment::Right) => NativeHorizontalAlignment::Right,
            Some(crate::ParagraphAlignment::Center) => NativeHorizontalAlignment::Center,
            _ => NativeHorizontalAlignment::Start,
        };
        let origin = self
            .block
            .aligned_origin(left, width, native_alignment)
            .map_err(|_| MeasurementError::InvalidCluster)?;
        let share = if alignment == Some(crate::ParagraphAlignment::Both) {
            self.justification_share(f64::from(width))?
        } else {
            0.0
        };
        let (positions, end) = self.positions_at(origin, share)?;
        Ok(NativeAlignedLine {
            origin,
            positions,
            end,
        })
    }

    fn positions_at(
        &self,
        origin: f32,
        share: f32,
    ) -> Result<(Vec<LinePosition>, f64), MeasurementError> {
        let mut slots = vec![None; self.entries.len()];
        let mut cursor =
            NativeEntryCursor::new(origin).map_err(|_| MeasurementError::InvalidCluster)?;
        let mut advance = 0.0;
        for &logical in &self.visual_to_logical {
            let entry = self
                .entries
                .get(logical)
                .ok_or(MeasurementError::InvalidCluster)?;
            let placed = (if share == 0.0 {
                cursor.advance(entry.advance)
            } else {
                cursor.advance_justified(entry.advance, entry.kind, share)
            })
            .map_err(|_| MeasurementError::InvalidCluster)?;
            slots[logical] = Some(placed);
            advance = placed.right;
        }
        let text = self
            .cluster_slots
            .iter()
            .enumerate()
            .map(|(index, range)| {
                let owner = slots[range.start].ok_or(MeasurementError::InvalidCluster)?;
                let mut extra = 0.0_f32;
                for slot in range.clone() {
                    let entry = self.entries[slot];
                    let placed = slots[slot].ok_or(MeasurementError::InvalidCluster)?;
                    extra += placed.right - (placed.left + entry.advance);
                }
                if !extra.is_finite() {
                    return Err(MeasurementError::InvalidCluster);
                }
                Ok(LinePosition {
                    x: f64::from(owner.left),
                    glyph_offset_x: 0.0,
                    extra_advance: f64::from(extra),
                    visual_rank: self.cluster_ranks[index],
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((text, f64::from(advance)))
    }
}

#[cfg(all(test, feature = "serde"))]
mod measurement_width_tests;
#[cfg(test)]
mod tests;
