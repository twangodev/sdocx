use crate::{Color, DocumentMetadata, Page};

use super::{DEFAULT_INK_DARK_MODE, DEFAULT_INK_LIGHT_MODE, RenderColorMode};

/// Resolved paper and foreground defaults shared by rendering and inspection.
#[derive(Debug, Clone, Copy)]
pub struct RenderTheme {
    background: Color,
}

impl RenderTheme {
    pub fn resolve(page: &Page, metadata: &DocumentMetadata, mode: RenderColorMode) -> Self {
        let light = Color {
            r: 252,
            g: 252,
            b: 252,
        };
        let dark = Color {
            r: 37,
            g: 37,
            b: 37,
        };
        let stored = page.background_color.or(metadata.background_color);
        let background = stored.map_or_else(
            || {
                if mode == RenderColorMode::Dark {
                    dark
                } else {
                    light
                }
            },
            |color| {
                if metadata.dark_mode_compatibility == Some(false) {
                    return color;
                }
                match mode {
                    RenderColorMode::Dark
                        if color.r == color.g
                            && color.g == color.b
                            && matches!(color.r, 252 | 255) =>
                    {
                        dark
                    }
                    RenderColorMode::Light
                        if color.r == color.g
                            && color.g == color.b
                            && matches!(color.r, 0 | 1 | 37) =>
                    {
                        light
                    }
                    _ => color,
                }
            },
        );
        Self { background }
    }

    pub fn background(self) -> Color {
        self.background
    }

    pub fn is_dark(self) -> bool {
        is_dark_background(self.background)
    }

    pub fn default_ink(self) -> &'static str {
        if self.is_dark() {
            DEFAULT_INK_DARK_MODE
        } else {
            DEFAULT_INK_LIGHT_MODE
        }
    }
}

pub(super) fn is_dark_background(color: Color) -> bool {
    299 * u32::from(color.r) + 587 * u32::from(color.g) + 114 * u32::from(color.b) < 128_000
}
