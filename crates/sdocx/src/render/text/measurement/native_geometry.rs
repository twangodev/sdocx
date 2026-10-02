use std::collections::BTreeMap;
use std::ops::Range;

use super::super::TextDiagnosticKind;
use super::super::native_entry::NativeEntryFacts;
use super::{MeasuredGlyph, MeasurementError};
use crate::fonts::RegisteredFontSource;
use crate::fonts::{
    PaintEntryError, PaintEntryGeometry, PaintLayoutError, PaintMeasuredPiece, PaintPieceError,
    PaintShapeError, ResolvedFace,
};
use crate::text_index::TextSource;

pub(super) fn supported_face(face: &ResolvedFace) -> bool {
    const REGULAR_SHA256: [u8; 32] = [
        0x56, 0xa4, 0x52, 0x33, 0xd2, 0x9f, 0x11, 0xb4, 0xdf, 0xb8, 0x6d, 0x24, 0x8e, 0x92, 0x19,
        0x39, 0xd1, 0x15, 0x77, 0x8f, 0x87, 0x32, 0x5e, 0x7a, 0xe8, 0xcc, 0x10, 0x83, 0x83, 0xd6,
        0x66, 0x4d,
    ];
    face.index == 0 && face.font_digest() == &REGULAR_SHA256
}

pub(super) fn shape_error(error: PaintShapeError) -> TextDiagnosticKind {
    use TextDiagnosticKind as Kind;
    match error {
        PaintShapeError::InputBudget | PaintShapeError::ShapingBudget => {
            Kind::UnsupportedMeasurementBudget
        }
        PaintShapeError::InvalidScale
        | PaintShapeError::Metrics(
            crate::fonts::PaintMetricError::InvalidSize
            | crate::fonts::PaintMetricError::UnsupportedTransform,
        ) => Kind::UnsupportedMeasurementStyle,
        PaintShapeError::Metrics(
            crate::fonts::PaintMetricError::UnsupportedFont
            | crate::fonts::PaintMetricError::InvalidFont,
        ) => Kind::UnsupportedMeasurementFont,
        _ => Kind::UnsupportedMeasurementShaping,
    }
}

pub(super) fn piece_error(error: PaintPieceError) -> TextDiagnosticKind {
    use TextDiagnosticKind as Kind;
    match error {
        PaintPieceError::InputBudget
        | PaintPieceError::Layout(PaintLayoutError::LimitExceeded)
        | PaintPieceError::Entry(PaintEntryError::LimitExceeded) => {
            Kind::UnsupportedMeasurementBudget
        }
        PaintPieceError::Shape(error) => shape_error(error),
        _ => Kind::UnsupportedMeasurementShaping,
    }
}

pub(in crate::render) struct NativeMeasuredEntries {
    source: TextSource,
    geometry: PaintEntryGeometry,
    source_instance: Option<RegisteredFontSource>,
    facts: Option<Vec<NativeEntryFacts>>,
}

impl NativeMeasuredEntries {
    pub fn source(&self) -> &TextSource {
        &self.source
    }
    pub fn geometry(&self) -> &PaintEntryGeometry {
        &self.geometry
    }

    pub fn source_instance(&self) -> Option<&RegisteredFontSource> {
        self.source_instance.as_ref()
    }

    pub fn entry_facts_at_utf16(&self, owner: u32) -> Option<NativeEntryFacts> {
        self.source_instance()?;
        let offset = owner.checked_sub(self.geometry.source_range_utf16().start)?;
        self.facts.as_ref()?.get(offset as usize).copied()
    }

    pub(super) fn advance(&self) -> f64 {
        f64::from(
            self.geometry()
                .entries()
                .iter()
                .fold(0.0_f32, |width, entry| width + entry.advance()),
        )
    }
}

pub(super) struct NativeGeometry {
    pub entries: NativeMeasuredEntries,
    pub glyphs: Vec<MeasuredGlyph>,
    pub clusters: Vec<(Range<usize>, f64, Range<usize>)>,
}

