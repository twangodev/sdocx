use read_fonts::tables::gpos::{
    AnchorTable, ExtensionSubtable, Gpos, MarkArray, PositionLookup, ValueRecord,
};
use read_fonts::{ReadError, TableProvider};

use super::{PaintMetricInput, PaintShapeError, PaintShapeRequest, PaintShapeScale};
use crate::render::harfrust::{FontRef, Tag};

const MAX_RECORDS: usize = 100_000;
const MAX_ATTACHMENT_GLYPHS: usize = i16::MAX as usize;

pub(super) struct PositioningDomain {
    lookups: Vec<Bounds>,
    scale: PaintShapeScale,
    upem: u64,
}

impl PositioningDomain {
    pub(super) fn new(
        font: &FontRef<'_>,
        _paint: PaintMetricInput,
        scale: PaintShapeScale,
    ) -> Result<Self, PaintShapeError> {
        if [b"kern", b"kerx", b"trak"]
            .into_iter()
            .any(|tag| font.table_data(Tag::new(tag)).is_some())
        {
            return Err(PaintShapeError::UnsupportedPositioningDomain);
        }
        let upem = u64::from(font.head().map_err(invalid_font)?.units_per_em());
        if upem == 0 {
            return Err(PaintShapeError::UnsupportedPositioningDomain);
        }
        let mut scan = Scan {
            remaining: MAX_RECORDS,
        };
        let lookups = match font.gpos() {
            Ok(gpos) => scan.gpos(gpos)?,
            Err(ReadError::TableIsMissing(_)) => Vec::new(),
            Err(error) => return Err(invalid_font(error)),
        };
        Ok(Self {
            lookups,
            scale,
            upem,
        })
    }

    pub(super) fn advance_limit(&self) -> u32 {
        i32::MAX as u32
    }

    pub(super) fn glyph_budget(
        &self,
        request: &PaintShapeRequest<'_>,
    ) -> Result<usize, PaintShapeError> {
        if !matches!(&request.script, b"Latn" | b"Grek" | b"Cyrl" | b"Zyyy") {
            return Err(PaintShapeError::UnsupportedPositioningDomain);
        }
        Ok(request
            .infos
            .len()
            .saturating_mul(2)
            .clamp(1, MAX_ATTACHMENT_GLYPHS))
    }

    pub(super) fn validate_positioning(
        &self,
        infos: &[crate::render::harfrust::GlyphInfo],
        positions: &[crate::render::harfrust::GlyphPosition],
    ) -> Result<(), PaintShapeError> {
        if infos.len() != positions.len() || infos.len() > MAX_ATTACHMENT_GLYPHS {
            return Err(PaintShapeError::UnsupportedPositioningDomain);
        }
        let glyphs: std::collections::HashSet<_> = infos.iter().map(|info| info.glyph_id).collect();
        for axis in 0..2 {
            let scale = if axis == 0 {
                self.scale.x
            } else {
                self.scale.y
            };
            let mut adjustment = 0_u64;
            for lookup in &self.lookups {
                adjustment += scaled_bound(lookup.placements[axis], scale, self.upem)?;
                adjustment += scaled_bound(lookup.advances[axis], scale, self.upem)?;
                if lookup
                    .mark_coverage
                    .iter()
                    .any(|glyph| glyphs.contains(glyph))
                {
                    adjustment += scaled_bound(lookup.anchors[axis], scale, self.upem)?;
                }
                adjustment += lookup.rounding_steps;
            }
            let mut initial_advances = 0_u64;
            let mut initial_offset = 0_u64;
            for position in positions {
                let (advance, offset) = if axis == 0 {
                    (position.x_advance, position.x_offset)
                } else {
                    (position.y_advance, position.y_offset)
                };
                initial_advances += u64::from(advance.unsigned_abs());
                initial_offset = initial_offset.max(u64::from(offset.unsigned_abs()));
            }
            let bound = (adjustment + initial_offset)
                .checked_mul(infos.len() as u64)
                .and_then(|value| value.checked_add(initial_advances));
            if bound.is_none_or(|value| value >= i32::MAX as u64) {
                return Err(PaintShapeError::UnsupportedPositioningDomain);
            }
        }
        Ok(())
    }
}

fn invalid_font(_: ReadError) -> PaintShapeError {
    super::PaintMetricError::InvalidFont.into()
}

