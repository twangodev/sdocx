use clap::{Parser, ValueEnum};
use sdocx::{Document, LayoutDocument, PageTemplate, PageTemplateSource, RenderOptions};
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

    let parsed = match sdocx::parse_detailed(&cli.path) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };
    for diagnostic in &parsed.report.diagnostics {
        if let Some(entry) = &diagnostic.archive_entry {
            eprintln!(
                "Warning [{:?}] {entry}: {}",
                diagnostic.code, diagnostic.message
            );
        } else {
            eprintln!("Warning [{:?}]: {}", diagnostic.code, diagnostic.message);
        }
    }
    let doc = parsed.document;
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
    let rendered_pages = sdocx::render_document_svg(&doc, &RenderOptions::default());

    if rendered_pages.len() == 1 {
        write_page(&output_base, &rendered_pages[0].svg, format);
    } else {
        for (i, rendered_page) in rendered_pages.iter().enumerate() {
            let stem = output_base
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let ext = output_base
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or(format.ext());
            let path = output_base.with_file_name(format!("{stem}_page{i}.{ext}"));
            write_page(&path, &rendered_page.svg, format);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Format, resolve_format, svg_to_png};
    use std::path::Path;

    #[test]
    fn format_flag_wins_over_extension() {
        let format = resolve_format(Some(Format::Svg), Some(Path::new("out.png"))).unwrap();
        assert_eq!(format, Format::Svg);
    }

    #[test]
    fn format_is_inferred_from_extension() {
        assert_eq!(
            resolve_format(None, Some(Path::new("out.png"))).unwrap(),
            Format::Png
        );
        assert_eq!(
            resolve_format(None, Some(Path::new("out.Svg"))).unwrap(),
            Format::Svg
        );
    }

    #[test]
    fn format_defaults_to_svg_without_an_output() {
        assert_eq!(resolve_format(None, None).unwrap(), Format::Svg);
    }

    #[test]
    fn unknown_extension_without_flag_is_an_error() {
        assert!(resolve_format(None, Some(Path::new("out.gif"))).is_err());
    }

    #[test]
    fn svg_to_png_produces_a_png_with_the_expected_size() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10" width="20" height="10"><rect x="0" y="0" width="20" height="10" fill="#252525"/><line x1="0" y1="0" x2="20" y2="10" stroke="#ffffff" stroke-width="1"/></svg>"##;
        let png = svg_to_png(svg).expect("render should succeed");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
        let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
        assert_eq!((width, height), (20, 10));
    }

    #[test]
    fn svg_to_png_rasterizes_text_with_system_fonts() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 60" width="200" height="60"><rect width="200" height="60" fill="#252525"/><text x="5" y="40" fill="#ffffff" font-family="Arial, sans-serif" font-size="32">visible</text></svg>"##;
        let png = svg_to_png(svg).expect("render should succeed");
        let pixmap = resvg::tiny_skia::Pixmap::decode_png(&png).expect("decode rendered PNG");
        assert!(
            pixmap
                .pixels()
                .iter()
                .any(|pixel| { pixel.red() > 0x80 && pixel.green() > 0x80 && pixel.blue() > 0x80 })
        );
    }

    #[test]
    fn svg_to_png_rasterizes_monospace_text() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 60" width="200" height="60"><rect width="200" height="60" fill="#fcfcfc"/><text x="5" y="40" fill="#252525" font-family="Roboto Mono, monospace" font-size="32">fn main()</text></svg>"##;
        let png = svg_to_png(svg).expect("render code text");
        let pixmap = resvg::tiny_skia::Pixmap::decode_png(&png).expect("decode rendered PNG");

        assert!(
            pixmap
                .pixels()
                .iter()
                .any(|pixel| { pixel.red() < 0x80 && pixel.green() < 0x80 && pixel.blue() < 0x80 })
        );
    }

    #[test]
    fn shared_renderer_output_rasterizes_to_png() {
        let document = sdocx::Document {
            pages: vec![sdocx::Page {
                uuid: "page".into(),
                width: 100,
                height: 100,
                content_bbox: sdocx::BoundingBox::default(),
                background_color: None,
                template: None,
                strokes: vec![sdocx::Stroke {
                    bbox: sdocx::BoundingBox::default(),
                    points: vec![
                        sdocx::Point { x: 1.0, y: 1.0 },
                        sdocx::Point { x: 9.0, y: 9.0 },
                    ],
                    pressures: Vec::new(),
                    timestamps: Vec::new(),
                    tilts: Vec::new(),
                    orientations: Vec::new(),
                    color: None,
                    pen_width: 2.0,
                }],
                elements: Vec::new(),
            }],
            metadata: sdocx::DocumentMetadata::default(),
        };
        let rendered = sdocx::render_document_svg(&document, &sdocx::RenderOptions::default());
        let png = svg_to_png(&rendered[0].svg).expect("render page to png");

        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert!(png.len() > 100, "PNG should be non-trivial");
    }
}
