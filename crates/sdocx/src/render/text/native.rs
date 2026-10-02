use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use krilla::color::rgb;
use krilla::geom::{Path, PathBuilder, Point, Transform};
use krilla::paint::{Fill, Stroke};
use krilla::surface::Surface;
use krilla::tagging::{ContentTag, SpanTag, Tag, TagGroup};
use krilla::text::{Font, GlyphId, KrillaGlyph};
use rustybuzz::ttf_parser;

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
    pub bold: bool,
    pub skew_x: f32,
}

#[derive(Debug)]
pub(crate) struct NativeGlyphRun {
    pub face: ResolvedFace,
    pub font_size: f64,
    pub paint: NativeTextPaint,
    pub glyphs: Vec<NativeGlyph>,
    pub variable: bool,
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
    #[error("retained glyph {0} has no supported outline")]
    UnsupportedOutline(u32),
    #[error("cannot embed retained font {0}")]
    InvalidFont(String),
}

#[derive(Debug, Default)]
pub(crate) struct NativeTextRegistry {
    blocks: Vec<RegisteredText>,
    scopes: Vec<ReadingScope>,
    next_source: usize,
}

#[derive(Debug)]
struct RegisteredText {
    block: NativeTextBlock,
    order: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadingScopeKind {
    TextSource,
    InlineObject,
    Marker,
}

#[derive(Clone, Copy)]
enum ReadingPhase {
    BeforeContent = 0,
    Content = 1,
}

#[derive(Debug)]
struct ReadingScope {
    path: Vec<usize>,
    next_source: usize,
    kind: ReadingScopeKind,
}

impl NativeTextRegistry {
    pub fn enter_text_source(&mut self) {
        let (mut path, ordinal) = if let Some(parent) = self.scopes.last_mut() {
            let ordinal = parent.next_source;
            parent.next_source += 1;
            (parent.path.clone(), ordinal)
        } else {
            let ordinal = self.next_source;
            self.next_source += 1;
            (Vec::new(), ordinal)
        };
        path.push(ordinal);
        self.scopes.push(ReadingScope {
            path,
            next_source: 0,
            kind: ReadingScopeKind::TextSource,
        });
    }

    pub fn leave_text_source(&mut self) {
        self.leave_scope(ReadingScopeKind::TextSource);
    }

    pub fn enter_inline_object(&mut self, anchor: usize) {
        self.enter_anchored_scope(
            anchor,
            ReadingPhase::Content,
            ReadingScopeKind::InlineObject,
        );
    }

    pub fn leave_inline_object(&mut self) {
        self.leave_scope(ReadingScopeKind::InlineObject);
    }

    pub fn enter_marker(&mut self, anchor: usize) {
        self.enter_anchored_scope(
            anchor,
            ReadingPhase::BeforeContent,
            ReadingScopeKind::Marker,
        );
    }

    pub fn leave_marker(&mut self) {
        self.leave_scope(ReadingScopeKind::Marker);
    }

    fn enter_anchored_scope(&mut self, anchor: usize, phase: ReadingPhase, kind: ReadingScopeKind) {
        let mut path = self
            .scopes
            .last()
            .expect("anchored text requires a parent text source")
            .path
            .clone();
        path.extend([anchor, phase as usize]);
        self.scopes.push(ReadingScope {
            path,
            next_source: 0,
            kind,
        });
    }

    fn leave_scope(&mut self, kind: ReadingScopeKind) {
        let scope = self.scopes.pop().expect("unbalanced reading scope");
        assert_eq!(scope.kind, kind, "mismatched reading scope");
    }

