//! Software coverage adapter for native shader comparisons.
//! Input: JSON array of { stroke, viewport }, output: cropped byte masks.
use base64::Engine as _;

#[derive(serde::Deserialize)]
struct Input {
    stroke: sdocx::Stroke,
    viewport: sdocx::InkViewport,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("expected input JSON")?;
    let inputs: Vec<Input> = serde_json::from_reader(std::fs::File::open(path)?)?;
    let mut output = Vec::with_capacity(inputs.len());
    for input in inputs {
        let paint = sdocx::prepare_stroke(&input.stroke, false);
        let mask = sdocx::rasterize_fountain(&paint, input.viewport)?;
        output.push(serde_json::json!({
            "viewport": input.viewport,
            "mask": mask.map(|m| serde_json::json!({
                "x": m.x, "y": m.y, "width": m.width, "height": m.height,
                "alpha": base64::engine::general_purpose::STANDARD.encode(m.alpha),
            }))
        }));
    }
    serde_json::to_writer(std::io::stdout().lock(), &output)?;
    Ok(())
}
