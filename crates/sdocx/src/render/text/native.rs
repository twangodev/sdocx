use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use krilla::color::rgb;
use krilla::geom::Point;
use krilla::paint::{Fill, Stroke};
use krilla::surface::Surface;
use krilla::tagging::{ContentTag, SpanTag, Tag, TagGroup, TagTree};
use krilla::text::{Font, GlyphId, KrillaGlyph};

use crate::Color;
use crate::fonts::{ResolvedFace, fontdb};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct NativeTextId(usize);

impl NativeTextId {
    pub fn svg_id(self) -> String {
        format!("sdocx-native-text-{}", self.0)
    }
}

#[derive(Debug)]
pub(crate) struct NativeGlyph {
    pub glyph_id: u32,
    pub origin: [f64; 2],
    pub advance: [f64; 2],
    pub source: Range<usize>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NativeTextPaint {
    pub color: Color,
    pub stroke_width: Option<f64>,
}

#[derive(Debug)]
pub(crate) struct NativeGlyphRun {
    pub face: ResolvedFace,
    pub font_size: f64,
    pub paint: NativeTextPaint,
    pub glyphs: Vec<NativeGlyph>,
    pub variable: bool,
    pub synthetic_italic: bool,
    pub synthetic_bold: bool,
}

#[derive(Debug)]
pub(crate) struct NativeTextBlock {
    pub source: Arc<str>,
    pub runs: Vec<NativeGlyphRun>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum NativeTextError {
    #[error("retained text has no glyphs")]
    Empty,
    #[error("retained text contains invalid geometry")]
    InvalidGeometry,
    #[error("retained glyph has an invalid UTF-8 source range")]
    InvalidSource,
    #[error("retained variable-font text is unsupported")]
    VariableFont,
    #[error("retained synthetic italic text is unsupported")]
    SyntheticItalic,
    #[error("retained synthetic bold text is unsupported")]
    SyntheticBold,
    #[error("cannot embed retained font {0}")]
    InvalidFont(String),
}

#[derive(Debug, Default)]
pub(crate) struct NativeTextRegistry {
    blocks: Vec<NativeTextBlock>,
}

impl NativeTextRegistry {
    pub fn register(&mut self, block: NativeTextBlock) -> Result<NativeTextId, NativeTextError> {
        block.validate()?;
        let id = NativeTextId(self.blocks.len());
        self.blocks.push(block);
        Ok(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (NativeTextId, &NativeTextBlock)> {
        self.blocks
            .iter()
            .enumerate()
            .map(|(index, block)| (NativeTextId(index), block))
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }
}

impl NativeTextBlock {
    fn validate(&self) -> Result<(), NativeTextError> {
        if self.runs.is_empty() || self.runs.iter().all(|run| run.glyphs.is_empty()) {
            return Err(NativeTextError::Empty);
        }
        for run in &self.runs {
            if run.variable {
                return Err(NativeTextError::VariableFont);
            }
            if run.synthetic_bold {
                return Err(NativeTextError::SyntheticBold);
            }
            if run.synthetic_italic {
                return Err(NativeTextError::SyntheticItalic);
            }
            if !positive_native(run.font_size)
                || run
                    .paint
                    .stroke_width
                    .is_some_and(|width| !positive_native(width))
            {
                return Err(NativeTextError::InvalidGeometry);
            }
            for glyph in &run.glyphs {
                if !glyph
                    .origin
                    .into_iter()
                    .chain(glyph.advance)
                    .all(finite_native)
                {
                    return Err(NativeTextError::InvalidGeometry);
                }
                if glyph.source.is_empty() || self.source.get(glyph.source.clone()).is_none() {
                    return Err(NativeTextError::InvalidSource);
                }
            }
        }
        Ok(())
    }
}

fn finite_native(value: f64) -> bool {
    value.is_finite() && (value as f32).is_finite()
}

fn positive_native(value: f64) -> bool {
    finite_native(value) && value as f32 > 0.0
}

#[derive(Default)]
pub(crate) struct NativePdfPainter {
    fonts: HashMap<(fontdb::ID, u32), Font>,
    tags: TagTree,
}

impl NativePdfPainter {
    pub fn paint(
        &mut self,
        block: &NativeTextBlock,
        surface: &mut Surface<'_>,
    ) -> Result<(), NativeTextError> {
        block.validate()?;
        let prepared = block
            .runs
            .iter()
            .filter(|run| !run.glyphs.is_empty())
            .map(|run| {
                let font = self.font(&run.face)?;
                let (start, glyphs) = positioned_glyphs(run)?;
                Ok((run, font, start, glyphs))
            })
            .collect::<Result<Vec<_>, NativeTextError>>()?;
        let old_fill = surface.get_fill().cloned();
        let old_stroke = surface.get_stroke().cloned();
        let identifier = surface.start_tagged(ContentTag::Span(
            SpanTag::empty().with_actual_text(Some(&block.source)),
        ));
        for (run, font, start, glyphs) in prepared {
            let color = rgb::Color::new(run.paint.color.r, run.paint.color.g, run.paint.color.b);
            if let Some(width) = run.paint.stroke_width {
                surface.set_fill(None);
                surface.set_stroke(Some(Stroke {
                    paint: color.into(),
                    width: width as f32,
                    miter_limit: 4.0,
                    ..Stroke::default()
                }));
                surface.draw_glyphs(
                    start,
                    &glyphs,
                    font.clone(),
                    &block.source,
                    run.font_size as f32,
                    true,
                );
            }
            surface.set_fill(Some(Fill {
                paint: color.into(),
                ..Fill::default()
            }));
            surface.set_stroke(None);
            surface.draw_glyphs(
                start,
                &glyphs,
                font,
                &block.source,
                run.font_size as f32,
                false,
            );
        }
        surface.set_fill(old_fill);
        surface.set_stroke(old_stroke);
        surface.end_tagged();
        self.tags
            .push(TagGroup::with_children(Tag::Span, vec![identifier.into()]));
        Ok(())
    }

    pub fn take_tag_tree(&mut self) -> TagTree {
        std::mem::take(&mut self.tags)
    }

    fn font(&mut self, face: &ResolvedFace) -> Result<Font, NativeTextError> {
        let key = (face.id, face.index);
        if let Some(font) = self.fonts.get(&key) {
            return Ok(font.clone());
        }
        let font = Font::new(face.shared_data().into(), face.index)
            .ok_or_else(|| NativeTextError::InvalidFont(face.family.clone()))?;
        self.fonts.insert(key, font.clone());
        Ok(font)
    }
}

fn positioned_glyphs(run: &NativeGlyphRun) -> Result<(Point, Vec<KrillaGlyph>), NativeTextError> {
    let first = run.glyphs.first().ok_or(NativeTextError::Empty)?;
    let start = Point::from_xy(first.origin[0] as f32, first.origin[1] as f32);
    let mut cursor = [start.x, start.y];
    let size = run.font_size as f32;
    let glyphs = run
        .glyphs
        .iter()
        .map(|glyph| {
            let advance_x = (glyph.advance[0] / run.font_size) as f32;
            let advance_y = (-glyph.advance[1] / run.font_size) as f32;
            let offset_x = (glyph.origin[0] as f32 - cursor[0]) / size;
            let offset_y = (cursor[1] - glyph.origin[1] as f32) / size;
            if ![advance_x, advance_y, offset_x, offset_y]
                .into_iter()
                .all(f32::is_finite)
            {
                return Err(NativeTextError::InvalidGeometry);
            }
            cursor[0] += advance_x * size;
            cursor[1] -= advance_y * size;
            Ok(KrillaGlyph::new(
                GlyphId::new(glyph.glyph_id),
                advance_x,
                offset_x,
                offset_y,
                advance_y,
                glyph.source.clone(),
                None,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((start, glyphs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::FontBook;
    use krilla::{Document, geom::Size, page::PageSettings, text::Glyph};
    use lopdf::{Object, content::Content};

    fn run() -> NativeGlyphRun {
        NativeGlyphRun {
            face: FontBook::default().resolve("Roboto", false, false).unwrap(),
            font_size: 45.0,
            paint: NativeTextPaint {
                color: Color {
                    r: 20,
                    g: 40,
                    b: 60,
                },
                stroke_width: None,
            },
            glyphs: vec![NativeGlyph {
                glyph_id: 38,
                origin: [40.0, 80.0],
                advance: [29.35546875, 0.0],
                source: 0..1,
            }],
            variable: false,
            synthetic_italic: false,
            synthetic_bold: false,
        }
    }

    fn block(source: &str, run: NativeGlyphRun) -> NativeTextBlock {
        NativeTextBlock {
            source: Arc::from(source),
            runs: vec![run],
        }
    }

    fn pdf(block: &NativeTextBlock) -> lopdf::Document {
        let mut document = Document::new();
        let mut page =
            document.start_page_with(PageSettings::new(Size::from_wh(300., 200.).unwrap()));
        let mut surface = page.surface();
        let previous_fill = Fill {
            paint: rgb::Color::new(11, 22, 33).into(),
            ..Fill::default()
        };
        let previous_stroke = Stroke::default();
        surface.set_fill(Some(previous_fill.clone()));
        surface.set_stroke(Some(previous_stroke.clone()));
        let mut painter = NativePdfPainter::default();
        painter.paint(block, &mut surface).unwrap();
        assert_eq!(surface.get_fill(), Some(&previous_fill));
        assert_eq!(surface.get_stroke(), Some(&previous_stroke));
        surface.finish();
        page.finish();
        document.set_tag_tree(painter.take_tag_tree());
        lopdf::Document::load_mem(&document.finish().unwrap()).unwrap()
    }

    fn operations(pdf: &lopdf::Document) -> Vec<lopdf::content::Operation> {
        Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
            .unwrap()
            .operations
    }

    #[test]
    fn registration_validates_utf8_and_does_not_retain_rejected_blocks() {
        let mut registry = NativeTextRegistry::default();
        let mut invalid = run();
        invalid.glyphs[0].source = 1..2;
        assert_eq!(
            registry.register(block("é", invalid)),
            Err(NativeTextError::InvalidSource)
        );
        assert_eq!(registry.len(), 0);
        let mut valid = run();
        valid.glyphs[0].source = 0..2;
        let id = registry.register(block("é", valid)).unwrap();
        assert_eq!(id.svg_id(), "sdocx-native-text-0");
        assert_eq!(&*registry.iter().next().unwrap().1.source, "é");
        assert_eq!(registry.iter().map(|(id, _)| id).collect::<Vec<_>>(), [id]);
    }

    #[test]
    fn unsupported_font_instances_are_explicit() {
        for (variable, italic, bold, expected) in [
            (true, false, false, NativeTextError::VariableFont),
            (false, true, false, NativeTextError::SyntheticItalic),
            (false, false, true, NativeTextError::SyntheticBold),
        ] {
            let mut run = run();
            run.variable = variable;
            run.synthetic_italic = italic;
            run.synthetic_bold = bold;
            assert_eq!(block("A", run).validate(), Err(expected));
        }
    }

    #[test]
    fn invalid_native_geometry_is_rejected_before_paint() {
        for value in [f64::NAN, f64::INFINITY, f64::MAX] {
            let mut run = run();
            run.glyphs[0].origin[0] = value;
            assert_eq!(
                block("A", run).validate(),
                Err(NativeTextError::InvalidGeometry)
            );
        }
        for value in [0.0, -1.0, f64::MIN_POSITIVE] {
            let mut run = run();
            run.font_size = value;
            assert_eq!(
                block("A", run).validate(),
                Err(NativeTextError::InvalidGeometry)
            );
        }
    }

    #[test]
    fn absolute_origins_compensate_actual_advances_and_mark_offsets() {
        let mut run = run();
        run.glyphs = vec![
            NativeGlyph {
                glyph_id: 1399,
                origin: [47.80029296875, 70.1123046875],
                advance: [0., 0.],
                source: 0..6,
            },
            NativeGlyph {
                glyph_id: 5365,
                origin: [40., 80.],
                advance: [25.6640625, 0.],
                source: 0..6,
            },
            NativeGlyph {
                glyph_id: 38,
                origin: [95., 82.],
                advance: [29.35546875, 0.],
                source: 6..7,
            },
        ];
        let (start, glyphs) = positioned_glyphs(&run).unwrap();
        let mut cursor = [start.x, start.y];
        for (glyph, expected) in glyphs.iter().zip(&run.glyphs) {
            let origin = [
                cursor[0] + glyph.x_offset(45.),
                cursor[1] - glyph.y_offset(45.),
            ];
            assert!((f64::from(origin[0]) - expected.origin[0]).abs() < 0.00001);
            assert!((f64::from(origin[1]) - expected.origin[1]).abs() < 0.00001);
            assert_eq!(glyph.glyph_id().to_u32(), expected.glyph_id);
            assert_eq!(glyph.text_range(), expected.source);
            cursor[0] += glyph.x_advance(45.);
            cursor[1] -= glyph.y_advance(45.);
        }
        assert!(glyphs[1].x_advance(45.) > 25.0);
    }

    #[test]
    fn overlapping_arabic_cluster_keeps_logical_actual_text() {
        let mut database = fontdb::Database::new();
        database
            .load_font_data(include_bytes!("../../../tests/assets/fonts/DejaVuSans.ttf").to_vec());
        let mut run = run();
        run.face = FontBook::new(Arc::new(database))
            .resolve("DejaVu Sans", false, false)
            .unwrap();
        run.glyphs = vec![
            NativeGlyph {
                glyph_id: 1399,
                origin: [47.80029296875, 70.1123046875],
                advance: [0., 0.],
                source: 0..6,
            },
            NativeGlyph {
                glyph_id: 5365,
                origin: [40., 80.],
                advance: [25.6640625, 0.],
                source: 0..6,
            },
        ];
        let pdf = pdf(&block("لَا", run));
        let actual_text = operations(&pdf)
            .into_iter()
            .filter(|operation| operation.operator == "BDC")
            .filter_map(|operation| {
                operation.operands[1]
                    .as_dict()
                    .ok()?
                    .get(b"ActualText")
                    .ok()?
                    .as_str()
                    .ok()
                    .map(<[u8]>::to_vec)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual_text.first().unwrap(),
            &[0xfe, 0xff, 0x06, 0x44, 0x06, 0x4e, 0x06, 0x27]
        );
        assert!(actual_text.iter().all(|source| source == &actual_text[0]));
        assert!(pdf.objects.values().any(|object| {
            object
                .as_dict()
                .is_ok_and(|dictionary| dictionary.has(b"FontFile2"))
        }));
    }

    #[test]
    fn flow_bold_strokes_before_one_embedded_fill() {
        let mut run = run();
        run.paint.stroke_width = Some(0.45);
        let pdf = pdf(&block("A", run));
        assert_eq!(pdf.extract_text(&[1]).unwrap().trim(), "A");
        let operations = operations(&pdf);
        let stroke = operations
            .iter()
            .position(|operation| matches!(operation.operator.as_str(), "S" | "s"))
            .unwrap();
        let text = operations
            .iter()
            .position(|operation| operation.operator == "BT")
            .unwrap();
        assert!(stroke < text);
        assert_eq!(
            operations
                .iter()
                .filter(|operation| operation.operator == "BT")
                .count(),
            1
        );
        assert!(operations.iter().any(|operation| {
            operation.operator == "w" && matches!(operation.operands.first(), Some(Object::Real(width)) if (*width - 0.45).abs() < 0.00001)
        }));
    }

    #[test]
    fn whole_block_actual_text_preserves_controls_with_shared_glyph_ids() {
        let mut run = run();
        run.glyphs = vec![
            NativeGlyph {
                glyph_id: 38,
                origin: [40., 80.],
                advance: [29.35546875, 0.],
                source: 0..1,
            },
            NativeGlyph {
                glyph_id: 5,
                origin: [95., 80.],
                advance: [0., 0.],
                source: 5..8,
            },
            NativeGlyph {
                glyph_id: 38,
                origin: [70., 80.],
                advance: [29.35546875, 0.],
                source: 4..5,
            },
            NativeGlyph {
                glyph_id: 5,
                origin: [70., 80.],
                advance: [0., 0.],
                source: 1..4,
            },
        ];
        let pdf = pdf(&block("A\u{2066}A\u{2069}", run));
        let operations = operations(&pdf);
        let outer = operations
            .iter()
            .find(|operation| operation.operator == "BDC")
            .unwrap();
        let properties = outer.operands[1].as_dict().unwrap();
        assert_eq!(
            properties.get(b"ActualText").unwrap().as_str().unwrap(),
            [0xfe, 0xff, 0x00, 0x41, 0x20, 0x66, 0x00, 0x41, 0x20, 0x69]
        );
        assert_eq!(properties.get(b"MCID").unwrap().as_i64().unwrap(), 0);
        let catalog = pdf
            .get_dictionary(pdf.trailer.get(b"Root").unwrap().as_reference().unwrap())
            .unwrap();
        assert!(catalog.has(b"StructTreeRoot"));
        assert_eq!(
            operations
                .iter()
                .filter(|operation| operation.operator == "BDC" || operation.operator == "BMC")
                .count(),
            operations
                .iter()
                .filter(|operation| operation.operator == "EMC")
                .count()
        );
    }

    #[test]
    fn invalid_later_font_does_not_start_painting_the_block() {
        let valid = run();
        let mut invalid = run();
        invalid.face.index = u32::MAX;
        let block = NativeTextBlock {
            source: Arc::from("A"),
            runs: vec![valid, invalid],
        };
        let mut document = Document::new();
        let mut page = document.start_page();
        let mut surface = page.surface();
        let previous_fill = surface.get_fill().cloned();
        let previous_stroke = surface.get_stroke().cloned();
        let mut painter = NativePdfPainter::default();
        assert!(matches!(
            painter.paint(&block, &mut surface),
            Err(NativeTextError::InvalidFont(_))
        ));
        assert_eq!(surface.get_fill(), previous_fill.as_ref());
        assert_eq!(surface.get_stroke(), previous_stroke.as_ref());
        surface.finish();
        page.finish();
        document.set_tag_tree(painter.take_tag_tree());
        let pdf = lopdf::Document::load_mem(&document.finish().unwrap()).unwrap();
        assert!(
            !operations(&pdf)
                .iter()
                .any(|operation| { matches!(operation.operator.as_str(), "BT" | "BDC" | "BMC") })
        );
    }
}
