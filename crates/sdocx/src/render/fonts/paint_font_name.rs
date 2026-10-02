use std::collections::BTreeMap;
use std::sync::Arc;

use super::{FontBook, FontError, PaintShapeDirection, ResolvedFace};

const MAX_NAME_BYTES: usize = 4096;
const MAX_REGISTERED_NAMES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeFontStyle {
    weight: u16,
    italic: bool,
}

impl NativeFontStyle {
    pub fn weight(self) -> u16 {
        self.weight
    }
    pub fn italic(self) -> bool {
        self.italic
    }
}

const REGULAR: NativeFontStyle = NativeFontStyle {
    weight: 400,
    italic: false,
};
const STYLE_PATTERNS: [(&str, NativeFontStyle); 12] = [
    ("-Regular", REGULAR),
    (
        "-Italic",
        NativeFontStyle {
            weight: 400,
            italic: true,
        },
    ),
    (
        "-BoldItalic",
        NativeFontStyle {
            weight: 700,
            italic: true,
        },
    ),
    (
        "-Bold",
        NativeFontStyle {
            weight: 700,
            italic: false,
        },
    ),
    (
        "-ThinItalic",
        NativeFontStyle {
            weight: 100,
            italic: true,
        },
    ),
    (
        "-Thin",
        NativeFontStyle {
            weight: 100,
            italic: false,
        },
    ),
    (
        "-LightItalic",
        NativeFontStyle {
            weight: 300,
            italic: true,
        },
    ),
    (
        "-Light",
        NativeFontStyle {
            weight: 300,
            italic: false,
        },
    ),
    (
        "-MediumItalic",
        NativeFontStyle {
            weight: 500,
            italic: true,
        },
    ),
    (
        "-Medium",
        NativeFontStyle {
            weight: 500,
            italic: false,
        },
    ),
    (
        "-BlackItalic",
        NativeFontStyle {
            weight: 900,
            italic: true,
        },
    ),
    (
        "-Black",
        NativeFontStyle {
            weight: 900,
            italic: false,
        },
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFontNameRequest {
    span_name: Option<Arc<str>>,
    default_name: Option<Arc<str>>,
    direction: PaintShapeDirection,
    style_override: Option<NativeFontStyle>,
}

impl NativeFontNameRequest {
    pub fn new(
        span_name: Option<&str>,
        default_name: Option<&str>,
        direction: PaintShapeDirection,
    ) -> Result<Self, NativeFontNameError> {
        for name in [span_name, default_name].into_iter().flatten() {
            validate_name(name)?;
        }
        let selected_name = span_name.or(default_name);
        let style_override = selected_name.and_then(|name| {
            STYLE_PATTERNS.iter().find_map(|(pattern, style)| {
                name.rfind(pattern)
                    .filter(|&index| index > 0)
                    .map(|_| *style)
            })
        });
        Ok(Self {
            span_name: span_name.map(Arc::from),
            default_name: default_name.map(Arc::from),
            direction,
            style_override,
        })
    }
    pub fn span_name(&self) -> Option<&str> {
        self.span_name.as_deref()
    }
    pub fn default_name(&self) -> Option<&str> {
        self.default_name.as_deref()
    }
    pub fn selected_name(&self) -> Option<&str> {
        self.span_name().or(self.default_name())
    }
    pub fn direction(&self) -> PaintShapeDirection {
        self.direction
    }
    pub fn style_override(&self) -> Option<NativeFontStyle> {
        self.style_override
    }
    pub fn parser_style(&self) -> NativeFontStyle {
        self.style_override.unwrap_or(REGULAR)
    }
}

/// Native name registrations and aliases into the caller's font database.
/// File aliases do not load files or restrict the database's family membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFontNameConfig {
    default_family: Arc<str>,
    registered_names: BTreeMap<Arc<str>, Arc<str>>,
    database_families: BTreeMap<Arc<str>, Arc<str>>,
}

impl NativeFontNameConfig {
    pub fn new(default_family: &str) -> Result<Self, NativeFontNameError> {
        validate_family(default_family)?;
        Ok(Self {
            default_family: Arc::from(default_family),
            registered_names: BTreeMap::new(),
            database_families: BTreeMap::new(),
        })
    }

    pub fn with_family_alias(
        mut self,
        native_family: &str,
        database_family: &str,
    ) -> Result<Self, NativeFontNameError> {
        validate_family(native_family)?;
        validate_family(database_family)?;
        if self.database_families.len() == MAX_REGISTERED_NAMES
            && !self.database_families.contains_key(native_family)
        {
            return Err(NativeFontNameError::InputBudget);
        }
        self.database_families
            .insert(Arc::from(native_family), Arc::from(database_family));
        Ok(self)
    }

    pub fn with_font_file(
        mut self,
        file_name: &str,
        family: &str,
    ) -> Result<Self, NativeFontNameError> {
        validate_name(file_name)?;
        validate_family(family)?;
        for delimiter in ['.', '-'] {
            if let Some(index) = file_name.rfind(delimiter).filter(|&index| index > 0) {
                if self
                    .registered_names
                    .get(&file_name[..index])
                    .is_some_and(|existing| existing.as_ref() != family)
                {
                    return Err(NativeFontNameError::ConflictingRegistration);
                }
                if self.registered_names.len() == MAX_REGISTERED_NAMES
                    && !self.registered_names.contains_key(&file_name[..index])
                {
                    return Err(NativeFontNameError::InputBudget);
                }
                self.registered_names
                    .insert(Arc::from(&file_name[..index]), Arc::from(family));
            }
        }
        Ok(self)
    }

    pub fn default_family(&self) -> &str {
        &self.default_family
    }
}

#[derive(Debug, Clone)]
pub struct NativeFontNameResolution {
    request: NativeFontNameRequest,
    family: Arc<str>,
    typeface_style: NativeFontStyle,
    face: ResolvedFace,
    used_default_family: bool,
}

impl NativeFontNameResolution {
    pub fn request(&self) -> &NativeFontNameRequest {
        &self.request
    }
    pub fn family(&self) -> &str {
        &self.family
    }
    pub fn typeface_style(&self) -> NativeFontStyle {
        self.typeface_style
    }
    pub fn face(&self) -> &ResolvedFace {
        &self.face
    }
    pub fn used_default_family(&self) -> bool {
        self.used_default_family
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NativeFontNameError {
    #[error("native font names exceed the bounded name or registration budget")]
    InputBudget,
    #[error("native font names containing embedded NUL require unsupported string transport")]
    EmbeddedNul,
    #[error("native font configuration requires a nonempty family")]
    EmptyFamily,
    #[error("conflicting native name registrations require unverified XML ordering semantics")]
    ConflictingRegistration,
    #[error("native font resolution requires an explicit family configuration and database alias")]
    UnsupportedConfiguration,
    #[error(transparent)]
    Font(#[from] FontError),
}

impl FontBook {
    /// Resolves native NAME metadata using explicit registrations and caller-database faces.
    /// Physical matching follows this database; native file parity is captured for pinned Roboto.
    pub fn resolve_native_name(
        &self,
        request: &NativeFontNameRequest,
    ) -> Result<NativeFontNameResolution, NativeFontNameError> {
        let configuration = self
            .native_name_config()
            .ok_or(NativeFontNameError::UnsupportedConfiguration)?;
        let registered_family = request
            .selected_name()
            .and_then(|name| configuration.registered_names.get(name));
        let native_family = registered_family.unwrap_or(&configuration.default_family);
        let physical_family = configuration
            .database_families
            .get(native_family)
            .ok_or(NativeFontNameError::UnsupportedConfiguration)?;
        let style = request.style_override.unwrap_or(REGULAR);
        let face = self.resolve_native_style(physical_family, style)?;
        let typeface_style = if let Some(style) = request.style_override {
            style
        } else {
            let physical =
                rustybuzz::Face::from_slice(face.bytes(), face.index).ok_or_else(|| {
                    FontError::InvalidData {
                        family: physical_family.to_string(),
                    }
                })?;
            NativeFontStyle {
                weight: if physical.is_bold() { 700 } else { 400 },
                italic: physical.is_italic(),
            }
        };
        let family = if request.selected_name().is_none_or(str::is_empty)
            && request.direction == PaintShapeDirection::RightToLeft
        {
            Arc::from("")
        } else {
            native_family.clone()
        };
        Ok(NativeFontNameResolution {
            request: request.clone(),
            family,
            typeface_style,
            face,
            used_default_family: registered_family.is_none(),
        })
    }
}

fn validate_name(name: &str) -> Result<(), NativeFontNameError> {
    if name.len() > MAX_NAME_BYTES {
        return Err(NativeFontNameError::InputBudget);
    }
    if name.contains('\0') {
        return Err(NativeFontNameError::EmbeddedNul);
    }
    Ok(())
}

fn validate_family(family: &str) -> Result<(), NativeFontNameError> {
    validate_name(family)?;
    if family.is_empty() {
        return Err(NativeFontNameError::EmptyFamily);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
