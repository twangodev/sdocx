use std::cell::RefCell;

use crate::fonts::{FontBook, FontError, ResolvedFace, UnicodeBuffer};

use super::objects::{ObjectDiagnostic, ObjectDiagnosticKind};
use super::{TextContext, TextSettings, TextStyle};
use crate::render::vector::{EmbeddedFont, Scene};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TextDiagnosticKind {
    UnavailableFamily,
    UnusableFontData,
    MissingGlyphs,
    MeasurementFailure,
    /// Measured glyph positions cannot be reproduced by independently positioned SVG text.
    UnsupportedGlyphPositioning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextDiagnostic {
    pub kind: TextDiagnosticKind,
    pub family: String,
    pub codepoints: Vec<u32>,
}

pub(in crate::render) struct TextRenderer<'a> {
    pub settings: TextSettings,
    pub fonts: &'a FontBook,
    faces: RefCell<Vec<ResolvedFace>>,
    diagnostics: RefCell<Vec<TextDiagnostic>>,
    object_diagnostics: RefCell<Vec<ObjectDiagnostic>>,
}

impl<'a> TextRenderer<'a> {
    pub fn new(settings: TextSettings, fonts: &'a FontBook) -> Self {
        Self {
            settings,
            fonts,
            faces: RefCell::new(Vec::new()),
            diagnostics: RefCell::new(Vec::new()),
            object_diagnostics: RefCell::new(Vec::new()),
        }
    }

    pub fn resolve(&self, style: &TextStyle, context: TextContext) -> Option<ResolvedFace> {
        let family = style.family.as_deref().unwrap_or("Roboto");
        let bold = style.bold && matches!(context, TextContext::Placed);
        match self.fonts.resolve_with_fallback(family, bold, style.italic) {
            Ok(selection) => {
                if selection.used_fallback {
                    self.record(TextDiagnostic {
                        kind: TextDiagnosticKind::UnavailableFamily,
                        family: family.into(),
                        codepoints: Vec::new(),
                    });
                }
                let mut faces = self.faces.borrow_mut();
                if !faces.iter().any(|face| face.id == selection.face.id) {
                    faces.push(selection.face.clone());
                }
                Some(selection.face)
            }
            Err(error) => {
                self.record(TextDiagnostic {
                    kind: match error {
                        FontError::MissingFont { .. } => TextDiagnosticKind::UnavailableFamily,
                        _ => TextDiagnosticKind::UnusableFontData,
                    },
                    family: family.into(),
                    codepoints: Vec::new(),
                });
                None
            }
        }
    }

    pub fn output_style(&self, text: &str, style: &TextStyle, context: TextContext) -> TextStyle {
        let mut output = style.clone();
        if let Some(face) = self.resolve(style, context) {
            output = self.output_style_with_face(style, &face);
            let mut buffer = UnicodeBuffer::new();
            buffer.push_str(text);
            if let Ok(run) = face.shape(buffer, &[]) {
                let mut clusters = run
                    .glyphs
                    .iter()
                    .map(|glyph| glyph.cluster as usize)
                    .collect::<Vec<_>>();
                clusters.extend([text.len()]);
                clusters.sort_unstable();
                clusters.dedup();
                let mut codepoints = Vec::new();
                for glyph in run.glyphs.iter().filter(|glyph| glyph.id == 0) {
                    let start = glyph.cluster as usize;
                    let end = clusters
                        .iter()
                        .copied()
                        .find(|index| *index > start)
                        .unwrap_or(text.len());
                    if let Some(cluster) = text.get(start..end) {
                        codepoints.extend(cluster.chars().map(u32::from));
                    }
                }
                if !codepoints.is_empty() {
                    self.record(TextDiagnostic {
                        kind: TextDiagnosticKind::MissingGlyphs,
                        family: face.family,
                        codepoints,
                    });
                }
            } else {
                self.record(TextDiagnostic {
                    kind: TextDiagnosticKind::UnusableFontData,
                    family: face.family,
                    codepoints: Vec::new(),
                });
            }
        }
        output
    }

    pub fn output_style_with_face(&self, style: &TextStyle, face: &ResolvedFace) -> TextStyle {
        let mut output = style.clone();
        if style.family.is_some() || face.family != "Roboto" {
            output.family = Some(face.family.clone());
        }
        output
    }

    pub fn embed_fonts(&self, svg: &mut Scene) {
        for face in self.faces.borrow().iter() {
            svg.push(EmbeddedFont::new(
                &face.family,
                face.weight.0,
                face.style,
                face.bytes(),
            ));
        }
    }

    pub fn diagnostics(&self) -> Vec<TextDiagnostic> {
        self.diagnostics.borrow().clone()
    }

    pub fn object_diagnostics(&self) -> Vec<ObjectDiagnostic> {
        self.object_diagnostics.borrow().clone()
    }

    pub fn report_object_issues(&self, issues: &[ObjectDiagnostic]) {
        let mut diagnostics = self.object_diagnostics.borrow_mut();
        for issue in issues {
            if !diagnostics.contains(issue) {
                diagnostics.push(*issue);
            }
        }
    }

    pub fn object_layout_unsupported(&self, anchor_utf16: i32) {
        self.report_object_issues(&[ObjectDiagnostic {
            anchor_utf16,
            kind: ObjectDiagnosticKind::MixedParagraphLayout,
        }]);
    }

