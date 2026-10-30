use sdocx::*;
fn main() {
 let args:Vec<String>=std::env::args().collect(); let case=&args[1]; let out=std::path::Path::new(&args[2]); std::fs::create_dir_all(out).unwrap();
 let (coords,pressures,argb)=match case.as_str(){
 "short" => (vec![[25.25,28.5],[32.25,39.5],[42.25,30.5],[49.25,41.5],[55.25,31.5]],vec![0.1,0.9,0.3,0.8,0.2],0x8033cc55u32),
 "crossing" => (vec![[35.5,35.75],[86.0,85.5],[35.5,85.5],[86.0,35.75]],vec![0.2,0.9,0.4,1.0],0x8033cc55u32), _=>panic!()};
 let rendering:StrokeRendering=serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Marker3","advanced_settings":"5;","tool_type_raw":1,"properties":{"compressed":false,"replay_only":false,"stylus_channels":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":true,"alpha_lock":false,"binary_added":true,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false},"style":{"color_argb":argb,"pen_size":9.0,"stroke_type_raw":0,"trailing_data":[]}})).unwrap();
 let stroke=Stroke {rendering:Some(rendering),bbox:BoundingBox::default(),points:coords.iter().map(|p|Point{x:p[0],y:p[1]}).collect(),pressures,timestamps:vec![],tilts:vec![],orientations:vec![],color:Some(Color {r:(argb>>16) as u8,g:(argb>>8) as u8,b:argb as u8}),pen_width:9.0};
 let doc=Document {pages:vec![Page {uuid:"control".into(),width:128,height:128,content_bbox:BoundingBox::default(),background_color:None,template:None,background:Default::default(),objects:vec![stroke.into()]}],metadata:Default::default()};
 for (name,scale) in [("baseline",None),("profile",Some(Marker3OutputScale::One))] {let mut options=RenderOptions::default(); options.marker3_output_scale=scale; std::fs::write(out.join(format!("{name}.svg")),&render_document_svg(&doc,&options)[0].svg).unwrap();std::fs::write(out.join(format!("{name}.pdf")),render_document_pdf(&doc,&options,&Default::default()).unwrap()).unwrap();}
}
