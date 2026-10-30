mod asset;
use std::{
    env, fs,
    io::{self, Write},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let path = args.next().ok_or("missing source")?;
    let size = args.next().ok_or("missing size")?.parse()?;
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    let source = fs::read_to_string(path)?;
    let image = asset::rasterize(&source, size).map_err(|e| format!("{e:?}"))?;
    io::stdout().lock().write_all(&image)?;
    Ok(())
}
