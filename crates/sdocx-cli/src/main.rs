use clap::Parser;
use sdocx::{Color, Document, Page, Stroke};
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::PathBuf;

const DEFAULT_INK: &str = "#ffffff";

#[derive(Parser)]
#[command(name = "sdocx", version, about = "Parse Samsung Notes .sdocx files")]
struct Cli {
    /// Path to an .sdocx file
    path: PathBuf,

    /// Output SVG file (defaults to input path with .svg extension)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn color_hex(c: &Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn render_page_svg(page: &Page, bg_color: Option<&Color>) -> String {
    let bg = bg_color.map(color_hex).unwrap_or_else(|| "#252525".into());

    // Compute content bounding box from all stroke points
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    for stroke in &page.strokes {
        for pt in &stroke.points {
            min_x = min_x.min(pt.x);
            min_y = min_y.min(pt.y);
            max_x = max_x.max(pt.x);
            max_y = max_y.max(pt.y);
        }
    }

    if page.strokes.is_empty() || min_x > max_x {
        return r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.into();
    }

    let margin = 10.0;
    let vb_x = min_x - margin;
    let vb_y = min_y - margin;
    let vb_w = max_x - min_x + 2.0 * margin;
    let vb_h = max_y - min_y + 2.0 * margin;

    let scale = (1200.0 / vb_w).min(1600.0 / vb_h).min(1.0);
    let svg_w = (vb_w * scale) as u32;
    let svg_h = (vb_h * scale) as u32;

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

    for stroke in &page.strokes {
        render_stroke(&mut svg, stroke);
    }

    svg.push_str("</svg>\n");
    svg
}

fn render_stroke(svg: &mut String, stroke: &Stroke) {
    if stroke.points.len() < 2 {
        return;
    }

    let color = stroke
        .color
        .as_ref()
        .map(color_hex)
        .unwrap_or_else(|| DEFAULT_INK.into());
    let base_width = stroke.pen_width as f64 / 2.5;
    let has_pressure = stroke.pressures.len() >= stroke.points.len() - 1;

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

fn print_info(doc: &Document) {
    if let Some(dims) = doc.metadata.page_dimensions {
        eprintln!("Page dimensions: {} x {}", dims.0, dims.1);
    }
    if let Some(bg) = doc.metadata.background_color {
        eprintln!("Background: #{:02x}{:02x}{:02x}", bg.r, bg.g, bg.b);
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
            "  Page {}: {} x {}, {} strokes, {} points, {} colors, {} with pressure",
            i,
            page.width,
            page.height,
            page.strokes.len(),
            total_points,
            colors.len(),
            with_pressure,
        );
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

    let output_base = cli
        .output
        .unwrap_or_else(|| cli.path.with_extension("svg"));

    if doc.pages.len() == 1 {
        let svg = render_page_svg(&doc.pages[0], doc.metadata.background_color.as_ref());
        fs::write(&output_base, &svg).expect("failed to write SVG");
        eprintln!(
            "Wrote {} ({} bytes)",
            output_base.display(),
            svg.len()
        );
    } else {
        for (i, page) in doc.pages.iter().enumerate() {
            let stem = output_base
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let ext = output_base
                .extension()
                .unwrap_or_default()
                .to_string_lossy();
            let path = output_base.with_file_name(format!("{stem}_page{i}.{ext}"));
            let svg = render_page_svg(page, doc.metadata.background_color.as_ref());
            fs::write(&path, &svg).expect("failed to write SVG");
            eprintln!("Wrote {} ({} bytes)", path.display(), svg.len());
        }
    }
}