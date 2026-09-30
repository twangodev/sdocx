use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::sync::{Arc, Mutex};

use unicode_bidi::BidiInfo;
use unicode_script::{Script, ScriptExtension, UnicodeScript};

use crate::PredefinedTextStyle;
use crate::fonts::{Direction, Feature, FontError, ResolvedFace, ShapedGlyph, UnicodeBuffer};
use crate::render::RenderTheme;

use super::{StyledText, TextRenderer, TextStyle};

pub(in crate::render) struct MeasuredText {
    pub advance: f64,
    pub font_size: f64,
    pub clusters: Vec<MeasuredCluster>,
}

struct MeasuredSegment {
    advance: f64,
    clusters: Vec<MeasuredCluster>,
}

#[derive(Clone)]
pub(in crate::render) struct MeasuredCluster {
    pub source: Range<usize>,
    pub advance: f64,
    pub run: Arc<MeasuredRun>,
    pub glyphs: Range<usize>,
    pub origin_x: f64,
}

pub(in crate::render) struct MeasuredRun {
    pub source: Range<usize>,
    pub style: TextStyle,
    pub face: ResolvedFace,
    pub direction: Direction,
    pub script: Script,
    pub glyphs: Vec<MeasuredGlyph>,
    pub variable: bool,
    standalone: Mutex<HashMap<String, Arc<Vec<ShapedGlyph>>>>,
}

pub(in crate::render) struct MeasuredGlyph {
    pub source: Range<usize>,
    pub raw: ShapedGlyph,
    pub pen_x: i64,
    pub pen_y: i64,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::render) struct ClusterPaintOffset {
    pub x: f64,
    pub y: f64,
}

impl MeasuredCluster {
    pub fn supports_positioned_text(&self) -> bool {
        self.run.direction == Direction::LeftToRight
            && !self.run.variable
            && matches!(
                self.run.script,
                Script::Latin | Script::Common | Script::Inherited
            )
            && self
                .run
                .glyphs
                .get(self.glyphs.clone())
                .is_some_and(|glyphs| glyphs.iter().all(|glyph| glyph.raw.id != 0))
    }

    pub fn paint_offset(&self, text: &str) -> Result<Option<ClusterPaintOffset>, FontError> {
        if !self.supports_positioned_text()
            || text.chars().count() != self.source.len()
            || self.source.start < self.run.source.start
            || self.source.end > self.run.source.end
        {
            return Ok(None);
        }
        let standalone = self.run.standalone(text)?;
        let original = &self.run.glyphs[self.glyphs.clone()];
        if original.len() != standalone.len() || original.is_empty() {
            return Ok(None);
        }
        let translation_x = original[0].pen_x + i64::from(original[0].raw.x_offset)
            - i64::from(standalone[0].x_offset);
        let translation_y = original[0].pen_y + i64::from(original[0].raw.y_offset)
            - i64::from(standalone[0].y_offset);
        let mut pen_x = 0;
        let mut pen_y = 0;
        for (expected, glyph) in original.iter().zip(standalone.iter()) {
            let byte = glyph.cluster as usize;
            let Some(prefix) = text.get(..byte) else {
                return Ok(None);
            };
            if glyph.id != expected.raw.id
                || self.source.start + prefix.chars().count() != expected.source.start
                || expected.pen_x + i64::from(expected.raw.x_offset)
                    != pen_x + i64::from(glyph.x_offset) + translation_x
                || expected.pen_y + i64::from(expected.raw.y_offset)
                    != pen_y + i64::from(glyph.y_offset) + translation_y
            {
                return Ok(None);
            }
            pen_x += i64::from(glyph.x_advance);
            pen_y += i64::from(glyph.y_advance);
        }
        let scale = self.run.style.font_size / f64::from(self.run.face.metrics.units_per_em);
        Ok(Some(ClusterPaintOffset {
            x: translation_x as f64 * scale - self.origin_x,
            y: -(translation_y as f64 * scale),
        }))
    }
}

