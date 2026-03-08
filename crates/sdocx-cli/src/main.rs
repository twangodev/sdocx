use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sdocx", version, about = "Parse Samsung Notes .sdocx files")]
struct Cli {
    /// Path to an .sdocx file
    path: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    match sdocx::parse(&cli.path) {
        Ok(doc) => {
            if let Some(dims) = doc.metadata.page_dimensions {
                println!("Page dimensions: {} x {}", dims.0, dims.1);
            }
            if let Some(bg) = doc.metadata.background_color {
                println!("Background: #{:02x}{:02x}{:02x}", bg.r, bg.g, bg.b);
            }
            println!("{} page(s)", doc.pages.len());
            for (i, page) in doc.pages.iter().enumerate() {
                let total_points: usize = page.strokes.iter().map(|s| s.points.len()).sum();
                let colors: std::collections::HashSet<_> =
                    page.strokes.iter().map(|s| s.color).collect();
                let with_pressure = page
                    .strokes
                    .iter()
                    .filter(|s| !s.pressures.is_empty())
                    .count();
                println!(
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
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
