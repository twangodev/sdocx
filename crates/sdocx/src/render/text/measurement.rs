use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::sync::{Arc, Mutex};

use unicode_script::{Script, ScriptExtension, UnicodeScript};
use unicode_segmentation::UnicodeSegmentation;

use crate::fonts::{
    Direction, Feature, FontError, FontMetrics, FontSynthesis, PaintContextError,
    PaintContextWindows, PaintItemization, PaintItemizationError, PaintShapeDirection,
    PaintSpanProfile, PaintTextRequest, ResolvedFace, ShapedGlyph, UnicodeBuffer, fontdb,
};
use crate::render::RenderTheme;
use crate::text_index::TextSource;
use crate::{BoundingBox, PredefinedTextStyle};

use super::bidi::{BidiError, ParagraphBidi};
use super::{
    NativeDrawSpan, NativeIdentityUnavailable, StyledText, TextDiagnosticKind, TextMeasureStyle,
    TextRenderer, TextStyle,
};

mod native_geometry;
use native_geometry::NativeMeasuredEntries;

pub(in crate::render) struct MeasuredText {
    pub advance: f64,
    pub font_size: f64,
    pub clusters: Vec<MeasuredCluster>,
}

struct MeasuredSegment {
    advance: f64,
    clusters: Vec<MeasuredCluster>,
}

struct RunFace {
    face: ResolvedFace,
    coverage_fallback: bool,
    measurement_issue: TextDiagnosticKind,
}

#[derive(Clone)]
pub(in crate::render) struct MeasuredCluster {
    pub source: Range<usize>,
    pub advance: f64,
    pub run: Arc<MeasuredRun>,
    pub glyphs: Range<usize>,
}

pub(in crate::render) struct MeasuredRun {
    pub source: Range<usize>,
    pub style: TextStyle,
    pub face: ResolvedFace,
    pub synthesis: FontSynthesis,
    pub direction: Direction,
    pub glyphs: Vec<MeasuredGlyph>,
    pub variable: bool,
    pub tab: bool,
    pub coverage_fallback: bool,
    pub native_entries: Option<NativeMeasuredEntries>,
    measurement_issue: Option<TextDiagnosticKind>,
    browser_shapes: Mutex<HashMap<String, Arc<Vec<ShapedGlyph>>>>,
}

pub(in crate::render) struct MeasuredGlyph {
    pub source: TextSource,
    pub id: u32,
    pub owner_offset: [f64; 2],
    #[cfg_attr(not(any(feature = "pdf", test)), allow(dead_code))]
    pub transport_advance: [f64; 2],
}

#[derive(Clone, Copy)]
pub(in crate::render) struct FontGeometry {
    scale: f64,
}

impl FontGeometry {
    pub fn new(font_size: f64, units_per_em: u16) -> Self {
        Self {
            scale: font_size / f64::from(units_per_em),
        }
    }

    pub fn is_valid(self) -> bool {
        self.scale.is_finite() && self.scale > 0.0
    }

    fn project(self, units: f64) -> f64 {
        units * self.scale
    }

    fn position(self, units: i64) -> f64 {
        self.project(units as f64)
    }

    fn horizontal_advance(self, units: i64, tab: bool) -> f64 {
        self.position(units) * if tab { 4.0 } else { 1.0 }
    }

    fn glyph_advance(self, glyph: &ShapedGlyph) -> [f64; 2] {
        [
            self.project(f64::from(glyph.x_advance)),
            -self.project(f64::from(glyph.y_advance)),
        ]
    }

    fn measured_glyphs(
        self,
        shaped: &[ShapedGlyph],
        sources: &BTreeMap<usize, TextSource>,
    ) -> Vec<MeasuredGlyph> {
        let mut pen_x = 0;
        let mut pen_y = 0;
        let mut owner_pen = [0; 2];
        let mut owner = None;
        shaped
            .iter()
            .map(|glyph| {
                if owner != Some(glyph.cluster) {
                    owner = Some(glyph.cluster);
                    owner_pen = [pen_x, pen_y];
                }
                let measured = MeasuredGlyph {
                    source: sources[&(glyph.cluster as usize)].clone(),
                    id: glyph.id,
                    owner_offset: [
                        self.position(pen_x - owner_pen[0] + i64::from(glyph.x_offset)),
                        -self.position(pen_y - owner_pen[1] + i64::from(glyph.y_offset)),
                    ],
                    transport_advance: self.glyph_advance(glyph),
                };
                pen_x += i64::from(glyph.x_advance);
                pen_y += i64::from(glyph.y_advance);
                measured
            })
            .collect()
    }

    pub fn glyph_ink_bounds(
        self,
        glyph: &MeasuredGlyph,
        ink: rustybuzz::ttf_parser::Rect,
        origin: [f64; 2],
    ) -> BoundingBox {
        let x = origin[0] + glyph.owner_offset[0];
        let baseline = origin[1] + glyph.owner_offset[1];
        BoundingBox {
            x_min: x + self.project(f64::from(ink.x_min)),
            y_min: baseline - self.project(f64::from(ink.y_max)),
            x_max: x + self.project(f64::from(ink.x_max)),
            y_max: baseline - self.project(f64::from(ink.y_min)),
        }
    }

    pub fn vertical_ink_bounds(
        self,
        glyph: &MeasuredGlyph,
        ink: rustybuzz::ttf_parser::Rect,
        baseline: f64,
        stroke: f64,
    ) -> [f64; 2] {
        let baseline = baseline + glyph.owner_offset[1];
        [
            baseline - self.project(f64::from(ink.y_max)) - stroke,
            baseline - self.project(f64::from(ink.y_min)) + stroke,
        ]
    }

    pub fn vertical_metrics(self, metrics: FontMetrics, baseline: f64) -> [f64; 2] {
        [
            baseline - self.project(f64::from(metrics.ascent)),
            baseline - self.project(f64::from(metrics.descent)),
        ]
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::render) struct ClusterPaintOffset {
    pub x: f64,
    pub y: f64,
}

#[cfg(any(feature = "pdf", test))]
pub(in crate::render) struct RetainedGlyph<'a> {
    pub face: &'a ResolvedFace,
    pub source: &'a TextSource,
    pub glyph_id: u32,
    pub offset_x: f64,
    pub offset_y: f64,
    pub advance_x: f64,
    pub advance_y: f64,
}

impl MeasuredCluster {
    #[cfg(any(feature = "pdf", test))]
    pub fn retained_glyphs(
        &self,
    ) -> Result<impl Iterator<Item = RetainedGlyph<'_>>, MeasurementError> {
        let glyphs = self
            .run
            .glyphs
            .get(self.glyphs.clone())
            .ok_or(MeasurementError::InvalidCluster)?;
        if glyphs
            .iter()
            .any(|glyph| glyph.source.characters() != &self.source)
        {
            return Err(MeasurementError::InvalidCluster);
        }
        Ok(glyphs.iter().map(move |glyph| {
            let [offset_x, offset_y] = glyph.owner_offset;
            let [advance_x, advance_y] = glyph.transport_advance;
            RetainedGlyph {
                face: &self.run.face,
                source: &glyph.source,
                glyph_id: glyph.id,
                offset_x,
                offset_y,
                advance_x,
                advance_y,
            }
        }))
    }

    pub fn supports_positioned_text(&self) -> bool {
        self.run.direction == Direction::LeftToRight && self.has_static_glyphs()
    }

    fn has_static_glyphs(&self) -> bool {
        !self.run.variable
            && self
                .run
                .glyphs
                .get(self.glyphs.clone())
                .is_some_and(|glyphs| glyphs.iter().all(|glyph| glyph.id != 0))
    }

    pub fn paint_offset(&self, text: &str) -> Result<Option<ClusterPaintOffset>, FontError> {
        self.paint_offset_impl(text, false)
    }

    pub fn native_paint_offset(&self, text: &str) -> Result<Option<ClusterPaintOffset>, FontError> {
        self.paint_offset_impl(text, true)
    }

    fn paint_offset_impl(
        &self,
        text: &str,
        native_transport: bool,
    ) -> Result<Option<ClusterPaintOffset>, FontError> {
        let supported = self.supports_positioned_text()
            || (native_transport
                && self.run.direction == Direction::RightToLeft
                && self.source.len() == 1
                && self.has_static_glyphs());
        if !supported
            || text.chars().count() != self.source.len()
            || self.source.start < self.run.source.start
            || self.source.end > self.run.source.end
        {
            return Ok(None);
        }
        if native_transport
            && self.source.len() > 1
            && unicode_bidi::BidiInfo::new(text, Some(unicode_bidi::Level::ltr()))
                .levels
                .iter()
                .any(|level| level.is_rtl())
        {
            return Ok(None);
        }
        let text = if self.run.tab && text == "\t" {
            " "
        } else {
            text
        };
        let standalone = self.run.browser_shape(text)?;
        let original = &self.run.glyphs[self.glyphs.clone()];
        if original.len() != standalone.len() || original.is_empty() {
            return Ok(None);
        }
        let geometry = self.run.geometry();
        let translation = [
            original[0].owner_offset[0] - geometry.position(i64::from(standalone[0].x_offset)),
            original[0].owner_offset[1] + geometry.position(i64::from(standalone[0].y_offset)),
        ];
        let mut pen_x = 0;
        let mut pen_y = 0;
        for (expected, glyph) in original.iter().zip(standalone.iter()) {
            let byte = glyph.cluster as usize;
            let Some(prefix) = text.get(..byte) else {
                return Ok(None);
            };
            let natural = [
                geometry.position(pen_x + i64::from(glyph.x_offset)),
                -geometry.position(pen_y + i64::from(glyph.y_offset)),
            ];
            if glyph.id != expected.id
                || self.source.start + prefix.chars().count() != expected.source.characters().start
                || !expected
                    .owner_offset
                    .into_iter()
                    .zip(natural)
                    .zip(translation)
                    .all(|((target, natural), shift)| {
                        translated_coordinate_matches(target, natural, shift)
                    })
            {
                return Ok(None);
            }
            pen_x += i64::from(glyph.x_advance);
            pen_y += i64::from(glyph.y_advance);
        }
        Ok(Some(ClusterPaintOffset {
            x: translation[0],
            y: translation[1],
        }))
    }
}

