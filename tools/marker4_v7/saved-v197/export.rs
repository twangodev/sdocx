use std::io::Write;
use std::path::PathBuf;

use sdocx::{BoundingBox, Color, Document, DocumentMetadata, Page, Stroke};
fn main() {
    let output = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("expected a new output directory"),
    );
    assert!(
        std::env::args().nth(2).is_none(),
        "expected only an output directory"
    );
    std::fs::create_dir(&output).expect("output directory must not exist");
    let v: serde_json::Value =
        serde_json::from_slice(&std::fs::read("tools/marker4_v7/saved-v197/source.json").unwrap())
            .unwrap();
    let m: serde_json::Value =
        serde_json::from_slice(&std::fs::read("tools/marker4_v7/saved-v197/native.json").unwrap())
            .unwrap();
    let mut s: Stroke = serde_json::from_value(v["cases"][0]["stroke"].clone()).unwrap();
    let x = m["matrix_f32"][12].as_f64().unwrap();
    let y = 512. - m["matrix_f32"][13].as_f64().unwrap();
    for p in &mut s.points {
        p.x += x;
        p.y += y;
    }
    for role in ["public", "generic"] {
        let mut s = s.clone();
        if role == "generic" {
            s.rendering.as_mut().unwrap().advanced_settings = Some("9;".into());
        }
        let doc = Document {
            pages: vec![Page {
                uuid: "marker".into(),
                width: 1024,
                height: 512,
                content_bbox: BoundingBox::default(),
                background_color: Some(Color {
                    r: 255,
                    g: 255,
                    b: 255,
                }),
                template: None,
                background: Default::default(),
                objects: vec![s.into()],
            }],
            metadata: DocumentMetadata::default(),
        };
        let svg = sdocx::render_page_svg(&doc, 0, &Default::default())
            .unwrap()
            .svg;
        let pdf =
            sdocx::render_document_pdf(&doc, &Default::default(), &Default::default()).unwrap();
        for (ext, data) in [("svg", svg.as_bytes()), ("pdf", pdf.as_slice())] {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output.join(format!("{role}.{ext}")))
                .unwrap();
            file.write_all(data).unwrap();
        }
    }
}
