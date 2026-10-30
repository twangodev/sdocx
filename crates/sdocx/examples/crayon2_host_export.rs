//! Rasterize a standalone host SVG and exercise the public SVG-page PDF exporter.
use sdocx::{PdfOptions, RenderedPage, render_svg_pages_pdf};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let input = args.next().expect("input SVG");
    let png = args.next().expect("output PNG");
    let pdf = args.next().expect("output PDF");
    assert!(args.next().is_none(), "expected SVG, PNG and PDF paths");
    let svg = std::fs::read_to_string(input).expect("read SVG");
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("parse SVG");
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height()).expect("allocate pixels");
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.save_png(png).expect("write PNG");
    let page = RenderedPage::new(0, size.width(), size.height(), svg);
    let bytes = render_svg_pages_pdf(&[page], &PdfOptions::default()).expect("export PDF");
    std::fs::write(pdf, bytes).expect("write PDF");
}
