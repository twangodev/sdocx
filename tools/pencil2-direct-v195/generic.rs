use sdocx::*;
fn main() {
 let args:Vec<_>=std::env::args().collect(); let w:f32=args[1].parse().unwrap();
 let mut style=serde_json::to_value(StrokeStyle::default()).unwrap();
 style["pen_size"]=w.into();style["color_argb"]=0xff000000u32.into();style["size_level"]=0.into();style["stroke_type_raw"]=0.into();style["color_type_raw"]=0.into();
 let props=serde_json::json!({"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false});
 let rendering:StrokeRendering=serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,"properties":props,"style":style})).unwrap();
 let stroke=Stroke {rendering:Some(rendering),bbox:BoundingBox::default(),points:vec![Point{x:0.,y:192.},Point{x:10.,y:192.},Point{x:20.,y:192.},Point{x:30.,y:182.}],pressures:if args.get(4).is_some_and(|s| s=="ramp") {vec![0.25,0.5,0.75,1.0]} else {vec![0.5;4]},timestamps:vec![10,20,30,40],tilts:vec![0.,0.25,0.5,0.75],orientations:vec![0.,0.25,0.5,0.75],color:Some(Color{r:0,g:0,b:0}),pen_width:w};
 let page=Page{uuid:"page".into(),width:192,height:192,content_bbox:BoundingBox::default(),background_color:Some(Color{r:255,g:255,b:255}),template:None,background:Default::default(),objects:vec![stroke.into()]};
 let doc=Document{pages:vec![page],metadata:DocumentMetadata::default()}; let opt=RenderOptions::default();
 let mut pdfopt=PdfOptions::default(); pdfopt.dpi=72.0; use std::io::Write; let mut svg=std::fs::OpenOptions::new().write(true).create_new(true).open(&args[2]).unwrap(); svg.write_all(render_document_svg(&doc,&opt)[0].svg.as_bytes()).unwrap(); let mut pdf=std::fs::OpenOptions::new().write(true).create_new(true).open(&args[3]).unwrap(); pdf.write_all(&render_document_pdf(&doc,&opt,&pdfopt).unwrap()).unwrap();
}