impl MeasuredRun {
    fn standalone(&self, text: &str) -> Result<Arc<Vec<ShapedGlyph>>, FontError> {
        let mut cache = self
            .standalone
            .lock()
            .expect("standalone shaping cache lock");
        if let Some(glyphs) = cache.get(text) {
            return Ok(glyphs.clone());
        }
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.set_direction(Direction::LeftToRight);
        let glyphs = Arc::new(self.face.shape(buffer, &[])?.glyphs);
        cache.insert(text.into(), glyphs.clone());
        Ok(glyphs)
    }
}

#[derive(Debug, thiserror::Error)]
pub(in crate::render) enum MeasurementError {
    #[error("text measurement range is outside its paragraph")]
    InvalidRange,
    #[error("no usable face is available for font family {0:?}")]
    UnavailableFace(String),
    #[error("shaping returned a cluster outside a Unicode scalar boundary")]
    InvalidCluster,
    #[error(transparent)]
    Font(#[from] FontError),
}

pub(in crate::render) struct ParagraphMeasurer<'a, 'text, 'fonts> {
    styled: &'a StyledText<'text>,
    renderer: &'a TextRenderer<'fonts>,
    range: Range<usize>,
    directions: Vec<Direction>,
    scripts: Vec<Script>,
    styles: Vec<(Range<usize>, TextStyle)>,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
}

impl<'a, 'text, 'fonts> ParagraphMeasurer<'a, 'text, 'fonts> {
    pub fn new(
        styled: &'a StyledText<'text>,
        range: Range<usize>,
        theme: RenderTheme,
        predefined: Option<PredefinedTextStyle>,
        renderer: &'a TextRenderer<'fonts>,
    ) -> Result<Self, MeasurementError> {
        let text = styled
            .index
            .slice(range.clone())
            .ok_or(MeasurementError::InvalidRange)?;
        let bidi = BidiInfo::new(text, None);
        let mut directions = Vec::new();
        let mut extensions = Vec::new();
        for (byte, character) in text.char_indices() {
            directions.push(if bidi.levels[byte].is_rtl() {
                Direction::RightToLeft
            } else {
                Direction::LeftToRight
            });
            extensions.push(character.script_extension());
        }
        let scripts = resolve_scripts(&extensions, &directions);
        let mut styles = Vec::<(Range<usize>, TextStyle)>::new();
        for segment in styled.segments(range.clone()) {
            let style = styled.style_at(segment.start, theme, predefined);
            if let Some((previous, previous_style)) = styles.last_mut()
                && previous.end == segment.start
                && joinable(previous_style, &style)
            {
                previous.end = segment.end;
            } else {
                styles.push((segment, style));
            }
        }
        Ok(Self {
            styled,
            renderer,
            range,
            directions,
            scripts,
            styles,
            theme,
            predefined,
        })
    }

    pub fn measure_line(&self, range: Range<usize>) -> Result<MeasuredText, MeasurementError> {
        if range.start > range.end || range.start < self.range.start || range.end > self.range.end {
            return Err(MeasurementError::InvalidRange);
        }
        let mut runs = Vec::new();
        for (segment, style) in &self.styles {
            let start = segment.start.max(range.start);
            let end = segment.end.min(range.end);
            if start >= end {
                continue;
            }
            let mut start = start - self.range.start;
            let end = end - self.range.start;
            while start < end {
                let direction = self.directions[start];
                let script = self.scripts[start];
                let tab = self.character(start) == Some('\t');
                let mut stop = start + 1;
                if !tab {
                    while stop < end
                        && self.directions[stop] == direction
                        && self.scripts[stop] == script
                        && self.character(stop) != Some('\t')
                    {
                        stop += 1;
                    }
                }
                runs.push(self.shape(start..stop, style, direction, script, tab)?);
                start = stop;
            }
        }
        Ok(MeasuredText {
            advance: runs.iter().map(|run| run.advance).sum(),
            font_size: self.font_size(range)?,
            clusters: runs.into_iter().flat_map(|run| run.clusters).collect(),
        })
    }

