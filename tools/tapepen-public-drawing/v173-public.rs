use sdocx::{BoundingBox, Color, Document, DocumentMetadata, Page, Point, Stroke};
fn main() {
    let fixture: serde_json::Value = serde_json::from_slice(&std::fs::read("conformance/marker4-v7.json").unwrap()).unwrap();
    let mut stroke: Stroke = serde_json::from_value(fixture["stroke"].clone()).unwrap();
    stroke.points = vec![Point{x:77.,y:59.},Point{x:89.,y:52.},Point{x:101.,y:65.},Point{x:109.,y:57.}];
    stroke.pressures = vec![0.1,0.35,0.7,1.];
    stroke.tilts = vec![0.,0.2,0.4,0.6];
    stroke.orientations = vec![0.,0.25,0.5,0.75];
    stroke.timestamps = vec![0,8,19,37];
    stroke.pen_width=12.;
    stroke.color=Some(Color{r:0xd7,g:0x44,b:0x33});
    let rendering=stroke.rendering.as_mut().unwrap();
    rendering.metadata=Some(Box::new(serde_json::from_value(serde_json::json!({"format_version":0,"uuid":"tape","modified_time_raw":0,"bbox":{"x_min":77.,"y_min":52.,"x_max":109.,"y_max":65.},"replay_timestamp_raw":0,"resize_mode_raw":0,"rotatable":true,"selectable":true,"movable":true,"visible":true,"replayable":false,"out_of_canvas_enabled":false,"template":false,"flip_enabled":false,"float_drawn_rect":false,"locked":false,"removable":true,"rotation_degrees":30.,"property_mask":[],"field_mask":[],"fixed_trailing_data":[],"flexible_trailing_data":[]})).unwrap()));
    rendering.style=Default::default();
    rendering.pen_name=Some("com.samsung.android.sdk.pen.pen.preload.TapePen".into());
    rendering.advanced_settings=Some("".into());
    rendering.tool_type_raw=2;
    rendering.style.pen_size=Some(12.);
    rendering.style.color_argb=Some(0xc0d74433);
    rendering.style.color_type_raw=Some(2);
    rendering.style.pattern_index=Some(1);
    rendering.style.pattern_scale=Some(1.);
    let doc=Document{pages:vec![Page{uuid:"tape".into(),width:256,height:256,content_bbox:BoundingBox::default(),background_color:Some(Color{r:255,g:255,b:255}),template:None,background:Default::default(),objects:vec![stroke.into()]}],metadata:DocumentMetadata::default()};
    let svg=sdocx::render_page_svg(&doc,0,&Default::default()).unwrap().svg;
    let pdf=sdocx::render_document_pdf(&doc,&Default::default(),&Default::default()).unwrap();
    let out = std::env::args().nth(1).expect("output directory");
    std::fs::write(format!("{out}/public.svg"),svg).unwrap();
    std::fs::write(format!("{out}/public.pdf"),pdf).unwrap();
}
