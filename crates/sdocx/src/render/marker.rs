use super::RenderTheme;
use super::text::{
    StyledText, TextContext, TextFrame, TextLayout, TextRenderer, TextSettings, TextStyle,
    finite_native_geometry, layout_text, wrap_paragraph,
};
use super::vector::{Circle, Group, Paint, Rectangle, Scene, Styled, Transform, decimal};
use crate::{
    BoundingBox, BulletType, ParagraphBullet, RichTextBox, RichTextParagraph, RichTextParagraphType,
};

mod checkbox;
use checkbox::CheckboxMarker;

/// Native point-marker image sizing for the rendering display.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum PointMarkerTarget {
    #[default]
    Mobile,
    Tablet,
    Uwp,
}

impl PointMarkerTarget {
    fn image_range(self) -> (i32, f32, f32) {
        match self {
            Self::Mobile => (8, 1.5, 5.0),
            Self::Tablet => (7, 1.35, 2.25),
            Self::Uwp => (7, 4.5, 9.0),
        }
    }
}

pub(super) enum PreparedMarker {
    Point {
        artwork: PointMarker,
        metrics: PointMarkerMetrics,
    },
    Number(Box<PreparedNumber>),
    Checkbox(CheckboxMarker),
}

impl PreparedMarker {
    pub fn prepare(
        mut bullet: ParagraphBullet,
        indent: u32,
        style: &TextStyle,
        theme: RenderTheme,
        renderer: &TextRenderer<'_>,
    ) -> Option<Self> {
        if bullet.kind == BulletType::SolidCircle && indent % 2 == 1 {
            bullet.kind = BulletType::WhiteCircle;
        }
        if let Some(artwork) = PointMarker::from_bullet(bullet.kind) {
            let metrics = PointMarkerMetrics::measure(
                style.font_size,
                renderer.settings,
                renderer.point_marker_target,
            );
            if metrics.is_none() {
                renderer.invalid_geometry("sans-serif");
            }
            return metrics.map(|metrics| Self::Point { artwork, metrics });
        }
        if bullet.kind == BulletType::Checker {
            let checkbox = CheckboxMarker::measure(
                style.font_size,
                renderer.settings,
                renderer.point_marker_target,
                bullet.checked,
            );
            if checkbox.is_none() {
                renderer.invalid_geometry("sans-serif");
            }
            return checkbox.map(Self::Checkbox);
        }
        if matches!(bullet.kind, BulletType::None | BulletType::Other(_)) {
            return None;
        }
        let number = bullet
            .number
            .wrapping_add(bullet.initial_number)
            .wrapping_sub(1) as i32;
        let Some(value) = numbered_value(bullet.kind, number) else {
            renderer.measurement_failed("sans-serif");
            return None;
        };
        PreparedNumber::prepare(value, number, style, theme, renderer)
            .map(|number| Self::Number(Box::new(number)))
    }

    pub fn reserved_width(&self) -> f64 {
        match self {
            Self::Point { metrics, .. } => metrics.reserved_width,
            Self::Number(number) => number.reserved_width,
            Self::Checkbox(checkbox) => checkbox.reserved_width,
        }
    }

    pub fn bounds(&self, x: f64, center_y: f64) -> Option<BoundingBox> {
        finite_native_geometry(x)?;
        finite_native_geometry(center_y)?;
        match self {
            Self::Point { metrics, .. } => {
                let center_x = x + metrics.button_width / 2.0;
                marker_bounds(
                    center_x - metrics.radius,
                    center_y - metrics.radius,
                    center_x + metrics.radius,
                    center_y + metrics.radius,
                )
            }
            Self::Number(number) => number.bounds(x, center_y),
            Self::Checkbox(checkbox) => checkbox.bounds(x, center_y),
        }
    }

    pub fn paint(
        &self,
        svg: &mut Scene,
        x: f64,
        center_y: f64,
        color: &str,
        theme: RenderTheme,
        renderer: &TextRenderer<'_>,
    ) -> Option<()> {
        finite_native_geometry(x)?;
        finite_native_geometry(center_y)?;
        match self {
            Self::Point { artwork, metrics } => artwork.paint(
                svg,
                *metrics,
                x + metrics.button_width / 2.0 - metrics.radius,
                center_y - metrics.radius,
                color,
            ),
            Self::Number(number) => number.paint(svg, x, center_y, theme, renderer),
            Self::Checkbox(checkbox) => checkbox.paint(svg, x, center_y, color),
        }
    }
}

pub(super) struct PreparedNumber {
    source: RichTextBox,
    layout: TextLayout,
    reserved_width: f64,
}