impl NativeGeometry {
    pub fn new(
        piece: &PaintMeasuredPiece,
        index: &crate::text_index::TextIndex<'_>,
        context: Range<usize>,
        source: Range<usize>,
        font_size: f32,
        source_instance: Option<RegisteredFontSource>,
    ) -> Result<Self, MeasurementError> {
        let context_source = index
            .source(context.clone())
            .ok_or(MeasurementError::InvalidRange)?;
        if index.slice(context) != Some(piece.source()) {
            return Err(MeasurementError::InvalidRange);
        }
        let source_utf16 = index
            .source(source.clone())
            .ok_or(MeasurementError::InvalidRange)?;
        let geometry = piece.entry_geometry().clone();
        let facts = (matches!(font_size, 17.0 | 50.0)
            && source_instance
                .as_ref()
                .is_some_and(|source| source.language().is_empty() && !source.bitmap()))
        .then(|| {
            piece
                .source()
                .encode_utf16()
                .skip(geometry.source_range_utf16().start as usize)
                .zip(geometry.entries())
                .map(|(unit, entry)| {
                    NativeEntryFacts::measured(unit, font_size, !entry.glyphs().is_empty())
                })
                .collect()
        });
        let entries = NativeMeasuredEntries {
            source: context_source,
            geometry,
            source_instance,
            facts,
        };
        let base = entries.source().utf16().start;
        let selected = source_utf16
            .utf16()
            .start
            .checked_sub(base)
            .ok_or(MeasurementError::InvalidRange)?
            ..source_utf16
                .utf16()
                .end
                .checked_sub(base)
                .ok_or(MeasurementError::InvalidRange)?;
        let entry = entries.geometry();
        if entry.source_range_utf16() != selected {
            return Err(MeasurementError::InvalidCluster);
        }
        let mut owners = BTreeMap::<u32, Range<usize>>::new();
        for glyph in entry.glyphs() {
            owners.entry(glyph.owner_utf16()).or_insert(0..0);
        }
        let anchors = owners.keys().copied().collect::<Vec<_>>();
        let mut sources = BTreeMap::new();
        for (i, &owner) in anchors.iter().enumerate() {
            let next = anchors.get(i + 1).copied().unwrap_or(selected.end);
            let start = base
                .checked_add(owner)
                .and_then(|v| index.utf16_to_char(v))
                .ok_or(MeasurementError::InvalidCluster)?;
            let end = base
                .checked_add(next)
                .and_then(|v| index.utf16_to_char(v))
                .ok_or(MeasurementError::InvalidCluster)?;
            let range = index
                .source(start..end)
                .ok_or(MeasurementError::InvalidCluster)?;
            sources.insert(owner, range);
        }
        let transport: Vec<_> = piece
            .shaped_runs()
            .iter()
            .flat_map(|run| run.glyphs())
            .collect();
        if transport.len() != entry.glyphs().len() {
            return Err(MeasurementError::InvalidCluster);
        }
        let mut glyphs = Vec::with_capacity(transport.len());
        for (position, (glyph, shaped)) in entry.glyphs().iter().zip(transport).enumerate() {
            if glyph.id() != shaped.id || glyph.owner_utf16() != shaped.owner_utf16 {
                return Err(MeasurementError::InvalidCluster);
            }
            let owner = owners
                .get_mut(&glyph.owner_utf16())
                .ok_or(MeasurementError::InvalidCluster)?;
            if owner.start == owner.end {
                owner.start = position;
            } else if owner.end != position {
                return Err(MeasurementError::InvalidCluster);
            }
            owner.end = position + 1;
            let advance = |value: i32| f64::from((value as f32 / 256.0_f32) / 100.0_f32);
            glyphs.push(MeasuredGlyph {
                source: sources[&glyph.owner_utf16()].clone(),
                id: glyph.id(),
                owner_offset: glyph.position().map(f64::from),
                transport_advance: [advance(shaped.x_advance), -advance(shaped.y_advance)],
            });
        }
        let logical_advance = |range: Range<u32>| -> Result<f64, MeasurementError> {
            let mut advance = 0.0_f32;
            for owner in range {
                advance += entry
                    .entry_at_utf16(owner)
                    .ok_or(MeasurementError::InvalidCluster)?
                    .advance();
            }
            if !advance.is_finite() {
                return Err(MeasurementError::InvalidCluster);
            }
            Ok(f64::from(advance))
        };
        let mut clusters = Vec::new();
        let first = anchors.first().copied().unwrap_or(selected.end);
        if first > selected.start {
            let end = base
                .checked_add(first)
                .and_then(|v| index.utf16_to_char(v))
                .ok_or(MeasurementError::InvalidCluster)?;
            clusters.push((
                source.start..end,
                logical_advance(selected.start..first)?,
                0..0,
            ));
        }
        for (&owner, glyph_range) in &owners {
            let range = sources[&owner].characters().clone();
            let end = index
                .char_to_utf16(range.end)
                .and_then(|v| v.checked_sub(base))
                .ok_or(MeasurementError::InvalidCluster)?;
            clusters.push((range, logical_advance(owner..end)?, glyph_range.clone()));
        }
        Ok(Self {
            entries,
            glyphs,
            clusters,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::{
        FontBook, PaintItemization, PaintShapeDirection, PaintSpanProfile, PaintTextRequest,
    };
    use crate::text_index::TextIndex;

    #[test]
    fn measured_entry_facts_match_actual_regular_cell_production() {
        use sha2::{Digest, Sha256};
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-cell-measurement.json"
        ));
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "890c2f236d6cb6711f0821520a491079fa3304d2b64e108a2d3dfcc787549416"
        );
        let capture: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let fonts = FontBook::default();
        let face = fonts.resolve("Roboto", false, false).unwrap();
        let paint = PaintSpanProfile::new(17.0, 0)
            .unwrap()
            .metric_input()
            .unwrap();
        let mut shaper = face.paint_shaper(paint).unwrap();
        let mut checked_cases = 0;
        let mut checked_entries = 0;
        for case in capture["cases"].as_array().unwrap() {
            if ![
                "subpixel-auto",
                "fractional-width",
                "combining-auto",
                "margin-auto",
            ]
            .contains(&case["name"].as_str().unwrap())
            {
                continue;
            }
            let text = case["native_text_utf8"].as_str().unwrap();
            let index = TextIndex::new(text);
            let itemization =
                PaintItemization::new(text, 0..text.encode_utf16().count() as u32).unwrap();
            let piece = shaper
                .shape_text(PaintTextRequest::new(
                    &itemization,
                    PaintShapeDirection::LeftToRight,
                ))
                .unwrap();
            let geometry = NativeGeometry::new(
                &piece,
                &index,
                0..index.len(),
                0..index.len(),
                17.0,
                fonts.registered_source(&face),
            )
            .unwrap();
            assert_eq!(
                geometry.entries.source_instance(),
                fonts.registered_source(&face).as_ref()
            );
            for expected in case["measured_entries"].as_array().unwrap() {
                let owner = expected["utf16_slot"].as_u64().unwrap() as u32;
                let facts = geometry.entries.entry_facts_at_utf16(owner).unwrap();
                assert_eq!(facts.kind as u32, expected["kind"].as_u64().unwrap() as u32);
                assert_eq!(
                    facts.font_size.to_bits(),
                    expected["font_size_bits"].as_u64().unwrap() as u32
                );
                assert_eq!(
                    facts.height.to_bits(),
                    expected["font_height_bits"].as_u64().unwrap() as u32
                );
                assert_eq!(facts.drawable, expected["drawable"].as_bool().unwrap());
                checked_entries += 1;
            }
            checked_cases += 1;
        }
        assert_eq!((checked_cases, checked_entries), (4, 17));
    }

    #[test]
    fn context_text_must_match_the_measured_source_before_owner_translation() {
        let face = FontBook::default().resolve("Roboto", false, false).unwrap();
        let paint = PaintSpanProfile::new(17.0, 0)
            .unwrap()
            .metric_input()
            .unwrap();
        let mut shaper = face.paint_shaper(paint).unwrap();
        let itemization = PaintItemization::new("AV", 0..2).unwrap();
        let piece = shaper
            .shape_text(PaintTextRequest::new(
                &itemization,
                PaintShapeDirection::LeftToRight,
            ))
            .unwrap();
        let different = TextIndex::new("VA");
        assert!(matches!(
            NativeGeometry::new(&piece, &different, 0..2, 0..2, 17.0, None),
            Err(MeasurementError::InvalidRange)
        ));
    }
}
