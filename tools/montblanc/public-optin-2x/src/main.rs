use sdocx::{
    Color, Document, DocumentMetadata, MontblancRenderProfile, PageObject, PdfOptions, Point,
    RenderOptions,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).unwrap();
    let doc = sdocx::parse("crates/sdocx/tests/assets/legacy-marker/synthetic-serialized.sdocx")?;
    for (name, coords, pressure, size) in [
        (
            "baseline",
            vec![[0., 0.], [10., 0.], [20., 2.]],
            vec![0.3, 0.6, 1.],
            2.,
        ),
        (
            "pressure",
            vec![[0., 0.], [10., 0.], [20., 2.]],
            vec![0.3, 0.85, 0.9],
            2.,
        ),
        (
            "size",
            vec![[0., 0.], [10., 0.], [20., 2.]],
            vec![0.3, 0.6, 1.],
            1.5,
        ),
        (
            "far_endpoint",
            vec![[0., 0.], [10., 0.], [30., 2.]],
            vec![0.3, 0.6, 1.],
            2.,
        ),
        (
            "four",
            vec![[1., 3.], [10., 10.], [20., 2.], [30., 0.]],
            vec![0.3, 0.6, 1., 0.7],
            2.,
        ),
        (
            "five",
            vec![[0., 0.], [5., 8.], [11., -1.], [15., 6.], [22., -3.]],
            vec![0.3, 0.6, 1., 0.7, 0.5],
            2.,
        ),
    ] {
        let mut stroke = doc
            .pages
            .iter()
            .flat_map(|p| p.strokes())
            .next()
            .unwrap()
            .clone();
        stroke.points = coords
            .into_iter()
            .map(|p| Point {
                x: 64. + p[0],
                y: 64. - p[1],
            })
            .collect();
        stroke.pressures = pressure.into_iter().map(|x| f64::from(x as f32)).collect();
        stroke.pen_width = size;
        stroke.color = Some(Color {
            r: 51,
            g: 102,
            b: 153,
        });
        stroke.timestamps.clear();
        stroke.tilts.clear();
        stroke.orientations.clear();
        let r = stroke.rendering.as_mut().unwrap();
        r.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.MontblancFountainPen".into());
        r.style.pen_size = Some(size);
        r.style.color_argb = Some(0xff336699);
        r.properties.stylus_channels = false;
        r.properties.millisecond_timestamps = false;
        let mut page = doc.pages[0].clone();
        page.objects = vec![PageObject::from(stroke)];
        page.width = 128;
        page.height = 128;
        page.template = None;
        page.background = Default::default();
        page.background_color = None;
        let doc = Document {
            pages: vec![page],
            metadata: DocumentMetadata::default(),
        };
        for (label, profile) in [
            ("candidate", Some(MontblancRenderProfile::HostUnitScale)),
            ("aware", None),
            ("fallback", None),
        ] {
            let mut opts = RenderOptions::default();
            opts.montblanc_profile = profile;
            opts.montblanc_two_pixels_per_page_unit = label == "aware";
            std::fs::write(
                format!("{dir}/{name}-{label}.svg"),
                &sdocx::render_document_svg(&doc, &opts)[0].svg,
            )?;
            std::fs::write(
                format!("{dir}/{name}-{label}.pdf"),
                sdocx::render_document_pdf(&doc, &opts, &PdfOptions::default())?,
            )?;
        }
    }
    Ok(())
}