    pub fn register(
        &mut self,
        source_start: usize,
        block: NativeTextBlock,
    ) -> Result<NativeTextId, NativeTextError> {
        block.validate()?;
        let mut order = self.scopes.last().map_or_else(
            || {
                let ordinal = self.next_source;
                self.next_source += 1;
                vec![ordinal]
            },
            |scope| scope.path.clone(),
        );
        order.extend([source_start, ReadingPhase::Content as usize]);
        let id = NativeTextId(self.blocks.len());
        self.blocks.push(RegisteredText { block, order });
        Ok(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (NativeTextId, &NativeTextBlock)> {
        self.blocks
            .iter()
            .enumerate()
            .map(|(index, registered)| (NativeTextId(index), &registered.block))
    }

    pub fn ordered_ids(&self) -> Vec<NativeTextId> {
        let mut ids = (0..self.blocks.len()).map(NativeTextId).collect::<Vec<_>>();
        ids.sort_by(|left, right| self.blocks[left.0].order.cmp(&self.blocks[right.0].order));
        ids
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
            if !positive_native(run.font_size) || !run.paint.skew_x.is_finite() {
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

pub(crate) struct NativePdfPainter {
    fonts: HashMap<(fontdb::ID, u32), Font>,
    outlines: HashMap<(fontdb::ID, u32, u32), Arc<[OutlineCommand]>>,
    stroke_width: f64,
}

impl Default for NativePdfPainter {
    fn default() -> Self {
        Self::new(72.0)
    }
}

impl NativePdfPainter {
    pub fn new(dpi: f32) -> Self {
        Self {
            fonts: HashMap::new(),
            outlines: HashMap::new(),
            stroke_width: 0.25 * f64::from(dpi) / 72.0,
        }
    }

    pub fn paint(
        &mut self,
        block: &NativeTextBlock,
        surface: &mut Surface<'_>,
    ) -> Result<TagGroup, NativeTextError> {
        block.validate()?;
        if block.runs.iter().any(|run| run.paint.bold) && !positive_native(self.stroke_width) {
            return Err(NativeTextError::InvalidGeometry);
        }
        let prepared = block
            .runs
            .iter()
            .filter(|run| !run.glyphs.is_empty())
            .map(|run| {
                let font = self.font(&run.face)?;
                let (start, glyphs) = positioned_glyphs(run)?;
                let transform = glyph_shear(run)?;
                let paths = if run.paint.bold {
                    run.glyphs
                        .iter()
                        .map(|glyph| {
                            let outline = self.outline(&run.face, glyph.glyph_id)?;
                            outline_path(&outline, run, glyph)
                        })
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    Vec::new()
                };
                Ok((run, font, start, glyphs, transform, paths))
            })
            .collect::<Result<Vec<_>, NativeTextError>>()?;
        let old_fill = surface.get_fill().cloned();
        let old_stroke = surface.get_stroke().cloned();
        let identifier = surface.start_tagged(ContentTag::Span(
            SpanTag::empty().with_actual_text(Some(&block.source)),
        ));
        for (run, font, start, glyphs, transform, paths) in prepared {
            let color = rgb::Color::new(run.paint.color.r, run.paint.color.g, run.paint.color.b);
            if run.paint.bold {
                surface.set_fill(None);
                surface.set_stroke(Some(Stroke {
                    paint: color.into(),
                    width: self.stroke_width as f32,
                    miter_limit: 4.0,
                    ..Stroke::default()
                }));
                for path in paths.into_iter().flatten() {
                    surface.draw_path(&path);
                }
            }
            surface.set_fill(Some(Fill {
                paint: color.into(),
                ..Fill::default()
            }));
            surface.set_stroke(None);
            if let Some(transform) = &transform {
                surface.push_transform(transform);
            }
            surface.draw_glyphs(
                start,
                &glyphs,
                font,
                &block.source,
                run.font_size as f32,
                false,
            );
            if transform.is_some() {
                surface.pop();
            }
        }
        surface.set_fill(old_fill);
        surface.set_stroke(old_stroke);
        surface.end_tagged();
        Ok(TagGroup::with_children(Tag::Span, vec![identifier.into()]))
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

    fn outline(
        &mut self,
        face: &ResolvedFace,
        glyph_id: u32,
    ) -> Result<Arc<[OutlineCommand]>, NativeTextError> {
        let key = (face.id, face.index, glyph_id);
        if let Some(outline) = self.outlines.get(&key) {
            return Ok(outline.clone());
        }
        let data = face.shared_data();
        let parsed = ttf_parser::Face::parse(data.as_ref().as_ref(), face.index)
            .map_err(|_| NativeTextError::InvalidFont(face.family.clone()))?;
        let id = u16::try_from(glyph_id)
            .ok()
            .filter(|&id| id < parsed.number_of_glyphs())
            .map(ttf_parser::GlyphId)
            .ok_or(NativeTextError::UnsupportedOutline(glyph_id))?;
        let tables = parsed.tables();
        if (tables.glyf.is_none() && tables.cff.is_none())
            || parsed.is_color_glyph(id)
            || parsed.glyph_svg_image(id).is_some()
            || parsed.glyph_raster_image(id, u16::MAX).is_some()
        {
            return Err(NativeTextError::UnsupportedOutline(glyph_id));
        }
        let mut outline = GlyphOutline::default();
        if parsed.outline_glyph(id, &mut outline).is_none()
            && (!outline.0.is_empty() || tables.glyf.is_some_and(|table| table.bbox(id).is_some()))
        {
            return Err(NativeTextError::UnsupportedOutline(glyph_id));
        }
        let outline: Arc<[OutlineCommand]> = outline.0.into();
        self.outlines.insert(key, outline.clone());
        Ok(outline)
    }
}

#[derive(Debug)]
enum OutlineCommand {
    Move([f32; 2]),
    Line([f32; 2]),
    Quad([[f32; 2]; 2]),
    Cubic([[f32; 2]; 3]),
    Close,
}

#[derive(Default)]
struct GlyphOutline(Vec<OutlineCommand>);

impl ttf_parser::OutlineBuilder for GlyphOutline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(OutlineCommand::Move([x, y]));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(OutlineCommand::Line([x, y]));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.push(OutlineCommand::Quad([[x1, y1], [x, y]]));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0
            .push(OutlineCommand::Cubic([[x1, y1], [x2, y2], [x, y]]));
    }

    fn close(&mut self) {
        self.0.push(OutlineCommand::Close);
    }
}

fn outline_path(
    outline: &[OutlineCommand],
    run: &NativeGlyphRun,
    glyph: &NativeGlyph,
) -> Result<Option<Path>, NativeTextError> {
    if outline.is_empty() {
        return Ok(None);
    }
    let scale = run.font_size / f64::from(run.face.metrics.units_per_em);
    let point = |[x, y]: [f32; 2]| {
        let dy = -f64::from(y) * scale;
        let result = [
            glyph.origin[0] + f64::from(x) * scale + f64::from(run.paint.skew_x) * dy,
            glyph.origin[1] + dy,
        ];
        if !result.into_iter().all(finite_native) {
            return Err(NativeTextError::InvalidGeometry);
        }
        Ok(result.map(|coordinate| coordinate as f32))
    };
    let mut path = PathBuilder::new();
    for command in outline {
        match *command {
            OutlineCommand::Move(p) => {
                let [x, y] = point(p)?;
                path.move_to(x, y);
            }
            OutlineCommand::Line(p) => {
                let [x, y] = point(p)?;
                path.line_to(x, y);
            }
            OutlineCommand::Quad([p1, p]) => {
                let [x1, y1] = point(p1)?;
                let [x, y] = point(p)?;
                path.quad_to(x1, y1, x, y);
            }
            OutlineCommand::Cubic([p1, p2, p]) => {
                let [x1, y1] = point(p1)?;
                let [x2, y2] = point(p2)?;
                let [x, y] = point(p)?;
                path.cubic_to(x1, y1, x2, y2, x, y);
            }
            OutlineCommand::Close => path.close(),
        }
    }
    path.finish()
        .map(Some)
        .ok_or(NativeTextError::UnsupportedOutline(glyph.glyph_id))
}

fn glyph_shear(run: &NativeGlyphRun) -> Result<Option<Transform>, NativeTextError> {
    if run.paint.skew_x == 0.0 {
        return Ok(None);
    }
    let first = run.glyphs.first().ok_or(NativeTextError::Empty)?;
    let tx = -f64::from(run.paint.skew_x) * first.origin[1];
    if !finite_native(tx) {
        return Err(NativeTextError::InvalidGeometry);
    }
    Ok(Some(Transform::from_row(
        1.0,
        0.0,
        run.paint.skew_x,
        1.0,
        tx as f32,
        0.0,
    )))
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
            let skew = f64::from(run.paint.skew_x);
            let origin_x = glyph.origin[0] - skew * (glyph.origin[1] - first.origin[1]);
            let advance_x = ((glyph.advance[0] - skew * glyph.advance[1]) / run.font_size) as f32;
            let advance_y = (-glyph.advance[1] / run.font_size) as f32;
            let offset_x = (origin_x as f32 - cursor[0]) / size;
            let offset_y = (cursor[1] - glyph.origin[1] as f32) / size;
            if ![advance_x, advance_y, offset_x, offset_y]
                .into_iter()
                .all(f32::is_finite)
            {
                return Err(NativeTextError::InvalidGeometry);
            }
            cursor[0] += advance_x * size;
            cursor[1] -= advance_y * size;
            if !cursor.into_iter().all(f32::is_finite) {
                return Err(NativeTextError::InvalidGeometry);
            }
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
#[path = "native_clip_tests.rs"]
mod clip_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::FontBook;
    use krilla::tagging::TagTree;
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
                bold: false,
                skew_x: 0.0,
            },
            glyphs: vec![NativeGlyph {
                glyph_id: 38,
                origin: [40.0, 80.0],
                advance: [29.35546875, 0.0],
                source: 0..1,
            }],
            variable: false,
        }
    }

    fn block(source: &str, run: NativeGlyphRun) -> NativeTextBlock {
        NativeTextBlock {
            source: Arc::from(source),
            runs: vec![run],
        }
    }

    fn pdf(block: &NativeTextBlock) -> lopdf::Document {
        pdf_at_dpi(block, 72.0)
    }

    fn pdf_at_dpi(block: &NativeTextBlock, dpi: f32) -> lopdf::Document {
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
        surface.push_transform(&Transform::from_scale(72.0 / dpi, 72.0 / dpi));
        let parent_transform = surface.ctm();
        let mut painter = NativePdfPainter::new(dpi);
        let tag = painter.paint(block, &mut surface).unwrap();
        assert_eq!(surface.ctm(), parent_transform);
        assert_eq!(surface.get_fill(), Some(&previous_fill));
        assert_eq!(surface.get_stroke(), Some(&previous_stroke));
        surface.pop();
        surface.finish();
        page.finish();
        document.set_tag_tree(TagTree::from(vec![tag.into()]));
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
            registry.register(0, block("é", invalid)),
            Err(NativeTextError::InvalidSource)
        );
        assert_eq!(registry.len(), 0);
        let mut valid = run();
        valid.glyphs[0].source = 0..2;
        let id = registry.register(0, block("é", valid)).unwrap();
        assert_eq!(id.svg_id(), "sdocx-native-text-0");
        assert_eq!(&*registry.iter().next().unwrap().1.source, "é");
        assert_eq!(registry.iter().map(|(id, _)| id).collect::<Vec<_>>(), [id]);
    }

