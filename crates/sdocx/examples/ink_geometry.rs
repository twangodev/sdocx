//! Numeric conformance adapter: original channels and prepared geometry.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("expected an SDOCX path")?;
    let doc = sdocx::parse(path)?;
    let strokes: Vec<_> = doc.pages.iter().flat_map(|p| &p.strokes).map(|stroke| {
        serde_json::json!({"stroke": stroke, "geometry": sdocx::prepare_stroke(stroke, false)})
    }).collect();
    serde_json::to_writer(std::io::stdout().lock(), &strokes)?;
    Ok(())
}