fn translated_coordinate_matches(target: f64, natural: f64, shift: f64) -> bool {
    let translated = natural + shift;
    if ![target, natural, shift, translated]
        .into_iter()
        .all(f64::is_finite)
    {
        return false;
    }
    let magnitude = target.abs().max(natural.abs()).max(shift.abs());
    (target - translated).abs() <= 8.0 * f64::EPSILON * magnitude
}

impl MeasuredRun {
    pub fn measurement_issue(&self) -> Option<&TextDiagnosticKind> {
        self.measurement_issue.as_ref()
    }

    pub fn geometry(&self) -> FontGeometry {
        FontGeometry::new(self.style.font_size, self.face.metrics.units_per_em)
    }

    fn browser_shape(&self, text: &str) -> Result<Arc<Vec<ShapedGlyph>>, FontError> {
        let mut cache = self
            .browser_shapes
            .lock()
            .expect("browser shaping cache lock");
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
    bidi: ParagraphBidi<'text>,
    scripts: Vec<Script>,
    styles: Vec<MeasureSpan>,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
}

struct MeasureSpan {
    source: Range<usize>,
    style: TextStyle,
    measurement: TextMeasureStyle,
    native: Result<NativeDrawSpan, NativeIdentityUnavailable>,
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
        let bidi =
            ParagraphBidi::new(text, range.start).map_err(|_| MeasurementError::InvalidRange)?;
        let directions = (0..range.len())
            .map(|scalar| {
                bidi.direction_at(scalar)
                    .expect("validated paragraph scalar")
            })
            .collect::<Vec<_>>();
        let mut extensions = Vec::new();
        for character in text.chars() {
            extensions.push(character.script_extension());
        }
        let scripts = resolve_scripts(&extensions, &directions);
        let mut styles = Vec::<MeasureSpan>::new();
        for segment in styled.segments(range.clone()) {
            let resolved = styled.resolved_style_at(segment.start, theme, predefined);
            if let Some(previous) = styles.last_mut()
                && previous.source.end == segment.start
                && previous.measurement == resolved.measurement
            {
                previous.source.end = segment.end;
            } else {
                styles.push(MeasureSpan {
                    source: segment,
                    style: resolved.paint,
                    measurement: resolved.measurement,
                    native: resolved.native_draw,
                });
            }
        }
        Ok(Self {
            styled,
            renderer,
            range,
            bidi,
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
        let first = self
            .styles
            .partition_point(|span| span.source.end <= range.start);
        for span in self.styles[first..]
            .iter()
            .take_while(|span| span.source.start < range.end)
        {
            let start = span.source.start.max(range.start);
            let end = span.source.end.min(range.end);
            if start >= end {
                continue;
            }
            let mut start = start - self.range.start;
            let end = end - self.range.start;
            while start < end {
                let direction = self
                    .bidi()
                    .direction_at(start)
                    .ok_or(MeasurementError::InvalidRange)?;
                let tab = self.character(start) == Some('\t');
                let mut stop = start + 1;
                if !tab {
                    while stop < end
                        && self.bidi().direction_at(stop) == Some(direction)
                        && self.character(stop) != Some('\t')
                    {
                        stop += 1;
                    }
                }
                runs.push(self.shape_piece(start..stop, span, direction, tab)?);
                start = stop;
            }
        }
        Ok(MeasuredText {
            advance: runs.iter().map(|run| run.advance).sum(),
            font_size: self.font_size(range)?,
            clusters: runs.into_iter().flat_map(|run| run.clusters).collect(),
        })
    }

    pub fn bidi(&self) -> &ParagraphBidi<'text> {
        &self.bidi
    }

    pub fn visual_order(
        &self,
        line: Range<usize>,
        logical_items: &[Range<usize>],
    ) -> Result<Vec<usize>, BidiError> {
        self.bidi().visual_order(line, logical_items)
    }

