use clap::{Parser, ValueEnum};
use sdocx::{Document, LayoutDocument, PageTemplate, PageTemplateSource, RenderOptions};
use std::fs;
use std::path::PathBuf;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Format {
    Svg,
    Png,
    Pdf,
}

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
        Some("pdf") => Ok(Format::Pdf),
        Some(other) => Err(format!(
            "unknown output extension '.{other}'; use -f/--format to set svg, png or pdf"
        )),
        None => Ok(Format::Svg),
    }
}

impl Format {
    fn ext(self) -> &'static str {
        match self {
            Format::Svg => "svg",
            Format::Png => "png",
            Format::Pdf => "pdf",
        }
    }
}

fn svg_options(font_files: &[PathBuf]) -> Result<resvg::usvg::Options<'static>, String> {
    let mut opt = resvg::usvg::Options::default();
    let fontdb = opt.fontdb_mut();
    for path in font_files {
        let before = fontdb.faces().count();
        fontdb
            .load_font_file(path)
            .map_err(|error| format!("cannot load font {}: {error}", path.display()))?;
        if fontdb.faces().count() == before {
            return Err(format!("no usable font faces in {}", path.display()));
        }
    }
    fontdb.load_system_fonts();
    fontdb.set_sans_serif_family("DejaVu Sans");
    fontdb.set_monospace_family("DejaVu Sans Mono");
    Ok(opt)
}

fn parse_pdf_dpi(value: &str) -> Result<f32, String> {
    let dpi = value.parse::<f32>().map_err(|error| error.to_string())?;
    if !dpi.is_finite() || dpi <= 0.0 {
        return Err("PDF DPI must be finite and greater than zero".into());
    }
    Ok(dpi)
}

fn svg_to_png(svg: &str, options: &resvg::usvg::Options<'_>) -> Result<Vec<u8>, String> {
    let tree =
        resvg::usvg::Tree::from_str(svg, options).map_err(|e| format!("invalid SVG: {e}"))?;
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
    #[arg(help = "Path to an .sdocx file")]
    path: PathBuf,

    #[arg(
        long,
        help = "Report stored hash matches, mismatches and unavailable checks during conversion"
    )]
    verify_integrity: bool,

    #[arg(
        short,
        long,
        help = "Output path (format inferred from extension; defaults to input with the selected format extension)"
    )]
    output: Option<PathBuf>,

    #[arg(
        short,
        long,
        value_enum,
        help = "Output format (overrides extension inference)"
    )]
    format: Option<Format>,

    #[arg(
        long = "font",
        value_name = "PATH",
        help = "Additional font for PNG/PDF rendering; repeat for multiple files"
    )]
    font_files: Vec<PathBuf>,

    #[arg(long, value_parser = parse_pdf_dpi, help = "SVG units per inch for PDF physical page size (default: 96)")]
    pdf_dpi: Option<f32>,
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