fn scaled_bound(units: u64, scale: i32, upem: u64) -> Result<u64, PaintShapeError> {
    if units == 0 {
        return Ok(0);
    }
    let bound = (units as f64 * f64::from(scale) / upem as f64
        * (1.0 + 2.0 * f64::from(f32::EPSILON)))
    .ceil()
        + 1.0;
    if !bound.is_finite() || bound >= i32::MAX as f64 {
        return Err(PaintShapeError::UnsupportedPositioningDomain);
    }
    Ok(bound as u64)
}

#[derive(Default, Clone)]
struct Bounds {
    placements: [u64; 2],
    advances: [u64; 2],
    anchors: [u64; 2],
    rounding_steps: u64,
    mark_coverage: Vec<u32>,
}

impl Bounds {
    fn include_value(&mut self, record: &ValueRecord) {
        for (bound, value) in self
            .placements
            .iter_mut()
            .zip([record.x_placement, record.y_placement])
        {
            *bound = (*bound).max(value.map_or(0, |value| u64::from(value.get().unsigned_abs())));
        }
        for (bound, value) in self
            .advances
            .iter_mut()
            .zip([record.x_advance, record.y_advance])
        {
            *bound = (*bound).max(value.map_or(0, |value| u64::from(value.get().unsigned_abs())));
        }
    }

    fn include_anchor(&mut self, anchor: AnchorTable<'_>) -> Result<(), PaintShapeError> {
        let coordinates = match anchor {
            AnchorTable::Format1(anchor) => [anchor.x_coordinate(), anchor.y_coordinate()],
            AnchorTable::Format3(anchor) => [anchor.x_coordinate(), anchor.y_coordinate()],
            AnchorTable::Format2(_) => return Err(PaintShapeError::UnsupportedPositioningDomain),
        };
        for (bound, coordinate) in self.anchors.iter_mut().zip(coordinates) {
            *bound = (*bound).max(u64::from(coordinate.unsigned_abs()));
        }
        Ok(())
    }

    fn doubled(mut self) -> Self {
        for bound in self
            .placements
            .iter_mut()
            .chain(&mut self.advances)
            .chain(&mut self.anchors)
        {
            *bound *= 2;
        }
        self
    }
}

struct Scan {
    remaining: usize,
}

impl Scan {
    fn records<T>(
        &mut self,
        records: impl IntoIterator<Item = Result<T, ReadError>>,
        mut visit: impl FnMut(&mut Self, T) -> Result<(), PaintShapeError>,
    ) -> Result<(), PaintShapeError> {
        for record in records {
            self.remaining = self
                .remaining
                .checked_sub(1)
                .ok_or(PaintShapeError::UnsupportedPositioningDomain)?;
            visit(self, record.map_err(invalid_font)?)?;
        }
        Ok(())
    }

    fn gpos(&mut self, table: Gpos<'_>) -> Result<Vec<Bounds>, PaintShapeError> {
        let mut total = Vec::new();
        self.records(
            table.lookup_list().map_err(invalid_font)?.lookups().iter(),
            |scan, lookup| {
                let mut lookup_bounds = Bounds::default();
                scan.lookup(lookup, &mut lookup_bounds)?;
                for bound in &mut lookup_bounds.anchors {
                    *bound *= 2;
                }
                lookup_bounds.rounding_steps = 4;
                total.push(lookup_bounds);
                Ok(())
            },
        )?;
        Ok(total)
    }

    fn lookup(
        &mut self,
        lookup: PositionLookup<'_>,
        bounds: &mut Bounds,
    ) -> Result<(), PaintShapeError> {
        match lookup {
            PositionLookup::Single(inner) => self
                .records(inner.subtables().iter(), |scan, table| {
                    scan.single(table, bounds)
                }),
            PositionLookup::Pair(inner) => self.records(inner.subtables().iter(), |scan, table| {
                scan.pair(table, bounds)
            }),
            PositionLookup::MarkToBase(inner) => self
                .records(inner.subtables().iter(), |scan, table| {
                    scan.mark_base(table, bounds)
                }),
            PositionLookup::MarkToLig(inner) => self
                .records(inner.subtables().iter(), |scan, table| {
                    scan.mark_ligature(table, bounds)
                }),
            PositionLookup::MarkToMark(inner) => self
                .records(inner.subtables().iter(), |scan, table| {
                    scan.mark_mark(table, bounds)
                }),
            PositionLookup::Extension(inner) => self
                .records(inner.subtables().iter(), |scan, table| {
                    scan.extension(table, bounds)
                }),
            _ => Err(PaintShapeError::UnsupportedPositioningDomain),
        }
    }