    pub fn font_size(&self, range: Range<usize>) -> Result<f64, MeasurementError> {
        if range.start > range.end || range.start < self.range.start || range.end > self.range.end {
            return Err(MeasurementError::InvalidRange);
        }
        let first = self
            .styles
            .partition_point(|span| span.source.end <= range.start);
        Ok(self.styles[first..]
            .iter()
            .take_while(|span| span.source.start < range.end)
            .map(|span| span.style.font_size)
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

    fn shape_piece(
        &self,
        range: Range<usize>,
        span: &MeasureSpan,
        direction: Direction,
        tab: bool,
    ) -> Result<MeasuredSegment, MeasurementError> {
        let source = self.range.start + range.start..self.range.start + range.end;
        let text = self
            .styled
            .index
            .slice(source.clone())
            .ok_or(MeasurementError::InvalidRange)?;
        if !tab
            && let Ok(native) = &span.native
            && let Some(face) = self
                .renderer
                .for_source(source.clone())
                .resolve_native(native.family.as_deref(), direction)
            && native_geometry::supported_face(&face)
            && !face.covers(text)?
        {
            let mut coverage = Vec::<(Range<usize>, bool)>::new();
            let mut cursor = range.start;
            for grapheme in text.graphemes(true) {
                let end = cursor + grapheme.chars().count();
                let covered = face.covers(grapheme)?;
                if let Some((previous, previous_covered)) = coverage.last_mut()
                    && *previous_covered == covered
                {
                    previous.end = end;
                } else {
                    coverage.push((cursor..end, covered));
                }
                cursor = end;
            }
            if coverage.len() > 1 {
                let mut measured = MeasuredSegment {
                    advance: 0.0,
                    clusters: Vec::new(),
                };
                for (range, _) in coverage {
                    let piece = self.shape_piece(range, span, direction, false)?;
                    measured.advance += piece.advance;
                    measured.clusters.extend(piece.clusters);
                }
                return Ok(measured);
            }
        }
        let measurement_issue = match self.shape_native(range.clone(), span, direction, tab) {
            Ok(measured) => return Ok(measured),
            Err(kind) => {
                self.renderer.for_source(source).measurement_unsupported(
                    kind.clone(),
                    span.style.family.as_deref().unwrap_or("Roboto"),
                    text,
                );
                kind
            }
        };
        let mut measured = MeasuredSegment {
            advance: 0.0,
            clusters: Vec::new(),
        };
        let mut start = range.start;
        while start < range.end {
            let script = self.scripts[start];
            let mut stop = start + 1;
            while stop < range.end && self.scripts[stop] == script {
                stop += 1;
            }
            let piece = self.shape(
                start..stop,
                &span.style,
                direction,
                script,
                tab,
                measurement_issue.clone(),
            )?;
            measured.advance += piece.advance;
            measured.clusters.extend(piece.clusters);
            start = stop;
        }
        Ok(measured)
    }

    fn shape_native(
        &self,
        range: Range<usize>,
        span: &MeasureSpan,
        direction: Direction,
        tab: bool,
    ) -> Result<MeasuredSegment, TextDiagnosticKind> {
        use TextDiagnosticKind as Kind;
        if tab {
            return Err(Kind::UnsupportedTabMeasurement);
        }
        let native = span
            .native
            .as_ref()
            .map_err(|_| Kind::UnsupportedMeasurementStyle)?;
        let profile = PaintSpanProfile::new(native.font_size, native.style_bits)
            .and_then(|profile| profile.metric_input())
            .map_err(|_| Kind::UnsupportedMeasurementStyle)?;
        let source = self.range.start + range.start..self.range.start + range.end;
        let face = self
            .renderer
            .for_source(source.clone())
            .resolve_native(native.family.as_deref(), direction)
            .ok_or(Kind::UnsupportedMeasurementFont)?;
        if !native_geometry::supported_face(&face) {
            return Err(Kind::UnsupportedMeasurementFont);
        }
        let selected_text = self
            .styled
            .index
            .slice(source.clone())
            .ok_or(Kind::UnsupportedMeasurementShaping)?;
        if !face
            .covers(selected_text)
            .map_err(|_| Kind::UnsupportedMeasurementFont)?
        {
            return Err(Kind::UnsupportedMeasurementFont);
        }
        let context = self.range.clone();
        let text = self
            .styled
            .index
            .slice(context.clone())
            .ok_or(Kind::UnsupportedMeasurementShaping)?;
        let base = self
            .styled
            .index
            .char_to_utf16(context.start)
            .ok_or(Kind::UnsupportedMeasurementShaping)?;
        let start = self
            .styled
            .index
            .char_to_utf16(source.start)
            .and_then(|v| v.checked_sub(base))
            .ok_or(Kind::UnsupportedMeasurementShaping)?;
        let end = self
            .styled
            .index
            .char_to_utf16(source.end)
            .and_then(|v| v.checked_sub(base))
            .ok_or(Kind::UnsupportedMeasurementShaping)?;
        let windows = PaintContextWindows::new(text, start..end).map_err(|error| match error {
            PaintContextError::InputBudget => Kind::UnsupportedMeasurementBudget,
            PaintContextError::InvalidRange => Kind::InvalidGeometry,
        })?;
        let paint_direction = match direction {
            Direction::LeftToRight => PaintShapeDirection::LeftToRight,
            Direction::RightToLeft => PaintShapeDirection::RightToLeft,
            _ => return Err(Kind::UnsupportedMeasurementShaping),
        };
        let requested_weight =
            if span.style.bold && matches!(self.styled.context(), super::TextContext::Placed) {
                fontdb::Weight::BOLD
            } else {
                fontdb::Weight::NORMAL
            };
        let synthesis = face.synthesis(requested_weight, span.style.italic);
        let mut shaper = face
            .paint_shaper(profile)
            .map_err(native_geometry::shape_error)?;
        let mut measured = MeasuredSegment {
            advance: 0.0,
            clusters: Vec::new(),
        };
        for window in windows.windows() {
            let local_context = window.context_scalar_range();
            let context =
                self.range.start + local_context.start..self.range.start + local_context.end;
            let local_source = window.selected_scalar_range();
            let source = self.range.start + local_source.start..self.range.start + local_source.end;
            let itemization =
                PaintItemization::new(window.source(), window.selected_range_in_window_utf16())
                    .map_err(|error| match error {
                        PaintItemizationError::InputBudget => Kind::UnsupportedMeasurementBudget,
                        PaintItemizationError::InvalidRange => Kind::InvalidGeometry,
                    })?;
            let piece = shaper
                .shape_text(PaintTextRequest::new(&itemization, paint_direction))
                .map_err(native_geometry::piece_error)?;
            if piece
                .entry_geometry()
                .glyphs()
                .iter()
                .any(|glyph| glyph.id() == 0)
            {
                return Err(Kind::UnsupportedMeasurementFont);
            }
            let geometry = native_geometry::NativeGeometry::new(
                &piece,
                &self.styled.index,
                context,
                source.clone(),
            )
            .map_err(|_| Kind::InvalidGeometry)?;
            let run = Arc::new(MeasuredRun {
                source,
                style: span.style.clone(),
                face: face.clone(),
                synthesis,
                direction,
                glyphs: geometry.glyphs,
                variable: false,
                tab: false,
                coverage_fallback: false,
                native_entries: Some(geometry.entries),
                measurement_issue: None,
                browser_shapes: Mutex::new(HashMap::new()),
            });
            measured.advance += run
                .native_entries
                .as_ref()
                .ok_or(Kind::InvalidGeometry)?
                .advance();
            measured.clusters.extend(geometry.clusters.into_iter().map(
                |(source, advance, glyphs)| MeasuredCluster {
                    source,
                    advance,
                    glyphs,
                    run: run.clone(),
                },
            ));
        }
        Ok(measured)
    }

    fn shape(
        &self,
        range: Range<usize>,
        style: &TextStyle,
        direction: Direction,
        script: Script,
        tab: bool,
        measurement_issue: TextDiagnosticKind,
    ) -> Result<MeasuredSegment, MeasurementError> {
        let source = self.range.start + range.start..self.range.start + range.end;
        let face = self
            .renderer
            .for_source(source.clone())
            .resolve(style, self.styled.context())
            .ok_or_else(|| {
                MeasurementError::UnavailableFace(
                    style.family.as_deref().unwrap_or("Roboto").to_owned(),
                )
            })?;
        let text = if tab {
            " "
        } else {
            self.styled
                .index
                .slice(source.clone())
                .ok_or(MeasurementError::InvalidRange)?
        };
        if face.covers(text)? {
            return self.shape_face(
                range,
                style,
                direction,
                script,
                tab,
                RunFace {
                    face,
                    coverage_fallback: false,
                    measurement_issue,
                },
            );
        }
        let bold = style.bold && matches!(self.styled.context(), super::TextContext::Placed);
        if tab
            || direction != Direction::LeftToRight
            || !matches!(script, Script::Latin | Script::Common | Script::Inherited)
        {
            let selected = self
                .renderer
                .fonts
                .resolve_for_text(&face, text, bold, style.italic)?;
            let coverage_fallback = selected.id != face.id;
            return self.shape_face(
                range,
                style,
                direction,
                script,
                tab,
                RunFace {
                    face: selected,
                    coverage_fallback,
                    measurement_issue,
                },
            );
        }
        let start_byte = self
            .styled
            .index
            .char_to_byte(source.start)
            .ok_or(MeasurementError::InvalidRange)?;
        let mut segments = Vec::<(Range<usize>, ResolvedFace)>::new();
        for (byte, grapheme) in text.grapheme_indices(true) {
            let selected =
                self.renderer
                    .fonts
                    .resolve_for_text(&face, grapheme, bold, style.italic)?;
            let start = self
                .styled
                .index
                .byte_to_char(start_byte + byte)
                .ok_or(MeasurementError::InvalidRange)?
                - self.range.start;
            let end = self
                .styled
                .index
                .byte_to_char(start_byte + byte + grapheme.len())
                .ok_or(MeasurementError::InvalidRange)?
                - self.range.start;
            if let Some((previous, previous_face)) = segments.last_mut()
                && previous.end == start
                && previous_face.id == selected.id
            {
                previous.end = end;
            } else {
                segments.push((start..end, selected));
            }
        }
        let mut measured = MeasuredSegment {
            advance: 0.0,
            clusters: Vec::new(),
        };
        for (range, selected) in segments {
            let coverage_fallback = selected.id != face.id;
            let part = self.shape_face(
                range,
                style,
                direction,
                script,
                tab,
                RunFace {
                    face: selected,
                    coverage_fallback,
                    measurement_issue: measurement_issue.clone(),
                },
            )?;
            measured.advance += part.advance;
            measured.clusters.extend(part.clusters);
        }
        Ok(measured)
    }

    fn shape_face(
        &self,
        range: Range<usize>,
        style: &TextStyle,
        direction: Direction,
        script: Script,
        tab: bool,
        selected: RunFace,
    ) -> Result<MeasuredSegment, MeasurementError> {
        let face = selected.face;
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
        let geometry = FontGeometry::new(style.font_size, shaped.metrics.units_per_em);
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
            sources.insert(
                byte,
                self.styled
                    .index
                    .source(source)
                    .ok_or(MeasurementError::InvalidCluster)?,
            );
        }
        let glyphs = geometry.measured_glyphs(&shaped.glyphs, &sources);
        let requested_weight =
            if style.bold && matches!(self.styled.context(), super::TextContext::Placed) {
                fontdb::Weight::BOLD
            } else {
                fontdb::Weight::NORMAL
            };
        let synthesis = face.synthesis(requested_weight, style.italic);
        let run = Arc::new(MeasuredRun {
            source,
            style: style.clone(),
            face,
            synthesis,
            direction,
            glyphs,
            variable,
            tab,
            coverage_fallback: selected.coverage_fallback,
            native_entries: None,
            measurement_issue: Some(selected.measurement_issue),
            browser_shapes: Mutex::new(HashMap::new()),
        });
        let mut clusters = Vec::new();
        for (&byte, (advance, glyphs)) in &cluster_glyphs {
            clusters.push(MeasuredCluster {
                source: sources[&byte].characters().clone(),
                advance: geometry.horizontal_advance(*advance, tab),
                run: run.clone(),
                glyphs: glyphs.clone(),
            });
        }
        Ok(MeasuredSegment {
            advance: geometry.horizontal_advance(shaped.advance_x(), tab),
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

    #[test]
    fn paragraph_measurement_uses_captured_native_entries_and_owner_positions() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-entry-geometry.json"
        )))
        .unwrap();
        let fonts = FontBook::default();
        let settings = TextSettings::resolved();
        for name in [
            "av_default",
            "fractional_size",
            "large_size",
            "marks",
            "partial_marks",
            "italic_marks",
        ] {
            let case = fixture["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|case| case["name"] == name)
                .unwrap();
            for prefix in ["", "😀\n"] {
                let paragraph_start = prefix.chars().count();
                let utf16_base = prefix.encode_utf16().count() as u32;
                let mut content =
                    text_box(&format!("{prefix}{}", case["text_utf8"].as_str().unwrap()));
                content.font_size = Some(f32::from_bits(
                    case["font_size_bits"].as_u64().unwrap() as u32
                ));
                if name == "italic_marks" {
                    content.spans.push(source_span(
                        RichTextSpanType::Underline,
                        utf16_base..utf16_base + 1,
                        vec![1, 0],
                    ));
                }
                let renderer = TextRenderer::new(settings, &fonts);
                let styled = StyledText::new(&content, TextContext::Flow, settings);
                let bounds = case["range_utf16"].as_array().unwrap();
                let range = styled
                    .index
                    .utf16_to_char(utf16_base + bounds[0].as_u64().unwrap() as u32)
                    .unwrap()
                    ..styled
                        .index
                        .utf16_to_char(utf16_base + bounds[1].as_u64().unwrap() as u32)
                        .unwrap();
                let measured = ParagraphMeasurer::new(
                    &styled,
                    paragraph_start..styled.index.len(),
                    RenderTheme::for_canvas(false),
                    None,
                    &renderer,
                )
                .unwrap()
                .measure_line(range)
                .unwrap();
                let run = &measured.clusters[0].run;
                assert!(run.measurement_issue().is_none());
                let native = run
                    .native_entries
                    .as_ref()
                    .expect("default pinned regular uses native producer");
                assert_eq!(
                    native.source().characters(),
                    &(paragraph_start..styled.index.len())
                );
                let expected = &case["entry_geometry"];
                let widths = expected["entry_widths_bits"].as_array().unwrap();
                let selected = native.geometry().source_range_utf16();
                for (slot, entry) in native.geometry().entries().iter().enumerate() {
                    assert_eq!(
                        entry.advance().to_bits(),
                        widths[selected.start as usize + slot].as_u64().unwrap() as u32,
                        "{name} UTF16 slot {slot}"
                    );
                }
                let captured = expected["glyphs"].as_array().unwrap();
                assert_eq!(run.glyphs.len(), captured.len(), "{name}");
                for (glyph, captured) in run.glyphs.iter().zip(captured) {
                    assert_eq!(
                        glyph.id,
                        captured["glyph_id"].as_u64().unwrap() as u32,
                        "{name}"
                    );
                    assert_eq!(
                        glyph.source.utf16().start,
                        utf16_base + captured["owner_utf16"].as_u64().unwrap() as u32,
                        "{name}"
                    );
                    let position = captured["entry_position_bits"].as_array().unwrap();
                    assert_eq!(
                        glyph.owner_offset,
                        [
                            f64::from(f32::from_bits(position[0].as_u64().unwrap() as u32)),
                            f64::from(f32::from_bits(position[1].as_u64().unwrap() as u32))
                        ],
                        "{name}"
                    );
                }
            }
        }
    }

    #[test]
    fn unsupported_native_profiles_report_the_source_before_compatibility_measurement() {
        use super::super::resources::SourceOwner;
        let mut italic = text_box("A");
        italic.runs.push(RichTextRun {
            start: 0,
            end: 1,
            bold: false,
            italic: true,
        });
        let mut skewed_composite = text_box("é");
        skewed_composite.underline = true;
        let fonts = FontBook::default();
        let settings = TextSettings::resolved();
        for (content, expected) in [
            (italic, TextDiagnosticKind::UnsupportedMeasurementStyle),
            (
                skewed_composite,
                TextDiagnosticKind::UnsupportedMeasurementShaping,
            ),
            (
                text_box("\t"),
                TextDiagnosticKind::UnsupportedTabMeasurement,
            ),
            (
                text_box(&"A".repeat(65_537)),
                TextDiagnosticKind::UnsupportedMeasurementBudget,
            ),
        ] {
            let styled = StyledText::new(&content, TextContext::Placed, settings);
            let renderer = TextRenderer::new(settings, &fonts);
            let measured = ParagraphMeasurer::new(
                &styled,
                0..styled.index.len(),
                RenderTheme::for_canvas(false),
                None,
                &renderer,
            )
            .unwrap()
            .measure_line(0..1)
            .unwrap();
            assert!(measured.advance.is_finite());
            assert!(measured.clusters[0].run.native_entries.is_none());
            assert!(
                measured
                    .clusters
                    .iter()
                    .all(|cluster| cluster.run.measurement_issue() == Some(&expected))
            );
            let diagnostics = renderer.scoped_diagnostics();
            assert!(diagnostics.iter().any(|diagnostic| diagnostic.owner
                == Some(SourceOwner::Text(0..1))
                && diagnostic.diagnostic.kind == expected));
        }
    }

    #[test]
    fn named_style_metadata_cannot_activate_regular_font_fakery() {
        use super::super::resources::SourceOwner;
        use crate::fonts::NativeFontNameConfig;
        let mut database = fontdb::Database::new();
        database
            .load_font_data(include_bytes!("../../../assets/fonts/Roboto-Regular.ttf").to_vec());
        let names = NativeFontNameConfig::new("sans-serif")
            .unwrap()
            .with_family_alias("sans-serif", "Roboto")
            .unwrap()
            .with_font_file("Roboto-Regular.ttf", "sans-serif")
            .unwrap()
            .with_font_file("Roboto-Bold.ttf", "sans-serif")
            .unwrap()
            .with_font_file("Roboto-Italic.ttf", "sans-serif")
            .unwrap();
        let fonts = FontBook::new(Arc::new(database)).with_native_name_config(names);
        let settings = TextSettings::resolved();
        for family in ["Roboto-Bold", "Roboto-Italic"] {
            let mut content = text_box("A");
            let name = format!("{family}\0");
            content.spans.push(source_span(
                RichTextSpanType::FontName,
                0..1,
                [
                    vec![0; 8],
                    (name.len() as u16).to_le_bytes().to_vec(),
                    name.into_bytes(),
                ]
                .concat(),
            ));
            let renderer = TextRenderer::new(settings, &fonts);
            let styled = StyledText::new(&content, TextContext::Placed, settings);
            let measured = measure_text(
                &styled,
                0..1,
                RenderTheme::for_canvas(false),
                None,
                &renderer,
            )
            .unwrap();
            assert!(measured.clusters[0].run.native_entries.is_none());
            assert_eq!(
                measured.clusters[0].run.measurement_issue(),
                Some(&TextDiagnosticKind::UnsupportedMeasurementFont)
            );
            assert!(
                renderer
                    .scoped_diagnostics()
                    .iter()
                    .any(
                        |diagnostic| diagnostic.owner == Some(SourceOwner::Text(0..1))
                            && diagnostic.diagnostic.kind
                                == TextDiagnosticKind::UnsupportedMeasurementFont
                    )
            );
        }
    }

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
        measure_with_fonts(text_box, context, &FontBook::default())
    }

