use std::sync::Arc;

use super::{FontBook, ResolvedFace};

#[derive(Debug)]
pub(super) struct RegisteredFontRegistry;

#[derive(Debug, Clone)]
pub(crate) struct RegisteredFontSource {
    registry: Arc<RegisteredFontRegistry>,
}

impl PartialEq for RegisteredFontSource {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.registry, &other.registry)
    }
}

impl Eq for RegisteredFontSource {}

impl RegisteredFontSource {
    pub(crate) fn language(&self) -> &str {
        ""
    }

    pub(crate) fn bitmap(&self) -> bool {
        false
    }
}

impl FontBook {
    pub(crate) fn registered_source(&self, face: &ResolvedFace) -> Option<RegisteredFontSource> {
        let registry = self.native_registry.as_ref()?;
        let regular = self.resolve("Roboto", false, false).ok()?;
        (face.id == regular.id && face.index == 0 && face.font_digest() == regular.font_digest())
            .then(|| RegisteredFontSource {
                registry: registry.clone(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_source_metadata_matches_the_initialized_native_registry() {
        use sha2::{Digest, Sha256};
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-font-registry.json"
        ));
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "41d5adbc3de53576d298b052395a22842be356308d186340513d7c891a983a30"
        );
        let capture: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let fonts = FontBook::default();
        let face = fonts.resolve("Roboto", false, false).unwrap();
        let source = fonts.registered_source(&face).unwrap();
        let mut checked = 0;
        for case in capture["cases"].as_array().unwrap() {
            if case["configuration"] != "omitted_language"
                || case["selection"]["physical_font_style"] != 400
            {
                continue;
            }
            assert_eq!(source.language(), case["font_language"].as_str().unwrap());
            assert_eq!(source.bitmap(), case["bitmap"].as_bool().unwrap());
            assert_eq!(case["source_first_request_index"], 0);
            for field in [
                "same_source_as_first_request",
                "same_parent_font_as_first_request",
                "same_implementation_as_first_request",
                "parent_implementation_retains_selected_source",
            ] {
                assert_eq!(case[field], true);
            }
            checked += 1;
        }
        assert_eq!(checked, 82);
    }

    #[test]
    fn clones_retain_source_instances_but_database_and_config_rebuilds_do_not() {
        let fonts = FontBook::default();
        let face = fonts.resolve("Roboto", false, false).unwrap();
        let source = fonts.registered_source(&face).unwrap();
        assert_eq!(source.language(), "");
        assert!(!source.bitmap());
        assert_eq!(source, fonts.clone().registered_source(&face).unwrap());
        assert!(
            fonts
                .registered_source(&fonts.resolve("Roboto", true, false).unwrap())
                .is_none()
        );
        let configuration = fonts.native_name_config().unwrap().clone();
        assert!(
            fonts
                .clone()
                .with_native_name_config(configuration.clone())
                .registered_source(&face)
                .is_none()
        );
        assert!(
            FontBook::new(fonts.database())
                .with_native_name_config(configuration)
                .registered_source(&face)
                .is_none()
        );
    }
}
