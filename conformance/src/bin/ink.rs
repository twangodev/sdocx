use std::fs::File;
use std::io::{self, BufReader};
use std::path::PathBuf;

use clap::Parser;
use sdocx_conformance::image::RgbImage;
use sdocx_conformance::ink::{InkOptions, compare};

#[derive(Debug, Parser)]
#[command(about = "Compare rendered ink inside selected prepared-stroke bounds")]
struct Args {
    geometry: PathBuf,
    reference: PathBuf,
    candidate: PathBuf,
    #[arg(long, help = "prepared pen profile, for example Marker4")]
    profile: Option<String>,
    #[arg(long, help = "exact saved settings, for example 7;")]
    settings: Option<String>,
    #[arg(long, num_args = 4, value_names = ["LEFT", "TOP", "RIGHT", "BOTTOM"])]
    region: Option<Vec<i64>>,
    #[arg(long, help = "isolate colored ink by max RGB minus min RGB")]
    chroma_threshold: Option<u8>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ink: {error}");
        std::process::exit(2);
    }
}

fn run() -> sdocx_conformance::Result<()> {
    let args = Args::parse();
    let geometry: serde_json::Value =
        serde_json::from_reader(BufReader::new(File::open(args.geometry)?))?;
    let rows = geometry.as_array().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "geometry JSON must be an array")
    })?;
    let reference = RgbImage::from_png(args.reference)?;
    let candidate = RgbImage::from_png(args.candidate)?;
    let region = args
        .region
        .map(|values| {
            values.try_into().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "region requires four coordinates",
                )
            })
        })
        .transpose()?;
    let options = InkOptions {
        profile: args.profile,
        settings: args.settings,
        region,
        chroma_threshold: args.chroma_threshold,
    };
    let output = compare(rows, &reference, &candidate, &options)?;
    serde_json::to_writer_pretty(io::stdout().lock(), &output)?;
    println!();
    Ok(())
}