    fn measure_compatibility(text_box: &RichTextBox, context: TextContext) -> MeasuredText {
        let mut database = fontdb::Database::new();
        database
            .load_font_data(include_bytes!("../../../assets/fonts/Roboto-Regular.ttf").to_vec());
        let fonts = FontBook::new(Arc::new(database));
        let measured = measure_with_fonts(text_box, context, &fonts);
        assert!(
            measured
                .clusters
                .iter()
                .all(|cluster| cluster.run.native_entries.is_none())
        );
        measured
    }

    fn measure_with_fonts(
        text_box: &RichTextBox,
        context: TextContext,
        fonts: &FontBook,
    ) -> MeasuredText {
        measure_with_theme(text_box, context, fonts, RenderTheme::for_canvas(false))
    }

    fn measure_with_theme(
        text_box: &RichTextBox,
        context: TextContext,
        fonts: &FontBook,
        theme: RenderTheme,
    ) -> MeasuredText {
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
        };
        let renderer = TextRenderer::new(settings, fonts);
        let styled = StyledText::new(text_box, context, settings);
        measure_text(&styled, 0..styled.index.len(), theme, None, &renderer).unwrap()
    }

    fn source_span(kind: RichTextSpanType, range: Range<u32>, payload: Vec<u8>) -> RichTextSpan {
        RichTextSpan {
            kind,
            start_utf16: range.start,
            end_utf16: range.end,
            interval_type: crate::SpanIntervalType::ClosedOpen,
            payload,
        }
    }