impl PreparedNumber {
    fn prepare(
        value: String,
        number: i32,
        style: &TextStyle,
        theme: RenderTheme,
        renderer: &TextRenderer<'_>,
    ) -> Option<Self> {
        let child = renderer.for_resolved_text("sans-serif");
        let measured = (|| {
            let font_size = finite_native_geometry(style.font_size)?;
            if font_size <= 0.0
                || !renderer.settings.scale.is_finite()
                || renderer.settings.scale <= 0.0
            {
                return None;
            }
            let mut spacing = 1_u32.to_le_bytes().to_vec();
            spacing.extend_from_slice(&1.3_f32.to_le_bytes());
            let mut source = RichTextBox {
                text_area_type: None,
                bbox: BoundingBox::default(),
                rotation_degrees: None,
                text: value,
                color: Some(style.source_color),
                highlight_color: None,
                underline: false,
                font_size: Some(font_size as f32),
                runs: Vec::new(),
                spans: Vec::new(),
                paragraphs: vec![RichTextParagraph {
                    kind: RichTextParagraphType::LineSpacing,
                    start_paragraph: 0,
                    end_paragraph: 1,
                    payload: spacing,
                }],
                object_spans: Vec::new(),
                text_sections: Vec::new(),
                margins: None,
                gravity: None,
            };
            let width = {
                let styled = StyledText::new(&source, TextContext::Placed, child.settings);
                let lines = wrap_paragraph(
                    &styled,
                    0..styled.index.len(),
                    f64::from(f32::MAX),
                    theme,
                    None,
                    &child,
                )
                .ok()?;
                let [line] = lines.as_slice() else {
                    return None;
                };
                finite_native_geometry(line.advance.ceil())?
            };
            if width < 0.0 {
                return None;
            }
            source.bbox.x_max = width;
            let styled = StyledText::new(&source, TextContext::Placed, child.settings);
            let layout = layout_text(
                &styled,
                TextFrame {
                    bbox: source.bbox,
                    gravity: None,
                    exclusions: &[],
                },
                theme,
                &child,
            );
            let [line] = layout.lines.as_slice() else {
                return None;
            };
            if layout.height() <= 0.0
                || line.line.placements.is_empty()
                || ![layout.height(), line.baseline, line.line.advance]
                    .into_iter()
                    .all(|value| finite_native_geometry(value).is_some())
            {
                return None;
            }
            let gap = renderer.settings.scale * if number < 10 { 9.0 } else { 6.0 };
            let reserved_width = finite_native_geometry(line.line.advance + f64::from(gap))?;
            Some(Self {
                source,
                layout,
                reserved_width,
            })
        })();
        if measured.is_none() {
            renderer.measurement_failed("sans-serif");
        }
        measured
    }

    fn bounds(&self, x: f64, center_y: f64) -> Option<BoundingBox> {
        let top = center_y - self.layout.height() / 2.0;
        let mut bounds = marker_bounds(
            x,
            top,
            x + self.source.bbox.x_max,
            top + self.layout.height(),
        )?;
        for line in &self.layout.lines {
            for placement in &line.line.placements {
                let cluster = &placement.cluster;
                let run = &cluster.run;
                let scale = run.style.font_size / f64::from(run.face.metrics.units_per_em);
                for glyph in run.glyphs.get(cluster.glyphs.clone())? {
                    let Some(ink) = run.face.glyph_ink_bounds(glyph.raw.id).ok()? else {
                        continue;
                    };
                    let glyph_x = x + line.x + placement.x - cluster.origin_x
                        + (glyph.pen_x + i64::from(glyph.raw.x_offset)) as f64 * scale;
                    let baseline = top + line.baseline
                        - (glyph.pen_y + i64::from(glyph.raw.y_offset)) as f64 * scale;
                    let ink = marker_bounds(
                        glyph_x + f64::from(ink.x_min) * scale,
                        baseline - f64::from(ink.y_max) * scale,
                        glyph_x + f64::from(ink.x_max) * scale,
                        baseline - f64::from(ink.y_min) * scale,
                    )?;
                    bounds.x_min = bounds.x_min.min(ink.x_min);
                    bounds.y_min = bounds.y_min.min(ink.y_min);
                    bounds.x_max = bounds.x_max.max(ink.x_max);
                    bounds.y_max = bounds.y_max.max(ink.y_max);
                }
            }
        }
        Some(bounds)
    }

    fn paint(
        &self,
        svg: &mut Scene,
        x: f64,
        center_y: f64,
        theme: RenderTheme,
        renderer: &TextRenderer<'_>,
    ) -> Option<()> {
        let top = finite_native_geometry(center_y - self.layout.height() / 2.0)?;
        finite_native_geometry(x + self.source.bbox.x_max)?;
        finite_native_geometry(top + self.layout.height())?;
        for line in &self.layout.lines {
            finite_native_geometry(x + line.x)?;
            finite_native_geometry(top + line.baseline)?;
        }
        let child = renderer.for_resolved_text("sans-serif");
        let styled = StyledText::new(&self.source, TextContext::Placed, child.settings);
        svg.scope(
            Group::new().transformed(Transform::translate(x, top, 5)),
            |svg| {
                super::paint_text_layout(svg, &styled, &self.layout, &[], theme, &child);
            },
        );
        Some(())
    }
}

