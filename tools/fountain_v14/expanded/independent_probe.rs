//! Diagnostic paired vector exports for the original native scene capture.
use sdocx::{Document, FountainV14OutputScaleProfile, Page, RenderOptions, Stroke};
use serde_json::Value;
use std::{collections::HashMap, path::PathBuf};

fn main() {
    let dir = PathBuf::from(std::env::args_os().nth(1).expect("output directory"));
    std::fs::create_dir_all(&dir).unwrap();
    let inputs: Value = serde_json::from_str(include_str!(
        "../../../conformance/fountain-v14-saved-scene-inputs.json"
    ))
    .unwrap();
    let sources: HashMap<u64, Stroke> = inputs["strokes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v["index"].as_u64().unwrap(),
                serde_json::from_value(v["stroke"].clone()).unwrap(),
            )
        })
        .collect();
    let sequence = [3, 6, 7, 18, 19, 28, 29, 30];
    for mode in ["canonical", "scale"] {
        let mut options = RenderOptions::default();
        if mode == "scale" {
            options.fountain_v14_output_scale_profile = Some(FountainV14OutputScaleProfile {
                pixels_per_page_unit: 1.0,
            });
        }
        for step in 0..sequence.len() {
            let document = Document {
                pages: vec![Page {
                    uuid: "native-scene-vector-probe".into(),
                    width: 208,
                    height: 328,
                    content_bbox: Default::default(),
                    background_color: Some(sdocx::Color {
                        r: 255,
                        g: 255,
                        b: 255,
                    }),
                    template: None,
                    background: Default::default(),
                    objects: sequence[step..=step]
                        .iter()
                        .map(|id| sources[id].clone().into())
                        .collect(),
                }],
                metadata: Default::default(),
            };
            let page = sdocx::render_document_svg(&document, &options).remove(0);
            let name = format!("{mode}_{step}");
            std::fs::write(dir.join(format!("{name}.svg")), &page.svg).unwrap();
            std::fs::write(
                dir.join(format!("{name}.svg.pdf")),
                sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap(),
            )
            .unwrap();
            std::fs::write(
                dir.join(format!("{name}.direct.pdf")),
                sdocx::render_document_pdf(&document, &options, &Default::default()).unwrap(),
            )
            .unwrap();
        }
    }
}