    fn assert_av_grouping(measured: &MeasuredText, joined: bool) {
        assert_eq!(measured.clusters.len(), 2);
        assert_eq!(
            Arc::ptr_eq(&measured.clusters[0].run, &measured.clusters[1].run),
            joined
        );
        let expected = if joined {
            measure(&text_box("AV"), TextContext::Placed).advance
        } else {
            measure(&text_box("A"), TextContext::Placed).advance
                + measure(&text_box("V"), TextContext::Placed).advance
        };
        assert_eq!(measured.advance, expected);
    }

    #[test]
    fn differing_foreground_alpha_splits_runs_with_identical_paint_colors() {
        let mut content = text_box("AV");
        content.spans.push(source_span(
            RichTextSpanType::ForegroundColor,
            0..1,
            0x0026_2626_u32.to_le_bytes().to_vec(),
        ));
        let styled = StyledText::new(&content, TextContext::Placed, TextSettings::default());
        let theme = RenderTheme::for_canvas(false);
        assert_eq!(
            styled.style_at(0, theme, None).color,
            styled.style_at(1, theme, None).color
        );
        assert_av_grouping(&measure(&content, TextContext::Placed), false);
    }

    #[test]
    fn absent_and_explicit_empty_font_names_split_runs_with_the_same_face() {
        let mut content = text_box("AV");
        let payload = [vec![0; 8], 1_u16.to_le_bytes().to_vec(), vec![0]].concat();
        content
            .spans
            .push(source_span(RichTextSpanType::FontName, 1..2, payload));
        let measured = measure(&content, TextContext::Placed);
        assert_av_grouping(&measured, false);
        for cluster in &measured.clusters {
            assert!(cluster.run.style.family.is_none());
            assert_eq!(cluster.run.face.family, "Roboto");
        }
    }

    #[test]
    fn hyperlink_paint_color_does_not_replace_measurement_foreground() {
        let hyperlink = [9_u32.to_le_bytes(), [0; 4], [0; 4]].concat();
        let mut content = text_box("AV");
        content.spans.push(source_span(
            RichTextSpanType::Hyperlink,
            1..2,
            hyperlink.clone(),
        ));
        let theme = RenderTheme::for_canvas(false);
        let styled = StyledText::new(&content, TextContext::Placed, TextSettings::default());
        assert_ne!(
            styled.style_at(0, theme, None).color,
            styled.style_at(1, theme, None).color
        );
        assert_av_grouping(&measure(&content, TextContext::Placed), true);

        content.spans[0] = source_span(RichTextSpanType::Hyperlink, 0..2, hyperlink);
        content.spans.push(source_span(
            RichTextSpanType::ForegroundColor,
            1..2,
            0xffff_0000_u32.to_le_bytes().to_vec(),
        ));
        let styled = StyledText::new(&content, TextContext::Placed, TextSettings::default());
        assert_eq!(
            styled.style_at(0, theme, None).color,
            styled.style_at(1, theme, None).color
        );
        assert_av_grouping(&measure(&content, TextContext::Placed), false);
    }

    #[test]
    fn equal_dark_paint_colors_preserve_different_native_measurement_foregrounds() {
        let mut content = text_box("AV");
        content.spans.push(source_span(
            RichTextSpanType::ForegroundColor,
            1..2,
            0xffd9_d9d9_u32.to_le_bytes().to_vec(),
        ));
        let fonts = FontBook::default();
        let styled = StyledText::new(&content, TextContext::Placed, TextSettings::default());
        let dark_theme = RenderTheme::for_canvas(true);
        assert_eq!(
            styled.style_at(0, dark_theme, None).color,
            styled.style_at(1, dark_theme, None).color
        );
        for dark in [false, true] {
            let measured = measure_with_theme(
                &content,
                TextContext::Placed,
                &fonts,
                RenderTheme::for_canvas(dark),
            );
            assert_av_grouping(&measured, false);
        }
    }

