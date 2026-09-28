use crate::{Color, DocumentMetadata, Page};

use super::{DEFAULT_INK_DARK_MODE, DEFAULT_INK_LIGHT_MODE, RenderColorMode};

/// Resolved paper and foreground defaults shared by rendering and inspection.
#[derive(Debug, Clone, Copy)]
pub struct RenderTheme {
    background: Color,
    adapt_colors: bool,
}

impl RenderTheme {
    pub(super) fn for_canvas(dark: bool) -> Self {
        let value = if dark { 37 } else { 252 };
        Self {
            background: Color {
                r: value,
                g: value,
                b: value,
            },
            adapt_colors: dark,
        }
    }

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
        Self {
            background,
            adapt_colors: metadata.dark_mode_compatibility != Some(false)
                && (is_dark_background(background)
                    || (mode == RenderColorMode::Light && stored.is_some_and(is_dark_background))),
        }
    }

    pub(super) fn on_background(self, background: Color) -> Self {
        Self { background, ..self }
    }

    pub(super) fn foreground_color(self, color: Color) -> Color {
        let reversed = reverse_color(color);
        if self.adapt_colors
            && contrast(reversed, self.background) > contrast(color, self.background)
        {
            reversed
        } else {
            color
        }
    }

    pub(super) fn foreground(self, color: Option<Color>) -> String {
        color.map_or_else(
            || self.default_ink().into(),
            |color| super::vector::color_hex(&self.foreground_color(color)),
        )
    }

    pub(super) fn on_surface(self, color: Color, opacity: f64) -> Self {
        let blend =
            |fg, bg| (f64::from(fg) * opacity + f64::from(bg) * (1.0 - opacity)).round() as u8;
        self.on_background(Color {
            r: blend(color.r, self.background.r),
            g: blend(color.g, self.background.g),
            b: blend(color.b, self.background.b),
        })
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

fn contrast(a: Color, b: Color) -> f64 {
    let luminance = |color: Color| {
        let linear = |v| {
            let v = f64::from(v) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
    };
    let a = luminance(a);
    let b = luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

pub(super) fn reverse_color(color: Color) -> Color {
    let min = color.r.min(color.g).min(color.b);
    let max = color.r.max(color.g).max(color.b);
    let lightness = (f32::from(max) / 255.0 + f32::from(min) / 255.0) * 0.5;
    if (0.4..=0.6).contains(&lightness) {
        return color;
    }
    // Reversing HSL lightness preserves chroma and shifts each RGB channel equally.
    let offset = 255 - i16::from(min) - i16::from(max);
    let channel = |value| (i16::from(value) + offset) as u8;
    Color {
        r: channel(color.r),
        g: channel(color.g),
        b: channel(color.b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_match_native_dark_theme() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/theme-colors.json"))
                .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let input = u32::from_str_radix(case["input"].as_str().unwrap(), 16).unwrap();
            let expected = u32::from_str_radix(case["dark"].as_str().unwrap(), 16).unwrap();
            let color = reverse_color(Color {
                r: (input >> 16) as u8,
                g: (input >> 8) as u8,
                b: input as u8,
            });
            let actual = (input & 0xff000000)
                | u32::from(color.r) << 16
                | u32::from(color.g) << 8
                | u32::from(color.b);
            assert_eq!(actual, expected, "input {input:08x}");
        }
    }
}