fn marker_bounds(x_min: f64, y_min: f64, x_max: f64, y_max: f64) -> Option<BoundingBox> {
    if x_max <= x_min || y_max <= y_min {
        return None;
    }
    Some(BoundingBox {
        x_min: finite_native_geometry(x_min)?,
        y_min: finite_native_geometry(y_min)?,
        x_max: finite_native_geometry(x_max)?,
        y_max: finite_native_geometry(y_max)?,
    })
}

fn numbered_value(kind: BulletType, number: i32) -> Option<String> {
    if number <= 0 {
        return Some(".".into());
    }
    let number = number as u32;
    match kind {
        BulletType::Digit | BulletType::CircledDigit => Some(format!("{number}.")),
        BulletType::Alphabet | BulletType::UppercaseAlphabet => Some(alphabetic_value(
            number,
            kind == BulletType::UppercaseAlphabet,
        )),
        BulletType::RomanNumeral => roman_value(number),
        _ => None,
    }
}

fn alphabetic_value(mut number: u32, uppercase: bool) -> String {
    let base = if uppercase { b'A' } else { b'a' };
    let mut characters = Vec::new();
    while number != 0 {
        number -= 1;
        characters.push(base + (number % 26) as u8);
        number /= 26;
    }
    characters.reverse();
    characters.push(b'.');
    String::from_utf8(characters).expect("alphabetic markers contain ASCII")
}

