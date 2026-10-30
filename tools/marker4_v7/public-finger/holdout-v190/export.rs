use sha2::{Digest, Sha256};
use sdocx::{BoundingBox,Color,Document,DocumentMetadata,Page,Stroke};
fn main() {
 let read_verified = |path: &str, expected: &str| { let bytes=std::fs::read(path).unwrap(); assert_eq!(format!("{:x}", Sha256::digest(&bytes)), expected, "input hash mismatch: {path}"); bytes };
 let base: serde_json::Value=serde_json::from_slice(&read_verified("conformance/marker4-v7.json", "d37d78cc65d889ca56c48c40b902402a546f7f8c90a33ed96d61e7a227a68920")).unwrap();
 let native: serde_json::Value=serde_json::from_slice(&read_verified("tools/marker4_v7/public-finger/wide-extra/geometry.json", "8b8a8a684f11560a9587631ca089a4a29eec0752ee5eae6935daacfe2a8016a7")).unwrap();
 let args: Vec<String> = std::env::args().collect();
 assert_eq!(args.len(), 5, "usage: export CAPTURE.json CAPTURE_SHA256 fallback|public OUTPUT_DIR");
 let out = std::path::Path::new(&args[4]); assert!(out.is_dir());
 let role=&args[3]; assert!(role == "fallback" || role == "public", "invalid export role");
 let meta: serde_json::Value=serde_json::from_slice(&read_verified(&args[1], &args[2])).unwrap();
 let scale=meta["scale"].as_f64().unwrap(); assert!(scale >= 1.0);
 assert_eq!(meta["target"], serde_json::json!([1024,512]), "unexpected target");
 assert_eq!(meta["source_json_sha256"], "8b8a8a684f11560a9587631ca089a4a29eec0752ee5eae6935daacfe2a8016a7");
 let c=native["cases"].as_array().unwrap().iter().find(|v|v["name"]==meta["case"]).unwrap();
 let cx=meta["frame_center_doc"][0].as_f64().unwrap(); let cy=meta["frame_center_doc"][1].as_f64().unwrap(); let px=meta["phase"][0].as_f64().unwrap()/scale; let py=meta["phase"][1].as_f64().unwrap()/scale;
 let mut s: Stroke=serde_json::from_value(base["stroke"].clone()).unwrap();
 s.points.clear(); s.pressures.clear(); s.tilts.clear(); s.timestamps.clear();
 for (i,p) in c["samples"].as_array().unwrap().iter().enumerate() {
  s.points.push(sdocx::Point{x:p[0].as_f64().unwrap()+512.0-cx+px,y:p[1].as_f64().unwrap()+256.0-cy+py});
  s.pressures.push(p[2].as_f64().unwrap()); s.tilts.push(p[3].as_f64().unwrap()); s.timestamps.push(i as i64*8);
 }
 let argb=u32::from_str_radix(meta["set_pen_argb"].as_str().unwrap().trim_start_matches("0x"),16).unwrap(); s.color=Some(Color{r:(argb >> 16) as u8,g:(argb >> 8) as u8,b:argb as u8}); s.pen_width=c["size"].as_f64().unwrap() as f32; let r=s.rendering.as_mut().unwrap(); r.tool_type_raw=0; r.style.pen_size=Some(s.pen_width); r.style.color_argb=Some(argb);
 let doc=Document{pages:vec![Page{uuid:"marker".into(),width:1024,height:512,content_bbox:BoundingBox::default(),background_color:Some(Color{r:255,g:255,b:255}),template:None,background:Default::default(),objects:vec![s.into()]}],metadata:DocumentMetadata::default()};
 let mut options=sdocx::RenderOptions::default(); if role == "public" { options.marker4_v7_finger_output_scale=Some(scale as u32); }
 let svg=sdocx::render_page_svg(&doc,0,&options).unwrap().svg;
 let pdf=sdocx::render_document_pdf(&doc,&options,&Default::default()).unwrap();
 let write_new = |name: &str, bytes: &[u8]| { use std::io::Write; let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(out.join(name)).unwrap(); f.write_all(bytes).unwrap(); };
 write_new(&format!("marker4-{role}.svg"), svg.as_bytes());
 write_new(&format!("marker4-{role}.pdf"), &pdf);
 let manifest=serde_json::json!({"capture_sha256":args[2], "role":role, "svg_sha256":format!("{:x}",Sha256::digest(svg.as_bytes())), "pdf_sha256":format!("{:x}",Sha256::digest(&pdf))});
 write_new(&format!("marker4-{role}.manifest.json"), manifest.to_string().as_bytes());
}