    #[test]
    fn semantic_order_inserts_object_text_without_changing_registration_order() {
        let mut registry = NativeTextRegistry::default();
        registry.enter_text_source();
        let before = registry.register(0, block("A", run())).unwrap();
        let after = registry.register(2, block("B", run())).unwrap();
        registry.enter_inline_object(1);
        registry.enter_text_source();
        let child = registry.register(0, block("Q", run())).unwrap();
        registry.leave_text_source();
        registry.leave_inline_object();
        registry.leave_text_source();
        assert_eq!(registry.ordered_ids(), [before, child, after]);
        assert_eq!(
            registry.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            [before, after, child]
        );
    }

    #[test]
    fn nested_objects_keep_distinct_child_sources_and_restore_parent_order() {
        let mut registry = NativeTextRegistry::default();
        registry.enter_text_source();
        let first = registry.register(0, block("A", run())).unwrap();
        let last = registry.register(5, block("Z", run())).unwrap();
        registry.enter_inline_object(2);
        registry.enter_text_source();
        let title_before = registry.register(0, block("T", run())).unwrap();
        let title_after = registry.register(2, block("U", run())).unwrap();
        registry.enter_inline_object(1);
        registry.enter_text_source();
        let nested = registry.register(0, block("N", run())).unwrap();
        registry.leave_text_source();
        registry.leave_inline_object();
        registry.leave_text_source();
        registry.enter_text_source();
        let body = registry.register(0, block("B", run())).unwrap();
        registry.enter_inline_object(1);
        registry.enter_text_source();
        let body_nested = registry.register(0, block("C", run())).unwrap();
        registry.leave_text_source();
        registry.leave_inline_object();
        registry.leave_text_source();
        registry.leave_inline_object();
        registry.enter_inline_object(4);
        registry.enter_text_source();
        let second_object = registry.register(0, block("Q", run())).unwrap();
        registry.leave_text_source();
        registry.leave_inline_object();
        registry.leave_text_source();
        assert_eq!(
            registry.ordered_ids(),
            [
                first,
                title_before,
                nested,
                title_after,
                body,
                body_nested,
                second_object,
                last,
            ]
        );
    }

