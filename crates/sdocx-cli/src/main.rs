use base64::Engine as _;
use clap::{Parser, ValueEnum};
use sdocx::{
    BulletType, Color, Document, HyperlinkType, LayoutDocument, LineSpacingType, MediaAsset, Page,
    PageElement, PageTemplate, PageTemplateSource, ParagraphAlignment, ParagraphBullet,
    ParagraphLineSpacing, PredefinedTextStyle, RichTextBox, RichTextObjectContent,
    RichTextObjectSpan, RichTextParagraphType, RichTextRun, RichTextSpanType, Stroke,
};
use std::fmt::Write as FmtWrite;
use std::fs;
use std::ops::Range;
use std::path::PathBuf;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Format {
    Svg,
    Png,
}

/// Resolve the output format: explicit flag wins, else infer from the output
/// file extension, else default to SVG.
fn resolve_format(
    flag: Option<Format>,
    output: Option<&std::path::Path>,
) -> Result<Format, String> {
    if let Some(f) = flag {
        return Ok(f);
    }
    match output
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("svg") => Ok(Format::Svg),
        Some("png") => Ok(Format::Png),
        Some(other) => Err(format!(
            "unknown output extension '.{other}'; use -f/--format to set svg or png"
        )),
        None => Ok(Format::Svg),
    }
}

impl Format {
    fn ext(self) -> &'static str {
        match self {
            Format::Svg => "svg",
            Format::Png => "png",
        }
    }
}

fn svg_to_png(svg: &str) -> Result<Vec<u8>, String> {
    let mut opt = resvg::usvg::Options::default();
    // Load system fonts so <text> elements render instead of being silently dropped.
    let fontdb = opt.fontdb_mut();
    fontdb.load_system_fonts();
    // Arial is not normally installed in Linux CI images. DejaVu Sans is the
    // portable fallback there; explicit Arial still wins on platforms that
    // provide it.
    fontdb.set_sans_serif_family("DejaVu Sans");
    fontdb.set_monospace_family("DejaVu Sans Mono");
    let tree = resvg::usvg::Tree::from_str(svg, &opt).map_err(|e| format!("invalid SVG: {e}"))?;
    let size = tree.size().to_int_size();
    let (w, h) = (size.width(), size.height());
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| "failed to allocate pixmap".to_string())?;
    let mut pm = pixmap.as_mut();
    resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pm);
    pixmap
        .encode_png()
        .map_err(|e| format!("PNG encode failed: {e}"))
}

// Default ink for uncolored strokes, by canvas: light on dark, dark on light.
const DEFAULT_INK_DARK_MODE: &str = "#ffffff";
const DEFAULT_INK_LIGHT_MODE: &str = "#1a1a1a";
// Fallback canvas when a note carries no background color, matched to the ink.
const FALLBACK_BG_DARK_MODE: &str = "#252525";
const FALLBACK_BG_LIGHT_MODE: &str = "#fcfcfc";
// Pressure channel on v4.4.x files can be present but all-zero; treat as absent.
const PRESSURE_PRESENT_EPSILON: f64 = 0.01;

#[derive(Parser)]
#[command(name = "sdocx", version, about = "Parse Samsung Notes .sdocx files")]
struct Cli {
    /// Path to an .sdocx file
    path: PathBuf,

    /// Output file path (format inferred from extension; defaults to the input path with a format-appropriate extension)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output format (overrides extension inference): svg or png
    #[arg(short, long, value_enum)]
    format: Option<Format>,
}

fn color_hex(c: &Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn render_page_svg(
    page: &Page,
    fallback_bg_color: Option<&Color>,
    media_assets: &[MediaAsset],
    flow_page_padding: Option<(u32, u32)>,
    dark_mode: bool,
) -> String {
    // Dark-mode notes have light ink, so prefer the document's dark background
    // over the light page template; otherwise keep the template background.
    let bg_color = if dark_mode {
        fallback_bg_color.or(page.background_color.as_ref())
    } else {
        page.background_color.as_ref().or(fallback_bg_color)
    };
    let bg = bg_color.map(color_hex).unwrap_or_else(|| {
        if dark_mode {
            FALLBACK_BG_DARK_MODE
        } else {
            FALLBACK_BG_LIGHT_MODE
        }
        .into()
    });
    let vb_x = 0.0;
    let vb_y = 0.0;
    let vb_w = page.width as f64;
    let vb_h = page.height as f64;
    let svg_w = page.width;
    let svg_h = page.height;

    let mut svg = String::with_capacity(page.strokes.len() * 256);

    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb_x:.1} {vb_y:.1} {vb_w:.1} {vb_h:.1}" width="{svg_w}" height="{svg_h}">"#,
    )
    .unwrap();

    writeln!(
        svg,
        r#"  <rect x="{vb_x}" y="{vb_y}" width="{vb_w}" height="{vb_h}" fill="{bg}"/>"#,
    )
    .unwrap();

    let default_ink = if dark_mode {
        DEFAULT_INK_DARK_MODE
    } else {
        DEFAULT_INK_LIGHT_MODE
    };
    for stroke in &page.strokes {
        render_stroke(&mut svg, stroke, default_ink);
    }
    for element in &page.elements {
        render_element(
            &mut svg,
            element,
            page,
            media_assets,
            flow_page_padding,
            dark_mode,
        );
    }

    svg.push_str("</svg>\n");
    svg
}

fn render_element(
    svg: &mut String,
    element: &PageElement,
    page: &Page,
    media_assets: &[MediaAsset],
    flow_page_padding: Option<(u32, u32)>,
    dark_mode: bool,
) {
    match element {
        PageElement::Image { bbox, media_index } => {
            let Some(asset) = media_assets.get(*media_index) else {
                return;
            };
            let encoded = base64::engine::general_purpose::STANDARD.encode(&asset.data);
            writeln!(
                svg,
                r#"  <image x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" href="data:{};base64,{}" preserveAspectRatio="none"/>"#,
                bbox.x_min,
                bbox.y_min,
                bbox.x_max - bbox.x_min,
                bbox.y_max - bbox.y_min,
                asset.mime_type,
                encoded,
            )
            .unwrap();
        }
        PageElement::TextBox(text_box) => {
            render_text_box(svg, text_box, page, flow_page_padding, dark_mode)
        }
        _ => {}
    }
}

