use std::cell::RefCell;

use crate::fonts::{FontBook, FontError, ResolvedFace, UnicodeBuffer};

use super::{TextContext, TextSettings, TextStyle};
use crate::render::vector::{EmbeddedFont, Scene};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TextDiagnosticKind {
    UnavailableFamily,
    UnusableFontData,
    MissingGlyphs,
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
}

impl<'a> TextRenderer<'a> {
    pub fn new(settings: TextSettings, fonts: &'a FontBook) -> Self {
        Self {
            settings,
            fonts,
            faces: RefCell::new(Vec::new()),
            diagnostics: RefCell::new(Vec::new()),
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
            if style.family.is_some() || face.family != "Roboto" {
                output.family = Some(face.family.clone());
            }
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
