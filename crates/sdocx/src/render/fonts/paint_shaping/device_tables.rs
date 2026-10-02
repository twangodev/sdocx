use read_fonts::tables::gpos::{
    AnchorTable, CursivePosFormat1, ExtensionSubtable, Gpos, MarkArray, MarkBasePosFormat1,
    MarkLigPosFormat1, MarkMarkPosFormat1, PairPos, PositionLookup, SinglePos, ValueRecord,
};
use read_fonts::{ReadError, TableProvider};

use crate::render::harfrust::FontRef;

const MAX_POSITIONING_RECORDS: usize = 100_000;

pub(super) fn requires_device_positioning(font: &FontRef<'_>) -> Result<bool, ReadError> {
    match font.gpos() {
        Ok(table) => Scan::new().gpos(table),
        Err(ReadError::TableIsMissing(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

struct Scan {
    remaining: usize,
}

impl Scan {
    fn new() -> Self {
        Self {
            remaining: MAX_POSITIONING_RECORDS,
        }
    }

    fn any<T>(
        &mut self,
        records: impl IntoIterator<Item = Result<T, ReadError>>,
        mut inspect: impl FnMut(&mut Self, T) -> Result<bool, ReadError>,
    ) -> Result<bool, ReadError> {
        for record in records {
            if self.remaining == 0 {
                return Ok(true);
            }
            self.remaining -= 1;
            if inspect(self, record?)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn gpos(&mut self, table: Gpos<'_>) -> Result<bool, ReadError> {
        self.any(table.lookup_list()?.lookups().iter(), Self::lookup)
    }

    fn lookup(&mut self, lookup: PositionLookup<'_>) -> Result<bool, ReadError> {
        match lookup {
            PositionLookup::Single(inner) => self.any(inner.subtables().iter(), Self::single),
            PositionLookup::Pair(inner) => self.any(inner.subtables().iter(), Self::pair),
            PositionLookup::Cursive(inner) => self.any(inner.subtables().iter(), Self::cursive),
            PositionLookup::MarkToBase(inner) => {
                self.any(inner.subtables().iter(), Self::mark_base)
            }
            PositionLookup::MarkToLig(inner) => {
                self.any(inner.subtables().iter(), Self::mark_ligature)
            }
            PositionLookup::MarkToMark(inner) => {
                self.any(inner.subtables().iter(), Self::mark_mark)
            }
            PositionLookup::Extension(inner) => self.any(inner.subtables().iter(), Self::extension),
            PositionLookup::Contextual(inner) => {
                self.any(inner.subtables().iter(), |_, _| Ok(false))
            }
            PositionLookup::ChainContextual(inner) => {
                self.any(inner.subtables().iter(), |_, _| Ok(false))
            }
        }
    }

    fn extension(&mut self, table: ExtensionSubtable<'_>) -> Result<bool, ReadError> {
        match table {
            ExtensionSubtable::Single(inner) => self.single(inner.extension()?),
            ExtensionSubtable::Pair(inner) => self.pair(inner.extension()?),
            ExtensionSubtable::Cursive(inner) => self.cursive(inner.extension()?),
            ExtensionSubtable::MarkToBase(inner) => self.mark_base(inner.extension()?),
            ExtensionSubtable::MarkToLig(inner) => self.mark_ligature(inner.extension()?),
            ExtensionSubtable::MarkToMark(inner) => self.mark_mark(inner.extension()?),
            ExtensionSubtable::Contextual(inner) => {
                inner.extension()?;
                Ok(false)
            }
            ExtensionSubtable::ChainContextual(inner) => {
                inner.extension()?;
                Ok(false)
            }
        }
    }

    fn value(record: &ValueRecord) -> bool {
        !record.x_placement_device.get().is_null()
            || !record.y_placement_device.get().is_null()
            || !record.x_advance_device.get().is_null()
            || !record.y_advance_device.get().is_null()
    }

    fn single(&mut self, table: SinglePos<'_>) -> Result<bool, ReadError> {
        match table {
            SinglePos::Format1(inner) => Ok(Self::value(&inner.value_record())),
            SinglePos::Format2(inner) => self.any(inner.value_records().iter(), |_, record| {
                Ok(Self::value(&record))
            }),
        }
    }

    fn pair(&mut self, table: PairPos<'_>) -> Result<bool, ReadError> {
        match table {
            PairPos::Format1(inner) => self.any(inner.pair_sets().iter(), |scan, set| {
                scan.any(set.pair_value_records().iter(), |_, record| {
                    Ok(Self::value(record.value_record1()) || Self::value(record.value_record2()))
                })
            }),
            PairPos::Format2(inner) => self.any(inner.class1_records().iter(), |scan, class| {
                scan.any(class.class2_records().iter(), |_, record| {
                    Ok(Self::value(record.value_record1()) || Self::value(record.value_record2()))
                })
            }),
        }
    }

    fn anchor(&mut self, anchor: AnchorTable<'_>) -> Result<bool, ReadError> {
        Ok(match anchor {
            AnchorTable::Format1(_) => false,
            AnchorTable::Format2(_) => true,
            AnchorTable::Format3(inner) => {
                !inner.x_device_offset().is_null() || !inner.y_device_offset().is_null()
            }
        })
    }

    fn nullable_anchor(
        &mut self,
        anchor: Option<Result<AnchorTable<'_>, ReadError>>,
    ) -> Result<bool, ReadError> {
        anchor.map_or(Ok(false), |anchor| self.anchor(anchor?))
    }

    fn cursive(&mut self, table: CursivePosFormat1<'_>) -> Result<bool, ReadError> {
        self.any(table.entry_exit_record().iter().map(Ok), |scan, record| {
            Ok(
                scan.nullable_anchor(record.entry_anchor(table.offset_data()))?
                    || scan.nullable_anchor(record.exit_anchor(table.offset_data()))?,
            )
        })
    }

    fn marks(&mut self, table: MarkArray<'_>) -> Result<bool, ReadError> {
        self.any(table.mark_records().iter().map(Ok), |scan, record| {
            scan.anchor(record.mark_anchor(table.offset_data())?)
        })
    }

    fn mark_base(&mut self, table: MarkBasePosFormat1<'_>) -> Result<bool, ReadError> {
        if self.marks(table.mark_array()?)? {
            return Ok(true);
        }
        let bases = table.base_array()?;
        self.any(bases.base_records().iter(), |scan, record| {
            scan.any(
                record.base_anchors(bases.offset_data()).iter().map(Ok),
                Self::nullable_anchor,
            )
        })
    }

    fn mark_ligature(&mut self, table: MarkLigPosFormat1<'_>) -> Result<bool, ReadError> {
        if self.marks(table.mark_array()?)? {
            return Ok(true);
        }
        self.any(
            table.ligature_array()?.ligature_attaches().iter(),
            |scan, ligature| {
                scan.any(ligature.component_records().iter(), |scan, record| {
                    scan.any(
                        record
                            .ligature_anchors(ligature.offset_data())
                            .iter()
                            .map(Ok),
                        Self::nullable_anchor,
                    )
                })
            },
        )
    }

    fn mark_mark(&mut self, table: MarkMarkPosFormat1<'_>) -> Result<bool, ReadError> {
        if self.marks(table.mark1_array()?)? {
            return Ok(true);
        }
        let marks = table.mark2_array()?;
        self.any(marks.mark2_records().iter(), |scan, record| {
            scan.any(
                record.mark2_anchors(marks.offset_data()).iter().map(Ok),
                Self::nullable_anchor,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use read_fonts::{FontData, FontRead};

    fn gpos(lookup_type: u16, subtable: &[u16]) -> Vec<u8> {
        [1, 0, 0, 0, 10, 1, 4, lookup_type, 0, 1, 8]
            .into_iter()
            .chain(subtable.iter().copied())
            .flat_map(u16::to_be_bytes)
            .collect()
    }

    fn inspect(lookup_type: u16, subtable: &[u16]) -> Result<bool, ReadError> {
        let bytes = gpos(lookup_type, subtable);
        Scan::new().gpos(Gpos::read(FontData::new(&bytes))?)
    }

    fn cursive(anchor: &[u16]) -> Vec<u16> {
        [1, 10, 1, 16, 0, 1, 1, 3]
            .into_iter()
            .chain(anchor.iter().copied())
            .collect()
    }

    fn mark_attachment(lookup_type: u16, mark: &[u16], attached: &[u16]) -> Vec<u16> {
        let mut table = vec![1, 12, 18, 1, 24, 0, 1, 1, 3, 1, 1, 4, 1, 0, 6];
        table.extend_from_slice(mark);
        table[5] = u16::try_from(table.len() * 2).unwrap();
        table.extend([1, 4]);
        if lookup_type == 5 {
            table.extend([1, 4]);
        }
        table.extend_from_slice(attached);
        table
    }

    #[test]
    fn device_fields_are_checked_in_both_single_formats_and_both_pair_formats() {
        for flag in [0x10, 0x20, 0x40, 0x80] {
            for offset in [0, 14] {
                assert_eq!(
                    inspect(1, &[1, 8, flag, offset, 1, 1, 3]).unwrap(),
                    offset != 0
                );
                assert_eq!(
                    inspect(1, &[2, 10, flag, 1, offset, 1, 1, 3]).unwrap(),
                    offset != 0
                );
                for (first, second) in [(flag, 0), (0, flag)] {
                    let pair1 = [1, 12, first, second, 1, 18, 1, 1, 3, 1, 4, offset];
                    assert_eq!(inspect(2, &pair1).unwrap(), offset != 0);
                    let pair2 = [
                        2, 18, first, second, 24, 30, 1, 1, offset, 1, 1, 3, 1, 0, 0, 1, 0, 0,
                    ];
                    assert_eq!(inspect(2, &pair2).unwrap(), offset != 0);
                }
            }
        }
    }

    #[test]
    fn anchor_devices_and_contour_points_are_rejected_across_attachment_lookups() {
        for (anchor, unsupported) in [
            (&[1, 10, 20][..], false),
            (&[2, 10, 20, 3][..], true),
            (&[3, 10, 20, 0, 0][..], false),
            (&[3, 10, 20, 10, 0][..], true),
            (&[3, 10, 20, 0, 10][..], true),
        ] {
            assert_eq!(inspect(3, &cursive(anchor)).unwrap(), unsupported);
            for lookup in [4, 5, 6] {
                assert_eq!(
                    inspect(lookup, &mark_attachment(lookup, anchor, &[1, 0, 0])).unwrap(),
                    unsupported,
                );
                assert_eq!(
                    inspect(lookup, &mark_attachment(lookup, &[1, 0, 0], anchor)).unwrap(),
                    unsupported,
                );
            }
        }
        assert!(!inspect(3, &[1, 10, 1, 0, 0, 1, 1, 3]).unwrap());
    }

    #[test]
    fn extensions_are_inspected_and_malformed_offsets_are_reported() {
        for offset in [0, 14] {
            let extension = [1, 1, 0, 8, 1, 8, 0x40, offset, 1, 1, 3];
            assert_eq!(inspect(9, &extension).unwrap(), offset != 0);
        }
        assert!(inspect(9, &[1, 9, 0, 8]).is_err());
        assert!(inspect(9, &[1, 1, 0xffff, 0xffff]).is_err());
        assert!(inspect(3, &[1, 10, 1, 0xffff, 0, 1, 1, 3]).is_err());
    }

    #[test]
    fn bounded_scan_rejects_unverified_excess_and_missing_gpos_is_accepted() {
        let bytes = gpos(1, &[1, 8, 0, 1, 1, 3]);
        let mut scan = Scan { remaining: 1 };
        assert!(
            scan.gpos(Gpos::read(FontData::new(&bytes)).unwrap())
                .unwrap()
        );
        let font = FontRef::new(&[0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        assert!(!requires_device_positioning(&font).unwrap());
    }

    #[test]
    fn paint_constructor_reports_unsupported_positioning_before_shaping() {
        use crate::render::fonts::{FontBook, PaintMetricInput, PaintMetricMode, PaintShapeError};

        let face = FontBook::default().resolve("Roboto", false, false).unwrap();
        let mut bytes = face.bytes().to_vec();
        let table = gpos(1, &[1, 8, 0x40, 14, 1, 1, 3, 17, 17, 1, 0]);
        let table_offset = u32::try_from(bytes.len()).unwrap();
        let table_length = u32::try_from(table.len()).unwrap();
        let count = usize::from(u16::from_be_bytes(bytes[4..6].try_into().unwrap()));
        let record = bytes[12..12 + count * 16]
            .as_chunks_mut::<16>()
            .0
            .iter_mut()
            .find(|record| &record[..4] == b"GPOS")
            .unwrap();
        record[8..12].copy_from_slice(&table_offset.to_be_bytes());
        record[12..16].copy_from_slice(&table_length.to_be_bytes());
        bytes.extend(table);
        let input = PaintMetricInput {
            size: 17.0,
            scale_x: 1.0,
            skew_x: 0.0,
            mode: PaintMetricMode::Normal,
        };
        assert!(matches!(
            super::super::PaintShaper::new(&bytes, face.index, input),
            Err(PaintShapeError::UnsupportedPositioningTables),
        ));
    }
}
