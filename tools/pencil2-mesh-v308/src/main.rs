use sdocx::*;

fn main(){
 let mut style=serde_json::to_value(StrokeStyle::default()).unwrap();
 for (k,v) in [("pen_size",serde_json::json!(2.)),("color_argb",serde_json::json!(0x80336699u32)),("size_level",serde_json::json!(0)),("stroke_type_raw",serde_json::json!(0)),("color_type_raw",serde_json::json!(0))] {style[k]=v;}
 let props=serde_json::json!({"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false});
 let rendering:StrokeRendering=serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,"properties":props,"style":style})).unwrap();
 let s=Stroke{bbox:BoundingBox::default(),points:vec![Point{x:0.,y:0.},Point{x:10.,y:0.},Point{x:20.,y:0.},Point{x:30.,y:10.}],pressures:vec![0.25,0.5,0.75,1.],tilts:vec![0.4;4],orientations:vec![0.;4],timestamps:vec![10,20,30,40],color:Some(Color{r:0x33,g:0x66,b:0x99}),pen_width:2.,rendering:Some(rendering)};
 let dir=std::env::var("PENCIL198_DIR").unwrap();
 let scale:f64=std::env::var("SCALE").unwrap().parse().unwrap();
 let backdrop=std::env::var("BACKDROP").unwrap(); let page_size=192./scale;
 let mut objects=Vec::new();
 for idx in 0..2 {let mut c=s.clone(); let (tr,_grain)=if idx==0 {([6.,9.],[6.,9.])} else { c.points=vec![Point{x:0.,y:2.},Point{x:10.,y:2.},Point{x:20.,y:4.},Point{x:30.,y:12.}];c.pressures=vec![0.9,0.7,0.5,0.3];c.pen_width=10.;c.color=Some(Color{r:0xcc,g:0x33,b:0x44});let mut r=serde_json::to_value(c.rendering.as_ref().unwrap()).unwrap();r["style"]["pen_size"]=serde_json::json!(10.);r["style"]["color_argb"]=serde_json::json!(0xc0cc3344u32);c.rendering=Some(serde_json::from_value(r).unwrap());([12.,12.],[12.,13.999998])};
 for p in &mut c.points {p.x+=tr[0] as f64/scale;p.y=page_size-p.y-tr[1] as f64/scale;} let mut r=serde_json::to_value(c.rendering.as_ref().unwrap()).unwrap(); r["style"]["pen_size"]=serde_json::json!(c.pen_width); c.rendering=Some(serde_json::from_value(r).unwrap()); objects.push(c.into());
 let page=Page{uuid:"page".into(),width:page_size as u32,height:page_size as u32,content_bbox:BoundingBox::default(),background_color:Some(if backdrop=="white" {Color{r:255,g:255,b:255}} else {Color{r:64,g:128,b:191}}),template:None,background:Default::default(),objects:objects.clone()};let doc=Document{pages:vec![page],metadata:DocumentMetadata::default()};std::fs::write(format!("{dir}/generic{idx}.svg"),render_document_svg(&doc,&RenderOptions::default())[0].svg.as_bytes()).unwrap();
 let mut pdfopt=PdfOptions::default(); pdfopt.dpi=72.; std::fs::write(format!("{dir}/generic{idx}.pdf"),render_document_pdf(&doc,&RenderOptions::default(),&pdfopt).unwrap()).unwrap();
 }
}