fn write_page(path: &std::path::Path, svg: &str, png_options: Option<&resvg::usvg::Options<'_>>) {
    match png_options {
        None => {
            if let Err(e) = fs::write(path, svg) {
                eprintln!("Error: failed to write {}: {e}", path.display());
                std::process::exit(1);
            }
            eprintln!("Wrote {} ({} bytes)", path.display(), svg.len());
        }
        Some(options) => {
            let png = svg_to_png(svg, options).unwrap_or_else(|e| {
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

    let options = sdocx::ParseOptions {
        verify_integrity: cli.verify_integrity,
        ..Default::default()
    };
    let parsed = match sdocx::parse_detailed_with_options(&cli.path, &options) {
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
    if let Some(integrity) = &parsed.integrity {
        for (name, counts) in [
            ("note", integrity.note),
            ("objects", integrity.objects),
            ("layers", integrity.layers),
            ("pages", integrity.pages),
            ("manifest", integrity.manifest),
        ] {
            eprintln!(
                "Integrity {name}: {} matched, {} mismatched, {} unavailable",
                counts.matched, counts.mismatched, counts.unavailable
            );
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
    if format == Format::Svg && !cli.font_files.is_empty() {
        eprintln!("Error: --font applies to PNG/PDF output; use -f png or -f pdf");
        std::process::exit(1);
    }
    if format != Format::Pdf && cli.pdf_dpi.is_some() {
        eprintln!("Error: --pdf-dpi applies to PDF output; use -f pdf or a .pdf output path");
        std::process::exit(1);
    }
    let svg_options = if format != Format::Svg {
        Some(svg_options(&cli.font_files).unwrap_or_else(|error| {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }))
    } else {
        None
    };

    let output_base = cli
        .output
        .unwrap_or_else(|| cli.path.with_extension(format.ext()));
    let rendered_pages = sdocx::render_document_svg(&doc, &RenderOptions::default());

    if format == Format::Pdf {
        let mut options = sdocx::PdfOptions::new(svg_options.as_ref().unwrap().fontdb.clone());
        if let Some(dpi) = cli.pdf_dpi {
            options.dpi = dpi;
        }
        let pdf = sdocx::render_svg_pages_pdf(&rendered_pages, &options).unwrap_or_else(|error| {
            eprintln!("Error: {error}");
            std::process::exit(1);
        });
        if let Err(error) = fs::write(&output_base, &pdf) {
            eprintln!("Error: failed to write {}: {error}", output_base.display());
            std::process::exit(1);
        }
        eprintln!(
            "Wrote {} ({} bytes, {} pages)",
            output_base.display(),
            pdf.len(),
            rendered_pages.len()
        );
        return;
    }

    if rendered_pages.len() == 1 {
        write_page(&output_base, &rendered_pages[0].svg, svg_options.as_ref());
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
            write_page(&path, &rendered_page.svg, svg_options.as_ref());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Format, resolve_format, svg_options};
    use std::path::Path;

    fn svg_to_png(svg: &str) -> Result<Vec<u8>, String> {
        super::svg_to_png(svg, &svg_options(&[])?)
    }

    struct FontFile(std::path::PathBuf);

    impl FontFile {
        fn new(data: &[u8]) -> Self {
            use std::io::Write;
            use std::sync::atomic::{AtomicUsize, Ordering};
            static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "sdocx-font-{}-{}.ttf",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            file.write_all(data).unwrap();
            Self(path)
        }
    }

    impl Drop for FontFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn explicit_font_file_wins_over_the_same_system_face() {
        use resvg::usvg::fontdb::{Family, Query, Source};
        let system = svg_options(&[]).unwrap();
        let face = system.fontdb.faces().next().expect("system font");
        let data = system
            .fontdb
            .with_face_data(face.id, |data, _| data.to_vec())
            .unwrap();
        let file = FontFile::new(&data);
        let explicit = svg_options(std::slice::from_ref(&file.0)).unwrap();
        let selected = explicit
            .fontdb
            .query(&Query {
                families: &[Family::Name(&face.families[0].0)],
                weight: face.weight,
                stretch: face.stretch,
                style: face.style,
            })
            .unwrap();
        match explicit.fontdb.face_source(selected).unwrap().0 {
            Source::File(path) | Source::SharedFile(path, _) => assert_eq!(path, file.0),
            _ => panic!("expected the explicitly supplied font file"),
        }
    }

    #[test]
    fn missing_or_invalid_explicit_fonts_fail_instead_of_falling_back() {
        let invalid = FontFile::new(b"not a font");
        assert!(matches!(
            svg_options(std::slice::from_ref(&invalid.0)),
            Err(message) if message.contains("no usable font faces")
        ));
        assert!(matches!(
            svg_options(&[invalid.0.with_extension("missing")]),
            Err(message) if message.contains("cannot load font")
        ));
    }

    #[test]
    fn cli_accepts_repeated_explicit_fonts() {
        use clap::Parser;
        let cli = super::Cli::try_parse_from([
            "sdocx",
            "note.sdocx",
            "-f",
            "png",
            "--font",
            "regular.ttf",
            "--font",
            "symbols.otf",
        ])
        .unwrap();
        assert_eq!(
            cli.font_files,
            vec![Path::new("regular.ttf"), Path::new("symbols.otf")]
        );
    }

    #[test]
    fn format_flag_wins_over_extension() {
        let format = resolve_format(Some(Format::Svg), Some(Path::new("out.png"))).unwrap();
        assert_eq!(format, Format::Svg);
        assert_eq!(
            resolve_format(Some(Format::Pdf), Some(Path::new("out.svg"))).unwrap(),
            Format::Pdf
        );
    }

    #[test]
    fn format_is_inferred_from_extension() {
        assert_eq!(
            resolve_format(None, Some(Path::new("out.PDF"))).unwrap(),
            Format::Pdf
        );
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
    fn pdf_scale_must_be_finite_and_positive() {
        use clap::Parser;
        for value in ["0", "NaN", "inf", "-96", "words"] {
            assert!(
                super::Cli::try_parse_from([
                    "sdocx",
                    "note.sdocx",
                    "-f",
                    "pdf",
                    "--pdf-dpi",
                    value
                ])
                .is_err()
            );
        }
        let cli = super::Cli::try_parse_from([
            "sdocx",
            "note.sdocx",
            "-f",
            "pdf",
            "--pdf-dpi",
            "129.6",
            "--font",
            "Roboto.ttf",
        ])
        .unwrap();
        assert_eq!(cli.format, Some(Format::Pdf));
        assert_eq!(cli.pdf_dpi, Some(129.6));
        assert_eq!(cli.font_files, vec![Path::new("Roboto.ttf")]);
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