    fn extension(
        &mut self,
        table: ExtensionSubtable<'_>,
        bounds: &mut Bounds,
    ) -> Result<(), PaintShapeError> {
        match table {
            ExtensionSubtable::Single(inner) => {
                self.single(inner.extension().map_err(invalid_font)?, bounds)
            }
            ExtensionSubtable::Pair(inner) => {
                self.pair(inner.extension().map_err(invalid_font)?, bounds)
            }
            ExtensionSubtable::MarkToBase(inner) => {
                self.mark_base(inner.extension().map_err(invalid_font)?, bounds)
            }
            ExtensionSubtable::MarkToLig(inner) => {
                self.mark_ligature(inner.extension().map_err(invalid_font)?, bounds)
            }
            ExtensionSubtable::MarkToMark(inner) => {
                self.mark_mark(inner.extension().map_err(invalid_font)?, bounds)
            }
            _ => Err(PaintShapeError::UnsupportedPositioningDomain),
        }
    }

    fn single(
        &mut self,
        table: read_fonts::tables::gpos::SinglePos<'_>,
        bounds: &mut Bounds,
    ) -> Result<(), PaintShapeError> {
        match table {
            read_fonts::tables::gpos::SinglePos::Format1(table) => {
                bounds.include_value(&table.value_record());
                Ok(())
            }
            read_fonts::tables::gpos::SinglePos::Format2(table) => {
                self.records(table.value_records().iter(), |_, record| {
                    bounds.include_value(&record);
                    Ok(())
                })
            }
        }
    }

    fn pair(
        &mut self,
        table: read_fonts::tables::gpos::PairPos<'_>,
        bounds: &mut Bounds,
    ) -> Result<(), PaintShapeError> {
        let mut pair = Bounds::default();
        match table {
            read_fonts::tables::gpos::PairPos::Format1(table) => {
                self.records(table.pair_sets().iter(), |scan, set| {
                    scan.records(set.pair_value_records().iter(), |_, record| {
                        pair.include_value(record.value_record1());
                        pair.include_value(record.value_record2());
                        Ok(())
                    })
                })?
            }
            read_fonts::tables::gpos::PairPos::Format2(table) => {
                self.records(table.class1_records().iter(), |scan, class| {
                    scan.records(class.class2_records().iter(), |_, record| {
                        pair.include_value(record.value_record1());
                        pair.include_value(record.value_record2());
                        Ok(())
                    })
                })?
            }
        }
        let pair = pair.doubled();
        for (bound, value) in bounds.placements.iter_mut().zip(pair.placements) {
            *bound = (*bound).max(value);
        }
        for (bound, value) in bounds.advances.iter_mut().zip(pair.advances) {
            *bound = (*bound).max(value);
        }
        Ok(())
    }

    fn coverage(
        &mut self,
        table: read_fonts::tables::layout::CoverageTable<'_>,
        bounds: &mut Bounds,
    ) -> Result<(), PaintShapeError> {
        self.records(table.iter().map(Ok), |_, glyph| {
            bounds.mark_coverage.push(glyph.to_u32());
            Ok(())
        })
    }

    fn marks(&mut self, table: MarkArray<'_>, bounds: &mut Bounds) -> Result<(), PaintShapeError> {
        self.records(table.mark_records().iter().map(Ok), |_, record| {
            bounds.include_anchor(
                record
                    .mark_anchor(table.offset_data())
                    .map_err(invalid_font)?,
            )
        })
    }

    fn nullable(
        &mut self,
        anchor: Option<Result<AnchorTable<'_>, ReadError>>,
        bounds: &mut Bounds,
    ) -> Result<(), PaintShapeError> {
        if let Some(anchor) = anchor {
            bounds.include_anchor(anchor.map_err(invalid_font)?)?;
        }
        Ok(())
    }

