use sdocx::{
    Color, Document, DocumentMetadata, MontblancRenderProfile, PageObject, PdfOptions, Point,
    RenderOptions,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).unwrap();
    let double = std::env::var_os("SDOCX_MONTBLANC_DOUBLE").is_some();
    let origin = if double { 64. } else { 128. };
    let doc = sdocx::parse("crates/sdocx/tests/assets/legacy-marker/synthetic-serialized.sdocx")?;
    for (name, coords, pressure, size) in [
        ("loop", vec![[0.,0.],[8.,0.],[12.,7.],[5.,10.],[-2.,8.],[-5.,1.],[1.,-5.],[8.,-2.],[13.,3.]], vec![1.,0.,1.,0.,1.,0.,1.,0.5,0.8], 2.),
        ("reverse", vec![[40.,10.],[34.,8.],[28.,14.],[22.,6.],[16.,11.],[10.,3.],[4.,9.],[-2.,2.],[-8.,7.]], vec![0.1,0.9,0.2,0.8,0.3,0.7,0.4,0.6,0.5], 0.75),
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
                x: origin + p[0],
                y: origin - p[1],
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
        page.width = if double { 128 } else { 256 };
        page.height = if double { 128 } else { 256 };
        page.template = None;
        page.background = Default::default();
        page.background_color = None;
        let doc = Document {
            pages: vec![page],
            metadata: DocumentMetadata::default(),
        };
        let candidate = if double { "aware" } else { "candidate" };
        for label in [candidate, "fallback"] {
            let profile =
                (!double && label == candidate).then_some(MontblancRenderProfile::HostUnitScale);
            let mut opts = RenderOptions::default();
            opts.montblanc_profile = profile;
            opts.montblanc_two_pixels_per_page_unit = double && label == candidate;
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
