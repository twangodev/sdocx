use sdocx::*;
fn main() {
 let tool3=std::env::args().nth(1).as_deref()==Some("tool3");
 let scene7=std::env::args().nth(1).as_deref()==Some("scene7");
 let mut stroke: Stroke = if scene7 { let v:serde_json::Value=serde_json::from_str(include_str!("../tests/assets/fountain-v14/scene-sources.json")).unwrap(); serde_json::from_value(v["strokes"].as_array().unwrap().iter().find(|x|x["index"]==7).unwrap()["stroke"].clone()).unwrap() } else {serde_json::from_str(include_str!("../tests/assets/fountain-v14/saved-source-29.json")).unwrap()};
 for p in &mut stroke.points { if scene7 { p.x-=148.24111938476562; p.y-=116.8331298828125; } else { p.x-=96.; p.y-=268.; } }
 if tool3 { stroke.points=vec![Point{x:16.,y:16.},Point{x:24.,y:16.},Point{x:24.,y:24.},Point{x:16.,y:24.}]; stroke.pressures=vec![0.2,0.4,0.6,0.8];stroke.timestamps=vec![0,10,20,30]; stroke.tilts=vec![0.;4];stroke.orientations=vec![0.;4]; }
 stroke.pen_width=if tool3 {4.} else {9.}; stroke.color=Some(Color{r:51,g:102,b:153});
 let r=stroke.rendering.as_mut().unwrap(); r.metadata=None; r.pen_name=Some("com.samsung.android.sdk.pen.pen.preload.InkPen2".into()); r.advanced_settings=None; r.tool_type_raw=if tool3 {3} else {2}; r.properties.generated=false; r.properties.millisecond_timestamps=false; r.properties.top_layer_pen=false; r.style=StrokeStyle::default(); r.style.color_argb=Some(0xff336699); r.style.pen_size=Some(stroke.pen_width); r.style.line_type_raw=Some(0); r.style.stroke_type_raw=Some(0);
 stroke.bbox=BoundingBox {x_min:0.,y_min:0.,x_max:64.,y_max:64.};
 let mut options=RenderOptions::default(); options.inkpen2_profile=Some(InkPen2RenderProfile {screen_width:if tool3 {1440} else {1920},screen_height:if tool3 {2560} else {1080},enhanced_antialias:false}); options.inkpen2_v1_output_scale=Some(if tool3 {InkPen2V1OutputScale::Double} else {InkPen2V1OutputScale::One});
 for (name,tolerance) in [("omitted",None),("zero",Some(0.)),("positive",Some(0.5)),("hundred",Some(100.))] {
  stroke.rendering.as_mut().unwrap().style.initial_tolerance=tolerance;
  let doc=Document {pages:vec![Page {uuid:"probe".into(),width:64,height:64,content_bbox:BoundingBox::default(),background_color:None,template:None,background:Default::default(),objects:vec![stroke.clone().into()]}],metadata:Default::default()};
  for (profile,opts) in [("profile",options.clone()),("fallback",RenderOptions::default())] {
   std::fs::write(format!("/tmp/inkpen-tolerance-probe/{}{name}-{profile}.svg",if tool3 {"tool3-"} else if scene7 {"scene7-"} else {""}),&render_document_svg(&doc,&opts)[0].svg).unwrap();
   std::fs::write(format!("/tmp/inkpen-tolerance-probe/{}{name}-{profile}.pdf",if tool3 {"tool3-"} else if scene7 {"scene7-"} else {""}),render_document_pdf(&doc,&opts,&Default::default()).unwrap()).unwrap();
  }
 }
}
