use sdocx::*;
use std::fmt::Write;
fn main(){
 let mut style=serde_json::to_value(StrokeStyle::default()).unwrap();
 for (k,v) in [("pen_size",serde_json::json!(2.)),("color_argb",serde_json::json!(0x80336699u32)),("size_level",serde_json::json!(0)),("stroke_type_raw",serde_json::json!(0)),("color_type_raw",serde_json::json!(0))] {style[k]=v;}
 let props=serde_json::json!({"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false});
 let rendering:StrokeRendering=serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,"properties":props,"style":style})).unwrap();
 let s=Stroke{bbox:BoundingBox::default(),points:vec![Point{x:0.,y:0.},Point{x:10.,y:0.},Point{x:20.,y:0.},Point{x:30.,y:10.}],pressures:vec![0.25,0.5,0.75,1.],tilts:vec![0.4;4],orientations:vec![0.;4],timestamps:vec![10,20,30,40],color:Some(Color{r:0x33,g:0x66,b:0x99}),pen_width:2.,rendering:Some(rendering)};
 let dir=std::env::var("PENCIL198_DIR").unwrap();
 let mut svg=String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"192\" height=\"192\"><rect width=\"192\" height=\"192\" fill=\"rgb(64,128,191)\"/>");
 let mut objects=Vec::new();
 let mut parts = Vec::new();
 for idx in 0..2 {let mut c=s.clone(); let (tr,grain)=if idx==0 {([6.,9.],[6.,9.])} else { c.points=vec![Point{x:0.,y:2.},Point{x:10.,y:2.},Point{x:20.,y:4.},Point{x:30.,y:12.}];c.pressures=vec![0.9,0.7,0.5,0.3];c.pen_width=10.;c.color=Some(Color{r:0xcc,g:0x33,b:0x44});let mut r=serde_json::to_value(c.rendering.as_ref().unwrap()).unwrap();r["style"]["pen_size"]=serde_json::json!(10.);r["style"]["color_argb"]=serde_json::json!(0xc0cc3344u32);c.rendering=Some(serde_json::from_value(r).unwrap());([12.,12.],[12.,13.999998])};
 let start = svg.len();
 let frames=pencil198(&c,tr,grain);eprintln!("{idx} stamps {}",frames.len());
 assert!(frames.len() <= 84 && frames.iter().all(|f| f.len() == 192*192));
 for frame in frames {for (i,p) in frame.iter().enumerate(){let a=p[3] as f64;if a>0. {assert!(svg.len() < 1_999_000);write!(svg,"<rect x=\"{}\" y=\"{}\" width=\"1\" height=\"1\" fill=\"rgb({},{},{})\" fill-opacity=\"{}\"/>",i%192,i/192, (p[0] as f64*255./a).round().min(255.),(p[1] as f64*255./a).round().min(255.),(p[2] as f64*255./a).round().min(255.),a/255.).unwrap();}}}
 parts.push(svg[start..].to_owned());
 assert!(svg.len() < 2_000_000); let path=format!("{dir}/vec{idx}.svg"); let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap(); std::io::Write::write_all(&mut file,format!("{svg}</svg>").as_bytes()).unwrap();
 for p in &mut c.points {p.x+=tr[0] as f64;p.y=192.-p.y-tr[1] as f64;} objects.push(c.into());
 let page=Page{uuid:"page".into(),width:192,height:192,content_bbox:BoundingBox::default(),background_color:Some(Color{r:64,g:128,b:191}),template:None,background:Default::default(),objects:objects.clone()};let doc=Document{pages:vec![page],metadata:DocumentMetadata::default()};let path=format!("{dir}/generic{idx}.svg"); let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap(); std::io::Write::write_all(&mut file,render_document_svg(&doc,&RenderOptions::default())[0].svg.as_bytes()).unwrap();
 }
 let prefix = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"192\" height=\"192\"><rect width=\"192\" height=\"192\" fill=\"rgb(64,128,191)\"/>";
 let count: usize = parts.iter().map(|p| p.matches("<rect ").count()).sum();
 let size = prefix.len() + parts.iter().map(String::len).sum::<usize>() + 6;
 let fallback = b"unchanged fallback scene".to_vec();
 for (rect_limit, byte_limit, accept) in [(count-1,size,false),(count,size-1,false),(count,size,true),(count+1,size+1,true)] {
     let mut destination = fallback.clone();
     if let Some(scene) = stage_scene(prefix, &parts, rect_limit, byte_limit) { destination = scene; }
     if accept { assert_eq!(destination.len(), size); assert_ne!(destination, fallback); }
     else { assert_eq!(destination, fallback); }
 }
 let empty_size = prefix.len() + 6;
 assert!(stage_scene(prefix, &[], 0, empty_size - 1).is_none());
 assert_eq!(stage_scene(prefix, &[], 0, empty_size).unwrap().len(), empty_size);
 assert_eq!(stage_scene(prefix, &[], 0, empty_size + 1).unwrap().len(), empty_size);
 let accepted = stage_scene(prefix, &parts, count, size).unwrap();
 let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(format!("{dir}/rust-accepted.svg")).unwrap();
 std::io::Write::write_all(&mut file, &accepted).unwrap();
 eprintln!("Rust aggregate scene: {count} rectangles, {size} bytes; boundary and fallback checks passed");
}

fn stage_scene(prefix: &str, parts: &[String], rect_limit: usize, byte_limit: usize) -> Option<Vec<u8>> {
 let mut count = 0usize;
 let mut size = prefix.len().checked_add(6)?;
 if size > byte_limit { return None; }
 for part in parts {
     count = count.checked_add(part.matches("<rect ").count())?;
     size = size.checked_add(part.len())?;
     if count > rect_limit || size > byte_limit { return None; }
 }
 let mut out = Vec::with_capacity(size);
 out.extend_from_slice(prefix.as_bytes());
 for part in parts { out.extend_from_slice(part.as_bytes()); }
 out.extend_from_slice(b"</svg>");
 Some(out)
}
