//! Pinned font resolution and shaping shared by vector text consumers.

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};

pub use fontdb;
use fontdb::{Database, Family, ID, Query, Style, Weight};
pub use rustybuzz::{Direction, Feature, UnicodeBuffer};

mod paint_metrics;
pub use paint_metrics::{
    PaintGlyphMetrics, PaintInkBounds, PaintMetricError, PaintMetricInput, PaintMetricMode,
    PaintMetrics,
};

mod paint_itemization;
mod paint_layout;
pub use paint_itemization::{PaintItemization, PaintItemizationError, PaintScriptChunk};
mod paint_span;
pub use paint_layout::{
    PaintEntryError, PaintEntryGeometry, PaintEntryGlyph, PaintGlyphPlacement, PaintLayout,
    PaintLayoutError, PaintLogicalEntry,
};
pub use paint_span::{PaintSpanError, PaintSpanProfile};

mod paint_shaping;
pub use paint_shaping::{
    PaintFeatureProfile, PaintMeasuredPiece, PaintPieceError, PaintShapeClusterLevel,
    PaintShapeDirection, PaintShapeError, PaintShapeFeature, PaintShapeRequest, PaintShapeScale,
    PaintShapedGlyph, PaintShapedRun, PaintShaper, PaintSourceInfo, PaintTextRequest,
};

#[cfg(test)]
#[path = "fonts/native_shaping_tests.rs"]
mod native_shaping_tests;

#[cfg(test)]
#[path = "fonts/native_gpos_tests.rs"]
mod native_gpos_tests;

#[cfg(test)]
#[path = "fonts/native_scaling_tests.rs"]
mod native_scaling_tests;

#[derive(Clone)]
/// A shared font database with cached faces. Defaults contain pinned Roboto families.
pub struct FontBook {
    database: Arc<Database>,
    faces: Arc<Mutex<HashMap<ID, ResolvedFace>>>,
}