    pub fn measurement_failed(&self, family: &str) {
        self.record(TextDiagnostic {
            kind: TextDiagnosticKind::MeasurementFailure,
            family: family.into(),
            codepoints: Vec::new(),
        });
    }

    pub fn glyph_positioning_unsupported(&self, family: &str, text: &str) {
        self.record(TextDiagnostic {
            kind: TextDiagnosticKind::UnsupportedGlyphPositioning,
            family: family.into(),
            codepoints: text.chars().map(u32::from).collect(),
        });
    }

    pub fn missing_glyphs(&self, family: &str, text: &str) {
        self.record(TextDiagnostic {
            kind: TextDiagnosticKind::MissingGlyphs,
            family: family.into(),
            codepoints: text.chars().map(u32::from).collect(),
        });
    }

    fn record(&self, mut diagnostic: TextDiagnostic) {
        let mut diagnostics = self.diagnostics.borrow_mut();
        if let Some(existing) = diagnostics.iter_mut().find(|existing| {
            existing.kind == diagnostic.kind && existing.family == diagnostic.family
        }) {
            existing.codepoints.append(&mut diagnostic.codepoints);
            existing.codepoints.sort_unstable();
            existing.codepoints.dedup();
        } else {
            diagnostic.codepoints.sort_unstable();
            diagnostic.codepoints.dedup();
            diagnostics.push(diagnostic);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::Color;
    use crate::fonts::fontdb::Database;

    fn renderer(fonts: &FontBook) -> TextRenderer<'_> {
        TextRenderer::new(
            TextSettings {
                scale: 1.0,
                font_size_delta: 0.0,
            },
            fonts,
        )
    }

    #[test]
    fn supplied_face_changes_family_without_resolving_or_losing_emphasis() {
        let fonts = FontBook::new(Arc::new(Database::new()));
        let renderer = renderer(&fonts);
        let face = FontBook::default()
            .resolve("Roboto Mono", false, false)
            .unwrap();
        let style = TextStyle {
            font_size: 19.0,
            family: Some("Unavailable".into()),
            color: "#123456".into(),
            source_color: Color {
                r: 18,
                g: 52,
                b: 86,
            },
            bold: true,
            italic: true,
            underline: true,
            strikethrough: true,
            link_target: Some("https://example.com".into()),
        };
        let output = renderer.output_style_with_face(&style, &face);
        assert_eq!(output.family.as_deref(), Some("Roboto Mono"));
        assert_eq!(output.font_size, style.font_size);
        assert_eq!(output.color, style.color);
        assert_eq!(output.source_color, style.source_color);
        assert!(output.bold && output.italic && output.underline && output.strikethrough);
        assert_eq!(output.link_target, style.link_target);
        assert!(renderer.diagnostics().is_empty());
        assert!(renderer.faces.borrow().is_empty());
    }

    #[test]
    fn positioning_and_coverage_diagnostics_merge_by_family_and_reason() {
        let fonts = FontBook::default();
        let renderer = renderer(&fonts);
        renderer.glyph_positioning_unsupported("Roboto", "e\u{301}😀e");
        renderer.glyph_positioning_unsupported("Roboto", "😀中");
        renderer.glyph_positioning_unsupported("Roboto Mono", "e");
        renderer.missing_glyphs("Roboto", "中😀中");
        assert_eq!(
            renderer.diagnostics(),
            [
                TextDiagnostic {
                    kind: TextDiagnosticKind::UnsupportedGlyphPositioning,
                    family: "Roboto".into(),
                    codepoints: vec![0x65, 0x301, 0x4e2d, 0x1f600],
                },
                TextDiagnostic {
                    kind: TextDiagnosticKind::UnsupportedGlyphPositioning,
                    family: "Roboto Mono".into(),
                    codepoints: vec![0x65],
                },
                TextDiagnostic {
                    kind: TextDiagnosticKind::MissingGlyphs,
                    family: "Roboto".into(),
                    codepoints: vec![0x4e2d, 0x1f600],
                },
            ]
        );
    }

    #[test]
    fn object_diagnostics_preserve_distinct_anchors_and_reasons() {
        let fonts = FontBook::default();
        let renderer = renderer(&fonts);
        let issues = [
            ObjectDiagnostic {
                anchor_utf16: -1,
                kind: ObjectDiagnosticKind::InvalidAnchor,
            },
            ObjectDiagnostic {
                anchor_utf16: 3,
                kind: ObjectDiagnosticKind::NonReplacementAnchor,
            },
            ObjectDiagnostic {
                anchor_utf16: 3,
                kind: ObjectDiagnosticKind::UnsupportedContent,
            },
        ];
        renderer.report_object_issues(&issues);
        renderer.report_object_issues(&issues);
        renderer.object_layout_unsupported(3);
        renderer.object_layout_unsupported(3);
        renderer.object_layout_unsupported(4);
        let expected = [
            issues.to_vec(),
            vec![
                ObjectDiagnostic {
                    anchor_utf16: 3,
                    kind: ObjectDiagnosticKind::MixedParagraphLayout,
                },
                ObjectDiagnostic {
                    anchor_utf16: 4,
                    kind: ObjectDiagnosticKind::MixedParagraphLayout,
                },
            ],
        ]
        .concat();
        assert_eq!(renderer.object_diagnostics(), expected);
        assert!(renderer.diagnostics().is_empty());
    }
}