    pub fn font_size(&self, range: Range<usize>) -> Result<f64, MeasurementError> {
        if range.start > range.end || range.start < self.range.start || range.end > self.range.end {
            return Err(MeasurementError::InvalidRange);
        }
        let first = self
            .styles
            .partition_point(|(segment, _)| segment.end <= range.start);
        Ok(self.styles[first..]
            .iter()
            .take_while(|(segment, _)| segment.start < range.end)
            .map(|(_, style)| style.font_size)
            .reduce(f64::max)
            .unwrap_or_else(|| {
                self.styled
                    .style_at(range.start, self.theme, self.predefined)
                    .font_size
            }))
    }

    fn character(&self, character: usize) -> Option<char> {
        let start = self.range.start + character;
        self.styled.index.slice(start..start + 1)?.chars().next()
    }

    fn shape(
        &self,
        range: Range<usize>,
        style: &TextStyle,
        direction: Direction,
        script: Script,
        tab: bool,
    ) -> Result<MeasuredSegment, MeasurementError> {
        let face = self
            .renderer
            .resolve(style, self.styled.context())
            .ok_or_else(|| {
                MeasurementError::UnavailableFace(
                    style.family.as_deref().unwrap_or("Roboto").to_owned(),
                )
            })?;
        let source = self.range.start + range.start..self.range.start + range.end;
        let start_byte = self
            .styled
            .index
            .char_to_byte(source.start)
            .ok_or(MeasurementError::InvalidRange)?;
        let text = if tab {
            " "
        } else {
            self.styled
                .index
                .slice(source.clone())
                .ok_or(MeasurementError::InvalidRange)?
        };
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.set_direction(direction);
        buffer.set_script(
            script
                .short_name()
                .parse()
                .expect("Unicode scripts have valid ISO 15924 tags"),
        );
        let latin_features = native_features();
        let features = if script == Script::Latin {
            latin_features.as_slice()
        } else {
            &[]
        };
        let shaped = face.shape(buffer, features)?;
        let variable = rustybuzz::Face::from_slice(face.bytes(), face.index)
            .ok_or_else(|| FontError::InvalidData {
                family: face.family.clone(),
            })?
            .is_variable();
        let scale = style.font_size / f64::from(shaped.metrics.units_per_em);
        let multiplier = if tab { 4.0 } else { 1.0 };
        let mut cluster_glyphs = BTreeMap::<usize, (i64, Range<usize>)>::new();
        for (index, glyph) in shaped.glyphs.iter().enumerate() {
            let byte = glyph.cluster as usize;
            if byte >= text.len() || !text.is_char_boundary(byte) {
                return Err(MeasurementError::InvalidCluster);
            }
            let cluster = cluster_glyphs.entry(byte).or_insert((0, index..index));
            if cluster.1.end != index {
                return Err(MeasurementError::InvalidCluster);
            }
            cluster.0 += i64::from(glyph.x_advance);
            cluster.1.end = index + 1;
        }
        let mut sources = BTreeMap::new();
        let keys = cluster_glyphs.keys().copied().collect::<Vec<_>>();
        for (index, &byte) in keys.iter().enumerate() {
            let next = keys.get(index + 1).copied().unwrap_or(text.len());
            let source = if tab {
                self.range.start + range.start..self.range.start + range.end
            } else {
                let start = self
                    .styled
                    .index
                    .byte_to_char(start_byte + byte)
                    .ok_or(MeasurementError::InvalidCluster)?;
                let end = self
                    .styled
                    .index
                    .byte_to_char(start_byte + next)
                    .ok_or(MeasurementError::InvalidCluster)?;
                start..end
            };
            sources.insert(byte, source);
        }
        let mut pen_x = 0;
        let mut pen_y = 0;
        let glyphs = shaped
            .glyphs
            .iter()
            .map(|glyph| {
                let measured = MeasuredGlyph {
                    source: sources[&(glyph.cluster as usize)].clone(),
                    raw: glyph.clone(),
                    pen_x,
                    pen_y,
                };
                pen_x += i64::from(glyph.x_advance);
                pen_y += i64::from(glyph.y_advance);
                measured
            })
            .collect();
        let run = Arc::new(MeasuredRun {
            source,
            style: style.clone(),
            face,
            direction,
            script,
            glyphs,
            variable,
            standalone: Mutex::new(HashMap::new()),
        });
        let mut clusters = Vec::new();
        for (&byte, (advance, glyphs)) in &cluster_glyphs {
            clusters.push(MeasuredCluster {
                source: sources[&byte].clone(),
                advance: *advance as f64 * scale * multiplier,
                origin_x: run.glyphs[glyphs.start].pen_x as f64 * scale,
                run: run.clone(),
                glyphs: glyphs.clone(),
            });
        }
        Ok(MeasuredSegment {
            advance: shaped.advance_x() as f64 * scale * multiplier,
            clusters,
        })
    }
}

