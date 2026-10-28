use sdocx::{Color, Document, DocumentMetadata, PageObject, PdfOptions, Point, RenderOptions};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args().nth(1).ok_or("missing output directory")?;
    let doc = sdocx::parse("tmp/legacy-marker-source-controls/serialized-marker.sdocx")?;
    for (name, end, pressure, width) in [
        ("baseline", 20., 0.6, 2.),
        ("pressure", 20., 0.85, 2.),
        ("size", 20., 0.6, 1.5),
        ("far_endpoint", 30., 0.6, 2.),
    ] {
        let mut stroke = doc
            .pages
            .iter()
            .flat_map(|p| p.strokes())
            .next()
            .unwrap()
            .clone();
        stroke.points = vec![
            Point { x: 128., y: 128. },
            Point { x: 138., y: 128. },
            Point {
                x: 128. + end,
                y: 126.,
            },
        ];
        stroke.pressures = vec![0.3, pressure, if name == "pressure" { 0.9 } else { 1. }];
        stroke.pen_width = width;
        stroke.color = Some(Color {
            r: 51,
            g: 102,
            b: 153,
        });
        let r = stroke.rendering.as_mut().unwrap();
        r.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.MontblancFountainPen".into());
        r.style.pen_size = Some(width);
        r.style.color_argb = Some(0xff336699);
        let mut page = doc.pages[0].clone();
        page.objects = vec![PageObject::from(stroke)];
        page.width = 256;
        page.height = 256;
        page.template = None;
        page.background = Default::default();
        page.background_color = None;
        let doc = Document {
            pages: vec![page],
            metadata: DocumentMetadata::default(),
        };
        let opts = RenderOptions::default();
        std::fs::write(
            format!("{output}/{name}.svg"),
            &sdocx::render_document_svg(&doc, &opts)[0].svg,
        )?;
        std::fs::write(
            format!("{output}/{name}.pdf"),
            sdocx::render_document_pdf(&doc, &opts, &PdfOptions::default())?,
        )?;
    }
    Ok(())
}