#[derive(Clone)]
pub struct ResolvedFace {
    pub id: ID,
    pub family: String,
    pub weight: Weight,
    pub style: Style,
    data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    ink_bounds: Arc<Mutex<HashMap<u16, Option<rustybuzz::ttf_parser::Rect>>>>,
    has_cbdt_table: bool,
    pub index: u32,
    pub metrics: FontMetrics,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FontSynthesis {
    pub bold: bool,
    pub italic: bool,
}

impl FontSynthesis {
    pub fn skew_x(self) -> f32 {
        if self.italic { -0.25 } else { 0.0 }
    }
}

impl fmt::Debug for ResolvedFace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedFace")
            .field("id", &self.id)
            .field("family", &self.family)
            .field("weight", &self.weight)
            .field("style", &self.style)
            .field("index", &self.index)
            .field("metrics", &self.metrics)
            .field("has_cbdt_table", &self.has_cbdt_table)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub struct FontSelection {
    pub face: ResolvedFace,
    pub requested_family: String,
    pub used_fallback: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontMetrics {
    /// Glyph positions and metrics use these font units per em.
    pub units_per_em: u16,
    pub ascent: i16,
    pub descent: i16,
    pub line_gap: i16,
    pub cap_height: Option<i16>,
}

impl FontMetrics {
    pub fn cap_height_ratio(self) -> Option<f64> {
        let height = self.cap_height?;
        if height <= 0 || self.units_per_em == 0 {
            return None;
        }
        Some(f64::from(height) / f64::from(self.units_per_em))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapedGlyph {
    pub id: u32,
    /// Caller cluster index; `UnicodeBuffer::push_str` uses UTF-8 byte offsets.
    pub cluster: u32,
    pub x_advance: i32,
    pub y_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub unsafe_to_break: bool,
}

#[derive(Debug)]
pub struct ShapedRun {
    pub glyphs: Vec<ShapedGlyph>,
    pub direction: Direction,
    pub metrics: FontMetrics,
}

#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("font family {family:?} is unavailable (bold={bold}, italic={italic})")]
    MissingFont {
        family: String,
        bold: bool,
        italic: bool,
    },
    #[error("font data for {family:?} is unavailable")]
    UnavailableData { family: String },
    #[error("font data for {family:?} is invalid")]
    InvalidData { family: String },
}

impl FontBook {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            faces: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn database(&self) -> Arc<Database> {
        self.database.clone()
    }

    /// Select a face from this database, falling back to its default or available families.
    pub fn resolve_with_fallback(
        &self,
        family: &str,
        bold: bool,
        italic: bool,
    ) -> Result<FontSelection, FontError> {
        let selection = |face, used_fallback| FontSelection {
            face,
            requested_family: family.to_owned(),
            used_fallback,
        };
        let requested_error = match self.resolve(family, bold, italic) {
            Ok(face) => return Ok(selection(face, false)),
            Err(error @ FontError::MissingFont { .. }) => error,
            Err(error) => return Err(error),
        };
        match self.resolve("sans-serif", bold, italic) {
            Ok(face) => return Ok(selection(face, true)),
            Err(FontError::MissingFont { .. }) => {}
            Err(error) => return Err(error),
        }
        let families = self
            .database
            .faces()
            .flat_map(|face| face.families.iter().map(|(name, _)| name.as_str()))
            .collect::<BTreeSet<_>>();
        for fallback in families {
            match self.resolve(fallback, bold, italic) {
                Ok(face) => return Ok(selection(face, true)),
                Err(FontError::MissingFont { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        Err(requested_error)
    }

    /// Select a covering face for an indivisible text cluster from this database.
    /// If none covers it, retain the preferred face for missing-glyph diagnostics.
    pub fn resolve_for_text(
        &self,
        preferred: &ResolvedFace,
        text: &str,
        bold: bool,
        italic: bool,
    ) -> Result<ResolvedFace, FontError> {
        if preferred.covers(text)? {
            return Ok(preferred.clone());
        }
        let families = self
            .database
            .faces()
            .flat_map(|face| face.families.iter().map(|(name, _)| name.as_str()))
            .collect::<BTreeSet<_>>();
        let mut candidates = self.database.as_ref().clone();
        for family in
            std::iter::once(Family::SansSerif).chain(families.into_iter().map(Family::Name))
        {
            let query = Query {
                families: &[family],
                weight: if bold { Weight::BOLD } else { Weight::NORMAL },
                style: if italic { Style::Italic } else { Style::Normal },
                ..Query::default()
            };
            while let Some(id) = candidates.query(&query) {
                candidates.remove_face(id);
                let name = self.database.family_name(&family);
                if let Ok(face) = self.resolve_id(id, name)
                    && self.can_select_face(&face)
                    && face.covers(text).unwrap_or(false)
                {
                    return Ok(face);
                }
            }
        }
        Ok(preferred.clone())
    }

    fn can_select_face(&self, face: &ResolvedFace) -> bool {
        self.database.query(&Query {
            families: &[Family::Name(&face.family)],
            weight: face.weight,
            style: face.style,
            ..Query::default()
        }) == Some(face.id)
    }

    pub fn resolve(
        &self,
        family: &str,
        bold: bool,
        italic: bool,
    ) -> Result<ResolvedFace, FontError> {
        let query_family = match family {
            "sans-serif" => Family::SansSerif,
            "monospace" => Family::Monospace,
            "serif" => Family::Serif,
            name => Family::Name(name),
        };
        let missing = || FontError::MissingFont {
            family: family.to_owned(),
            bold,
            italic,
        };
        let id = self
            .database
            .query(&Query {
                families: &[query_family],
                weight: if bold { Weight::BOLD } else { Weight::NORMAL },
                style: if italic { Style::Italic } else { Style::Normal },
                ..Query::default()
            })
            .ok_or_else(missing)?;
        self.resolve_id(id, family)
    }

    fn resolve_id(&self, id: ID, family: &str) -> Result<ResolvedFace, FontError> {
        let mut faces = self.faces.lock().expect("font cache lock");
        if let Some(face) = faces.get(&id) {
            return Ok(face.clone());
        }
        let info = self
            .database
            .face(id)
            .ok_or_else(|| FontError::UnavailableData {
                family: family.to_owned(),
            })?;
        let (source, index) =
            self.database
                .face_source(id)
                .ok_or_else(|| FontError::UnavailableData {
                    family: family.to_owned(),
                })?;
        let data = match source {
            fontdb::Source::Binary(data) => data,
            #[allow(unreachable_patterns)]
            _ => self
                .database
                .with_face_data(id, |data, _| {
                    Arc::new(data.to_vec()) as Arc<dyn AsRef<[u8]> + Send + Sync>
                })
                .ok_or_else(|| FontError::UnavailableData {
                    family: family.to_owned(),
                })?,
        };
        let face = rustybuzz::Face::from_slice(data.as_ref().as_ref(), index).ok_or_else(|| {
            FontError::InvalidData {
                family: family.to_owned(),
            }
        })?;
        let resolved = ResolvedFace {
            id,
            family: info
                .families
                .first()
                .map_or_else(|| family.to_owned(), |(name, _)| name.clone()),
            weight: info.weight,
            style: info.style,
            metrics: FontMetrics {
                units_per_em: face.as_ref().units_per_em(),
                ascent: face.ascender(),
                descent: face.descender(),
                line_gap: face.line_gap(),
                cap_height: face.as_ref().capital_height(),
            },
            has_cbdt_table: face
                .as_ref()
                .raw_face()
                .table_records
                .into_iter()
                .any(|record| record.tag == rustybuzz::ttf_parser::Tag::from_bytes(b"CBDT")),
            data,
            ink_bounds: Arc::new(Mutex::new(HashMap::new())),
            index,
        };
        faces.insert(id, resolved.clone());
        Ok(resolved)
    }
}

impl Default for FontBook {
    fn default() -> Self {
        static DEFAULT: OnceLock<FontBook> = OnceLock::new();
        DEFAULT
            .get_or_init(|| {
                let mut database = Database::new();
                for data in [
                    include_bytes!("../../assets/fonts/Roboto-Regular.ttf").as_slice(),
                    include_bytes!("../../assets/fonts/Roboto-Bold.ttf").as_slice(),
                    include_bytes!("../../assets/fonts/Roboto-Italic.ttf").as_slice(),
                    include_bytes!("../../assets/fonts/Roboto-BoldItalic.ttf").as_slice(),
                    include_bytes!("../../assets/fonts/RobotoMono-Regular.ttf").as_slice(),
                    include_bytes!("../../assets/fonts/RobotoMono-Bold.ttf").as_slice(),
                    include_bytes!("../../assets/fonts/RobotoMono-Italic.ttf").as_slice(),
                    include_bytes!("../../assets/fonts/RobotoMono-BoldItalic.ttf").as_slice(),
                ] {
                    database.load_font_source(fontdb::Source::Binary(Arc::new(data)));
                }
                database.set_sans_serif_family("Roboto");
                database.set_monospace_family("Roboto Mono");
                Self::new(Arc::new(database))
            })
            .clone()
    }
}

impl ResolvedFace {
    pub fn paint_metrics(
        &self,
        input: PaintMetricInput,
    ) -> Result<PaintMetrics<'_>, PaintMetricError> {
        PaintMetrics::new(self.bytes(), self.index, input)
    }

    pub fn paint_shaper(
        &self,
        input: PaintMetricInput,
    ) -> Result<PaintShaper<'_>, PaintShapeError> {
        PaintShaper::new(self.bytes(), self.index, input)
    }

    /// Matches the native bitmap-font gate: the selected face declares a CBDT table.
    pub fn is_bitmap_font(&self) -> bool {
        self.has_cbdt_table
    }

    pub(crate) fn synthesis(
        &self,
        requested_weight: Weight,
        requested_italic: bool,
    ) -> FontSynthesis {
        FontSynthesis {
            bold: requested_weight.0 >= 600
                && requested_weight.0.saturating_sub(self.weight.0) >= 200,
            italic: requested_italic && self.style == Style::Normal,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        self.data.as_ref().as_ref()
    }

    #[cfg(feature = "pdf")]
    pub(crate) fn shared_data(&self) -> Arc<dyn AsRef<[u8]> + Send + Sync> {
        self.data.clone()
    }

    /// Test glyph coverage while preserving shaping normalization and default ignorables.
    pub fn covers(&self, text: &str) -> Result<bool, FontError> {
        let face = rustybuzz::Face::from_slice(self.bytes(), self.index).ok_or_else(|| {
            FontError::InvalidData {
                family: self.family.clone(),
            }
        })?;
        if text.chars().all(|character| {
            face.as_ref()
                .glyph_index(character)
                .is_some_and(|glyph| glyph.0 != 0)
        }) {
            return Ok(true);
        }
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let shaped = rustybuzz::shape(&face, &[], buffer);
        Ok(shaped.glyph_infos().iter().all(|glyph| glyph.glyph_id != 0))
    }

    pub(crate) fn glyph_ink_bounds(
        &self,
        glyph_id: u32,
    ) -> Result<Option<rustybuzz::ttf_parser::Rect>, FontError> {
        let invalid = || FontError::InvalidData {
            family: self.family.clone(),
        };
        let glyph_id = u16::try_from(glyph_id).map_err(|_| invalid())?;
        let mut bounds = self.ink_bounds.lock().expect("glyph ink bounds cache lock");
        if let Some(bounds) = bounds.get(&glyph_id) {
            return Ok(*bounds);
        }
        let face = rustybuzz::Face::from_slice(self.bytes(), self.index).ok_or_else(invalid)?;
        let ink = face
            .as_ref()
            .glyph_bounding_box(rustybuzz::ttf_parser::GlyphId(glyph_id));
        bounds.insert(glyph_id, ink);
        Ok(ink)
    }

    pub fn shape(
        &self,
        mut buffer: UnicodeBuffer,
        features: &[Feature],
    ) -> Result<ShapedRun, FontError> {
        let face = rustybuzz::Face::from_slice(self.bytes(), self.index).ok_or_else(|| {
            FontError::InvalidData {
                family: self.family.clone(),
            }
        })?;
        buffer.guess_segment_properties();
        let direction = buffer.direction();
        let shaped = rustybuzz::shape(&face, features, buffer);
        let glyphs = shaped
            .glyph_infos()
            .iter()
            .zip(shaped.glyph_positions())
            .map(|(info, position)| ShapedGlyph {
                id: info.glyph_id,
                cluster: info.cluster,
                x_advance: position.x_advance,
                y_advance: position.y_advance,
                x_offset: position.x_offset,
                y_offset: position.y_offset,
                unsafe_to_break: info.unsafe_to_break(),
            })
            .collect();
        Ok(ShapedRun {
            glyphs,
            direction,
            metrics: self.metrics,
        })
    }
}

impl ShapedRun {
    pub fn has_missing_glyphs(&self) -> bool {
        self.glyphs.iter().any(|glyph| glyph.id == 0)
    }

    pub fn advance_x(&self) -> i64 {
        self.glyphs
            .iter()
            .map(|glyph| i64::from(glyph.x_advance))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(text: &str) -> UnicodeBuffer {
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer
    }

    fn retag_optional_table(bytes: &mut [u8], index: usize, tag: [u8; 4]) {
        let offset = if bytes.starts_with(b"ttcf") {
            u32::from_be_bytes(bytes[12 + index * 4..16 + index * 4].try_into().unwrap()) as usize
        } else {
            assert_eq!(index, 0);
            0
        };
        let count = usize::from(u16::from_be_bytes(
            bytes[offset + 4..offset + 6].try_into().unwrap(),
        ));
        let directory = &mut bytes[offset + 12..offset + 12 + count * 16];
        let (records, remainder) = directory.as_chunks_mut::<16>();
        assert!(remainder.is_empty());
        let record = records
            .iter_mut()
            .find(|record| &record[..4] == b"gasp")
            .unwrap();
        record[..4].copy_from_slice(&tag);
        records.sort_unstable_by_key(|record| <[u8; 4]>::try_from(&record[..4]).unwrap());
    }

    fn font_book(bytes: Vec<u8>) -> FontBook {
        let mut database = Database::new();
        database.load_font_data(bytes);
        FontBook::new(Arc::new(database))
    }

    #[test]
    fn bitmap_gate_retains_cbdt_directory_presence_without_requiring_bitmap_payloads() {
        let roboto = include_bytes!("../../assets/fonts/Roboto-Regular.ttf");
        assert!(
            !FontBook::default()
                .resolve("Roboto", false, false)
                .unwrap()
                .is_bitmap_font()
        );
        for tag in [*b"CBDT", *b"COLR", *b"SVG ", *b"sbix", *b"EBDT"] {
            let mut bytes = roboto.to_vec();
            retag_optional_table(&mut bytes, 0, tag);
            let raw = rustybuzz::ttf_parser::RawFace::parse(&bytes, 0).unwrap();
            assert!(
                raw.table_records
                    .into_iter()
                    .any(|record| record.tag == rustybuzz::ttf_parser::Tag::from_bytes(&tag))
            );
            let book = font_book(bytes);
            let face = book.resolve("Roboto", false, false).unwrap();
            assert_eq!(face.is_bitmap_font(), tag == *b"CBDT");
            assert_eq!(
                book.resolve("Roboto", false, false)
                    .unwrap()
                    .is_bitmap_font(),
                face.is_bitmap_font()
            );
            assert_eq!(face.clone().is_bitmap_font(), face.is_bitmap_font());
        }
    }

    #[test]
    fn bitmap_gate_uses_selected_collection_face() {
        let collection = include_bytes!("../../tests/assets/fonts/Roboto-DejaVuSans.ttc");
        for selected in [0, 1] {
            let mut bytes = collection.to_vec();
            retag_optional_table(&mut bytes, selected, *b"CBDT");
            let book = font_book(bytes);
            for (index, family) in ["Roboto", "DejaVu Sans"].into_iter().enumerate() {
                let face = book.resolve(family, false, false).unwrap();
                assert_eq!(face.index as usize, index);
                assert_eq!(face.is_bitmap_font(), index == selected);
            }
        }
    }

    #[test]
    fn glyph_ink_bounds_keep_font_units_and_distinguish_inkless_space() {
        let face = FontBook::default().resolve("Roboto", false, false).unwrap();
        let cap = face.shape(buffer("A"), &[]).unwrap().glyphs[0].id;
        let bounds = face.glyph_ink_bounds(cap).unwrap().unwrap();
        assert_eq!(bounds.y_min, 0);
        assert_eq!(bounds.y_max, 1456);
        let space = face.shape(buffer(" "), &[]).unwrap().glyphs[0].id;
        assert_eq!(face.glyph_ink_bounds(space).unwrap(), None);
        assert_eq!(face.glyph_ink_bounds(space).unwrap(), None);
        assert!(matches!(
            face.glyph_ink_bounds(u32::MAX),
            Err(FontError::InvalidData { .. })
        ));
    }

    #[test]
    fn bundled_fonts_resolve_all_styles_and_share_owned_face_data() {
        let book = FontBook::default();
        assert!(Arc::ptr_eq(
            &book.database(),
            &FontBook::default().database()
        ));
        assert_eq!(book.database().faces().count(), 8);
        for family in ["Roboto", "Roboto Mono"] {
            for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
                let face = book.resolve(family, bold, italic).unwrap();
                assert_eq!(face.family, family);
                assert_eq!(
                    face.weight,
                    if bold { Weight::BOLD } else { Weight::NORMAL }
                );
                assert_eq!(
                    face.style,
                    if italic { Style::Italic } else { Style::Normal }
                );
                let repeated = book.resolve(family, bold, italic).unwrap();
                assert_eq!(face.id, repeated.id);
                assert!(Arc::ptr_eq(&face.data, &repeated.data));
            }
        }
        assert_eq!(
            book.resolve("sans-serif", false, false).unwrap().family,
            "Roboto"
        );
        assert_eq!(
            book.resolve("monospace", false, false).unwrap().family,
            "Roboto Mono"
        );
    }

    #[test]
    fn native_synthesis_requires_both_weight_thresholds() {
        let mut face = FontBook::default().resolve("Roboto", false, false).unwrap();
        for (requested, selected, expected) in [
            (599, 399, false),
            (600, 400, true),
            (600, 401, false),
            (700, 500, true),
            (700, 501, false),
            (700, 550, false),
            (700, 700, false),
            (700, 900, false),
        ] {
            face.weight = Weight(selected);
            assert_eq!(face.synthesis(Weight(requested), false).bold, expected);
        }
    }

    #[test]
    fn selected_faces_determine_synthesis_without_changing_the_font() {
        let defaults = FontBook::default();
        let regular = defaults.resolve("Roboto", false, false).unwrap();
        let mut database = Database::new();
        database.load_font_data(regular.bytes().to_vec());
        let regular_only = FontBook::new(Arc::new(database));
        let selected = regular_only.resolve("Roboto", true, true).unwrap();
        assert_eq!(selected.weight, Weight::NORMAL);
        assert_eq!(selected.style, Style::Normal);
        assert_eq!(selected.bytes(), regular.bytes());
        let synthesis = selected.synthesis(Weight::BOLD, true);
        assert_eq!(
            synthesis,
            FontSynthesis {
                bold: true,
                italic: true
            }
        );
        assert_eq!(synthesis.skew_x(), -0.25);
        for (bold, italic) in [(false, false), (false, true), (true, false), (true, true)] {
            let selected = defaults.resolve("Roboto", bold, italic).unwrap();
            let synthesis =
                selected.synthesis(if bold { Weight::BOLD } else { Weight::NORMAL }, italic);
            assert_eq!(synthesis, FontSynthesis::default());
            assert_eq!(synthesis.skew_x(), 0.0);
        }
        let mut oblique = regular;
        oblique.style = Style::Oblique;
        assert!(!oblique.synthesis(Weight::NORMAL, true).italic);
    }

    #[test]
    fn roboto_metrics_and_kerning_use_pinned_font_units() {
        let face = FontBook::default().resolve("Roboto", false, false).unwrap();
        assert_eq!(
            face.metrics,
            FontMetrics {
                units_per_em: 2048,
                ascent: 2146,
                descent: -555,
                line_gap: 0,
                cap_height: Some(1456),
            }
        );
        let run = face.shape(buffer("AV"), &[]).unwrap();
        assert_eq!(run.direction, Direction::LeftToRight);
        assert_eq!(run.metrics, face.metrics);
        assert_eq!(run.advance_x(), 2552);
        assert_eq!(
            run.glyphs
                .iter()
                .map(|glyph| (glyph.id, glyph.cluster, glyph.x_advance))
                .collect::<Vec<_>>(),
            [(38, 0, 1249), (59, 1, 1303)]
        );
        assert!(run.glyphs[1].unsafe_to_break);
        assert!(!run.has_missing_glyphs());
    }

    #[test]
    fn default_face_cap_height_retains_actual_font_metrics_without_estimating_missing_values() {
        let face = FontBook::default()
            .resolve("sans-serif", false, false)
            .unwrap();
        assert_eq!(face.metrics.cap_height_ratio(), Some(0.7109375));
        for cap_height in [None, Some(0), Some(-1)] {
            assert_eq!(
                FontMetrics {
                    cap_height,
                    ..face.metrics
                }
                .cap_height_ratio(),
                None
            );
        }
        assert_eq!(
            FontMetrics {
                units_per_em: 0,
                ..face.metrics
            }
            .cap_height_ratio(),
            None
        );
    }

    #[test]
    fn shaping_preserves_combining_clusters_and_reports_missing_glyphs() {
        let face = FontBook::default().resolve("Roboto", false, false).unwrap();
        let combined = face.shape(buffer("e\u{301}"), &[]).unwrap();
        assert_eq!(combined.glyphs.len(), 1);
        assert_eq!(
            (
                combined.glyphs[0].id,
                combined.glyphs[0].cluster,
                combined.advance_x()
            ),
            (2317, 0, 1085)
        );
        assert!(!combined.has_missing_glyphs());
        let missing = face.shape(buffer("\u{10ffff}"), &[]).unwrap();
        assert_eq!(missing.glyphs.len(), 1);
        assert_eq!((missing.glyphs[0].id, missing.glyphs[0].cluster), (0, 0));
        assert!(missing.has_missing_glyphs());
    }

    #[test]
    fn caller_direction_and_font_database_remain_authoritative() {
        let book = FontBook::new(Arc::new(Database::new()));
        assert!(matches!(
            book.resolve("Roboto", false, false),
            Err(FontError::MissingFont { .. })
        ));
        assert!(matches!(
            FontBook::default().resolve("Missing family", false, false),
            Err(FontError::MissingFont { .. })
        ));
        let mut database = FontBook::default().database().as_ref().clone();
        database.set_sans_serif_family("Roboto Mono");
        assert_eq!(
            FontBook::new(Arc::new(database))
                .resolve("sans-serif", false, false)
                .unwrap()
                .family,
            "Roboto Mono"
        );
        let face = FontBook::default().resolve("Roboto", false, false).unwrap();
        let mut input = buffer("AV");
        input.set_direction(Direction::RightToLeft);
        let run = face.shape(input, &[]).unwrap();
        assert_eq!(run.direction, Direction::RightToLeft);
        assert_eq!(
            run.glyphs
                .iter()
                .map(|glyph| glyph.cluster)
                .collect::<Vec<_>>(),
            [1, 0]
        );
    }

    #[test]
    fn missing_named_families_use_the_configured_default_with_actual_style() {
        let default = FontBook::default();
        for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
            let selected = default
                .resolve_with_fallback("Missing family", bold, italic)
                .unwrap();
            let expected = default.resolve("Roboto", bold, italic).unwrap();
            assert_eq!(selected.requested_family, "Missing family");
            assert!(selected.used_fallback);
            assert_eq!(selected.face.id, expected.id);
            assert_eq!(selected.face.family, expected.family);
            assert_eq!(selected.face.weight, expected.weight);
            assert_eq!(selected.face.style, expected.style);
            assert_eq!(selected.face.metrics, expected.metrics);
            assert_eq!(selected.face.index, expected.index);
            assert!(std::ptr::eq(selected.face.bytes(), expected.bytes()));
        }

        let mut database = default.database().as_ref().clone();
        database.set_sans_serif_family("Roboto Mono");
        let custom = FontBook::new(Arc::new(database));
        let selected = custom
            .resolve_with_fallback("Missing family", true, true)
            .unwrap();
        assert!(selected.used_fallback);
        assert_eq!(selected.face.family, "Roboto Mono");
        assert_eq!(selected.face.weight, Weight::BOLD);
        assert_eq!(selected.face.style, Style::Italic);
        let named = custom
            .resolve_with_fallback("Roboto", false, false)
            .unwrap();
        assert!(!named.used_fallback);
        assert_eq!(named.face.family, "Roboto");
        assert_eq!(named.requested_family, "Roboto");
    }

    #[test]
    fn fallback_uses_only_custom_faces_and_reports_the_closest_actual_style() {
        let source = FontBook::default()
            .resolve("Roboto Mono", true, true)
            .unwrap();
        let mut database = Database::new();
        database.load_font_data(source.bytes().to_vec());
        let book = FontBook::new(Arc::new(database));
        let selected = book
            .resolve_with_fallback("Missing family", false, false)
            .unwrap();
        assert!(selected.used_fallback);
        assert_eq!(selected.face.family, "Roboto Mono");
        assert_eq!(selected.face.weight, Weight::BOLD);
        assert_eq!(selected.face.style, Style::Italic);
        assert_eq!(selected.face.metrics, source.metrics);
        assert_eq!(selected.face.bytes(), source.bytes());
        assert_eq!(book.database().faces().count(), 1);
    }

    #[test]
    fn fallback_family_order_is_independent_of_database_insertion_order() {
        let default = FontBook::default();
        for families in [["Roboto Mono", "Roboto"], ["Roboto", "Roboto Mono"]] {
            let mut database = Database::new();
            database.set_sans_serif_family("Unavailable default");
            for family in families {
                database.load_font_data(
                    default
                        .resolve(family, false, false)
                        .unwrap()
                        .bytes()
                        .to_vec(),
                );
            }
            let selected = FontBook::new(Arc::new(database))
                .resolve_with_fallback("Missing family", true, true)
                .unwrap();
            assert!(selected.used_fallback);
            assert_eq!(selected.face.family, "Roboto");
            assert_eq!(selected.face.weight, Weight::NORMAL);
            assert_eq!(selected.face.style, Style::Normal);
        }
    }

    #[test]
    fn empty_database_fallback_returns_the_original_request_error() {
        let error = FontBook::new(Arc::new(Database::new()))
            .resolve_with_fallback("Original request", true, true)
            .unwrap_err();
        assert!(matches!(
            error,
            FontError::MissingFont {
                family,
                bold: true,
                italic: true
            } if family == "Original request"
        ));
    }

    #[test]
    fn cluster_coverage_keeps_the_requested_face_and_matches_fallback_style() {
        let book = FontBook::default();
        for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
            let preferred = book.resolve("Roboto", bold, italic).unwrap();
            assert!(preferred.covers("AB").unwrap());
            assert!(!preferred.covers("∕").unwrap());
            assert_eq!(
                book.resolve_for_text(&preferred, "AB", bold, italic)
                    .unwrap()
                    .id,
                preferred.id
            );
            let fallback = book
                .resolve_for_text(&preferred, "∕", bold, italic)
                .unwrap();
            assert_eq!(
                fallback.id,
                book.resolve("Roboto Mono", bold, italic).unwrap().id
            );
            assert!(fallback.covers("∕").unwrap());
            assert!(
                !fallback
                    .shape(buffer("∕"), &[])
                    .unwrap()
                    .has_missing_glyphs()
            );
        }
    }

    #[test]
    fn shaping_coverage_preserves_combining_text_and_default_ignorables() {
        let book = FontBook::default();
        let preferred = book.resolve("Roboto", false, false).unwrap();
        for text in [
            "e\u{301}",
            "A\u{200d}",
            "A\u{fe0e}",
            "A\u{fe0f}",
            "\u{2066}A\u{2069}",
            "",
        ] {
            assert!(preferred.covers(text).unwrap(), "{text:?}");
            assert_eq!(
                book.resolve_for_text(&preferred, text, false, false)
                    .unwrap()
                    .id,
                preferred.id,
                "{text:?}"
            );
        }
    }

    #[test]
    fn uncovered_clusters_retain_missing_glyphs_without_hidden_fonts_or_partial_coverage() {
        let bundled = FontBook::default();
        let mut database = Database::new();
        database.load_font_data(
            bundled
                .resolve("Roboto", false, false)
                .unwrap()
                .bytes()
                .to_vec(),
        );
        let custom = FontBook::new(Arc::new(database));
        let preferred = custom.resolve("Roboto", false, false).unwrap();
        let selected = custom
            .resolve_for_text(&preferred, "∕", false, false)
            .unwrap();
        assert_eq!(selected.id, preferred.id);
        assert!(
            selected
                .shape(buffer("∕"), &[])
                .unwrap()
                .has_missing_glyphs()
        );

        let preferred = bundled.resolve("Roboto", false, false).unwrap();
        for text in ["\u{10ffff}", "∕\u{10ffff}"] {
            let selected = bundled
                .resolve_for_text(&preferred, text, false, false)
                .unwrap();
            assert_eq!(selected.id, preferred.id);
            assert!(
                selected
                    .shape(buffer(text), &[])
                    .unwrap()
                    .has_missing_glyphs()
            );
        }
    }

    #[test]
    fn caller_database_controls_coverage_order_and_available_style() {
        let bundled = FontBook::default();
        let roboto = bundled.resolve("Roboto", false, false).unwrap();
        let mono = bundled.resolve("Roboto Mono", true, true).unwrap();
        for reversed in [false, true] {
            let mut database = Database::new();
            database.set_sans_serif_family("Z Configured");
            database.load_font_data(roboto.bytes().to_vec());
            let mono_info = bundled.database.face(mono.id).unwrap();
            let names = if reversed {
                ["Z Configured", "A Alternative"]
            } else {
                ["A Alternative", "Z Configured"]
            };
            for name in names {
                let mut info = mono_info.clone();
                info.families[0].0 = name.into();
                database.push_face_info(info);
            }
            let custom = FontBook::new(Arc::new(database));
            let preferred = custom.resolve("Roboto", false, false).unwrap();
            let selected = custom
                .resolve_for_text(&preferred, "∕", false, false)
                .unwrap();
            assert_eq!(selected.family, "Z Configured");
            assert_eq!(selected.weight, Weight::BOLD);
            assert_eq!(selected.style, Style::Italic);

            let mut database = custom.database.as_ref().clone();
            database.set_sans_serif_family("Unavailable default");
            let custom = FontBook::new(Arc::new(database));
            let selected = custom
                .resolve_for_text(&preferred, "∕", false, false)
                .unwrap();
            assert_eq!(selected.family, "A Alternative");
        }
    }

    #[test]
    fn coverage_can_select_a_less_matching_style_when_the_closest_face_has_no_glyph() {
        let bundled = FontBook::default();
        let mut database = Database::new();
        database.set_sans_serif_family("Variant");
        for family in ["Roboto", "Roboto Mono"] {
            let face = bundled
                .resolve(family, family == "Roboto Mono", false)
                .unwrap();
            let mut info = bundled.database.face(face.id).unwrap().clone();
            info.families[0].0 = "Variant".into();
            database.push_face_info(info);
        }
        let custom = FontBook::new(Arc::new(database));
        let preferred = custom.resolve("Variant", false, false).unwrap();
        assert_eq!(preferred.weight, Weight::NORMAL);
        let selected = custom
            .resolve_for_text(&preferred, "∕", false, false)
            .unwrap();
        assert_eq!(selected.family, "Variant");
        assert_eq!(selected.weight, Weight::BOLD);
        assert!(selected.covers("∕").unwrap());
    }

    #[test]
    fn malformed_requested_data_fails_while_unselected_bad_candidates_are_skipped() {
        let bundled = FontBook::default();
        let preferred = bundled.resolve("Roboto", false, false).unwrap();
        let mut invalid = preferred.clone();
        invalid.data = Arc::new(vec![0_u8; 16]);
        assert!(matches!(
            bundled.resolve_for_text(&invalid, "A", false, false),
            Err(FontError::InvalidData { .. })
        ));

        let mut database = bundled.database.as_ref().clone();
        let mut info = database.face(preferred.id).unwrap().clone();
        info.families[0].0 = "A Broken".into();
        info.source = fontdb::Source::Binary(Arc::new(vec![0_u8; 16]));
        database.push_face_info(info);
        database.set_sans_serif_family("A Broken");
        let custom = FontBook::new(Arc::new(database));
        assert!(matches!(
            custom.resolve_with_fallback("A Broken", false, false),
            Err(FontError::InvalidData { .. })
        ));
        let preferred = custom.resolve("Roboto", false, false).unwrap();
        let selected = custom
            .resolve_for_text(&preferred, "∕", false, false)
            .unwrap();
        assert_eq!(selected.family, "Roboto Mono");
    }

    #[test]
    fn coverage_skips_duplicate_faces_that_svg_cannot_select_by_family_and_style() {
        let bundled = FontBook::default();
        let mut database = Database::new();
        let roboto = bundled.resolve("Roboto", false, false).unwrap();
        let mono = bundled.resolve("Roboto Mono", false, false).unwrap();
        database.load_font_data(roboto.bytes().to_vec());
        database.set_sans_serif_family("Ambiguous");
        for face in [&roboto, &mono] {
            let mut info = bundled.database.face(face.id).unwrap().clone();
            info.families[0].0 = "Ambiguous".into();
            database.push_face_info(info);
        }
        let custom = FontBook::new(Arc::new(database));
        let preferred = custom.resolve("Roboto", false, false).unwrap();
        let selected = custom
            .resolve_for_text(&preferred, "∕", false, false)
            .unwrap();
        assert_eq!(selected.id, preferred.id);

        let mut database = custom.database.as_ref().clone();
        database.load_font_data(mono.bytes().to_vec());
        let custom = FontBook::new(Arc::new(database));
        let selected = custom
            .resolve_for_text(&preferred, "∕", false, false)
            .unwrap();
        assert_eq!(selected.family, "Roboto Mono");
        assert!(selected.covers("∕").unwrap());
    }
}
