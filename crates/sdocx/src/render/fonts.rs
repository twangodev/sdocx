//! Pinned font resolution and shaping shared by vector text consumers.

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};

pub use fontdb;
use fontdb::{Database, Family, ID, Query, Style, Weight};
pub use rustybuzz::{Direction, Feature, UnicodeBuffer};

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
    pub index: u32,
    pub metrics: FontMetrics,
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
        let mut faces = self.faces.lock().expect("font cache lock");
        if let Some(face) = faces.get(&id) {
            return Ok(face.clone());
        }
        let info = self.database.face(id).ok_or_else(missing)?;
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
    pub fn bytes(&self) -> &[u8] {
        self.data.as_ref().as_ref()
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
}