    #[test]
    fn unrelated_sources_do_not_sort_by_their_local_scalar_offsets() {
        let mut registry = NativeTextRegistry::default();
        registry.enter_text_source();
        let first = registry.register(100, block("A", run())).unwrap();
        registry.leave_text_source();
        registry.enter_text_source();
        let second = registry.register(0, block("B", run())).unwrap();
        registry.leave_text_source();
        let unscoped = registry.register(0, block("C", run())).unwrap();
        assert_eq!(registry.ordered_ids(), [first, second, unscoped]);
    }

    #[test]
    fn markers_precede_same_anchor_content_and_follow_earlier_paragraphs() {
        let mut registry = NativeTextRegistry::default();
        registry.enter_text_source();
        let first = registry.register(0, block("A", run())).unwrap();
        let second = registry.register(10, block("B", run())).unwrap();
        registry.enter_marker(10);
        registry.enter_text_source();
        let second_marker = registry.register(0, block("2", run())).unwrap();
        registry.leave_text_source();
        registry.leave_marker();
        registry.enter_marker(0);
        registry.enter_text_source();
        let first_marker = registry.register(0, block("1", run())).unwrap();
        registry.leave_text_source();
        registry.leave_marker();
        registry.leave_text_source();
        assert_eq!(
            registry.ordered_ids(),
            [first_marker, first, second_marker, second]
        );
    }

