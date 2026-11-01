//! Mixed public-document exports for the PDF reuse comparison.
use sdocx::{
    Color, Document, DocumentMetadata, MontblancRenderProfile, PageObject, PdfOptions, Point,
    RenderOptions, RenderedPage,
};

fn forms(bytes: &[u8]) -> usize {
    lopdf::Document::load_mem(bytes)
        .unwrap()
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| {
            s.dict
                .get(b"Subtype")
                .and_then(|o| o.as_name())
                .is_ok_and(|n| n == b"Form")
        })
        .count()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args_os().nth(1).expect("output directory"));
    std::fs::create_dir_all(&out)?;
    let source =
        sdocx::parse("crates/sdocx/tests/assets/legacy-marker/synthetic-serialized.sdocx")?;
    let mut stroke = source
        .pages
        .iter()
        .flat_map(|p| p.strokes())
        .next()
        .unwrap()
        .clone();
    stroke.points = [
        [1., 2.],
        [7., 9.],
        [13., 3.],
        [20., 10.],
        [27., 4.],
        [34., 11.],
    ]
    .into_iter()
    .map(|p| Point {
        x: 128. + p[0],
        y: 128. - p[1],
    })
    .collect();
    stroke.pressures = [0.25_f32, 0.4, 0.8, 0.35, 0.95, 0.6]
        .into_iter()
        .map(f64::from)
        .collect();
    stroke.pen_width = 1.5;
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
    r.style.pen_size = Some(1.5);
    r.style.color_argb = Some(0xff336699);
    r.properties.stylus_channels = false;
    r.properties.millisecond_timestamps = false;
    let mut other = stroke.clone();
    for point in &mut other.points {
        point.x += 24.;
        point.y += 20.;
    }
    let mut page = source.pages[0].clone();
    let ordinary = page.objects.first().expect("mixed ordinary object").clone();
    page.objects = vec![PageObject::from(stroke), ordinary, PageObject::from(other)];
    page.width = 256;
    page.height = 256;
    page.template = None;
    page.background = Default::default();
    page.background_color = None;
    let doc = Document {
        pages: vec![page],
        metadata: DocumentMetadata::default(),
    };
    let mut opts = RenderOptions::default();
    opts.montblanc_profile = Some(MontblancRenderProfile::HostUnitScale);
    let rendered = sdocx::render_document_svg(&doc, &opts).remove(0);
    assert!(rendered.svg.contains("sdocx-montblanc-reuse-"));
    std::fs::write(out.join("scene.svg"), &rendered.svg)?;
    std::fs::write(
        out.join("direct.pdf"),
        sdocx::render_document_pdf(&doc, &opts, &PdfOptions::default())?,
    )?;
    for (name, start, end) in [
        ("normal", "", ""),
        ("rotate", "<g transform='rotate(7 128 128)'>", "</g>"),
        (
            "reflect",
            "<g transform='translate(256 0) scale(-1 0.8)'>",
            "</g>",
        ),
        (
            "cancel",
            "<g transform='rotate(90)'><g transform='rotate(-90)'>",
            "</g></g>",
        ),
    ] {
        let svg = &rendered.svg;
        let open = svg.find('>').unwrap() + 1;
        let wrapped = format!(
            "{}{start}{}",
            &svg[..open],
            svg[open..].replace("</svg>", &format!("{end}</svg>"))
        );
        let mut counts = Vec::new();
        for (kind, text) in [
            ("marked", wrapped.clone()),
            (
                "flat",
                wrapped.replace("sdocx-montblanc-reuse-", "ordinary-"),
            ),
        ] {
            let page = RenderedPage::new(0, 256, 256, text);
            let bytes = sdocx::render_svg_pages_pdf(&[page], &PdfOptions::default())?;
            counts.push(forms(&bytes));
            std::fs::write(out.join(format!("{name}-{kind}.pdf")), bytes)?;
        }
        if name == "rotate" {
            assert_eq!(counts[0], counts[1]);
        } else {
            assert!(
                counts[0] > counts[1],
                "reuse must actually occur for {name}: {counts:?}"
            );
        }
    }
    Ok(())
}
