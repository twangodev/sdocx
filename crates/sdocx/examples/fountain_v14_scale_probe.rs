use sdocx::*;
fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::create_dir_all(&out).unwrap();
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/fountain-v14-scaled-source-rgba-native.json"
    ))
    .unwrap();
    let stroke: Stroke = serde_json::from_value(receipt["source_stroke"].clone()).unwrap();
    let doc = Document {
        pages: vec![Page {
            uuid: String::new(),
            width: 1024,
            height: 1536,
            content_bbox: stroke.bbox,
            background_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
            }),
            template: None,
            background: Default::default(),
            objects: vec![stroke.into()],
        }],
        metadata: Default::default(),
    };
    for scale in [1, 2] {
        let mut options = RenderOptions::default();
        options.fountain_v14_output_scale_profile = Some(FountainV14OutputScaleProfile {
            pixels_per_page_unit: scale as f32,
        });
        let svg = render_document_svg(&doc, &options).remove(0);
        std::fs::write(out.join(format!("{scale}.svg")), svg.svg).unwrap();
        std::fs::write(
            out.join(format!("{scale}.pdf")),
            render_document_pdf(&doc, &options, &Default::default()).unwrap(),
        )
        .unwrap();
    }
}
