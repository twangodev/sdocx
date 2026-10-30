use lopdf::{Document, Object};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf};
fn sha(b:&[u8])->String { format!("{:x}",Sha256::digest(b)) }
fn count(pdf:&Document,key:&[u8])->usize {
 pdf.objects.values().filter_map(|o|match o { Object::Dictionary(d)=>Some(d),Object::Stream(s)=>Some(&s.dict),_=>None }).filter(|d|d.has(key)).count()
}
fn main()->Result<(),Box<dyn std::error::Error>> {
 let mut a=env::args_os().skip(1); let input=PathBuf::from(a.next().ok_or("usage: <svg> <pdf> <receipt> <width> <height>")?); let output=PathBuf::from(a.next().ok_or("pdf path")?); let receipt=PathBuf::from(a.next().ok_or("receipt path")?); let w:u32=a.next().ok_or("width")?.to_string_lossy().parse()?; let h:u32=a.next().ok_or("height")?.to_string_lossy().parse()?; if a.next().is_some(){return Err("extra arguments".into())}
 let source=fs::read(&input)?; let svg=String::from_utf8(source.clone())?; let page=sdocx::RenderedPage::new(0,w,h,svg); let bytes=sdocx::render_svg_pages_pdf(&[page],&sdocx::PdfOptions::default())?; fs::write(&output,&bytes)?; let pdf=Document::load_mem(&bytes)?;
 let v=json!({"input":input,"input_sha256":sha(&source),"output":output,"output_sha256":sha(&bytes),"output_bytes":bytes.len(),"image_xobjects":pdf.objects.values().filter(|o|match o {Object::Dictionary(d)=>d.get(b"Subtype").and_then(Object::as_name).ok()==Some(b"Image"),Object::Stream(s)=>s.dict.get(b"Subtype").and_then(Object::as_name).ok()==Some(b"Image"),_=>false}).count(),"soft_mask_objects":count(&pdf,b"SMask"),"shading_objects":count(&pdf,b"Shading"),"pages":pdf.get_pages().len()}); fs::write(receipt,serde_json::to_vec_pretty(&v)?)?; println!("{}",serde_json::to_string(&v)?); Ok(())
}