    #[test]
    fn a_marker_and_an_inline_object_at_the_same_anchor_do_not_collide() {
        let mut registry = NativeTextRegistry::default();
        registry.enter_text_source();
        let after = registry.register(1, block("A", run())).unwrap();
        registry.enter_inline_object(0);
        registry.enter_text_source();
        let object = registry.register(0, block("Q", run())).unwrap();
        registry.leave_text_source();
        registry.leave_inline_object();
        registry.enter_marker(0);
        registry.enter_text_source();
        let marker = registry.register(0, block("1", run())).unwrap();
        registry.leave_text_source();
        registry.leave_marker();
        registry.leave_text_source();
        assert_eq!(registry.ordered_ids(), [marker, object, after]);
    }

    #[test]
    #[should_panic(expected = "mismatched reading scope")]
    fn semantic_scopes_require_balanced_kinds() {
        let mut registry = NativeTextRegistry::default();
        registry.enter_text_source();
        registry.leave_inline_object();
    }

    #[test]
    fn variable_font_instances_remain_explicit() {
        let mut run = run();
        run.variable = true;
        assert_eq!(
            block("A", run).validate(),
            Err(NativeTextError::VariableFont)
        );
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
    fn italic_compensation_preserves_mark_origins_and_vertical_advances() {
        let mut run = run();
        run.paint.skew_x = -0.25;
        run.glyphs = vec![
            NativeGlyph {
                glyph_id: 1399,
                origin: [47.80029296875, 70.1123046875],
                advance: [0., 2.],
                source: 0..6,
            },
            NativeGlyph {
                glyph_id: 5365,
                origin: [40., 80.],
                advance: [25.6640625, -1.],
                source: 0..6,
            },
        ];
        let (start, glyphs) = positioned_glyphs(&run).unwrap();
        let transform = glyph_shear(&run).unwrap().unwrap();
        let mut cursor = [start.x, start.y];
        for (glyph, expected) in glyphs.iter().zip(&run.glyphs) {
            let origin = [
                cursor[0] + glyph.x_offset(45.),
                cursor[1] - glyph.y_offset(45.),
            ];
            let actual = [
                origin[0] + transform.kx() * origin[1] + transform.tx(),
                origin[1],
            ];
            for (actual, expected) in actual.iter().zip(expected.origin) {
                assert!((f64::from(*actual) - expected).abs() < 0.00001);
            }
            let advance = [glyph.x_advance(45.), -glyph.y_advance(45.)];
            assert!(
                (f64::from(advance[0] + transform.kx() * advance[1]) - expected.advance[0]).abs()
                    < 0.00001
            );
            assert!((f64::from(advance[1]) - expected.advance[1]).abs() < 0.00001);
            cursor[0] += advance[0];
            cursor[1] += advance[1];
        }
    }

    #[test]
    fn glyph_outlines_are_cached_and_spaces_remain_empty() {
        let run = run();
        let mut painter = NativePdfPainter::default();
        let first = painter.outline(&run.face, 38).unwrap();
        let second = painter.outline(&run.face, 38).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert!(!first.is_empty());
        assert!(painter.outline(&run.face, 5).unwrap().is_empty());
        assert_eq!(
            painter.outline(&run.face, u32::MAX).unwrap_err(),
            NativeTextError::UnsupportedOutline(u32::MAX)
        );
    }

    #[test]
    fn combined_style_strokes_unsheared_paths_then_embeds_sheared_text() {
        let mut run = run();
        run.paint.bold = true;
        run.paint.skew_x = -0.25;
        let pdf = pdf(&block("A", run));
        let operations = operations(&pdf);
        let stroke = operations
            .iter()
            .position(|operation| operation.operator == "S")
            .unwrap();
        let shear = operations
            .iter()
            .position(|operation| {
                operation.operator == "cm" && operation.operands[2].as_float().unwrap() == -0.25
            })
            .unwrap();
        let text = operations
            .iter()
            .position(|operation| operation.operator == "BT")
            .unwrap();
        assert!(stroke < shear && shear < text);
        assert_eq!(pdf.extract_text(&[1]).unwrap().trim(), "A");
        assert!(pdf.objects.values().any(|object| {
            object
                .as_dict()
                .is_ok_and(|dictionary| dictionary.has(b"FontFile2"))
        }));
    }

    #[test]
    fn bold_pen_width_is_one_quarter_pdf_point_at_each_dpi() {
        for dpi in [72.0_f32, 96.0, 144.0] {
            let mut run = run();
            run.paint.bold = true;
            let pdf = pdf_at_dpi(&block("A", run), dpi);
            let widths = operations(&pdf)
                .into_iter()
                .filter(|operation| operation.operator == "w")
                .map(|operation| operation.operands[0].as_float().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(widths.len(), 1);
            assert!((widths[0] * (72.0 / dpi) - 0.25).abs() < 0.000001);
        }
    }

    #[test]
    fn a_normal_neighbor_does_not_inherit_the_italic_transform() {
        let mut italic = run();
        italic.paint.bold = true;
        italic.paint.skew_x = -0.25;
        let mut normal = run();
        normal.glyphs[0].glyph_id = 39;
        normal.glyphs[0].origin = [100.0, 80.0];
        normal.glyphs[0].source = 1..2;
        let pdf = pdf(&NativeTextBlock {
            source: Arc::from("AB"),
            runs: vec![italic, normal],
        });
        let mut ctm = [1.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0];
        let mut stack = Vec::new();
        let mut text_shears = Vec::new();
        for operation in operations(&pdf) {
            match operation.operator.as_str() {
                "q" => stack.push(ctm),
                "Q" => ctm = stack.pop().unwrap(),
                "cm" => {
                    let right = operation
                        .operands
                        .iter()
                        .map(|number| number.as_float().unwrap())
                        .collect::<Vec<_>>();
                    let left = ctm;
                    ctm = [
                        left[0] * right[0] + left[2] * right[1],
                        left[1] * right[0] + left[3] * right[1],
                        left[0] * right[2] + left[2] * right[3],
                        left[1] * right[2] + left[3] * right[3],
                        left[0] * right[4] + left[2] * right[5] + left[4],
                        left[1] * right[4] + left[3] * right[5] + left[5],
                    ];
                }
                "Tj" | "TJ" => text_shears.push(ctm[2]),
                _ => {}
            }
        }
        assert_eq!(text_shears, [-0.25, 0.0]);
        assert_eq!(pdf.extract_text(&[1]).unwrap().replace('\n', ""), "AB");
    }

    #[test]
    fn an_invalid_later_outline_does_not_mutate_the_surface() {
        let valid = run();
        let mut invalid = run();
        invalid.paint.bold = true;
        invalid.glyphs[0].glyph_id = u32::MAX;
        let block = NativeTextBlock {
            source: Arc::from("A"),
            runs: vec![valid, invalid],
        };
        let mut document = Document::new();
        let mut page = document.start_page();
        let mut surface = page.surface();
        let previous_transform = surface.ctm();
        let previous_fill = surface.get_fill().cloned();
        let previous_stroke = surface.get_stroke().cloned();
        let mut painter = NativePdfPainter::default();
        assert!(matches!(
            painter.paint(&block, &mut surface),
            Err(NativeTextError::UnsupportedOutline(u32::MAX))
        ));
        assert_eq!(surface.ctm(), previous_transform);
        assert_eq!(surface.get_fill(), previous_fill.as_ref());
        assert_eq!(surface.get_stroke(), previous_stroke.as_ref());
        surface.finish();
        page.finish();
        let pdf = lopdf::Document::load_mem(&document.finish().unwrap()).unwrap();
        assert!(!operations(&pdf).iter().any(|operation| {
            matches!(
                operation.operator.as_str(),
                "BT" | "BDC" | "BMC" | "S" | "s"
            )
        }));
    }

    #[test]
    fn derived_stroke_geometry_must_remain_in_the_native_domain() {
        let mut run = run();
        run.paint.bold = true;
        run.font_size = f64::from(f32::MAX);
        run.glyphs[0].origin = [f64::from(f32::MAX), 80.0];
        run.glyphs[0].advance = [0.0, 0.0];
        let block = block("A", run);
        assert!(block.validate().is_ok());
        let mut document = Document::new();
        let mut page = document.start_page();
        let mut surface = page.surface();
        let mut painter = NativePdfPainter::default();
        assert!(matches!(
            painter.paint(&block, &mut surface),
            Err(NativeTextError::InvalidGeometry)
        ));
        surface.finish();
        page.finish();
        let pdf = lopdf::Document::load_mem(&document.finish().unwrap()).unwrap();
        assert!(!operations(&pdf).iter().any(|operation| {
            matches!(
                operation.operator.as_str(),
                "BT" | "BDC" | "BMC" | "S" | "s"
            )
        }));
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
    fn pdf_bold_strokes_before_one_embedded_fill() {
        let mut run = run();
        run.paint.bold = true;
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
            operation.operator == "w" && matches!(operation.operands.first(), Some(Object::Real(width)) if (*width - 0.25).abs() < 0.00001)
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
        document.set_tag_tree(TagTree::new());
        let pdf = lopdf::Document::load_mem(&document.finish().unwrap()).unwrap();
        assert!(
            !operations(&pdf)
                .iter()
                .any(|operation| { matches!(operation.operator.as_str(), "BT" | "BDC" | "BMC") })
        );
    }
}