    #[test]
    fn source_span_projection_matches_supported_native_measurement_join_cases() {
        use sha2::Digest;

        const FIXTURE: &str =
            include_str!("../../../../../conformance/table-text-measurement-join.json");
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(FIXTURE.as_bytes())),
            "45cfbe69af0ba5a931c5a06a93b289af44e3723f68790bb82c0c6da59b218e2b"
        );
        let capture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        let mut checked = 0;
        for case in capture["cases"].as_array().unwrap() {
            let word = |side: &str, field: &str| {
                u32::try_from(case[side][field].as_u64().unwrap()).unwrap()
            };
            if ["left", "right"].iter().any(|side| {
                let size = f32::from_bits(word(side, "font_size_bits"));
                word(side, "style") & 0xc0 != 0
                    || word(side, "flags") & 2 != 0
                    || !size.is_finite()
                    || size < 1.0
            }) {
                continue;
            }
            let mut content = text_box("AV");
            for (start, side) in [(0, "left"), (1, "right")] {
                let mut append = |kind, payload| {
                    content
                        .spans
                        .push(source_span(kind, start..start + 1, payload));
                };
                append(
                    RichTextSpanType::FontSize,
                    word(side, "font_size_bits").to_le_bytes().to_vec(),
                );
                append(
                    RichTextSpanType::ForegroundColor,
                    [word(side, "foreground").to_le_bytes(), [0; 4]].concat(),
                );
                for (kind, bit) in [
                    (RichTextSpanType::Bold, 1),
                    (RichTextSpanType::Italic, 2),
                    (RichTextSpanType::Underline, 4),
                    (RichTextSpanType::Strikethrough, 8),
                ] {
                    append(
                        kind,
                        u32::from(word(side, "style") & bit != 0)
                            .to_le_bytes()
                            .to_vec(),
                    );
                }
                if word(side, "style") & 0x10 != 0 {
                    append(RichTextSpanType::Suggestion, vec![0; 12]);
                }
                if word(side, "flags") & 1 != 0 {
                    append(
                        RichTextSpanType::Hyperlink,
                        [1_u32.to_le_bytes(), [0; 4], [0; 4]].concat(),
                    );
                }
                if let Some(units) = case[format!("{side}_font_utf16")].as_array() {
                    let units = units
                        .iter()
                        .map(|unit| u16::try_from(unit.as_u64().unwrap()).unwrap())
                        .collect::<Vec<_>>();
                    let name = String::from_utf16(&units).unwrap();
                    let encoded = cesu8::to_cesu8(&name);
                    let mut payload = vec![0; 8];
                    payload.extend_from_slice(
                        &u16::try_from(encoded.len() + 1).unwrap().to_le_bytes(),
                    );
                    payload.extend_from_slice(&encoded);
                    payload.push(0);
                    append(RichTextSpanType::FontName, payload);
                }
            }
            let styled = StyledText::new(&content, TextContext::Placed, TextSettings::default());
            let theme = RenderTheme::for_canvas(false);
            let left = styled.resolved_style_at(0, theme, None).measurement;
            let right = styled.resolved_style_at(1, theme, None).measurement;
            let expected = case["join_forward_reverse_self_left_self_right"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_bool().unwrap())
                .collect::<Vec<_>>();
            let comparisons = [
                (&left, &right),
                (&right, &left),
                (&left, &left),
                (&right, &right),
            ]
            .map(|(left, right)| left == right);
            assert_eq!(comparisons.as_slice(), expected, "{}", case["name"]);
            checked += 1;
        }
        assert_eq!(checked, 89);
    }

    #[test]
    fn retained_arabic_mark_and_ligature_keep_original_glyphs_and_source_bytes() {
        let mut database = crate::fonts::fontdb::Database::new();
        database
            .load_font_data(include_bytes!("../../../tests/assets/fonts/DejaVuSans.ttf").to_vec());
        database.set_sans_serif_family("DejaVu Sans");
        let fonts = FontBook::new(Arc::new(database));
        let content = text_box("Aلَّا");
        let measured = measure_with_fonts(&content, TextContext::Placed, &fonts);
        assert_eq!(measured.clusters.len(), 2);
        let cluster = &measured.clusters[1];
        assert_eq!(cluster.source, 1..5);
        assert_eq!(cluster.run.direction, Direction::RightToLeft);
        assert_eq!(cluster.advance, 25.6640625);
        let glyphs: Vec<_> = cluster.retained_glyphs().unwrap().collect();
        assert_eq!(glyphs.len(), 2);
        assert_eq!(glyphs[0].glyph_id, 6020);
        assert_eq!(glyphs[1].glyph_id, 5365);
        for glyph in &glyphs {
            assert_eq!(glyph.source.characters(), &(1..5));
            assert_eq!(glyph.source.bytes(), &(1..9));
            assert_eq!(glyph.source.utf16(), &(1..5));
            assert_eq!(glyph.face.family, "DejaVu Sans");
            assert_eq!(glyph.advance_y, 0.0);
        }
        assert_eq!(glyphs[0].offset_x, 7.80029296875);
        assert_eq!(glyphs[0].offset_y, -9.8876953125);
        assert_eq!(glyphs[0].advance_x, 0.0);
        assert_eq!(glyphs[1].offset_x, 0.0);
        assert_eq!(glyphs[1].offset_y, 0.0);
        assert_eq!(glyphs[1].advance_x, 25.6640625);
        assert!(cluster.native_paint_offset("لَّا").unwrap().is_none());
    }

    #[test]
    fn retained_supplementary_and_combining_clusters_preserve_utf16_ownership() {
        let measured = measure(&text_box("A😀e\u{301}"), TextContext::Placed);
        assert_eq!(measured.clusters.len(), 3);
        for (cluster, (characters, bytes, utf16)) in measured.clusters.iter().zip([
            (0..1, 0..1, 0..1),
            (1..2, 1..5, 1..3),
            (2..4, 5..8, 3..5),
        ]) {
            let retained: Vec<_> = cluster.retained_glyphs().unwrap().collect();
            assert!(!retained.is_empty());
            for glyph in retained {
                assert_eq!(glyph.source.characters(), &characters);
                assert_eq!(glyph.source.bytes(), &bytes);
                assert_eq!(glyph.source.utf16(), &utf16);
            }
        }
    }

    #[test]
    fn retained_rtl_clusters_keep_absolute_utf16_owners_after_supplementary_prefix() {
        let content = text_box("😀\nAאב");
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
        };
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let styled = StyledText::new(&content, TextContext::Placed, settings);
        let paragraph = ParagraphMeasurer::new(
            &styled,
            2..5,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
        )
        .unwrap();
        let measured = paragraph.measure_line(3..5).unwrap();
        assert_eq!(measured.clusters.len(), 2);
        let run = &measured.clusters[0].run;
        assert_eq!(run.direction, Direction::RightToLeft);
        assert_eq!(
            run.glyphs
                .iter()
                .map(|glyph| glyph.source.utf16().start)
                .collect::<Vec<_>>(),
            [5, 4]
        );
        for (cluster, (characters, bytes, utf16)) in measured
            .clusters
            .iter()
            .zip([(3..4, 6..8, 4..5), (4..5, 8..10, 5..6)])
        {
            let retained: Vec<_> = cluster.retained_glyphs().unwrap().collect();
            assert_eq!(retained.len(), 1);
            assert_eq!(retained[0].source.characters(), &characters);
            assert_eq!(retained[0].source.bytes(), &bytes);
            assert_eq!(retained[0].source.utf16(), &utf16);
        }
    }

    #[test]
    fn retained_glyphs_reject_mismatched_cluster_ownership_and_invalid_glyph_ranges() {
        let measured = measure(&text_box("x\u{301}"), TextContext::Placed);
        let mut cluster = measured.clusters[0].clone();
        cluster.source = 0..1;
        assert!(matches!(
            cluster.retained_glyphs(),
            Err(MeasurementError::InvalidCluster)
        ));
        cluster.source = 0..2;
        cluster.glyphs = 0..0;
        assert_eq!(cluster.retained_glyphs().unwrap().count(), 0);
        cluster.glyphs = 0..usize::MAX;
        assert!(matches!(
            cluster.retained_glyphs(),
            Err(MeasurementError::InvalidCluster)
        ));
    }

    #[test]
    fn measured_fallback_issues_keep_full_source_run_ranges() {
        use super::super::resources::{SourceOwner, TextDiagnosticKind};

        let mut content = text_box("ok\nabcd");
        let family = b"Missing family\0";
        let payload = [
            vec![0; 8],
            (family.len() as u16).to_le_bytes().to_vec(),
            family.to_vec(),
        ]
        .concat();
        content.spans.push(RichTextSpan {
            kind: RichTextSpanType::FontName,
            start_utf16: 3,
            end_utf16: 7,
            interval_type: crate::SpanIntervalType::from(0),
            payload,
        });
        let settings = TextSettings::default();
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        let paragraph = ParagraphMeasurer::new(
            &styled,
            3..7,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
        )
        .unwrap();
        let measured = paragraph.measure_line(3..7).unwrap();
        assert_eq!(measured.clusters[0].source.start, 3);
        let issues = renderer.scoped_diagnostics();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].owner, Some(SourceOwner::Text(3..7)));
        assert_eq!(
            issues[0].diagnostic.kind,
            TextDiagnosticKind::UnavailableFamily
        );
        assert_eq!(issues[0].diagnostic.family, "Missing family");
    }

    #[test]
    fn compatibility_glyph_geometry_matches_independent_ava_positions() {
        let measured = measure_compatibility(&text_box("AVA"), TextContext::Placed);
        let run = &measured.clusters[0].run;
        assert_eq!(run.source, 0..3);
        assert_eq!(run.face.family, "Roboto");
        assert_eq!(run.style.font_size, 45.0);
        assert_eq!(run.direction, Direction::LeftToRight);
        assert!(!run.variable);
        assert_eq!(
            run.glyphs
                .iter()
                .map(|glyph| (
                    glyph.id,
                    glyph.source.characters().clone(),
                    glyph.transport_advance,
                    glyph.owner_offset,
                ))
                .collect::<Vec<_>>(),
            [
                (38, 0..1, [27.44384765625, 0.0], [0.0, 0.0]),
                (59, 1..2, [26.982421875, 0.0], [0.0, 0.0]),
                (38, 2..3, [29.35546875, 0.0], [0.0, 0.0])
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
    fn compatibility_combining_offsets_and_nfc_glyphs_reproduce_with_the_selected_face() {
        let marked = measure_compatibility(&text_box("x\u{301}"), TextContext::Placed);
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
                    glyph.id,
                    glyph.transport_advance,
                    glyph.owner_offset,
                    glyph.source.characters().clone(),
                ))
                .collect::<Vec<_>>(),
            [
                (93, [22.30224609375, 0.0], [0.0, 0.0], 0..2),
                (434, [0.0, 0.0], [23.5546875, 0.2197265625], 0..2),
            ],
        );
        let offset = cluster.paint_offset("x\u{301}").unwrap().unwrap();
        assert_eq!(offset.x, 0.0);
        assert_eq!(offset.y, 0.0);
        let decomposed = measure_compatibility(&text_box("e\u{301}"), TextContext::Placed);
        let composed = measure_compatibility(&text_box("é"), TextContext::Placed);
        assert_eq!(decomposed.clusters[0].run.glyphs[0].id, 2317);
        assert_eq!(composed.clusters[0].run.glyphs[0].id, 2317);
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
            run.glyphs.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
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
                interval_type: crate::SpanIntervalType::from(0),
                payload,
            });
        }
        content.spans.push(RichTextSpan {
            kind: RichTextSpanType::Strikethrough,
            start_utf16: 0,
            end_utf16: 1,
            interval_type: crate::SpanIntervalType::from(0),
            payload: vec![1, 0],
        });
        let measured = measure(&content, TextContext::Placed);
        assert_eq!(
            measured.advance,
            measure(&text_box("AV"), TextContext::Placed).advance
        );
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
                .map(|glyph| glyph.source.characters().clone())
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
    }

    #[test]
    fn native_rtl_scalar_positions_require_exact_standalone_glyphs() {
        let measured = measure(&text_box("\u{202e}AV"), TextContext::Placed);
        let letters = &measured.clusters[1..];
        assert_eq!(letters.len(), 2);
        assert_eq!(
            letters[0]
                .run
                .glyphs
                .iter()
                .map(|glyph| (glyph.id, glyph.source.characters().clone()))
                .collect::<Vec<_>>(),
            [(59, 2..3), (38, 1..2)]
        );
        for (cluster, character) in letters.iter().zip(["A", "V"]) {
            assert_eq!(cluster.run.direction, Direction::RightToLeft);
            assert!(cluster.paint_offset(character).unwrap().is_none());
            let offset = cluster.native_paint_offset(character).unwrap().unwrap();
            assert_eq!(offset.x, 0.0);
            assert_eq!(offset.y, 0.0);
        }
        let mirrored = measure(&text_box("\u{202e}("), TextContext::Placed);
        let bracket = &mirrored.clusters[1];
        assert_eq!(bracket.run.direction, Direction::RightToLeft);
        assert!(bracket.native_paint_offset("(").unwrap().is_none());
        let marked = measure(&text_box("\u{202e}x\u{301}"), TextContext::Placed);
        let combined = &marked.clusters[1];
        assert_eq!(combined.source, 1..3);
        assert!(combined.native_paint_offset("x\u{301}").unwrap().is_none());
    }

    #[test]
    fn compatibility_greek_and_cyrillic_reproduce_pinned_glyph_positions() {
        for (text, glyphs, advance) in [
            ("λΩ", [(579, 1134, 0), (569, 1362, 1134)], 54.84375),
            ("Жя", [(624, 1859, 0), (663, 1124, 1859)], 65.54443359375),
            ("λλ", [(579, 1150, 0), (579, 1134, 1150)], 50.185546875),
        ] {
            let measured = measure_compatibility(&text_box(text), TextContext::Placed);
            assert_eq!(measured.advance, advance);
            assert_eq!(measured.clusters.len(), 2);
            let run = &measured.clusters[0].run;
            assert_eq!(run.face.family, "Roboto");
            assert_eq!(run.direction, Direction::LeftToRight);
            assert_eq!(
                run.glyphs
                    .iter()
                    .map(|glyph| (glyph.id, glyph.transport_advance[0], glyph.owner_offset))
                    .collect::<Vec<_>>(),
                glyphs.map(|(id, advance, _)| (id, f64::from(advance) * 45.0 / 2048.0, [0.0, 0.0])),
            );
            for (index, (cluster, character)) in
                measured.clusters.iter().zip(text.chars()).enumerate()
            {
                assert!(Arc::ptr_eq(run, &cluster.run));
                assert_eq!(cluster.source, index..index + 1);
                assert_eq!(cluster.glyphs, index..index + 1);
                assert!(cluster.supports_positioned_text());
                let offset = cluster
                    .paint_offset(&character.to_string())
                    .unwrap()
                    .unwrap();
                assert_eq!(offset.x, 0.0);
                assert_eq!(offset.y, 0.0);
            }
        }
    }

    #[test]
    fn mixed_supported_scripts_keep_missing_and_mismatched_glyph_fallbacks_local() {
        let measured = measure(&text_box("λЖ😀"), TextContext::Placed);
        assert_eq!(measured.clusters.len(), 3);
        let greek = &measured.clusters[0];
        let cyrillic = &measured.clusters[1];
        let missing = &measured.clusters[2];
        assert_eq!(greek.source, 0..1);
        assert_eq!(cyrillic.source, 1..2);
        assert_eq!(missing.source, 2..3);
        assert!(greek.paint_offset("λ").unwrap().is_some());
        assert!(cyrillic.paint_offset("Ж").unwrap().is_some());
        assert!(greek.paint_offset("Ω").unwrap().is_none());
        assert!(cyrillic.paint_offset("я").unwrap().is_none());
        assert_eq!(missing.run.glyphs[missing.glyphs.start].id, 0);
        assert!(!missing.supports_positioned_text());
        assert!(missing.paint_offset("😀").unwrap().is_none());
        assert!(missing.native_paint_offset("😀").unwrap().is_none());
    }

    #[test]
    fn compatibility_regular_advance_uses_font_units_and_local_size() {
        let measured = measure_compatibility(&text_box("ABC"), TextContext::Placed);
        assert_eq!(measured.advance, 86.66015625);
        assert_eq!(measured.font_size, 45.0);
        let mut smaller = text_box("ABC");
        smaller.font_size = Some(22.5);
        assert_eq!(
            measure_compatibility(&smaller, TextContext::Placed).advance,
            measured.advance / 2.0
        );
    }

    #[test]
    fn native_features_disable_ligatures_without_disabling_kerning() {
        let office = measure(&text_box("office"), TextContext::Placed);
        assert_eq!(office.clusters.len(), 6);
        let av = measure(&text_box("AV"), TextContext::Placed);
        assert!(av.clusters[0].run.native_entries.is_some());
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
            measure(&text_box("AV"), TextContext::Placed).advance
        );
        same.spans.push(RichTextSpan {
            kind: RichTextSpanType::ForegroundColor,
            start_utf16: 1,
            end_utf16: 2,
            interval_type: crate::SpanIntervalType::from(0),
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
        for (kind, payload) in [
            (RichTextSpanType::Underline, vec![1, 0]),
            (RichTextSpanType::Strikethrough, vec![1, 0]),
            (RichTextSpanType::Composing, vec![0; 8]),
            (
                RichTextSpanType::BackgroundColor,
                0xffff_0000_u32.to_le_bytes().to_vec(),
            ),
            (
                RichTextSpanType::ComposingBackgroundColor,
                [0xffff_0000_u32.to_le_bytes(), [0; 4]].concat(),
            ),
            (RichTextSpanType::Suggestion, vec![0; 12]),
        ] {
            let mut decorated = text_box("AV");
            decorated.spans.push(RichTextSpan {
                kind,
                start_utf16: 0,
                end_utf16: 1,
                interval_type: crate::SpanIntervalType::from(0),
                payload,
            });
            let measured = measure(&decorated, TextContext::Placed);
            assert_eq!(measured.advance, baseline);
            assert_av_grouping(&measured, true);
        }
        for kind in [RichTextSpanType::Bold, RichTextSpanType::Italic] {
            let mut changed = text_box("AV");
            changed.spans.push(RichTextSpan {
                kind,
                start_utf16: 0,
                end_utf16: 1,
                interval_type: crate::SpanIntervalType::from(0),
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
            ..Default::default()
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
        assert_eq!(
            paragraph.bidi().direction_at(3),
            Some(Direction::RightToLeft)
        );
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
    fn unsupported_tabs_keep_four_compatibility_spaces() {
        let tab = measure(&text_box("\t"), TextContext::Placed);
        let space = measure_compatibility(&text_box(" "), TextContext::Placed);
        assert_eq!(tab.advance, space.advance * 4.0);
        assert_eq!(tab.clusters[0].source, 0..1);
        assert!(tab.clusters[0].run.tab);
        assert!(tab.clusters[0].paint_offset("\t").unwrap().is_some());
    }

    #[test]
    fn coverage_segments_keep_adjacent_latin_kerning_and_cluster_sources() {
        let measured = measure(&text_box("AV∕AV"), TextContext::Placed);
        let covered = measure(&text_box("AV"), TextContext::Placed);
        let fallback = measure(&text_box("∕"), TextContext::Placed);
        assert_eq!(measured.advance, covered.advance * 2.0 + fallback.advance);
        assert_eq!(measured.clusters.len(), 5);
        assert!(Arc::ptr_eq(
            &measured.clusters[0].run,
            &measured.clusters[1].run
        ));
        assert!(Arc::ptr_eq(
            &measured.clusters[3].run,
            &measured.clusters[4].run
        ));
        assert_eq!(measured.clusters[2].source, 2..3);
        assert_eq!(measured.clusters[2].run.face.family, "Roboto Mono");
        assert!(measured.clusters[2].run.coverage_fallback);
        assert!(!measured.clusters[0].run.coverage_fallback);
        assert!(!measured.clusters[3].run.coverage_fallback);
        assert!(measured.clusters[0].run.measurement_issue().is_none());
        assert!(measured.clusters[3].run.measurement_issue().is_none());
        assert_eq!(
            measured.clusters[2].run.measurement_issue(),
            Some(&TextDiagnosticKind::UnsupportedMeasurementFont)
        );
    }

    #[test]
    fn logical_bold_preserves_native_name_selected_face_and_draw_synthesis() {
        let mut bold = text_box("ABC");
        bold.runs.push(RichTextRun {
            start: 0,
            end: 3,
            bold: true,
            italic: false,
        });
        let regular = measure(&text_box("ABC"), TextContext::Placed);
        for context in [TextContext::Flow, TextContext::Placed] {
            let measured = measure(&bold, context);
            assert_eq!(measured.advance, regular.advance);
            assert!(measured.clusters[0].run.native_entries.is_some());
            assert_eq!(measured.clusters[0].run.face.weight, fontdb::Weight::NORMAL);
            assert_eq!(
                measured.clusters[0].run.synthesis.bold,
                matches!(context, TextContext::Placed)
            );
        }
    }

    #[test]
    fn retained_synthesis_uses_context_and_selected_face_without_metric_changes() {
        let mut database = fontdb::Database::new();
        database
            .load_font_data(include_bytes!("../../../assets/fonts/Roboto-Regular.ttf").to_vec());
        let fonts = FontBook::new(Arc::new(database));
        let mut content = text_box("ABC");
        content.runs.push(RichTextRun {
            start: 0,
            end: 3,
            bold: true,
            italic: true,
        });
        for context in [TextContext::Placed, TextContext::Flow] {
            let measured = measure_with_fonts(&content, context, &fonts);
            assert_eq!(measured.advance, 86.66015625);
            for cluster in &measured.clusters {
                assert_eq!(cluster.run.face.weight, fontdb::Weight::NORMAL);
                assert_eq!(cluster.run.face.style, fontdb::Style::Normal);
                assert_eq!(
                    cluster.run.synthesis,
                    FontSynthesis {
                        bold: matches!(context, TextContext::Placed),
                        italic: true,
                    }
                );
            }
        }
        let real_styles = measure(&content, TextContext::Placed);
        assert!(real_styles.clusters.iter().all(|cluster| {
            cluster.run.face.weight == fontdb::Weight::BOLD
                && cluster.run.face.style == fontdb::Style::Italic
                && cluster.run.synthesis == FontSynthesis::default()
        }));
    }

    #[test]
    fn line_measurement_keeps_paragraph_direction_context_and_checks_ranges() {
        let text_box = text_box("אב 123 cd");
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
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
        assert_eq!(
            paragraph.bidi().direction_at(3),
            Some(Direction::LeftToRight)
        );
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
    #[test]
    fn browser_reproduction_accepts_rigid_translation_and_rejects_tiny_internal_displacement() {
        let mut content = text_box("x\u{301}");
        content.font_size = Some(1.0e-12);
        let fonts = FontBook::default();
        let settings = TextSettings::resolved();
        let renderer = TextRenderer::new(settings, &fonts);
        let styled = StyledText::new(&content, TextContext::Placed, settings);
        let mut measured = measure_text(
            &styled,
            0..styled.index.len(),
            RenderTheme::for_canvas(false),
            None,
            &renderer,
        )
        .unwrap();
        assert_eq!(
            measured.clusters[0].run.style.font_size,
            f64::from(content.font_size.unwrap())
        );
        let cluster = &mut measured.clusters[0];
        assert!(cluster.paint_offset("x\u{301}").unwrap().is_some());
        let run = Arc::get_mut(&mut cluster.run).unwrap();
        for glyph in &mut run.glyphs {
            glyph.owner_offset[0] += 2.5e-13;
            glyph.owner_offset[1] -= 1.5e-13;
        }
        let translated = cluster.paint_offset("x\u{301}").unwrap().unwrap();
        assert_eq!(translated.x, 2.5e-13);
        assert_eq!(translated.y, -1.5e-13);
        Arc::get_mut(&mut cluster.run).unwrap().glyphs[1].owner_offset[0] += 1.0e-17;
        assert!(cluster.paint_offset("x\u{301}").unwrap().is_none());
        Arc::get_mut(&mut cluster.run).unwrap().glyphs[1].owner_offset[0] = f64::INFINITY;
        assert!(cluster.paint_offset("x\u{301}").unwrap().is_none());
    }

    #[test]
    fn prior_owner_vertical_advance_cannot_move_the_next_owners_paint_or_ink() {
        let content = text_box("Ax\u{301}");
        let measured = measure(&content, TextContext::Placed);
        let original = &measured.clusters[0].run;
        let index = crate::text_index::TextIndex::new(&content.text);
        let sources = BTreeMap::from([
            (0, index.source(0..1).unwrap()),
            (1, index.source(1..3).unwrap()),
        ]);
        let shaped = [
            ShapedGlyph {
                id: 38,
                cluster: 0,
                x_advance: 1249,
                y_advance: 1024,
                x_offset: 0,
                y_offset: 0,
                unsafe_to_break: false,
            },
            ShapedGlyph {
                id: 93,
                cluster: 1,
                x_advance: 1015,
                y_advance: 512,
                x_offset: 0,
                y_offset: 0,
                unsafe_to_break: false,
            },
            ShapedGlyph {
                id: 434,
                cluster: 1,
                x_advance: 0,
                y_advance: 0,
                x_offset: 57,
                y_offset: -10,
                unsafe_to_break: false,
            },
        ];
        let geometry = original.geometry();
        let glyphs = geometry.measured_glyphs(&shaped, &sources);
        assert_eq!(glyphs[0].transport_advance[1], -22.5);
        assert_eq!(glyphs[1].owner_offset, [0.0, 0.0]);
        assert_eq!(glyphs[2].owner_offset, [23.5546875, -11.0302734375]);
        let run = Arc::new(MeasuredRun {
            source: 0..3,
            style: original.style.clone(),
            face: original.face.clone(),
            synthesis: original.synthesis,
            direction: original.direction,
            glyphs,
            variable: false,
            tab: false,
            coverage_fallback: false,
            native_entries: None,
            measurement_issue: None,
            browser_shapes: Mutex::new(HashMap::new()),
        });
        let cluster = MeasuredCluster {
            source: 1..3,
            advance: 22.30224609375,
            run,
            glyphs: 1..3,
        };
        let retained: Vec<_> = cluster.retained_glyphs().unwrap().collect();
        assert_eq!(retained[0].offset_y, 0.0);
        assert_eq!(retained[1].offset_y, -11.0302734375);
        let ink = rustybuzz::ttf_parser::Rect {
            x_min: 0,
            y_min: -20,
            x_max: 30,
            y_max: 40,
        };
        for (glyph, painted) in cluster.run.glyphs[1..].iter().zip(retained) {
            let bounds = geometry.glyph_ink_bounds(glyph, ink, [10.0, 100.0]);
            assert_eq!(bounds.y_min, 100.0 + painted.offset_y - 0.87890625);
            assert_eq!(bounds.y_max, 100.0 + painted.offset_y + 0.439453125);
            assert_eq!(
                geometry.vertical_ink_bounds(glyph, ink, 100.0, 0.0),
                [bounds.y_min, bounds.y_max]
            );
        }
    }
}