    fn mark_base(
        &mut self,
        table: read_fonts::tables::gpos::MarkBasePosFormat1<'_>,
        bounds: &mut Bounds,
    ) -> Result<(), PaintShapeError> {
        self.coverage(table.mark_coverage().map_err(invalid_font)?, bounds)?;
        self.marks(table.mark_array().map_err(invalid_font)?, bounds)?;
        let bases = table.base_array().map_err(invalid_font)?;
        self.records(bases.base_records().iter(), |scan, record| {
            scan.records(
                record.base_anchors(bases.offset_data()).iter().map(Ok),
                |scan, anchor| scan.nullable(anchor, bounds),
            )
        })
    }

    fn mark_ligature(
        &mut self,
        table: read_fonts::tables::gpos::MarkLigPosFormat1<'_>,
        bounds: &mut Bounds,
    ) -> Result<(), PaintShapeError> {
        self.coverage(table.mark_coverage().map_err(invalid_font)?, bounds)?;
        self.marks(table.mark_array().map_err(invalid_font)?, bounds)?;
        self.records(
            table
                .ligature_array()
                .map_err(invalid_font)?
                .ligature_attaches()
                .iter(),
            |scan, ligature| {
                scan.records(ligature.component_records().iter(), |scan, record| {
                    scan.records(
                        record
                            .ligature_anchors(ligature.offset_data())
                            .iter()
                            .map(Ok),
                        |scan, anchor| scan.nullable(anchor, bounds),
                    )
                })
            },
        )
    }

