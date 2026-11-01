use sdocx::*;
fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::create_dir_all(&out).unwrap();
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/fountain-v14-scaled-source-rgba-native.json"
    ))
    .unwrap();
    let source: Stroke = serde_json::from_value(receipt["source_stroke"].clone()).unwrap();
    for (name, argb) in [("black", 0xff000000), ("blue", 0x8044aaee), ("orange", 0x00c04020)] {
    let mut stroke = source.clone();
    stroke.color = Some(Color {r: ((argb >> 16) & 255) as u8, g: ((argb >> 8) & 255) as u8, b: (argb & 255) as u8});
    stroke.rendering.as_mut().unwrap().style.color_argb = Some(argb);
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
        std::fs::write(out.join(format!("{name}_{scale}.svg")), svg.svg).unwrap();
        std::fs::write(
            out.join(format!("{name}_{scale}.pdf")),
            render_document_pdf(&doc, &options, &Default::default()).unwrap(),
        )
        .unwrap();
    }
    }
}