#[cfg(test)]
fn measure_text(
    styled: &StyledText<'_>,
    range: Range<usize>,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
    renderer: &TextRenderer<'_>,
) -> Result<MeasuredText, MeasurementError> {
    ParagraphMeasurer::new(styled, range.clone(), theme, predefined, renderer)?.measure_line(range)
}

fn native_features() -> [Feature; 2] {
    [
        Feature::new(rustybuzz::ttf_parser::Tag::from_bytes(b"liga"), 0, ..),
        Feature::new(rustybuzz::ttf_parser::Tag::from_bytes(b"clig"), 0, ..),
    ]
}

fn joinable(left: &TextStyle, right: &TextStyle) -> bool {
    left.font_size == right.font_size
        && left.source_color == right.source_color
        && left.family == right.family
        && left.bold == right.bold
        && left.italic == right.italic
}

fn resolve_scripts(extensions: &[ScriptExtension], directions: &[Direction]) -> Vec<Script> {
    let mut scripts = vec![Script::Common; extensions.len()];
    let mut start = 0;
    while start < extensions.len() {
        let mut shared = extensions[start];
        let mut end = start + 1;
        while end < extensions.len() && directions[end] == directions[start] {
            let intersection = shared.intersection(extensions[end]);
            if intersection.is_empty() {
                break;
            }
            shared = intersection;
            end += 1;
        }
        let script = Script::try_from(shared)
            .unwrap_or_else(|_| shared.iter().next().unwrap_or(Script::Unknown));
        scripts[start..end].fill(script);
        start = end;
    }
    scripts
}

#[cfg(test)]
mod tests {
    use super::super::{TextContext, TextSettings};
    use super::*;
    use crate::fonts::FontBook;
    use crate::{BoundingBox, RichTextBox, RichTextRun, RichTextSpan, RichTextSpanType};

    fn text_box(text: &str) -> RichTextBox {
        RichTextBox {
            text_area_type: None,
            bbox: BoundingBox::default(),
            rotation_degrees: None,
            text: text.into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: Some(45.0),
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: Vec::new(),
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        }
    }

