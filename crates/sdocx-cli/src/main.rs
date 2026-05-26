use base64::Engine as _;
use clap::{Parser, ValueEnum};
use sdocx::{
    Color, Document, MediaAsset, Page, PageElement, PageTemplate, PageTemplateSource, RichTextBox,
    RichTextRun, Stroke,
};
use std::fmt::Write as FmtWrite;
use std::fs;
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
    opt.fontdb_mut().load_system_fonts();
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
        render_element(&mut svg, element, page, media_assets);
    }

    svg.push_str("</svg>\n");
    svg
}

fn render_element(
    svg: &mut String,
    element: &PageElement,
    page: &Page,
    media_assets: &[MediaAsset],
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
        PageElement::TextBox(text_box) => render_text_box(svg, text_box, page),
    }
}

fn render_text_box(svg: &mut String, text_box: &RichTextBox, page: &Page) {
    let text = text_box.text.trim_end_matches('\n');
    if text.trim().is_empty() {
        return;
    }

    let is_note_body =
        text_box.bbox.x_max <= text_box.bbox.x_min || text_box.bbox.y_max <= text_box.bbox.y_min;
    let (x, y, width, height) = if is_note_body {
        (50.0, 0.0, page.width as f64 - 100.0, page.height as f64)
    } else {
        (
            text_box.bbox.x_min,
            text_box.bbox.y_min,
            text_box.bbox.x_max - text_box.bbox.x_min,
            text_box.bbox.y_max - text_box.bbox.y_min,
        )
    };
    let color = text_box
        .color
        .as_ref()
        .map(color_hex)
        .unwrap_or_else(|| "#252525".into());
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
        (size * 2.18).clamp(8.0, 96.0)
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
            let pressure = stroke.pressures[p_idx].max(0.05);
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

fn print_info(doc: &Document) {
    if let Some(dims) = doc.metadata.page_dimensions {
        eprintln!("Page dimensions: {} x {}", dims.0, dims.1);
    }
    if let Some(bg) = doc.metadata.background_color {
        eprintln!("Document background: #{:02x}{:02x}{:02x}", bg.r, bg.g, bg.b);
    }
    if let Some(enabled) = doc.metadata.dark_mode_compatibility {
        eprintln!("Dark mode compatibility: {enabled}");
    }
    eprintln!("{} page(s)", doc.pages.len());
    for (i, page) in doc.pages.iter().enumerate() {
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
    }
}

#[cfg(test)]
mod tests {
    use super::{Format, normalized_stroke_width, render_page_svg, resolve_format, svg_to_png};
    use sdocx::{BoundingBox, Color, Page, Point, Stroke};
    use std::path::Path;

    #[test]
    fn normalizes_invalid_stroke_widths() {
        assert_eq!(normalized_stroke_width(f32::NAN), 1.0);
        assert_eq!(normalized_stroke_width(f32::INFINITY), 1.0);
        assert_eq!(normalized_stroke_width(0.0), 1.0);
        assert_eq!(normalized_stroke_width(-1.0), 1.0);
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

        let svg = render_page_svg(&page, None, &[], false);

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
                tilt_x: Vec::new(),
                tilt_y: Vec::new(),
                color: None,
                pen_width: 2.0,
            }],
            elements: Vec::new(),
        }
    }

    #[test]
    fn uncolored_stroke_defaults_to_dark_ink_in_light_mode() {
        let svg = render_page_svg(&page_with_uncolored_stroke(), None, &[], false);
        assert!(
            svg.contains(r##"stroke="#1a1a1a""##),
            "light-mode default ink"
        );
        assert!(!svg.contains(r##"stroke="#ffffff""##));
    }

    #[test]
    fn uncolored_stroke_defaults_to_light_ink_in_dark_mode() {
        let svg = render_page_svg(&page_with_uncolored_stroke(), None, &[], true);
        assert!(
            svg.contains(r##"stroke="#ffffff""##),
            "dark-mode default ink"
        );
        assert!(!svg.contains(r##"stroke="#1a1a1a""##));
    }

    #[test]
    fn missing_background_falls_back_to_mode_matched_canvas() {
        // No page or document background: the fallback canvas must match the ink
        // mode, or dark ink lands on a dark fallback (or vice versa) and vanishes.
        let light = render_page_svg(&page_with_uncolored_stroke(), None, &[], false);
        assert!(
            light.contains(r##"fill="#fcfcfc""##),
            "light-mode fallback bg"
        );
        let dark = render_page_svg(&page_with_uncolored_stroke(), None, &[], true);
        assert!(
            dark.contains(r##"fill="#252525""##),
            "dark-mode fallback bg"
        );
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
    fn renders_sample_to_valid_png() {
        let doc = sdocx::parse("../../samples/handwritten.sdocx").expect("parse sample");
        assert!(!doc.pages.is_empty(), "sample has no pages");
        let svg = render_page_svg(
            &doc.pages[0],
            doc.metadata.background_color.as_ref(),
            &doc.metadata.media_assets,
            doc.metadata.dark_mode_compatibility.unwrap_or(false),
        );
        let png = svg_to_png(&svg).expect("render sample to png");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert!(
            png.len() > 100,
            "PNG should be non-trivial, got {} bytes",
            png.len()
        );
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

    print_info(&doc);

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

    let dark_mode = doc.metadata.dark_mode_compatibility.unwrap_or(false);

    if doc.pages.len() == 1 {
        let svg = render_page_svg(
            &doc.pages[0],
            doc.metadata.background_color.as_ref(),
            &doc.metadata.media_assets,
            dark_mode,
        );
        write_page(&output_base, &svg, format);
    } else {
        for (i, page) in doc.pages.iter().enumerate() {
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
                page,
                doc.metadata.background_color.as_ref(),
                &doc.metadata.media_assets,
                dark_mode,
            );
            write_page(&path, &svg, format);
        }
    }
}
