use std::path::PathBuf;

use clap::Parser;
use sdocx_conformance::image::MetricsOptions;
use sdocx_conformance::visual::{VisualConfig, read_fixtures, run_visual};

#[derive(Debug, Parser)]
#[command(about = "Compare SDK PNG/PDF output with locked Samsung exports")]
struct Args {
    #[arg(long, env = "SDOCX_CORPUS_DIR", default_value = "hf")]
    corpus_dir: PathBuf,
    #[arg(long, default_value = "conformance/corpus.json")]
    manifest: PathBuf,
    #[arg(long = "fixture")]
    fixtures: Vec<String>,
    #[arg(long, default_value = "target/debug/sdocx-cli")]
    cli: PathBuf,
    #[arg(long, default_value="png", value_parser=["png", "pdf"])]
    format: String,
    #[arg(long = "font")]
    fonts: Vec<PathBuf>,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value_t=16, value_parser=clap::value_parser!(u8).range(0..=254))]
    channel_threshold: u8,
    #[arg(long, default_value_t=32, value_parser=clap::value_parser!(u8).range(0..=254))]
    ink_threshold: u8,
    #[arg(long, default_value_t=1, value_parser=clap::value_parser!(u8).range(0..=5))]
    tolerance: u8,
    #[arg(long, env = "SDOCX_PDF_PYTHON", default_value = "python3")]
    pdf_python: PathBuf,
}

fn main() {
    if let Err(error) = execute(Args::parse()) {
        eprintln!("Visual comparison failed: {error}");
        std::process::exit(1);
    }
}
fn execute(args: Args) -> sdocx_conformance::Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("no workspace root")?
        .to_path_buf();
    let manifest = if args.manifest.is_absolute() {
        args.manifest
    } else {
        root.join(args.manifest)
    };
    let corpus = if args.corpus_dir.is_absolute() {
        args.corpus_dir
    } else {
        root.join(args.corpus_dir)
    };
    let cli = if args.cli.is_absolute() {
        args.cli
    } else {
        root.join(args.cli)
    };
    let output = if args.output.is_absolute() {
        args.output
    } else {
        root.join(args.output)
    };
    let fonts = args
        .fonts
        .into_iter()
        .map(|p| if p.is_absolute() { p } else { root.join(p) })
        .collect();
    let fixtures = read_fixtures(&manifest, &corpus, &args.fixtures)?;
    run_visual(
        &fixtures,
        &VisualConfig {
            cli,
            output,
            format: args.format,
            fonts,
            options: MetricsOptions {
                channel_threshold: args.channel_threshold,
                ink_threshold: args.ink_threshold,
                tolerance: args.tolerance,
            },
            python: args.pdf_python,
        },
        &root,
    )?;
    Ok(())
}
