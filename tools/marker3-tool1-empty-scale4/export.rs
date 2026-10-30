use sdocx::*;
fn main() {
 let args:Vec<String>=std::env::args().collect(); let case=&args[1]; let out=std::path::Path::new(&args[2]); std::fs::create_dir_all(out).unwrap();
 let (mut coords,pressures,argb)=match case.as_str(){
 "short4" | "short6" | "short" => (vec![[25.3125,28.5625],[32.3125,39.5625],[42.3125,30.5625],[49.3125,41.5625],[55.3125,31.5625]],vec![0.1,0.9,0.3,0.8,0.2],if case=="short4" || case=="short" {0x8033cc55} else {0xffd14a87}),
 "fractional" => (vec![[25.3125,28.5625],[67.8125,46.1875]],vec![0.2,0.9],0x8033cc55u32),
 "crossing" => (vec![[35.3125,35.5625],[85.8125,85.3125],[35.3125,85.3125],[85.8125,35.5625]],vec![0.2,0.9,0.4,1.0],0xffd14a87u32), _=>panic!()};
 let shift = if case == "short4" { -0.0625 } else if case == "short6" { 0.0625 } else { 0.0 };
 for p in &mut coords { p[0] += shift; p[1] += shift; }
 let width=if case=="short4" {4.0} else if case=="short6" {6.0} else {8.0};
 let mut rendering:StrokeRendering=serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.StraightMarker","advanced_settings":"","tool_type_raw":1,"properties":{"compressed":false,"replay_only":false,"stylus_channels":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":true,"alpha_lock":false,"binary_added":true,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false},"style":{"color_argb":argb,"pen_size":width,"stroke_type_raw":0,"trailing_data":[]}})).unwrap();
 if std::env::var_os("SDOCX_FALLBACK").is_some() { rendering.pen_name = Some("generic-fallback".into()); }
 let stroke=Stroke {rendering:Some(rendering),bbox:BoundingBox::default(),points:coords.iter().map(|p|Point{x:p[0],y:p[1]}).collect(),pressures,timestamps:vec![],tilts:vec![],orientations:vec![],color:Some(Color {r:(argb>>16) as u8,g:(argb>>8) as u8,b:argb as u8}),pen_width:width};
 let doc=Document {pages:vec![Page {uuid:"control".into(),width:128,height:128,content_bbox:BoundingBox::default(),background_color:None,template:None,background:Default::default(),objects:vec![stroke.into()]}],metadata:Default::default()};
 for (name,scale) in [("baseline",None),("profile",Some(Marker3OutputScale::Four))] {let mut options=RenderOptions::default(); options.marker3_output_scale=scale; std::fs::write(out.join(format!("{name}.svg")),&render_document_svg(&doc,&options)[0].svg).unwrap();std::fs::write(out.join(format!("{name}.pdf")),render_document_pdf(&doc,&options,&Default::default()).unwrap()).unwrap();}
}