fn roman_value(number: u32) -> Option<String> {
    const MAX_MARKER_BYTES: usize = 4096;
    const ROMAN: &[(u32, &str)] = &[
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut remaining = number;
    let mut bytes = 1;
    for &(value, numeral) in ROMAN {
        bytes += (remaining / value) as usize * numeral.len();
        if bytes > MAX_MARKER_BYTES {
            return None;
        }
        remaining %= value;
    }
    let mut result = String::with_capacity(bytes);
    remaining = number;
    for &(value, numeral) in ROMAN {
        for _ in 0..remaining / value {
            result.push_str(numeral);
        }
        remaining %= value;
    }
    result.push('.');
    Some(result)
}

pub(super) fn marker_center_y(
    baseline: f64,
    post_line_cursor: f64,
    line_base_height: f64,
    pixel_spacing: f64,
    default_cap_height_ratio: Option<f64>,
) -> Option<f64> {
    if ![baseline, post_line_cursor, line_base_height, pixel_spacing]
        .into_iter()
        .all(|value| finite_native_geometry(value).is_some())
        || line_base_height < 0.0
    {
        return None;
    }
    let center = if pixel_spacing == 0.0 {
        post_line_cursor - finite_native_geometry(1.35 * line_base_height)? * 0.5
    } else {
        let ratio = default_cap_height_ratio
            .filter(|ratio| finite_native_geometry(*ratio).is_some() && *ratio > 0.0)?;
        baseline - finite_native_geometry(ratio * line_base_height)? * 0.5
    };
    finite_native_geometry(center)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PointMarker {
    SolidCircle,
    OpenCircle,
    SolidSquare,
    OpenSquare,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PointMarkerMetrics {
    pub reserved_width: f64,
    pub button_width: f64,
    pub radius: f64,
}

impl PointMarkerMetrics {
    pub fn measure(
        font_size: f64,
        settings: TextSettings,
        target: PointMarkerTarget,
    ) -> Option<Self> {
        let font_size = font_size as f32;
        let scale = settings.scale;
        if !font_size.is_finite() || !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let (max_font, min_pixels, max_pixels) = target.image_range();
        let min_pixels = min_pixels * scale;
        let max_pixels = max_pixels * scale;
        let button_width = 20.0_f32 * scale;
        let reserved_width = button_width + 6.0_f32 * scale;
        if !reserved_width.is_finite() || !max_pixels.is_finite() {
            return None;
        }
        let font = ((font_size / scale) as i32).max(1);
        let step = (max_pixels - min_pixels) / (max_font - 1) as f32;
        let mut pixels = max_pixels;
        for threshold in (1..=max_font).rev() {
            if threshold == 1 {
                pixels = min_pixels;
            }
            if font >= threshold {
                break;
            }
            pixels -= step;
        }
        let image_size = pixels.round() as i32;
        let radius = ((image_size as f32) / 2.0).ceil();
        Some(Self {
            reserved_width: f64::from(reserved_width),
            button_width: f64::from(button_width),
            radius: f64::from(radius),
        })
    }
}

impl PointMarker {
    pub fn from_bullet(kind: BulletType) -> Option<Self> {
        match kind {
            BulletType::Arrow | BulletType::Diamond | BulletType::SolidCircle => {
                Some(Self::SolidCircle)
            }
            BulletType::WhiteCircle => Some(Self::OpenCircle),
            BulletType::BlackSquare => Some(Self::SolidSquare),
            BulletType::WhiteSquare => Some(Self::OpenSquare),
            _ => None,
        }
    }

    pub fn paint(
        self,
        scene: &mut Scene,
        metrics: PointMarkerMetrics,
        left: f64,
        top: f64,
        color: &str,
    ) -> Option<()> {
        let radius = metrics.radius;
        let diameter = 2.0 * radius;
        let right = left + diameter;
        let bottom = top + diameter;
        if ![left, top, radius, diameter, right, bottom]
            .into_iter()
            .all(|value| finite_native_geometry(value).is_some())
            || radius <= 0.0
            || right <= left
            || bottom <= top
        {
            return None;
        }
        let paint = Paint::from_hex(color)?;
        let cx = left + radius;
        let cy = top + radius;
        match self {
            Self::SolidCircle => scene.push(
                Circle::new()
                    .cx(decimal(cx, 5))
                    .cy(decimal(cy, 5))
                    .r(decimal(radius, 5))
                    .fill(paint),
            ),
            Self::OpenCircle => scene.push(
                Circle::new()
                    .cx(decimal(cx, 5))
                    .cy(decimal(cy, 5))
                    .r(decimal(radius * 0.875, 5))
                    .fill(Paint::None)
                    .stroke(paint)
                    .stroke_width(decimal(diameter / 8.0, 5)),
            ),
            Self::SolidSquare => scene.push(
                Rectangle::new()
                    .x(decimal(left, 5))
                    .y(decimal(top, 5))
                    .width(decimal(diameter, 5))
                    .height(decimal(diameter, 5))
                    .fill(paint),
            ),
            Self::OpenSquare => scene.push(
                Rectangle::new()
                    .x(decimal(left + diameter / 16.0, 5))
                    .y(decimal(top + diameter / 16.0, 5))
                    .width(decimal(diameter * 0.875, 5))
                    .height(decimal(diameter * 0.875, 5))
                    .fill(Paint::None)
                    .stroke(paint)
                    .stroke_width(decimal(diameter / 8.0, 5)),
            ),
        }
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::vector::Svg;
    use super::*;

    fn numbered_bullet(kind: BulletType, number: u32, initial_number: u32) -> ParagraphBullet {
        ParagraphBullet {
            kind,
            number,
            checked: false,
            initial_number,
        }
    }

    fn body_style() -> TextStyle {
        TextStyle {
            font_size: 20.0,
            family: Some("Roboto Mono".into()),
            color: "#262626".into(),
            source_color: crate::Color {
                r: 38,
                g: 38,
                b: 38,
            },
            bold: true,
            italic: true,
            underline: true,
            strikethrough: true,
            link_target: Some("https://example.com".into()),
        }
    }

    #[test]
    fn point_bounds_follow_outer_artwork_and_exclude_the_reserved_gap() {
        let metrics = metrics(45.0, 3.0, PointMarkerTarget::Uwp);
        for artwork in [
            PointMarker::SolidCircle,
            PointMarker::OpenCircle,
            PointMarker::SolidSquare,
            PointMarker::OpenSquare,
        ] {
            let marker = PreparedMarker::Point { artwork, metrics };
            let bounds = marker.bounds(48.0, 401.625).unwrap();
            assert_eq!(bounds.x_min, 64.0);
            assert_eq!(bounds.x_max, 92.0);
            assert_eq!(bounds.y_min, 387.625);
            assert_eq!(bounds.y_max, 415.625);
            assert!(bounds.x_max < 48.0 + marker.reserved_width());
        }
    }

    #[test]
    fn checkbox_bounds_cover_assets_larger_than_their_reservation() {
        for checked in [false, true] {
            let marker = PreparedMarker::Checkbox(
                CheckboxMarker::measure(
                    1000.0,
                    TextSettings {
                        scale: 3.0,
                        ..Default::default()
                    },
                    PointMarkerTarget::Mobile,
                    checked,
                )
                .unwrap(),
            );
            let bounds = marker.bounds(48.0, 401.625).unwrap();
            assert_eq!(bounds.x_min, 30.0);
            assert_eq!(bounds.x_max, 126.0);
            assert_eq!(bounds.y_min, 352.625);
            assert_eq!(bounds.y_max, 448.625);
            assert_eq!(bounds.x_max - bounds.x_min, 96.0);
            assert!(bounds.x_min < 48.0);
            assert!(marker.bounds(f64::NAN, 401.625).is_none());
        }
    }

    #[test]
    fn numeric_bounds_retain_negative_glyph_bearings_and_layout_height() {
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(TextSettings::default(), &fonts);
        let marker = PreparedMarker::prepare(
            numbered_bullet(BulletType::Alphabet, 10, 1),
            0,
            &body_style(),
            RenderTheme::for_canvas(false),
            &renderer,
        )
        .unwrap();
        let bounds = marker.bounds(70.0, 100.0).unwrap();
        let PreparedMarker::Number(number) = &marker else {
            panic!("numeric marker");
        };
        assert_eq!(number.source.text, "j.");
        assert!(bounds.x_min < 70.0, "the j overhang must remain visible");
        assert!(bounds.y_min <= 100.0 - number.layout.height() / 2.0);
        assert!(bounds.y_max >= 100.0 + number.layout.height() / 2.0);
        assert!(bounds.x_max < 70.0 + marker.reserved_width());
        assert!(marker.bounds(f64::INFINITY, 100.0).is_none());
    }

    #[test]
    fn numbered_markers_use_native_decimal_spreadsheet_and_greedy_roman_conversions() {
        for (kind, number, expected) in [
            (BulletType::Digit, 12, "12."),
            (BulletType::CircledDigit, 12, "12."),
            (BulletType::Alphabet, 26, "z."),
            (BulletType::Alphabet, 27, "aa."),
            (BulletType::Alphabet, 52, "az."),
            (BulletType::Alphabet, 53, "ba."),
            (BulletType::UppercaseAlphabet, 702, "ZZ."),
            (BulletType::UppercaseAlphabet, 703, "AAA."),
            (BulletType::RomanNumeral, 4, "iv."),
            (BulletType::RomanNumeral, 1999, "mcmxcix."),
            (BulletType::RomanNumeral, 4000, "mmmm."),
        ] {
            assert_eq!(numbered_value(kind, number).as_deref(), Some(expected));
        }
        for kind in [
            BulletType::Digit,
            BulletType::CircledDigit,
            BulletType::Alphabet,
            BulletType::UppercaseAlphabet,
            BulletType::RomanNumeral,
        ] {
            for number in [0, -1, i32::MIN] {
                assert_eq!(numbered_value(kind, number).as_deref(), Some("."));
            }
        }
    }

    #[test]
    fn numeric_child_uses_resolved_size_actual_default_face_and_native_gaps() {
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(
            TextSettings {
                scale: 3.0,
                font_size_delta: 7.0,
                ..Default::default()
            },
            &fonts,
        );
        let default_face = fonts.resolve("sans-serif", false, false).unwrap();
        for (number, gap) in [(9, 27.0), (10, 18.0)] {
            let prepared = PreparedMarker::prepare(
                numbered_bullet(BulletType::Digit, number, 1),
                0,
                &body_style(),
                RenderTheme::for_canvas(false),
                &renderer,
            )
            .unwrap();
            let PreparedMarker::Number(prepared) = prepared else {
                panic!("numeric marker");
            };
            assert_eq!(prepared.source.text, format!("{number}."));
            assert_eq!(prepared.source.font_size, Some(20.0));
            assert_eq!(prepared.source.color, Some(body_style().source_color));
            assert!(prepared.source.runs.is_empty() && prepared.source.spans.is_empty());
            let line = &prepared.layout.lines[0];
            assert_eq!(line.line.font_size, 20.0);
            assert!((prepared.layout.height() - 26.0).abs() < 0.00001);
            assert!((line.baseline - 19.0).abs() < 0.00001);
            let mut buffer = crate::fonts::UnicodeBuffer::new();
            buffer.push_str(&prepared.source.text);
            let shaped = default_face.shape(buffer, &[]).unwrap();
            let expected_advance =
                shaped.advance_x() as f64 * 20.0 / f64::from(shaped.metrics.units_per_em);
            assert_eq!(line.line.advance, expected_advance);
            assert_eq!(prepared.source.bbox.x_max, expected_advance.ceil());
            assert_eq!(prepared.reserved_width, expected_advance + gap);
            for placement in &line.line.placements {
                assert_eq!(placement.cluster.run.face.id, default_face.id);
                let style = &placement.cluster.run.style;
                assert!(!style.bold && !style.italic && !style.underline && !style.strikethrough);
                assert!(style.link_target.is_none());
            }
        }
        assert!(renderer.diagnostics().is_empty());
    }

    #[test]
    fn resolved_subpixel_number_size_is_not_clamped_or_scaled_again() {
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(
            TextSettings {
                scale: 0.5,
                font_size_delta: 7.0,
                ..Default::default()
            },
            &fonts,
        );
        let style = TextStyle {
            font_size: 0.5,
            ..body_style()
        };
        let theme = RenderTheme::for_canvas(false);
        let prepared = PreparedMarker::prepare(
            numbered_bullet(BulletType::Digit, 1, 1),
            0,
            &style,
            theme,
            &renderer,
        )
        .unwrap();
        let PreparedMarker::Number(number) = &prepared else {
            panic!("numeric marker");
        };
        assert_eq!(number.source.font_size, Some(0.5));
        assert_eq!(number.layout.lines[0].line.font_size, 0.5);
        assert!((number.layout.height() - 0.65).abs() < 0.0000001);
        assert!((number.layout.lines[0].baseline - 0.475).abs() < 0.0000001);
        assert_eq!(
            number.reserved_width,
            number.layout.lines[0].line.advance + 4.5
        );
        let mut scene = Scene::new(Svg::new());
        prepared
            .paint(&mut scene, 10.0, 10.0, "#262626", theme, &renderer)
            .unwrap();
        let output = scene.finish();
        let document = roxmltree::Document::parse(&output).unwrap();
        let sizes = document
            .descendants()
            .filter_map(|node| node.attribute("font-size"))
            .map(|size| size.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        assert!(!sizes.is_empty());
        assert!(sizes.iter().all(|size| *size == 0.5));
        assert!(renderer.diagnostics().is_empty());
    }

    #[test]
    fn initial_number_and_signed_wrapping_are_preserved_without_clamping() {
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(
            TextSettings {
                scale: 3.0,
                font_size_delta: 0.0,
                ..Default::default()
            },
            &fonts,
        );
        for (number, initial, expected) in [
            (2, 4, "5."),
            (1, 0, "."),
            (u32::MAX, 1, "."),
            (i32::MAX as u32, 2, "."),
        ] {
            let prepared = PreparedMarker::prepare(
                numbered_bullet(BulletType::Digit, number, initial),
                0,
                &body_style(),
                RenderTheme::for_canvas(false),
                &renderer,
            )
            .unwrap();
            let PreparedMarker::Number(prepared) = prepared else {
                panic!("numeric marker");
            };
            assert_eq!(prepared.source.text, expected);
        }
    }

    #[test]
    fn numeric_paint_reuses_measured_layout_and_shared_font_registry() {
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(
            TextSettings {
                scale: 3.0,
                font_size_delta: 5.0,
                ..Default::default()
            },
            &fonts,
        );
        let theme = RenderTheme::for_canvas(false);
        let prepared = PreparedMarker::prepare(
            numbered_bullet(BulletType::Alphabet, 27, 1),
            0,
            &body_style(),
            theme,
            &renderer,
        )
        .unwrap();
        let mut scene = Scene::new(Svg::new());
        prepared
            .paint(&mut scene, 70.0, 100.0, "#262626", theme, &renderer)
            .unwrap();
        renderer.embed_fonts(&mut scene);
        let output = scene.finish();
        let document = roxmltree::Document::parse(&output).unwrap();
        let group = document
            .descendants()
            .find(|node| node.has_tag_name("g"))
            .unwrap();
        assert_eq!(group.attribute("transform"), Some("translate(70 87.00000)"));
        let text = document
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .filter_map(|node| node.text())
            .collect::<String>();
        assert_eq!(text, "aa.");
        assert!(output.contains("@font-face"));
        assert!(!output.contains("Roboto Mono") && !output.contains("text-decoration"));
        assert!(renderer.diagnostics().is_empty());
    }

    #[test]
    fn unsupported_roman_expansion_and_missing_fonts_report_bounded_failures() {
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(
            TextSettings {
                scale: 3.0,
                font_size_delta: 0.0,
                ..Default::default()
            },
            &fonts,
        );
        assert!(
            PreparedMarker::prepare(
                numbered_bullet(BulletType::RomanNumeral, i32::MAX as u32, 1),
                0,
                &body_style(),
                RenderTheme::for_canvas(false),
                &renderer,
            )
            .is_none()
        );
        assert_eq!(renderer.diagnostics().len(), 1);
        assert_eq!(
            renderer.diagnostics()[0].kind,
            super::super::TextDiagnosticKind::MeasurementFailure
        );
        let fonts =
            crate::fonts::FontBook::new(std::sync::Arc::new(crate::fonts::fontdb::Database::new()));
        let renderer = TextRenderer::new(
            TextSettings {
                scale: 3.0,
                font_size_delta: 0.0,
                ..Default::default()
            },
            &fonts,
        );
        assert!(
            PreparedMarker::prepare(
                numbered_bullet(BulletType::Digit, 1, 1),
                0,
                &body_style(),
                RenderTheme::for_canvas(false),
                &renderer,
            )
            .is_none()
        );
        assert!(
            renderer
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.kind
                    == super::super::TextDiagnosticKind::MeasurementFailure)
        );
    }

    fn metrics(font_size: f64, scale: f32, target: PointMarkerTarget) -> PointMarkerMetrics {
        PointMarkerMetrics::measure(
            font_size,
            TextSettings {
                scale,
                font_size_delta: 0.0,
                ..Default::default()
            },
            target,
        )
        .unwrap()
    }

    #[test]
    fn zero_pixel_spacing_centers_on_native_post_line_cursor_and_base_height() {
        assert_eq!(
            marker_center_y(416.25, 432.0, 45.0, 0.0, None),
            Some(401.625)
        );
        assert_eq!(
            marker_center_y(484.25, 500.0, 100.0, 0.0, None),
            Some(432.5)
        );
        assert_eq!(marker_center_y(500.0, 500.0, 100.0, 0.0, None), Some(432.5));
    }

    #[test]
    fn pixel_spacing_centers_on_baseline_and_actual_default_face_cap_height() {
        let ratio = crate::fonts::FontBook::default()
            .resolve("sans-serif", false, false)
            .unwrap()
            .metrics
            .cap_height_ratio();
        let expected = Some(384.00390625);
        assert_eq!(marker_center_y(400.0, 420.0, 45.0, 6.0, ratio), expected);
        assert_eq!(marker_center_y(400.0, 500.0, 45.0, 90.0, ratio), expected);
        assert_eq!(marker_center_y(400.0, 420.0, 45.0, -6.0, ratio), expected);
        assert_eq!(
            marker_center_y(400.0, 420.0, 100.0, 6.0, ratio),
            Some(364.453125)
        );
        for ratio in [
            None,
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
        ] {
            assert_eq!(marker_center_y(400.0, 420.0, 45.0, 6.0, ratio), None);
        }
    }

    #[test]
    fn marker_centers_reject_nonfinite_geometry_and_arithmetic_overflow() {
        let native_max = f64::from(f32::MAX);
        for (baseline, cursor, height, spacing) in [
            (f64::NAN, 420.0, 45.0, 0.0),
            (400.0, f64::INFINITY, 45.0, 0.0),
            (400.0, 420.0, f64::INFINITY, 0.0),
            (400.0, 420.0, -1.0, 0.0),
            (400.0, 420.0, 45.0, f64::NAN),
            (400.0, -f64::MAX, f64::MAX, 0.0),
            (-f64::MAX, 420.0, f64::MAX, 1.0),
            (native_max * 2.0, 420.0, 45.0, 0.0),
            (400.0, native_max * 2.0, 45.0, 0.0),
            (400.0, 420.0, native_max * 2.0, 0.0),
            (400.0, 420.0, 45.0, native_max * 2.0),
            (400.0, -native_max, native_max / 2.0, 0.0),
            (-native_max, 420.0, native_max / 2.0, 1.0),
            (400.0, 420.0, native_max, 0.0),
        ] {
            assert_eq!(
                marker_center_y(baseline, cursor, height, spacing, Some(1.0)),
                None
            );
        }
    }

    #[test]
    fn target_profiles_select_native_minimum_and_maximum_images() {
        for (target, min_radius, max_radius) in [
            (PointMarkerTarget::Mobile, 3.0, 8.0),
            (PointMarkerTarget::Tablet, 2.0, 4.0),
            (PointMarkerTarget::Uwp, 7.0, 14.0),
        ] {
            assert_eq!(metrics(0.0, 3.0, target).radius, min_radius);
            assert_eq!(metrics(-45.0, 3.0, target).radius, min_radius);
            assert_eq!(metrics(3.0, 3.0, target).radius, min_radius);
            assert_eq!(metrics(45.0, 3.0, target).radius, max_radius);
            assert_eq!(metrics(450.0, 3.0, target).radius, max_radius);
        }
    }

    #[test]
    fn interpolation_truncates_font_thresholds_and_rounds_image_sizes_away() {
        let mobile = PointMarkerTarget::Mobile;
        for (font, radius) in [
            (1.0, 3.0),
            (2.0, 3.0),
            (3.0, 4.0),
            (4.0, 5.0),
            (5.0, 6.0),
            (6.0, 6.0),
            (7.0, 7.0),
            (8.0, 8.0),
        ] {
            assert_eq!(metrics(font * 3.0, 3.0, mobile).radius, radius);
        }
        assert_eq!(metrics(11.999, 3.0, mobile).radius, 4.0);
        assert_eq!(metrics(12.0, 3.0, mobile).radius, 5.0);
    }

    #[test]
    fn density_changes_reserved_width_independently_of_font_and_rounded_artwork() {
        for (density, radius) in [(1.0, 3.0), (2.0, 5.0), (3.0, 8.0)] {
            let measured = metrics(
                15.0 * f64::from(density),
                density,
                PointMarkerTarget::Mobile,
            );
            assert_eq!(measured.reserved_width, 26.0 * f64::from(density));
            assert_eq!(measured.button_width, 20.0 * f64::from(density));
            assert_eq!(measured.radius, radius);
        }
    }

    #[test]
    fn all_native_point_resources_paint_typed_shapes_with_transparent_open_centers() {
        let measured = metrics(45.0, 3.0, PointMarkerTarget::Mobile);
        let mut scene = Scene::new(Svg::new());
        for marker in [
            PointMarker::SolidCircle,
            PointMarker::OpenCircle,
            PointMarker::SolidSquare,
            PointMarker::OpenSquare,
        ] {
            marker
                .paint(&mut scene, measured, 70.0, 393.625, "#262626")
                .unwrap();
        }
        let output = scene.finish();
        let xml = roxmltree::Document::parse(&output).unwrap();
        let shapes = xml
            .root_element()
            .children()
            .filter(roxmltree::Node::is_element)
            .collect::<Vec<_>>();
        assert_eq!(shapes.len(), 4);
        assert_eq!(shapes[0].tag_name().name(), "circle");
        assert_eq!(shapes[0].attribute("cx"), Some("78.00000"));
        assert_eq!(shapes[0].attribute("cy"), Some("401.62500"));
        assert_eq!(shapes[0].attribute("r"), Some("8.00000"));
        assert_eq!(shapes[0].attribute("fill"), Some("#262626"));
        assert_eq!(shapes[1].attribute("r"), Some("7.00000"));
        assert_eq!(shapes[1].attribute("fill"), Some("none"));
        assert_eq!(shapes[1].attribute("stroke"), Some("#262626"));
        assert_eq!(shapes[1].attribute("stroke-width"), Some("2.00000"));
        assert_eq!(shapes[2].tag_name().name(), "rect");
        assert_eq!(shapes[2].attribute("width"), Some("16.00000"));
        assert_eq!(shapes[3].attribute("x"), Some("71.00000"));
        assert_eq!(shapes[3].attribute("y"), Some("394.62500"));
        assert_eq!(shapes[3].attribute("width"), Some("14.00000"));
        assert_eq!(shapes[3].attribute("fill"), Some("none"));
        assert_eq!(shapes[3].attribute("stroke-width"), Some("2.00000"));
        assert!(!output.contains("<text"));
    }

    #[test]
    fn white_open_markers_keep_transparent_centers_and_none_tint_has_no_ink() {
        let measured = metrics(45.0, 3.0, PointMarkerTarget::Mobile);
        for marker in [PointMarker::OpenCircle, PointMarker::OpenSquare] {
            for color in ["#ffffff", "none"] {
                let mut scene = Scene::new(Svg::new().width(20).height(20));
                marker.paint(&mut scene, measured, 2.0, 2.0, color).unwrap();
                let tree =
                    resvg::usvg::Tree::from_str(&scene.finish(), &resvg::usvg::Options::default())
                        .unwrap();
                let mut image = resvg::tiny_skia::Pixmap::new(20, 20).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut image.as_mut(),
                );
                assert_eq!(image.pixel(10, 10).unwrap().alpha(), 0);
                if color == "none" {
                    assert!(image.pixels().iter().all(|pixel| pixel.alpha() == 0));
                } else {
                    let pixel = image.pixel(2, 10).unwrap();
                    assert!(pixel.alpha() > 0);
                    assert_eq!(pixel.red(), pixel.alpha());
                    assert_eq!(pixel.green(), pixel.alpha());
                    assert_eq!(pixel.blue(), pixel.alpha());
                }
            }
        }
    }

    #[test]
    fn invalid_metrics_or_paint_geometry_produce_no_svg_nodes() {
        for (font, scale) in [
            (f64::INFINITY, 3.0),
            (f64::NAN, 3.0),
            (45.0, f32::INFINITY),
            (45.0, 0.0),
            (45.0, -1.0),
            (45.0, f32::MAX),
        ] {
            assert!(
                PointMarkerMetrics::measure(
                    font,
                    TextSettings {
                        scale,
                        font_size_delta: 0.0,
                        ..Default::default()
                    },
                    PointMarkerTarget::Mobile
                )
                .is_none()
            );
        }
        let measured = metrics(45.0, 3.0, PointMarkerTarget::Mobile);
        let mut scene = Scene::new(Svg::new());
        for (left, top, color) in [
            (f64::INFINITY, 0.0, "#262626"),
            (0.0, f64::NAN, "#262626"),
            (f64::MAX, f64::MAX, "#262626"),
            (f64::from(f32::MAX) * 2.0, 0.0, "#262626"),
            (0.0, f64::from(f32::MAX) * 2.0, "#262626"),
            (0.0, 0.0, "invalid"),
        ] {
            assert!(
                PointMarker::SolidCircle
                    .paint(&mut scene, measured, left, top, color)
                    .is_none()
            );
        }
        let output = scene.finish();
        let xml = roxmltree::Document::parse(&output).unwrap();
        assert!(!xml.root_element().children().any(|node| node.is_element()));
    }

    #[test]
    fn saved_bullet_types_select_native_resource_shapes() {
        assert_eq!(
            PointMarker::from_bullet(BulletType::Arrow),
            Some(PointMarker::SolidCircle)
        );
        assert_eq!(
            PointMarker::from_bullet(BulletType::Diamond),
            Some(PointMarker::SolidCircle)
        );
        assert_eq!(
            PointMarker::from_bullet(BulletType::WhiteCircle),
            Some(PointMarker::OpenCircle)
        );
        assert!(PointMarker::from_bullet(BulletType::Digit).is_none());
        assert!(PointMarker::from_bullet(BulletType::Checker).is_none());
    }

    #[test]
    #[cfg(feature = "serde")]
    fn target_configuration_uses_lowercase_names() {
        assert_eq!(
            serde_json::to_string(&PointMarkerTarget::Mobile).unwrap(),
            "\"mobile\""
        );
        assert_eq!(
            serde_json::from_str::<PointMarkerTarget>("\"tablet\"").unwrap(),
            PointMarkerTarget::Tablet
        );
        assert_eq!(
            serde_json::from_str::<PointMarkerTarget>("\"uwp\"").unwrap(),
            PointMarkerTarget::Uwp
        );
    }
}