    fn measure(text_box: &RichTextBox, context: TextContext) -> MeasuredText {
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
        };
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let styled = StyledText::new(text_box, context, settings);
        measure_text(
            &styled,
            0..styled.index.len(),
            RenderTheme::for_canvas(false),
            None,
            &renderer,
        )
        .unwrap()
    }

    #[test]
    fn retained_glyph_geometry_matches_independent_ava_positions() {
        let measured = measure(&text_box("AVA"), TextContext::Placed);
        let run = &measured.clusters[0].run;
        assert_eq!(run.source, 0..3);
        assert_eq!(run.face.family, "Roboto");
        assert_eq!(run.style.font_size, 45.0);
        assert_eq!(run.direction, Direction::LeftToRight);
        assert_eq!(run.script, Script::Latin);
        assert!(!run.variable);
        assert_eq!(
            run.glyphs
                .iter()
                .map(|glyph| (
                    glyph.raw.id,
                    glyph.source.clone(),
                    glyph.raw.x_advance,
                    glyph.pen_x,
                    glyph.raw.x_offset,
                    glyph.raw.y_offset,
                ))
                .collect::<Vec<_>>(),
            [
                (38, 0..1, 1249, 0, 0, 0),
                (59, 1..2, 1228, 1249, 0, 0),
                (38, 2..3, 1336, 2477, 0, 0)
            ],
        );
        for (index, cluster) in measured.clusters.iter().enumerate() {
            assert!(Arc::ptr_eq(run, &cluster.run));
            assert_eq!(cluster.glyphs, index..index + 1);
            assert!(cluster.supports_positioned_text());
            let text = if index == 1 { "V" } else { "A" };
            let offset = cluster.paint_offset(text).unwrap().unwrap();
            assert_eq!(offset.x, 0.0);
            assert_eq!(offset.y, 0.0);
        }
    }

    #[test]
    fn combining_mark_offsets_and_nfc_glyphs_reproduce_with_the_selected_face() {
        let marked = measure(&text_box("x\u{301}"), TextContext::Placed);
        assert_eq!(marked.clusters.len(), 1);
        let cluster = &marked.clusters[0];
        assert_eq!(cluster.source, 0..2);
        assert_eq!(cluster.glyphs, 0..2);
        assert_eq!(
            cluster
                .run
                .glyphs
                .iter()
                .map(|glyph| (
                    glyph.raw.id,
                    glyph.raw.x_advance,
                    glyph.raw.x_offset,
                    glyph.raw.y_offset,
                    glyph.pen_x,
                    glyph.source.clone(),
                ))
                .collect::<Vec<_>>(),
            [(93, 1015, 0, 0, 0, 0..2), (434, 0, 57, -10, 1015, 0..2)],
        );
        let offset = cluster.paint_offset("x\u{301}").unwrap().unwrap();
        assert_eq!(offset.x, 0.0);
        assert_eq!(offset.y, 0.0);
        let decomposed = measure(&text_box("e\u{301}"), TextContext::Placed);
        let composed = measure(&text_box("é"), TextContext::Placed);
        assert_eq!(decomposed.clusters[0].run.glyphs[0].raw.id, 2317);
        assert_eq!(composed.clusters[0].run.glyphs[0].raw.id, 2317);
        assert_eq!(decomposed.advance, composed.advance);
        assert!(
            decomposed.clusters[0]
                .paint_offset("e\u{301}")
                .unwrap()
                .is_some()
        );
        assert!(decomposed.clusters[0].paint_offset("é").unwrap().is_none());
    }

    #[test]
    fn latin_office_retains_six_glyphs_and_standalone_reproduction() {
        let measured = measure(&text_box("office"), TextContext::Placed);
        let run = &measured.clusters[0].run;
        assert_eq!(
            run.glyphs
                .iter()
                .map(|glyph| glyph.raw.id)
                .collect::<Vec<_>>(),
            [84, 75, 75, 78, 72, 74],
        );
        assert_eq!(measured.clusters.len(), 6);
        for (cluster, character) in measured.clusters.iter().zip("office".chars()) {
            assert!(
                cluster
                    .paint_offset(&character.to_string())
                    .unwrap()
                    .is_some()
            );
        }
    }

    #[test]
    fn paint_only_link_and_decoration_changes_keep_one_native_shaping_run() {
        let mut content = text_box("AV");
        for (start, target) in [(0, "https://example.com/a"), (1, "https://example.com/v")] {
            let payload = [
                9_u32.to_le_bytes().to_vec(),
                0_u32.to_le_bytes().to_vec(),
                (target.encode_utf16().count() as u32)
                    .to_le_bytes()
                    .to_vec(),
                target.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            ]
            .concat();
            content.spans.push(RichTextSpan {
                kind: RichTextSpanType::Hyperlink,
                start_utf16: start,
                end_utf16: start + 1,
                expand: false,
                payload,
            });
        }
        content.spans.push(RichTextSpan {
            kind: RichTextSpanType::Strikethrough,
            start_utf16: 0,
            end_utf16: 1,
            expand: false,
            payload: vec![1, 0],
        });
        let measured = measure(&content, TextContext::Placed);
        assert_eq!(measured.advance, 56.07421875);
        assert!(Arc::ptr_eq(
            &measured.clusters[0].run,
            &measured.clusters[1].run
        ));
        assert_eq!(measured.clusters[0].run.glyphs.len(), 2);
    }

    #[test]
    fn rtl_glyph_order_is_retained_and_unsupported_positioning_returns_none() {
        let measured = measure(&text_box("אב"), TextContext::Placed);
        assert_eq!(measured.clusters[0].source, 0..1);
        assert_eq!(measured.clusters[1].source, 1..2);
        let run = &measured.clusters[0].run;
        assert_eq!(run.direction, Direction::RightToLeft);
        assert_eq!(
            run.glyphs
                .iter()
                .map(|glyph| glyph.source.clone())
                .collect::<Vec<_>>(),
            [1..2, 0..1]
        );
        assert_eq!(measured.clusters[0].glyphs, 1..2);
        assert_eq!(measured.clusters[1].glyphs, 0..1);
        for (cluster, character) in measured.clusters.iter().zip("אב".chars()) {
            assert!(!cluster.supports_positioned_text());
            assert!(
                cluster
                    .paint_offset(&character.to_string())
                    .unwrap()
                    .is_none()
            );
        }
        let greek = measure(&text_box("λ"), TextContext::Placed);
        assert_ne!(greek.clusters[0].run.glyphs[0].raw.id, 0);
        assert_eq!(greek.clusters[0].run.script, Script::Greek);
        assert!(greek.clusters[0].paint_offset("λ").unwrap().is_none());
    }

    #[test]
    fn pinned_regular_advance_uses_font_units_and_local_size() {
        let measured = measure(&text_box("ABC"), TextContext::Placed);
        assert_eq!(measured.advance, 86.66015625);
        assert_eq!(measured.font_size, 45.0);
        let mut smaller = text_box("ABC");
        smaller.font_size = Some(22.5);
        assert_eq!(
            measure(&smaller, TextContext::Placed).advance,
            measured.advance / 2.0
        );
    }

    #[test]
    fn native_features_disable_ligatures_without_disabling_kerning() {
        let office = measure(&text_box("office"), TextContext::Placed);
        assert_eq!(office.clusters.len(), 6);
        let av = measure(&text_box("AV"), TextContext::Placed);
        assert_eq!(av.advance, 2552.0 / 2048.0 * 45.0);
        let separate = measure(&text_box("A"), TextContext::Placed).advance
            + measure(&text_box("V"), TextContext::Placed).advance;
        assert!(av.advance < separate);
    }

    #[test]
    fn matching_native_styles_rejoin_and_color_changes_split_shaping() {
        let mut same = text_box("AV");
        same.runs.push(RichTextRun {
            start: 0,
            end: 1,
            bold: false,
            italic: false,
        });
        assert_eq!(
            measure(&same, TextContext::Placed).advance,
            2552.0 / 2048.0 * 45.0
        );
        same.spans.push(RichTextSpan {
            kind: RichTextSpanType::ForegroundColor,
            start_utf16: 1,
            end_utf16: 2,
            expand: false,
            payload: vec![0, 0, 255, 255],
        });
        let changed = measure(&same, TextContext::Placed);
        let independent = measure(&text_box("A"), TextContext::Placed).advance
            + measure(&text_box("V"), TextContext::Placed).advance;
        assert_eq!(changed.advance, independent);
    }

    #[test]
    fn decoration_boundaries_preserve_shaping_context_but_font_styles_split() {
        let baseline = measure(&text_box("AV"), TextContext::Placed).advance;
        for kind in [RichTextSpanType::Underline, RichTextSpanType::Strikethrough] {
            let mut decorated = text_box("AV");
            decorated.spans.push(RichTextSpan {
                kind,
                start_utf16: 0,
                end_utf16: 1,
                expand: false,
                payload: vec![1, 0],
            });
            let measured = measure(&decorated, TextContext::Placed);
            assert_eq!(measured.advance, baseline);
        }
        for kind in [RichTextSpanType::Bold, RichTextSpanType::Italic] {
            let mut changed = text_box("AV");
            changed.spans.push(RichTextSpan {
                kind,
                start_utf16: 0,
                end_utf16: 1,
                expand: false,
                payload: vec![1, 0],
            });
            let measured = measure(&changed, TextContext::Placed);
            assert_ne!(measured.advance, baseline);
        }
    }

    #[test]
    fn combining_and_supplementary_clusters_keep_scalar_source_ranges() {
        let measured = measure(&text_box("e\u{301}😀"), TextContext::Placed);
        assert_eq!(measured.clusters[0].source, 0..2);
        assert_eq!(measured.clusters[1].source, 2..3);
    }

    #[test]
    fn bidi_and_script_runs_preserve_logical_source_order() {
        let text_box = text_box("ab אב cd");
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
        };
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let styled = StyledText::new(&text_box, TextContext::Placed, settings);
        let paragraph = ParagraphMeasurer::new(
            &styled,
            0..8,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
        )
        .unwrap();
        assert!(paragraph.directions.contains(&Direction::RightToLeft));
        assert!(paragraph.scripts.contains(&Script::Hebrew));
        let measured = paragraph.measure_line(0..8).unwrap();
        assert_eq!(measured.clusters.first().unwrap().source.start, 0);
        assert_eq!(measured.clusters.last().unwrap().source.end, 8);
        for pair in measured.clusters.windows(2) {
            assert_eq!(pair[0].source.end, pair[1].source.start);
        }
    }

    #[test]
    fn script_extensions_keep_japanese_shared_marks_with_their_script() {
        let directions = [Direction::LeftToRight; 2];
        let extensions = "ーあ"
            .chars()
            .map(|character| character.script_extension())
            .collect::<Vec<_>>();
        assert_eq!(
            resolve_scripts(&extensions, &directions),
            [Script::Hiragana; 2]
        );
    }

    #[test]
    fn tabs_use_four_measured_spaces() {
        let tab = measure(&text_box("\t"), TextContext::Placed);
        let space = measure(&text_box(" "), TextContext::Placed);
        assert_eq!(tab.advance, space.advance * 4.0);
        assert_eq!(tab.clusters[0].source, 0..1);
    }

    #[test]
    fn flow_faux_bold_uses_regular_metrics_and_placed_bold_uses_bold_face() {
        let mut bold = text_box("ABC");
        bold.runs.push(RichTextRun {
            start: 0,
            end: 3,
            bold: true,
            italic: false,
        });
        assert_eq!(measure(&bold, TextContext::Flow).advance, 86.66015625);
        assert_eq!(measure(&bold, TextContext::Placed).advance, 88.43994140625);
    }

    #[test]
    fn line_measurement_keeps_paragraph_direction_context_and_checks_ranges() {
        let text_box = text_box("אב 123 cd");
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
        };
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let styled = StyledText::new(&text_box, TextContext::Placed, settings);
        let paragraph = ParagraphMeasurer::new(
            &styled,
            0..9,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
        )
        .unwrap();
        let number_line = paragraph.measure_line(3..6).unwrap();
        assert_eq!(paragraph.directions[3], Direction::LeftToRight);
        assert_eq!(number_line.clusters.first().unwrap().source.start, 3);
        assert_eq!(number_line.clusters.last().unwrap().source.end, 6);
        assert!(matches!(
            paragraph.measure_line(0..10),
            Err(MeasurementError::InvalidRange)
        ));
        let empty = paragraph.measure_line(0..0).unwrap();
        assert_eq!(empty.advance, 0.0);
        assert!(empty.clusters.is_empty());
    }
}
