use std::sync::Arc;

use sha2::{Digest, Sha256};

const FAMILY_PREFIX: &str = "sdocx-face-";

pub(super) fn data_digest(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

pub(super) fn family(data: &[u8], index: u32) -> Arc<str> {
    let mut digest = Sha256::new();
    digest.update(data);
    digest.update(index.to_be_bytes());
    format!("{FAMILY_PREFIX}{:x}", digest.finalize()).into()
}

/// Physical SVG family identities in one font database.
#[derive(Debug, Clone)]
pub struct SvgFontFamilies {
    faces: std::collections::HashMap<Arc<str>, fontdb::ID>,
}

impl SvgFontFamilies {
    pub fn new(database: &fontdb::Database) -> Self {
        Self {
            faces: database
                .faces()
                .filter_map(|face| {
                    database.with_face_data(face.id, |data, index| (family(data, index), face.id))
                })
                .collect(),
        }
    }

    pub fn resolve(&self, family: &str) -> Option<fontdb::ID> {
        self.faces.get(family).copied()
    }

    #[cfg(feature = "pdf")]
    pub(crate) fn is_unavailable(&self, family: &str) -> bool {
        family.strip_prefix(FAMILY_PREFIX).is_some_and(|digest| {
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) && self.resolve(family).is_none()
    }

    #[cfg(feature = "pdf")]
    pub(crate) fn usvg_resolver(&self) -> usvg::FontResolver<'static> {
        let faces = self.clone();
        let default = usvg::FontResolver::default_font_selector();
        usvg::FontResolver {
            select_font: Box::new(move |font, database| {
                font.families()
                    .first()
                    .and_then(|requested| match requested {
                        usvg::FontFamily::Named(name) => faces.resolve(name),
                        _ => None,
                    })
                    .or_else(|| default(font, database))
            }),
            select_fallback: usvg::FontResolver::default_fallback_selector(),
        }
    }
}

#[cfg(feature = "pdf")]
/// Resolves physical SVG families while preserving logical font lookup.
/// Generated carriers put their physical family first. The returned resolver
/// must be paired with the same database in `usvg::Options::fontdb`.
pub fn svg_font_resolver(database: &fontdb::Database) -> usvg::FontResolver<'static> {
    SvgFontFamilies::new(database).usvg_resolver()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::{FontBook, fontdb};

    #[test]
    fn physical_family_keeps_collection_faces_distinct() {
        let data = include_bytes!("../../../tests/assets/fonts/Roboto-DejaVuSans.ttc");
        assert_ne!(family(data, 0), family(data, 1));
    }

    #[test]
    fn changing_public_collection_index_keeps_current_physical_identity() {
        let mut database = fontdb::Database::new();
        database.load_font_data(
            include_bytes!("../../../tests/assets/fonts/Roboto-DejaVuSans.ttc").to_vec(),
        );
        let book = FontBook::new(Arc::new(database));
        let mut first = book.resolve("Roboto", false, false).unwrap();
        let second = book.resolve("DejaVu Sans", false, false).unwrap();
        let cached = first.svg_family();
        assert!(Arc::ptr_eq(&cached, &first.svg_family()));
        first.index = second.index;
        assert_ne!(cached, first.svg_family());
        assert_eq!(first.svg_family(), second.svg_family());
        let families = SvgFontFamilies::new(&book.database());
        assert_eq!(families.resolve(&first.svg_family()), Some(second.id));
        first.index = 0;
        assert!(Arc::ptr_eq(&cached, &first.svg_family()));
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn logical_family_selection_delegates_to_the_default_selector() {
        use crate::render::vector::{FontFamily, Scene, Svg, TSpan, Text};

        let book = FontBook::default();
        let database = book.database();
        let mut scene = Scene::new(Svg::new().width(100).height(100));
        scene.scope(Text::new("").x(10).y(30), |scene| {
            scene.push(
                TSpan::new("W")
                    .font_size(20)
                    .family(FontFamily::Named("Roboto"))
                    .bold(),
            );
        });
        let options = usvg::Options {
            font_resolver: svg_font_resolver(&database),
            fontdb: database,
            ..Default::default()
        };
        let tree = usvg::Tree::from_str(&scene.finish(), &options).unwrap();
        let actual = tree.root().children().iter().find_map(|node| match node {
            usvg::Node::Text(text) => text
                .layouted()
                .first()?
                .positioned_glyphs
                .first()
                .map(|glyph| glyph.font),
            _ => None,
        });
        assert_eq!(
            actual,
            Some(book.resolve("Roboto", true, false).unwrap().id)
        );
    }

    #[test]
    fn changing_logical_family_preserves_physical_family() {
        let defaults = FontBook::default();
        let original = defaults.resolve("Roboto", false, false).unwrap();
        let mut renamed = defaults.database().face(original.id).unwrap().clone();
        renamed.id = fontdb::ID::dummy();
        renamed.families = vec![(
            "Renamed Logical Family".into(),
            fontdb::Language::English_UnitedStates,
        )];
        let mut database = fontdb::Database::new();
        database.push_face_info(renamed);
        let book = FontBook::new(Arc::new(database));
        let renamed = book
            .resolve("Renamed Logical Family", false, false)
            .unwrap();
        assert_ne!(original.family, renamed.family);
        assert_eq!(original.svg_family(), renamed.svg_family());
    }
}
