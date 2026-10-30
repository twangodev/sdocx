use sdocx::*;
use std::io::Write;

fn write_new(path: &str, bytes: &[u8]) {
    std::fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap().write_all(bytes).unwrap();
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let size: f64 = args[1].parse().unwrap();
    let f = |x: f32| x.to_bits();
    let bbox = serde_json::json!({"x_min":40.,"y_min":40.,"x_max":40.+size,"y_max":40.+size});
    let metadata = serde_json::json!({"format_version":5500,"uuid":"alpha-radial","modified_time_raw":0,"bbox":bbox,"replay_timestamp_raw":0,"resize_mode_raw":0,"rotatable":false,"selectable":false,"movable":false,"visible":true,"replayable":false,"out_of_canvas_enabled":false,"template":false,"flip_enabled":false,"float_drawn_rect":false,"locked":false,"removable":true,"rotation_degrees":null,"property_mask":[],"field_mask":[],"fixed_trailing_data":[],"flexible_trailing_data":[]});
    let mut style = serde_json::to_value(ShapeStyle::default()).unwrap();
    style["width"] = size.into();
    style["paint"] = serde_json::json!("Gradient");
    style["paint_source"] = serde_json::json!({"Color":{"property_flags":0,"outline_color_type":1,"solid_argb":0,"gradient_type":1,"linear_angle":0,"position":[f(0.5),f(0.5)],"stops":[{"argb":4278190080u32,"position":f(0.25)},{"argb":0,"position":f(0.75)}],"trailing_data":[]}});
    let line: NativeLine = serde_json::from_value(serde_json::json!({"metadata":metadata,"base_source":null,"line_type":0,"routing":0,"control_points":[],"begin":[40.,40.+size/2.],"end":[40.+size,40.+size/2.],"geometry_bbox":bbox,"reference_bbox":bbox,"raw_setting":0,"pen_name_id":null,"legacy_pen_source":null,"pen_settings_id":null,"path_data":[],"style":style})).unwrap();
    let page = Page { uuid:"page".into(), width:192, height:192, content_bbox:BoundingBox::default(), background_color:Some(Color{r:255,g:255,b:255}), template:None, background:Default::default(), objects:vec![PageElement::Line(line).into()] };
    let doc = Document { pages:vec![page], metadata:DocumentMetadata::default() };
    let options = RenderOptions::default();
    let mut pdf_options = PdfOptions::default();
    pdf_options.dpi = 72.;
    let pages = render_document_svg(&doc, &options);
    assert!(pages[0].paint_diagnostics.is_empty());
    write_new(&args[2], pages[0].svg.as_bytes());
    write_new(&args[3], &render_document_pdf(&doc, &options, &pdf_options).unwrap());
}