    fn mark_mark(
        &mut self,
        table: read_fonts::tables::gpos::MarkMarkPosFormat1<'_>,
        bounds: &mut Bounds,
    ) -> Result<(), PaintShapeError> {
        self.coverage(table.mark1_coverage().map_err(invalid_font)?, bounds)?;
        self.marks(table.mark1_array().map_err(invalid_font)?, bounds)?;
        let marks = table.mark2_array().map_err(invalid_font)?;
        self.records(marks.mark2_records().iter(), |scan, record| {
            scan.records(
                record.mark2_anchors(marks.offset_data()).iter().map(Ok),
                |scan, anchor| scan.nullable(anchor, bounds),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::fonts::{
        FontBook, PaintMetricMode, PaintShapeClusterLevel, PaintShapeDirection, PaintSourceInfo,
    };

    fn with_gpos(value: i16, count: u16) -> Vec<u8> {
        let face = FontBook::default().resolve("Roboto", false, false).unwrap();
        let mut bytes = face.bytes().to_vec();
        let mut table = Vec::new();
        table.extend(
            [1_u16, 0, 0, 0, 10, count]
                .into_iter()
                .flat_map(u16::to_be_bytes),
        );
        for lookup in 0..count {
            table.extend((2 + count * 2 + lookup * 22).to_be_bytes());
        }
        for _ in 0..count {
            table.extend(
                [1_u16, 0, 1, 8, 1, 8, 4, value as u16, 1, 1, 38]
                    .into_iter()
                    .flat_map(u16::to_be_bytes),
            );
        }
        let offset = bytes.len() as u32;
        let length = table.len() as u32;
        let count = u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize;
        let record = bytes[12..12 + count * 16]
            .as_chunks_mut::<16>()
            .0
            .iter_mut()
            .find(|record| &record[..4] == b"GPOS")
            .unwrap();
        record[8..12].copy_from_slice(&offset.to_be_bytes());
        record[12..16].copy_from_slice(&length.to_be_bytes());
        bytes.extend(table);
        bytes
    }

    fn shape(bytes: &[u8], size: f32) -> Result<super::super::PaintShapedRun, PaintShapeError> {
        let paint = PaintMetricInput {
            size,
            scale_x: 1.0,
            skew_x: 0.0,
            mode: PaintMetricMode::Normal,
        };
        let mut shaper = super::super::PaintShaper::new(bytes, 0, paint)?;
        let infos = [PaintSourceInfo {
            character: 'A',
            owner_utf16: 0,
        }];
        shaper.shape(PaintShapeRequest {
            source: "A",
            infos: &infos,
            direction: PaintShapeDirection::LeftToRight,
            script: *b"Latn",
            language: None,
            flags: 0,
            cluster_level: PaintShapeClusterLevel::MonotoneGraphemes,
            pre_context: "",
            post_context: "",
            features: &[],
        })
    }

    #[test]
    fn large_single_coordinates_are_rejected_before_integer_narrowing() {
        let bytes = with_gpos(i16::MAX, 1);
        assert!(shape(&bytes, 1700.0).is_ok());
        let mathematical_scaled = i64::from(i16::MAX) * 2_048_000_000 / 2048;
        assert!(mathematical_scaled > i64::from(i32::MAX));
        assert!(matches!(
            shape(&bytes, 8_000_000.0),
            Err(PaintShapeError::UnsupportedPositioningDomain)
        ));
    }

    #[test]
    fn individually_valid_coordinates_cannot_accumulate_beyond_signed_positions() {
        let bytes = with_gpos(i16::MAX, 3);
        let each_scaled = i64::from(i16::MAX) * 51_200_000 / 2048;
        assert!(each_scaled < i64::from(i32::MAX));
        assert!(each_scaled * 3 > i64::from(i32::MAX));
        assert!(matches!(
            shape(&bytes, 200_000.0),
            Err(PaintShapeError::UnsupportedPositioningDomain)
        ));
        assert!(shape(&bytes, 1700.0).is_ok());
    }
    #[test]
    fn unbounded_lookup_families_and_legacy_positioning_tables_fail_explicitly() {
        for lookup_type in [3_u16, 7, 8] {
            let mut bytes = with_gpos(1, 1);
            let count = u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize;
            let record = bytes[12..12 + count * 16]
                .as_chunks::<16>()
                .0
                .iter()
                .find(|record| &record[..4] == b"GPOS")
                .unwrap();
            let offset = u32::from_be_bytes(record[8..12].try_into().unwrap()) as usize;
            bytes[offset + 14..offset + 16].copy_from_slice(&lookup_type.to_be_bytes());
            let font = FontRef::new(&bytes).unwrap();
            let paint = PaintMetricInput {
                size: 1700.0,
                scale_x: 1.0,
                skew_x: 0.0,
                mode: PaintMetricMode::Normal,
            };
            assert!(matches!(
                PositioningDomain::new(&font, paint, PaintShapeScale::new(paint).unwrap()),
                Err(PaintShapeError::UnsupportedPositioningDomain)
            ));
        }
        for tag in [b"kern", b"kerx", b"trak"] {
            let mut bytes = with_gpos(1, 1);
            let count = u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize;
            let records = bytes[12..12 + count * 16].as_chunks_mut::<16>().0;
            records
                .iter_mut()
                .find(|record| &record[..4] == b"gasp")
                .unwrap()[..4]
                .copy_from_slice(tag);
            records.sort_unstable_by_key(|record| <[u8; 4]>::try_from(&record[..4]).unwrap());
            assert!(matches!(
                shape(&bytes, 1700.0),
                Err(PaintShapeError::UnsupportedPositioningDomain)
            ));
        }
    }

    #[test]
    fn duplicate_features_do_not_multiply_lookup_stages() {
        use super::super::PaintShapeFeature;
        let face = FontBook::default().resolve("Roboto", false, false).unwrap();
        let paint = PaintMetricInput {
            size: 200_000.0,
            scale_x: 1.0,
            skew_x: 0.0,
            mode: PaintMetricMode::Normal,
        };
        let mut shaper = face.paint_shaper(paint).unwrap();
        let infos = [
            PaintSourceInfo {
                character: 'A',
                owner_utf16: 0,
            },
            PaintSourceInfo {
                character: 'V',
                owner_utf16: 1,
            },
        ];
        let request = PaintShapeRequest {
            source: "AV",
            infos: &infos,
            direction: PaintShapeDirection::LeftToRight,
            script: *b"Latn",
            language: None,
            flags: 0,
            cluster_level: PaintShapeClusterLevel::MonotoneGraphemes,
            pre_context: "",
            post_context: "",
            features: &[],
        };
        let baseline = shaper.shape(request).unwrap();
        let features = [PaintShapeFeature {
            tag: *b"kern",
            value: 1,
            start: 0,
            end: u32::MAX,
        }; 128];
        assert_eq!(
            baseline,
            shaper
                .shape(PaintShapeRequest {
                    features: &features,
                    ..request
                })
                .unwrap()
        );
        assert!(matches!(
            shaper.shape(PaintShapeRequest {
                script: *b"Arab",
                ..request
            }),
            Err(PaintShapeError::UnsupportedPositioningDomain)
        ));
    }
}