fn render_text_box(
    svg: &mut String,
    text_box: &RichTextBox,
    page: &Page,
    flow_page_padding: Option<(u32, u32)>,
    dark_mode: bool,
) {
    let text = text_box.text.trim_end_matches('\n');
    if text.trim().is_empty() {
        return;
    }

    let is_note_body =
        text_box.bbox.x_max <= text_box.bbox.x_min || text_box.bbox.y_max <= text_box.bbox.y_min;
    if is_note_body {
        render_flow_text_box(svg, text_box, page, flow_page_padding, dark_mode);
        return;
    }
    let (x, y, width, height) = (
        text_box.bbox.x_min,
        text_box.bbox.y_min,
        text_box.bbox.x_max - text_box.bbox.x_min,
        text_box.bbox.y_max - text_box.bbox.y_min,
    );
    let color = text_box
        .color
        .filter(|color| !dark_mode || !is_dark_compatibility_color(*color))
        .as_ref()
        .map(color_hex)
        .unwrap_or_else(|| {
            if dark_mode {
                DEFAULT_INK_DARK_MODE
            } else {
                DEFAULT_INK_LIGHT_MODE
            }
            .into()
        });
    let font_size = text_box.font_size.map(samsung_font_to_svg).unwrap_or(37.0);
    let line_height = font_size * 1.35;
    let mut transform = String::new();
    if let Some(rotation) = text_box.rotation_degrees {
        let cx = x + width / 2.0;
        let cy = y + height / 2.0;
        transform = format!(r#" transform="rotate({rotation:.2} {cx:.2} {cy:.2})""#);
    }

    writeln!(svg, r#"  <g{transform}>"#).unwrap();
    if let Some(highlight) = text_box.highlight_color.as_ref() {
        writeln!(
            svg,
            r#"    <rect x="{x:.2}" y="{y:.2}" width="{width:.2}" height="{height:.2}" fill="{}"/>"#,
            color_hex(highlight),
        )
        .unwrap();
    }
    for (line_idx, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let text_y = y + font_size + line_idx as f64 * line_height;
        let decoration = if text_box.underline {
            r#" text-decoration="underline""#
        } else {
            ""
        };
        let line_start = text
            .lines()
            .take(line_idx)
            .map(|line| line.chars().count() + 1)
            .sum::<usize>();
        let spans = styled_line_spans(line, line_start, &text_box.runs);
        write!(
            svg,
            r#"    <text x="{x:.2}" y="{text_y:.2}" fill="{color}" font-family="Arial, sans-serif" font-size="{font_size:.2}"{decoration}>"#,
        )
        .unwrap();
        for span in spans {
            write!(
                svg,
                r#"<tspan{}{}>{}</tspan>"#,
                if span.bold {
                    r#" font-weight="bold""#
                } else {
                    ""
                },
                if span.italic {
                    r#" font-style="italic""#
                } else {
                    ""
                },
                escape_xml(span.text),
            )
            .unwrap();
        }
        svg.push_str("</text>\n");
    }
    svg.push_str("  </g>\n");
}

const SAMSUNG_TEXT_SCALE: f64 = 3.0;
const FLOW_HORIZONTAL_PADDING: f64 = 48.0;
const FLOW_INDENT: f64 = 48.0;
const SAMSUNG_LINK_COLOR: &str = "#0054ff";

#[derive(Clone)]
struct SvgTextStyle {
    font_size: f64,
    color: String,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    link_target: Option<String>,
}

#[derive(Default)]
struct ParagraphLayout {
    alignment: Option<ParagraphAlignment>,
    indent_level: u32,
    line_spacing: Option<ParagraphLineSpacing>,
    bullet: Option<ParagraphBullet>,
    spacing_before: f64,
    spacing_after: f64,
    predefined_style: Option<PredefinedTextStyle>,
}

fn render_flow_text_box(
    svg: &mut String,
    text_box: &RichTextBox,
    page: &Page,
    flow_page_padding: Option<(u32, u32)>,
    dark_mode: bool,
) {
    let (horizontal_padding, vertical_padding) = flow_page_padding
        .map(|(horizontal, vertical)| (f64::from(horizontal), f64::from(vertical)))
        .unwrap_or((FLOW_HORIZONTAL_PADDING, 0.0));
    let margins = text_box.margins.unwrap_or([0.0; 4]);
    let content_left = horizontal_padding + f64::from(margins[0]) * SAMSUNG_TEXT_SCALE;
    let content_top = vertical_padding + f64::from(margins[1]) * SAMSUNG_TEXT_SCALE;
    let content_right =
        f64::from(page.width) - horizontal_padding - f64::from(margins[2]) * SAMSUNG_TEXT_SCALE;
    let characters = text_box.text.chars().collect::<Vec<_>>();
    let utf16_offsets = char_utf16_offsets(&text_box.text);
    let byte_offsets = char_byte_offsets(&text_box.text);
    let mut paragraph_start = 0_usize;
    let mut cursor_y = content_top;

    let paragraphs = text_box.text.split_inclusive('\n').collect::<Vec<_>>();
    writeln!(svg, r#"  <g data-sdocx-flow="true">"#).unwrap();
    for (paragraph_index, paragraph) in paragraphs.iter().enumerate() {
        let content = paragraph.trim_end_matches(['\n', '\r']);
        let content_length = content.chars().count();
        let paragraph_end = paragraph_start + content_length;
        let layout = paragraph_layout(text_box, paragraph_index as u32);
        let previous_is_list_item = paragraph_index > 0
            && paragraph_layout(text_box, paragraph_index as u32 - 1)
                .bullet
                .and_then(bullet_marker)
                .is_some();
        let current_is_list_item = layout.bullet.and_then(bullet_marker).is_some();
        let next_is_list_item = paragraph_index + 1 < paragraphs.len()
            && paragraph_layout(text_box, paragraph_index as u32 + 1)
                .bullet
                .and_then(bullet_marker)
                .is_some();
        if paragraph_start != 0 && !(previous_is_list_item && current_is_list_item) {
            cursor_y += layout.spacing_before;
        }

        let paragraph_start_utf16 = utf16_offsets[paragraph_start];
        let paragraph_end_utf16 = utf16_offsets[paragraph_end];
        let embedded = text_box
            .object_spans
            .iter()
            .filter(|object| {
                u32::try_from(object.text_index_utf16).is_ok_and(|index| {
                    index >= paragraph_start_utf16 && index <= paragraph_end_utf16
                })
            })
            .collect::<Vec<_>>();
        if !embedded.is_empty() {
            for object in embedded {
                if let Some(bottom) = render_embedded_object(svg, object, cursor_y, dark_mode) {
                    cursor_y = cursor_y.max(bottom + object_bottom_margin(object));
                }
            }
            cursor_y += layout.spacing_after;
            paragraph_start += paragraph.chars().count();
            continue;
        }

        let base_style = text_style_at(
            text_box,
            paragraph_start_utf16,
            dark_mode,
            layout.predefined_style,
        );
        let marker = layout
            .bullet
            .and_then(|bullet| bullet_marker_for_indent(bullet, layout.indent_level));
        let marker_width = marker.as_ref().map_or(0.0, |(_, width, _, _)| *width);
        let base_x = content_left + f64::from(layout.indent_level) * FLOW_INDENT;
        let text_x = base_x + marker_width;
        let available_width = (content_right - text_x).max(base_style.font_size);
        let lines = if content.is_empty() {
            std::iter::once(paragraph_start..paragraph_start).collect::<Vec<_>>()
        } else {
            wrap_paragraph(
                text_box,
                &characters,
                &utf16_offsets,
                paragraph_start..paragraph_end,
                available_width,
                dark_mode,
                layout.predefined_style,
            )
        };
        let line_height = paragraph_line_height(base_style.font_size, layout.line_spacing);

        for (line_index, line_range) in lines.iter().enumerate() {
            let baseline = cursor_y + base_style.font_size;
            if line_index == 0
                && let Some((marker, _, marker_size, marker_offset)) = marker.as_ref()
            {
                writeln!(
                    svg,
                    r#"    <text x="{:.2}" y="{:.2}" fill="{}" font-family="Roboto, Arial, sans-serif" font-size="{:.2}">{}</text>"#,
                    base_x + marker_offset,
                    baseline - if *marker_size < 40.0 { 8.0 } else { 0.0 },
                    base_style.color,
                    marker_size,
                    escape_xml(marker),
                )
                .unwrap();
            }
            render_flow_line(
                svg,
                text_box,
                &utf16_offsets,
                &byte_offsets,
                line_range.clone(),
                text_x,
                content_right,
                baseline,
                layout.alignment,
                dark_mode,
                layout.predefined_style,
            );
            cursor_y += line_height;
        }
        if !(current_is_list_item && next_is_list_item) {
            cursor_y += layout.spacing_after;
        }
        paragraph_start += paragraph.chars().count();
    }
    svg.push_str("  </g>\n");
}

fn paragraph_layout(text_box: &RichTextBox, paragraph_index: u32) -> ParagraphLayout {
    let mut layout = ParagraphLayout::default();
    for paragraph in text_box.paragraphs.iter().filter(|paragraph| {
        paragraph.start_paragraph <= paragraph_index && paragraph.end_paragraph > paragraph_index
    }) {
        match paragraph.kind {
            RichTextParagraphType::Alignment => layout.alignment = paragraph.alignment(),
            RichTextParagraphType::IndentLevel => {
                if let Some(indent) = paragraph.indent() {
                    layout.indent_level = indent.level;
                }
            }
            RichTextParagraphType::LineSpacing => layout.line_spacing = paragraph.line_spacing(),
            RichTextParagraphType::Bullet => layout.bullet = paragraph.bullet(),
            RichTextParagraphType::SpacingBefore => {
                layout.spacing_before = paragraph
                    .spacing()
                    .filter(|spacing| spacing.is_finite() && *spacing > 0.0)
                    .map_or(0.0, |spacing| f64::from(spacing) * SAMSUNG_TEXT_SCALE);
            }
            RichTextParagraphType::SpacingAfter => {
                layout.spacing_after = paragraph
                    .spacing()
                    .filter(|spacing| spacing.is_finite() && *spacing > 0.0)
                    .map_or(0.0, |spacing| f64::from(spacing) * SAMSUNG_TEXT_SCALE);
            }
            RichTextParagraphType::PredefinedStyle => {
                layout.predefined_style = paragraph.predefined_style().map(|style| style.style)
            }
            _ => {}
        }
    }
    layout
}

fn paragraph_line_height(font_size: f64, spacing: Option<ParagraphLineSpacing>) -> f64 {
    match spacing {
        Some(spacing)
            if spacing.value.is_finite()
                && spacing.value > 0.0
                && spacing.kind == LineSpacingType::Percent =>
        {
            font_size * f64::from(spacing.value)
        }
        Some(spacing)
            if spacing.value.is_finite()
                && spacing.value > 0.0
                && spacing.kind == LineSpacingType::Pixels =>
        {
            f64::from(spacing.value) * SAMSUNG_TEXT_SCALE
        }
        _ => font_size * 1.6,
    }
}

fn bullet_marker(bullet: ParagraphBullet) -> Option<(String, f64, f64, f64)> {
    let marker_kind = bullet.kind;
    let marker = match marker_kind {
        BulletType::None => return None,
        BulletType::Arrow => "➤".to_string(),
        BulletType::Checker => {
            if bullet.checked {
                "☑".to_string()
            } else {
                "☐".to_string()
            }
        }
        BulletType::Diamond => "◆".to_string(),
        BulletType::Digit => format!("{}.", bullet.number),
        BulletType::CircledDigit => format!("{}", bullet.number),
        BulletType::Alphabet => alphabetic_marker(bullet.number, false),
        BulletType::RomanNumeral => roman_marker(bullet.number),
        BulletType::SolidCircle => "●".to_string(),
        BulletType::WhiteCircle => "○".to_string(),
        BulletType::UppercaseAlphabet => alphabetic_marker(bullet.number, true),
        BulletType::BlackSquare => "■".to_string(),
        BulletType::WhiteSquare => "□".to_string(),
        _ => "•".to_string(),
    };
    let (width, font_size, offset) = match marker_kind {
        BulletType::Digit => (64.0, 45.0, 0.0),
        BulletType::SolidCircle => (48.0, 24.0, 20.0),
        BulletType::WhiteCircle => (78.0, 27.0, 20.0),
        _ => (78.0, 32.0, 12.0),
    };
    Some((marker, width, font_size, offset))
}

fn bullet_marker_for_indent(
    mut bullet: ParagraphBullet,
    indent_level: u32,
) -> Option<(String, f64, f64, f64)> {
    if bullet.kind == BulletType::SolidCircle && indent_level % 2 == 1 {
        bullet.kind = BulletType::WhiteCircle;
    }
    bullet_marker(bullet)
}

fn alphabetic_marker(number: u32, uppercase: bool) -> String {
    let offset = number.saturating_sub(1) % 26;
    let base = if uppercase { b'A' } else { b'a' };
    format!("{}.", char::from(base + offset as u8))
}

fn roman_marker(number: u32) -> String {
    const VALUES: &[(u32, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut remaining = number.max(1);
    let mut result = String::new();
    for (value, numeral) in VALUES {
        while remaining >= *value {
            result.push_str(numeral);
            remaining -= value;
        }
    }
    result.make_ascii_lowercase();
    result.push('.');
    result
}

fn wrap_paragraph(
    text_box: &RichTextBox,
    characters: &[char],
    utf16_offsets: &[u32],
    range: Range<usize>,
    max_width: f64,
    dark_mode: bool,
    predefined_style: Option<PredefinedTextStyle>,
) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut start = range.start;
    while start < range.end {
        let mut width = 0.0;
        let mut index = start;
        let mut last_break = None;
        while index < range.end {
            let character = characters[index];
            let style = text_style_at(text_box, utf16_offsets[index], dark_mode, predefined_style);
            let next_width = width + estimated_character_width(character, &style);
            if next_width > max_width && index > start {
                break;
            }
            width = next_width;
            index += 1;
            if character.is_whitespace() {
                last_break = Some((index - 1, index));
            } else if matches!(character, '/' | '?' | '&' | '#' | '-' | '.')
                && text_box.spans.iter().any(|span| {
                    span.kind == RichTextSpanType::Hyperlink
                        && span.start_utf16 <= utf16_offsets[index - 1]
                        && span.end_utf16 > utf16_offsets[index - 1]
                })
            {
                // Samsung's URL line breaker keeps a link with its prefix and
                // prefers URL punctuation over an arbitrary character split.
                last_break = Some((index, index));
            }
        }
        if index == range.end {
            lines.push(start..range.end);
            break;
        }
        let (end, next) = last_break
            .filter(|(end, _)| *end > start)
            .unwrap_or((index, index));
        lines.push(start..end);
        start = next;
        while start < range.end && characters[start].is_whitespace() {
            start += 1;
        }
    }
    lines
}

fn estimated_character_width(character: char, style: &SvgTextStyle) -> f64 {
    // The analyzed Samsung PDF exporter embeds Roboto-Regular with these
    // advances. Printable ASCII glyph IDs are codepoint - 27 in that font.
    const ROBOTO_ADVANCES: [u16; 100] = [
        443, 0, 0, 248, 248, 248, 257, 320, 615, 562, 732, 622, 174, 342, 348, 430, 567, 196, 276,
        263, 412, 562, 562, 562, 562, 562, 562, 562, 562, 562, 562, 242, 211, 508, 548, 522, 472,
        897, 652, 623, 650, 656, 568, 552, 681, 713, 271, 551, 627, 538, 873, 713, 687, 630, 687,
        616, 593, 596, 648, 636, 887, 626, 600, 599, 265, 410, 265, 417, 451, 309, 543, 561, 523,
        563, 530, 347, 561, 550, 243, 239, 506, 243, 876, 552, 570, 561, 568, 338, 516, 327, 551,
        484, 751, 496, 473, 496, 338, 244, 338, 680,
    ];
    let latin_base = match character {
        'À'..='Å' => Some('A'),
        'Ç' => Some('C'),
        'È'..='Ë' => Some('E'),
        'Ì'..='Ï' => Some('I'),
        'Ñ' => Some('N'),
        'Ò'..='Ö' => Some('O'),
        'Ù'..='Ü' => Some('U'),
        'Ý' => Some('Y'),
        'à'..='å' => Some('a'),
        'ç' => Some('c'),
        'è'..='ë' => Some('e'),
        'ì'..='ï' => Some('i'),
        'ñ' => Some('n'),
        'ò'..='ö' => Some('o'),
        'ù'..='ü' => Some('u'),
        'ý' | 'ÿ' => Some('y'),
        _ => None,
    };
    let metric_character = latin_base.unwrap_or(character);
    let factor = if (' '..='~').contains(&metric_character) {
        let glyph = metric_character as usize - 27;
        f64::from(ROBOTO_ADVANCES[glyph]) / 1000.0
    } else if character.is_whitespace() {
        0.248
    } else if character == 'ß' || ('\u{0370}'..='\u{052f}').contains(&character) {
        0.62
    } else if ('\u{2e80}'..='\u{d7af}').contains(&character) {
        1.0
    } else {
        0.65
    };
    style.font_size * factor
}

#[allow(clippy::too_many_arguments)]
fn render_flow_line(
    svg: &mut String,
    text_box: &RichTextBox,
    utf16_offsets: &[u32],
    byte_offsets: &[usize],
    range: Range<usize>,
    left: f64,
    right: f64,
    baseline: f64,
    alignment: Option<ParagraphAlignment>,
    dark_mode: bool,
    predefined_style: Option<PredefinedTextStyle>,
) {
    if range.is_empty() {
        return;
    }
    let (x, anchor) = match alignment {
        Some(ParagraphAlignment::Center) => ((left + right) / 2.0, "middle"),
        Some(ParagraphAlignment::Right) => (right, "end"),
        _ => (left, "start"),
    };
    let mut boundaries = vec![range.start, range.end];
    for span in &text_box.spans {
        if let Some(start) = utf16_to_char_index(&text_box.text, span.start_utf16)
            && start > range.start
            && start < range.end
        {
            boundaries.push(start);
        }
        if let Some(end) = utf16_to_char_index(&text_box.text, span.end_utf16)
            && end > range.start
            && end < range.end
        {
            boundaries.push(end);
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    write!(
        svg,
        r#"    <text x="{x:.2}" y="{baseline:.2}" text-anchor="{anchor}" font-family="Roboto, Arial, sans-serif" xml:space="preserve">"#,
    )
    .unwrap();
    for segment in boundaries.windows(2) {
        let start = segment[0];
        let end = segment[1];
        let style = text_style_at(text_box, utf16_offsets[start], dark_mode, predefined_style);
        write_styled_tspan(
            svg,
            &text_box.text[byte_offsets[start]..byte_offsets[end]],
            &style,
        );
    }
    svg.push_str("</text>\n");
}

fn text_style_at(
    text_box: &RichTextBox,
    utf16_index: u32,
    dark_mode: bool,
    predefined_style: Option<PredefinedTextStyle>,
) -> SvgTextStyle {
    let mut font_size = text_box.font_size.map(samsung_font_to_svg).unwrap_or(45.0);
    if let Some(style) = predefined_style {
        font_size = match style {
            PredefinedTextStyle::Heading1 => 63.0,
            PredefinedTextStyle::Heading2 => 57.0,
            PredefinedTextStyle::Heading3 => 51.0,
            PredefinedTextStyle::Body1 | PredefinedTextStyle::Other(_) => font_size,
            _ => font_size,
        };
    }
    let mut color = text_box
        .color
        .filter(|color| !dark_mode || !is_dark_compatibility_color(*color));
    let mut style = SvgTextStyle {
        font_size,
        color: color
            .as_ref()
            .map(color_hex)
            .unwrap_or_else(|| default_text_color(dark_mode).to_string()),
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
        link_target: None,
    };
    let mut is_hyperlink = false;
    for span in text_box
        .spans
        .iter()
        .filter(|span| span.start_utf16 <= utf16_index && span.end_utf16 > utf16_index)
    {
        match span.kind {
            RichTextSpanType::ForegroundColor => {
                color = span
                    .color_value()
                    .filter(|color| !dark_mode || !is_dark_compatibility_color(*color));
                style.color = color
                    .as_ref()
                    .map(color_hex)
                    .unwrap_or_else(|| default_text_color(dark_mode).to_string());
            }
            RichTextSpanType::FontSize => {
                if let Some(size) = span.font_size_value() {
                    style.font_size = samsung_font_to_svg(size);
                }
            }
            RichTextSpanType::Bold => style.bold = span.boolean_value() == Some(true),
            RichTextSpanType::Italic => style.italic = span.boolean_value() == Some(true),
            RichTextSpanType::Underline => style.underline = span.boolean_value() == Some(true),
            RichTextSpanType::Strikethrough => {
                style.strikethrough = span.boolean_value() == Some(true)
            }
            RichTextSpanType::Hyperlink => {
                is_hyperlink = true;
                style.link_target = hyperlink_target(text_box, span);
            }
            _ => {}
        }
    }
    if is_hyperlink {
        style.color = SAMSUNG_LINK_COLOR.to_string();
        style.underline = true;
    }
    if matches!(
        predefined_style,
        Some(
            PredefinedTextStyle::Heading1
                | PredefinedTextStyle::Heading2
                | PredefinedTextStyle::Heading3
        )
    ) {
        // Markdown headings carry a bold span, but Samsung's PDF exporter uses
        // the heading face at regular weight.
        style.bold = false;
    }
    style
}

fn hyperlink_target(text_box: &RichTextBox, span: &sdocx::RichTextSpan) -> Option<String> {
    let hyperlink = span.hyperlink_value()?;
    if let Some(target) = hyperlink.custom_data.filter(|target| !target.is_empty()) {
        return Some(target);
    }
    let start = utf16_to_char_index(&text_box.text, span.start_utf16)?;
    let end = utf16_to_char_index(&text_box.text, span.end_utf16)?;
    let byte_offsets = char_byte_offsets(&text_box.text);
    let visible_text = &text_box.text[byte_offsets[start]..byte_offsets[end]];
    match hyperlink.kind {
        HyperlinkType::Email => Some(format!("mailto:{visible_text}")),
        HyperlinkType::Telephone => Some(format!("tel:{visible_text}")),
        HyperlinkType::Url => Some(visible_text.to_string()),
        _ => None,
    }
}

fn write_styled_tspan(svg: &mut String, text: &str, style: &SvgTextStyle) {
    let decoration = match (style.underline, style.strikethrough) {
        (true, true) => r#" text-decoration="underline line-through""#,
        (true, false) => r#" text-decoration="underline""#,
        (false, true) => r#" text-decoration="line-through""#,
        (false, false) => "",
    };
    if let Some(target) = &style.link_target {
        write!(svg, r#"<a href="{}">"#, escape_xml(target)).unwrap();
    }
    let bold_stroke = if style.bold {
        format!(
            r#" stroke="{}" stroke-width="0.45" paint-order="stroke fill""#,
            style.color
        )
    } else {
        String::new()
    };
    write!(
        svg,
        r#"<tspan fill="{}" font-size="{:.2}"{}{}{}>{}</tspan>"#,
        style.color,
        style.font_size,
        bold_stroke,
        if style.italic {
            r#" font-style="italic""#
        } else {
            ""
        },
        decoration,
        escape_xml(text),
    )
    .unwrap();
    if style.link_target.is_some() {
        svg.push_str("</a>");
    }
}

fn render_embedded_object(
    svg: &mut String,
    object: &RichTextObjectSpan,
    cursor_y: f64,
    dark_mode: bool,
) -> Option<f64> {
    match object.content.as_ref() {
        Some(RichTextObjectContent::Table(table)) => {
            let offset_y =
                object_flow_offset(table.bbox.y_min, cursor_y, object_top_margin(object));
            writeln!(svg, r#"    <g data-sdocx-object="table">"#).unwrap();
            let stroke = if dark_mode { "#777777" } else { "#b8b0a3" };
            let clip_id = format!("sdocx-table-{:x}", object.text_index_utf16);
            writeln!(
                svg,
                r#"      <defs><clipPath id="{clip_id}"><rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="24"/></clipPath></defs>"#,
                table.bbox.x_min,
                table.bbox.y_min + offset_y,
                table.bbox.x_max - table.bbox.x_min,
                table.bbox.y_max - table.bbox.y_min,
            )
            .unwrap();
            writeln!(svg, r#"      <g clip-path="url(#{clip_id})">"#).unwrap();
            for row in &table.rows {
                for cell in &row.cells {
                    let fill = table_cell_fill(cell, dark_mode);
                    writeln!(
                        svg,
                        r#"        <rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{fill}"/>"#,
                        cell.bbox.x_min,
                        cell.bbox.y_min + offset_y,
                        cell.bbox.x_max - cell.bbox.x_min,
                        cell.bbox.y_max - cell.bbox.y_min,
                    )
                    .unwrap();
                    if cell.bbox.x_min > table.bbox.x_min + 1.0 {
                        writeln!(
                            svg,
                            r#"        <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{stroke}" stroke-width="1"/>"#,
                            cell.bbox.x_min,
                            cell.bbox.y_min + offset_y,
                            cell.bbox.x_min,
                            cell.bbox.y_max + offset_y,
                        )
                        .unwrap();
                    }
                    if cell.bbox.y_min > table.bbox.y_min + 1.0 {
                        writeln!(
                            svg,
                            r#"        <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{stroke}" stroke-width="1"/>"#,
                            cell.bbox.x_min,
                            cell.bbox.y_min + offset_y,
                            cell.bbox.x_max,
                            cell.bbox.y_min + offset_y,
                        )
                        .unwrap();
                    }
                    if let Some(line) = cell.content.text.lines().next() {
                        let mut style = text_style_at(&cell.content, 0, dark_mode, None);
                        style.bold = false;
                        write!(
                            svg,
                            r#"        <text x="{:.2}" y="{:.2}" font-family="Roboto, Arial, sans-serif" xml:space="preserve">"#,
                            cell.bbox.x_min + 23.0,
                            cell.bbox.y_min + offset_y + 81.0,
                        )
                        .unwrap();
                        write_styled_tspan(svg, line, &style);
                        svg.push_str("</text>\n");
                    }
                }
            }
            svg.push_str("      </g>\n");
            writeln!(
                svg,
                r#"      <rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="24" fill="none" stroke="{stroke}" stroke-width="1"/>"#,
                table.bbox.x_min,
                table.bbox.y_min + offset_y,
                table.bbox.x_max - table.bbox.x_min,
                table.bbox.y_max - table.bbox.y_min,
            )
            .unwrap();
            svg.push_str("    </g>\n");
            Some(table.bbox.y_max + offset_y)
        }
        Some(RichTextObjectContent::CodeBlock(code)) => {
            let offset_y = object_flow_offset(code.bbox.y_min, cursor_y, object_top_margin(object));
            let fill = if dark_mode { "#333333" } else { "#efefef" };
            let stroke = if dark_mode { "#5f5f5f" } else { "#dddddd" };
            writeln!(
                svg,
                r#"    <g data-sdocx-object="code-block"><rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="36" fill="{fill}" stroke="{stroke}" stroke-width="1"/>"#,
                code.bbox.x_min,
                code.bbox.y_min + offset_y,
                code.bbox.x_max - code.bbox.x_min,
                code.bbox.y_max - code.bbox.y_min,
            )
            .unwrap();
            let object_top = code.bbox.y_min + offset_y;
            let text_x = code.bbox.x_min + 81.75;
            if let Some(title) = &code.title {
                render_embedded_line(
                    svg,
                    title,
                    title.text.lines().next().unwrap_or_default(),
                    0,
                    text_x,
                    object_top + 81.6,
                    "Roboto, Arial, sans-serif",
                    dark_mode,
                );
            }
            let icon_stroke = if dark_mode { "#b7b7b7" } else { "#8b8b8b" };
            writeln!(
                svg,
                r#"      <g fill="none" stroke="{icon_stroke}" stroke-width="6" stroke-linejoin="round"><path d="M {:.2} {:.2} v -4 q 0 -8 8 -8 h 17 q 8 0 8 8 v 29"/><rect x="{:.2}" y="{:.2}" width="31" height="38" rx="5"/></g>"#,
                code.bbox.x_min + 895.0,
                object_top + 61.0,
                code.bbox.x_min + 879.0,
                object_top + 59.0,
            )
            .unwrap();
            if let Some(body) = &code.body {
                let mut baseline = object_top + 177.6;
                let mut character_start = 0_usize;
                for (line_index, line) in body.text.lines().enumerate() {
                    render_embedded_line(
                        svg,
                        body,
                        line,
                        character_start,
                        text_x,
                        baseline,
                        "Roboto, Arial, sans-serif",
                        dark_mode,
                    );
                    character_start += line.chars().count() + 1;
                    baseline += if line_index == 0 { 98.25 } else { 60.75 };
                }
            }
            svg.push_str("    </g>\n");
            Some(code.bbox.y_max + offset_y)
        }
        None | Some(_) => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_embedded_line(
    svg: &mut String,
    text_box: &RichTextBox,
    line: &str,
    character_start: usize,
    x: f64,
    baseline: f64,
    font_family: &str,
    dark_mode: bool,
) {
    let utf16_index = text_box
        .text
        .chars()
        .take(character_start)
        .map(|character| character.len_utf16() as u32)
        .sum();
    let style = text_style_at(text_box, utf16_index, dark_mode, None);
    write!(
        svg,
        r#"      <text x="{x:.2}" y="{baseline:.2}" font-family="{font_family}" xml:space="preserve">"#,
    )
    .unwrap();
    write_styled_tspan(svg, line, &style);
    svg.push_str("</text>\n");
}

fn object_flow_offset(stored_top: f64, cursor_y: f64, top_margin: f64) -> f64 {
    if stored_top < 0.0 {
        0.0
    } else {
        cursor_y + top_margin - stored_top
    }
}

fn table_cell_fill(cell: &sdocx::RichTextTableCell, dark_mode: bool) -> String {
    if !cell.has_own_background_color {
        return if dark_mode { "#252525" } else { "#fcfcfc" }.to_string();
    }
    if cell.background_color == 0 {
        return if dark_mode { "#45413d" } else { "#eeebe7" }.to_string();
    }
    format!("#{:06x}", cell.background_color & 0x00ff_ffff)
}

fn object_top_margin(object: &RichTextObjectSpan) -> f64 {
    match object.layout_option {
        sdocx::ObjectSpanLayoutOption::Block => 38.0,
        sdocx::ObjectSpanLayoutOption::Inline => 0.0,
        sdocx::ObjectSpanLayoutOption::BlockWithSmallMargin => 18.0,
        sdocx::ObjectSpanLayoutOption::BlockWithMediumMargin => 36.0,
        _ => 0.0,
    }
}

fn object_bottom_margin(object: &RichTextObjectSpan) -> f64 {
    match object.layout_option {
        // The paragraph itself contributes 12 px after the object. Together
        // these reproduce the 40 px block-to-text gap in Samsung's PDF.
        sdocx::ObjectSpanLayoutOption::Block => 28.0,
        sdocx::ObjectSpanLayoutOption::BlockWithSmallMargin => 12.0,
        sdocx::ObjectSpanLayoutOption::BlockWithMediumMargin => 24.0,
        _ => 0.0,
    }
}

fn default_text_color(dark_mode: bool) -> &'static str {
    if dark_mode {
        DEFAULT_INK_DARK_MODE
    } else {
        DEFAULT_INK_LIGHT_MODE
    }
}

fn char_utf16_offsets(text: &str) -> Vec<u32> {
    let mut offsets = Vec::with_capacity(text.chars().count() + 1);
    let mut offset = 0_u32;
    for character in text.chars() {
        offsets.push(offset);
        offset = offset.saturating_add(character.len_utf16() as u32);
    }
    offsets.push(offset);
    offsets
}

fn utf16_to_char_index(text: &str, target: u32) -> Option<usize> {
    let target = usize::try_from(target).ok()?;
    let mut utf16_offset = 0_usize;
    for (char_index, character) in text.chars().enumerate() {
        if utf16_offset == target {
            return Some(char_index);
        }
        utf16_offset = utf16_offset.checked_add(character.len_utf16())?;
        if utf16_offset > target {
            return None;
        }
    }
    (utf16_offset == target).then_some(text.chars().count())
}

fn is_dark_compatibility_color(color: Color) -> bool {
    // Samsung stores theme-adaptive body text as a dark RGB color even when
    // dark-mode compatibility is enabled. Treat only near-black colors as
    // adaptive so intentional accent colors remain unchanged.
    u16::from(color.r) + u16::from(color.g) + u16::from(color.b) <= 192
}

fn is_dark_background(color: Color) -> bool {
    // Integer form of the standard luma approximation. Compatibility tells
    // Samsung that text may adapt to dark mode; the canvas color tells us
    // whether the exported page is actually dark.
    299 * u32::from(color.r) + 587 * u32::from(color.g) + 114 * u32::from(color.b) < 128_000
}

struct StyledSpan<'a> {
    text: &'a str,
    bold: bool,
    italic: bool,
}

fn styled_line_spans<'a>(
    line: &'a str,
    line_start: usize,
    runs: &[RichTextRun],
) -> Vec<StyledSpan<'a>> {
    let char_count = line.chars().count();
    let mut boundaries = vec![0, char_count];
    for run in runs {
        let start = run.start.saturating_sub(line_start).min(char_count);
        let end = run.end.saturating_sub(line_start).min(char_count);
        if start < end {
            boundaries.push(start);
            boundaries.push(end);
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let byte_offsets = char_byte_offsets(line);
    let mut spans = Vec::new();
    for pair in boundaries.windows(2) {
        let start = pair[0];
        let end = pair[1];
        if start == end {
            continue;
        }
        let global_start = line_start + start;
        let global_end = line_start + end;
        let mut bold = false;
        let mut italic = false;
        for run in runs {
            if run.start < global_end && run.end > global_start {
                bold |= run.bold;
                italic |= run.italic;
            }
        }
        spans.push(StyledSpan {
            text: &line[byte_offsets[start]..byte_offsets[end]],
            bold,
            italic,
        });
    }
    spans
}

fn char_byte_offsets(text: &str) -> Vec<usize> {
    let mut offsets: Vec<usize> = text.char_indices().map(|(offset, _)| offset).collect();
    offsets.push(text.len());
    offsets
}

fn samsung_font_to_svg(size: f32) -> f64 {
    let size = size as f64;
    if size.is_finite() && size > 0.0 {
        (size * SAMSUNG_TEXT_SCALE).clamp(8.0, 144.0)
    } else {
        37.0
    }
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_stroke(svg: &mut String, stroke: &Stroke, default_ink: &str) {
    if stroke.points.len() < 2 {
        return;
    }

    let color = stroke
        .color
        .as_ref()
        .map(color_hex)
        .unwrap_or_else(|| default_ink.into());
    let base_width = normalized_stroke_width(stroke.pen_width);
    let has_pressure = stroke.pressures.len() >= stroke.points.len() - 1
        && stroke
            .pressures
            .iter()
            .any(|&p| p > PRESSURE_PRESENT_EPSILON);

    if has_pressure {
        for j in 1..stroke.points.len() {
            let p_idx = (j - 1).min(stroke.pressures.len() - 1);
            // Preserve raw SDK pressure values in the model, but keep malformed or
            // unsupported records from producing unbounded SVG stroke widths.
            let pressure = stroke.pressures[p_idx].clamp(0.05, 1.0);
            let sw = base_width * (0.3 + 0.7 * pressure);

            let p1 = &stroke.points[j - 1];
            let p2 = &stroke.points[j];
            writeln!(
                svg,
                r#"  <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{color}" stroke-width="{sw:.2}" stroke-linecap="round"/>"#,
                p1.x, p1.y, p2.x, p2.y,
            )
            .unwrap();
        }
    } else {
        let pts_str: String = stroke
            .points
            .iter()
            .map(|p| format!("{:.2},{:.2}", p.x, p.y))
            .collect::<Vec<_>>()
            .join(" ");
        writeln!(
            svg,
            r#"  <polyline points="{pts_str}" fill="none" stroke="{color}" stroke-width="{base_width:.2}" stroke-linecap="round" stroke-linejoin="round"/>"#,
        )
        .unwrap();
    }
}

fn normalized_stroke_width(pen_width: f32) -> f64 {
    let raw_width = pen_width as f64 / 2.5;
    if raw_width.is_finite() && raw_width > 0.0 {
        raw_width.clamp(0.4, 12.0)
    } else {
        1.0
    }
}

fn print_info(doc: &Document, layout: &LayoutDocument) {
    if let Some(dims) = doc.metadata.page_dimensions {
        eprintln!("Page dimensions: {} x {}", dims.0, dims.1);
    }
    if let Some(bg) = doc.metadata.background_color {
        eprintln!("Document background: #{:02x}{:02x}{:02x}", bg.r, bg.g, bg.b);
    }
    if let (Some(dimensions), Some(padding)) =
        (doc.metadata.flow_dimensions, doc.metadata.flow_page_padding)
    {
        eprintln!(
            "Text flow: {} x {}, padding {} x {}",
            dimensions.0, dimensions.1, padding.0, padding.1
        );
    }
    if let Some(margins) = doc
        .metadata
        .note_text
        .as_ref()
        .and_then(|text| text.margins)
    {
        eprintln!(
            "Text margins: {:.0}, {:.0}, {:.0}, {:.0}",
            margins[0], margins[1], margins[2], margins[3]
        );
    }
    if let Some(enabled) = doc.metadata.dark_mode_compatibility {
        eprintln!("Dark mode compatibility: {enabled}");
    }
    eprintln!(
        "{} visible page(s), {} stored page record(s)",
        layout.pages.len(),
        layout.stored_page_count
    );
    for (i, layout_page) in layout.pages.iter().enumerate() {
        let page = &layout_page.page;
        let total_points: usize = page.strokes.iter().map(|s| s.points.len()).sum();
        let colors: std::collections::HashSet<_> = page.strokes.iter().map(|s| s.color).collect();
        let with_pressure = page
            .strokes
            .iter()
            .filter(|s| !s.pressures.is_empty())
            .count();
        eprintln!(
            "  Page {}: {} x {}, background {}, template {}, {} strokes, {} points, {} colors, {} with pressure",
            i,
            page.width,
            page.height,
            page.background_color
                .map(|color| format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b))
                .unwrap_or_else(|| "none".to_string()),
            page.template
                .map(format_template)
                .unwrap_or_else(|| "none".to_string()),
            page.strokes.len(),
            total_points,
            colors.len(),
            with_pressure,
        );
    }
}

fn format_template(template: PageTemplate) -> String {
    match template.source {
        PageTemplateSource::BuiltIn => format!("built-in {}", template.id),
        PageTemplateSource::CustomPdf { page_index } => {
            format!("custom PDF page {}", page_index + 1)
        }
        _ => format!("template {}", template.id),
    }
}

fn write_page(path: &std::path::Path, svg: &str, format: Format) {
    match format {
        Format::Svg => {
            if let Err(e) = fs::write(path, svg) {
                eprintln!("Error: failed to write {}: {e}", path.display());
                std::process::exit(1);
            }
            eprintln!("Wrote {} ({} bytes)", path.display(), svg.len());
        }
        Format::Png => {
            let png = svg_to_png(svg).unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            if let Err(e) = fs::write(path, &png) {
                eprintln!("Error: failed to write {}: {e}", path.display());
                std::process::exit(1);
            }
            eprintln!("Wrote {} ({} bytes)", path.display(), png.len());
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let doc = match sdocx::parse(&cli.path) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };
    let layout = sdocx::layout_document(&doc);

    print_info(&doc, &layout);

    let format = match resolve_format(cli.format, cli.output.as_deref()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let output_base = cli
        .output
        .unwrap_or_else(|| cli.path.with_extension(format.ext()));

    if layout.pages.len() == 1 {
        let dark_mode = layout.pages[0]
            .page
            .background_color
            .or(doc.metadata.background_color)
            .is_some_and(is_dark_background);
        let svg = render_page_svg(
            &layout.pages[0].page,
            doc.metadata.background_color.as_ref(),
            &doc.metadata.media_assets,
            doc.metadata.flow_page_padding,
            dark_mode,
        );
        write_page(&output_base, &svg, format);
    } else {
        for (i, layout_page) in layout.pages.iter().enumerate() {
            let dark_mode = layout_page
                .page
                .background_color
                .or(doc.metadata.background_color)
                .is_some_and(is_dark_background);
            let stem = output_base
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let ext = output_base
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or(format.ext());
            let path = output_base.with_file_name(format!("{stem}_page{i}.{ext}"));
            let svg = render_page_svg(
                &layout_page.page,
                doc.metadata.background_color.as_ref(),
                &doc.metadata.media_assets,
                doc.metadata.flow_page_padding,
                dark_mode,
            );
            write_page(&path, &svg, format);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Format, is_dark_background, normalized_stroke_width, object_flow_offset, render_page_svg,
        resolve_format, samsung_font_to_svg, svg_to_png,
    };
    use sdocx::{
        BoundingBox, Color, Page, PageElement, Point, RichTextBox, RichTextSpan, RichTextSpanType,
        Stroke,
    };
    use std::path::Path;

    #[test]
    fn normalizes_invalid_stroke_widths() {
        assert_eq!(normalized_stroke_width(f32::NAN), 1.0);
        assert_eq!(normalized_stroke_width(f32::INFINITY), 1.0);
        assert_eq!(normalized_stroke_width(0.0), 1.0);
        assert_eq!(normalized_stroke_width(-1.0), 1.0);
    }

    #[test]
    fn uses_pdf_measured_font_and_object_flow_units() {
        assert_eq!(samsung_font_to_svg(15.0), 45.0);
        assert_eq!(object_flow_offset(949.5, 984.0, 36.0), 70.5);
        assert_eq!(object_flow_offset(-229.25, 42.0, 38.0), 0.0);
    }

    #[test]
    fn canvas_color_determines_the_active_theme() {
        assert!(!is_dark_background(Color {
            r: 0xfc,
            g: 0xfc,
            b: 0xfc,
        }));
        assert!(is_dark_background(Color {
            r: 0x25,
            g: 0x25,
            b: 0x25,
        }));
    }

    #[test]
    fn clamps_extreme_stroke_widths() {
        assert_eq!(normalized_stroke_width(0.1), 0.4);
        assert_eq!(normalized_stroke_width(10_000.0), 12.0);
        assert_eq!(normalized_stroke_width(5.0), 2.0);
    }

    #[test]
    fn renders_empty_page_with_page_dimensions_and_background() {
        let page = Page {
            uuid: "page".into(),
            width: 1080,
            height: 1527,
            content_bbox: BoundingBox::default(),
            background_color: Some(Color {
                r: 0xcb,
                g: 0xda,
                b: 0xdd,
            }),
            template: None,
            strokes: Vec::new(),
            elements: Vec::new(),
        };

        let svg = render_page_svg(&page, None, &[], None, false);

        assert!(svg.contains(r#"viewBox="0.0 0.0 1080.0 1527.0""#));
        assert!(svg.contains(r#"width="1080" height="1527""#));
        assert!(svg.contains(r##"fill="#cbdadd""##));
    }

    fn page_with_uncolored_stroke() -> Page {
        Page {
            uuid: "page".into(),
            width: 100,
            height: 100,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            strokes: vec![Stroke {
                bbox: BoundingBox::default(),
                points: vec![Point { x: 1.0, y: 1.0 }, Point { x: 9.0, y: 9.0 }],
                pressures: Vec::new(),
                timestamps: Vec::new(),
                tilts: Vec::new(),
                orientations: Vec::new(),
                color: None,
                pen_width: 2.0,
            }],
            elements: Vec::new(),
        }
    }

    #[test]
    fn uncolored_stroke_defaults_to_dark_ink_in_light_mode() {
        let svg = render_page_svg(&page_with_uncolored_stroke(), None, &[], None, false);
        assert!(
            svg.contains(r##"stroke="#1a1a1a""##),
            "light-mode default ink"
        );
        assert!(!svg.contains(r##"stroke="#ffffff""##));
    }

    #[test]
    fn uncolored_stroke_defaults_to_light_ink_in_dark_mode() {
        let svg = render_page_svg(&page_with_uncolored_stroke(), None, &[], None, true);
        assert!(
            svg.contains(r##"stroke="#ffffff""##),
            "dark-mode default ink"
        );
        assert!(!svg.contains(r##"stroke="#1a1a1a""##));
    }

    #[test]
    fn clamps_pressure_when_rendering_strokes() {
        let mut page = page_with_uncolored_stroke();
        page.strokes[0].pressures = vec![f64::MAX, f64::MAX];

        let svg = render_page_svg(&page, None, &[], None, false);

        assert!(svg.contains(r#"stroke-width="0.80""#));
        assert!(!svg.contains("inf"));
    }

    #[test]
    fn missing_background_falls_back_to_mode_matched_canvas() {
        // No page or document background: the fallback canvas must match the ink
        // mode, or dark ink lands on a dark fallback (or vice versa) and vanishes.
        let light = render_page_svg(&page_with_uncolored_stroke(), None, &[], None, false);
        assert!(
            light.contains(r##"fill="#fcfcfc""##),
            "light-mode fallback bg"
        );
        let dark = render_page_svg(&page_with_uncolored_stroke(), None, &[], None, true);
        assert!(
            dark.contains(r##"fill="#252525""##),
            "dark-mode fallback bg"
        );
    }

    #[test]
    fn dark_mode_makes_compatibility_text_visible() {
        let mut page = page_with_uncolored_stroke();
        page.strokes.clear();
        page.elements.push(PageElement::TextBox(RichTextBox {
            bbox: BoundingBox::default(),
            rotation_degrees: None,
            text: "visible body text".into(),
            color: Some(Color {
                r: 0x25,
                g: 0x25,
                b: 0x25,
            }),
            highlight_color: None,
            underline: false,
            font_size: None,
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: Vec::new(),
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        }));

        let svg = render_page_svg(&page, None, &[], None, true);

        assert!(svg.contains(r#"<text x="48.00" y="45.00""#));
        assert!(svg.contains(r##"<tspan fill="#ffffff""##));
        assert!(!svg.contains(r##"<tspan fill="#252525""##));
    }

    #[test]
    fn renders_hyperlink_color_and_target_from_the_sdk_span() {
        let target = "https://example.com/markdown-test";
        let mut payload = Vec::new();
        payload.extend_from_slice(&3_u32.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&(target.encode_utf16().count() as u32).to_le_bytes());
        payload.extend(target.encode_utf16().flat_map(|unit| unit.to_le_bytes()));
        let mut page = page_with_uncolored_stroke();
        page.width = 1080;
        page.strokes.clear();
        page.elements.push(PageElement::TextBox(RichTextBox {
            bbox: BoundingBox::default(),
            rotation_degrees: None,
            text: "Example link".into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: Some(15.0),
            runs: Vec::new(),
            spans: vec![RichTextSpan {
                kind: RichTextSpanType::Hyperlink,
                start_utf16: 0,
                end_utf16: 12,
                expand: false,
                payload,
            }],
            paragraphs: Vec::new(),
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        }));

        let svg = render_page_svg(&page, None, &[], None, false);

        assert!(svg.contains(r##"<a href="https://example.com/markdown-test">"##));
        assert!(svg.contains(r##"fill="#0054ff""##));
        assert!(svg.contains(r#"text-decoration="underline""#));
    }

    #[test]
    fn format_flag_wins_over_extension() {
        let f = resolve_format(Some(Format::Svg), Some(Path::new("out.png"))).unwrap();
        assert_eq!(f, Format::Svg);
    }

    #[test]
    fn format_inferred_from_png_extension() {
        let f = resolve_format(None, Some(Path::new("out.png"))).unwrap();
        assert_eq!(f, Format::Png);
    }

    #[test]
    fn format_inferred_from_svg_extension() {
        let f = resolve_format(None, Some(Path::new("out.svg"))).unwrap();
        assert_eq!(f, Format::Svg);
    }

    #[test]
    fn format_defaults_to_svg_when_no_output_and_no_flag() {
        let f = resolve_format(None, None).unwrap();
        assert_eq!(f, Format::Svg);
    }

    #[test]
    fn unknown_extension_without_flag_is_error() {
        assert!(resolve_format(None, Some(Path::new("out.gif"))).is_err());
    }

    #[test]
    fn extension_inference_is_case_insensitive() {
        assert_eq!(
            resolve_format(None, Some(Path::new("out.PNG"))).unwrap(),
            Format::Png
        );
        assert_eq!(
            resolve_format(None, Some(Path::new("out.Svg"))).unwrap(),
            Format::Svg
        );
    }

    #[test]
    fn svg_to_png_produces_valid_png_with_expected_size() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10" width="20" height="10"><rect x="0" y="0" width="20" height="10" fill="#252525"/><line x1="0" y1="0" x2="20" y2="10" stroke="#ffffff" stroke-width="1"/></svg>"##;
        let png = svg_to_png(svg).expect("render should succeed");
        // Full 8-byte PNG signature.
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        // IHDR width/height are big-endian u32 at byte offsets 16 and 20.
        let w = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
        let h = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
        assert_eq!((w, h), (20, 10));
    }

    #[test]
    fn svg_to_png_rasterizes_text_with_a_system_font() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 60" width="200" height="60"><rect width="200" height="60" fill="#252525"/><text x="5" y="40" fill="#ffffff" font-family="Arial, sans-serif" font-size="32">visible</text></svg>"##;

        let png = svg_to_png(svg).expect("render should succeed");
        let pixmap = resvg::tiny_skia::Pixmap::decode_png(&png).expect("decode rendered PNG");

        assert!(
            pixmap
                .pixels()
                .iter()
                .any(|pixel| pixel.red() > 0x80 && pixel.green() > 0x80 && pixel.blue() > 0x80),
            "rendered PNG should contain light text pixels"
        );
    }

    #[test]
    fn svg_to_png_rasterizes_monospace_code_text() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 60" width="200" height="60"><rect width="200" height="60" fill="#fcfcfc"/><text x="5" y="40" fill="#252525" font-family="Roboto Mono, monospace" font-size="32">fn main()</text></svg>"##;

        let png = svg_to_png(svg).expect("render code text");
        let pixmap = resvg::tiny_skia::Pixmap::decode_png(&png).expect("decode rendered PNG");

        assert!(
            pixmap
                .pixels()
                .iter()
                .any(|pixel| { pixel.red() < 0x80 && pixel.green() < 0x80 && pixel.blue() < 0x80 }),
            "rendered PNG should contain dark monospace glyphs"
        );
    }

    #[test]
    fn renders_page_to_valid_png() {
        let svg = render_page_svg(&page_with_uncolored_stroke(), None, &[], None, false);
        let png = svg_to_png(&svg).expect("render page to png");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert!(
            png.len() > 100,
            "PNG should be non-trivial, got {} bytes",
            png.len()
        );
    }
}
