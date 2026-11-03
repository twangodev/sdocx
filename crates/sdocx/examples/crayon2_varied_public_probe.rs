use sdocx::{
    BoundingBox, Color, Document, Page, Point, RenderOptions, Stroke, StrokeRendering, StrokeStyle,
};
use std::path::PathBuf;
fn main() {
    let dir = PathBuf::from(std::env::args_os().nth(1).expect("output directory"));
    std::fs::create_dir(&dir).expect("create new output directory");
    let mut style = serde_json::to_value(StrokeStyle::default()).unwrap();
    style["color_argb"] = serde_json::json!(0x80336699u32);
    style["pen_size"] = serde_json::json!(10.0);
    let stroke = Stroke {
        rendering: Some(serde_json::from_value::<StrokeRendering>(serde_json::json!({
            "metadata": null, "pen_name": "com.samsung.android.sdk.pen.pen.preload.Crayon2", "advanced_settings": "2;", "tool_type_raw": 2,
            "properties": {"compressed":true,"replay_only":false,"stylus_channels":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":true,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false},
            "style": style
        })).unwrap()),
        bbox: BoundingBox::default(),
        points: vec![Point{x:64.0,y:64.0}, Point{x:72.0,y:70.0}, Point{x:82.0,y:60.0}],
        pressures: vec![0.3,0.6,1.0], timestamps: vec![100,110,120], tilts: vec![0.0;3], orientations: vec![0.0;3], color: Some(Color{r:51,g:102,b:153}), pen_width: 10.0,
    };
    let document = Document {
        pages: vec![Page {
            uuid: "crayon2-varied".into(),
            width: 256,
            height: 256,
            content_bbox: BoundingBox::default(),
            background_color: Some(Color {
                r: 64,
                g: 128,
                b: 192,
            }),
            template: None,
            background: Default::default(),
            objects: vec![stroke.into()],
        }],
        metadata: Default::default(),
    };
    let options = RenderOptions::default();
    let page = sdocx::render_document_svg(&document, &options).remove(0);
    std::fs::write(dir.join("public.svg"), &page.svg).unwrap();
    std::fs::write(
        dir.join("public.pdf"),
        sdocx::render_document_pdf(&document, &options, &Default::default()).unwrap(),
    )
    .unwrap();
}
