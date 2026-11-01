//! Export one identified saved stroke through the public vector routes.
use sdocx::{Document, Page, RenderOptions};
use sha2::{Digest, Sha256};
fn main() {
    let path = std::env::args().nth(1).expect("original SDOCX input path");
    let out = std::path::PathBuf::from(std::env::args().nth(2).expect("private output directory"));
    let orig = sdocx::parse(path).unwrap();
    let stroke = orig.pages[0].strokes().next().unwrap().clone();
    let rendering = stroke.rendering.as_ref().expect("saved rendering metadata");
    let uuid = &rendering.metadata.as_ref().expect("saved identity").uuid;
    assert_eq!(format!("{:x}", Sha256::digest(uuid.as_bytes())), "3d1819b137be66c5d3955b112bceb22a8f46c48d7d3960e43118c15b478c40c5");
    assert_eq!(stroke.points.len(), 121);
    assert_eq!(stroke.pen_width.to_bits(), 1091690090);
    assert_eq!(rendering.style.color_argb, Some(4278387459));
    assert_eq!(rendering.style.advanced_pen_setting_id, Some(1));
    assert_eq!(rendering.pen_name.as_deref(), Some("com.samsung.android.sdk.pen.pen.preload.FountainPen"));
    let doc = Document { pages: vec![Page { uuid: "probe".into(), width: 208, height: 401, content_bbox: Default::default(), background_color: None, template: None, background: Default::default(), objects: vec![stroke.into()] }], metadata: Default::default() };
    let opt = RenderOptions::default();
    let page = sdocx::render_document_svg(&doc, &opt).remove(0);
    std::fs::write(out.join("selected.svg"), &page.svg).unwrap();
    std::fs::write(out.join("selected.svg.pdf"), sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap()).unwrap();
    std::fs::write(out.join("selected.direct.pdf"), sdocx::render_document_pdf(&doc, &opt, &Default::default()).unwrap()).unwrap();
}
