//! Pinned font resolution and shaping shared by vector text consumers.

use std::collections::HashMap;
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
    pub index: u32,
    pub metrics: FontMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontMetrics {
    /// Glyph positions and metrics use these font units per em.
    pub units_per_em: u16,
    pub ascent: i16,
    pub descent: i16,
    pub line_gap: i16,
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
            },
            data,
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
}
