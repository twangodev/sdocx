fn main() {
 let a: Vec<_> = std::env::args().collect();
 assert_eq!(a.len(),3,"input.svg output.pdf");
 let svg=std::fs::read_to_string(&a[1]).unwrap();
 let page=sdocx::RenderedPage::new(0,192,192,svg);
 let mut options=sdocx::PdfOptions::new(std::sync::Arc::new(sdocx::pdf::fontdb::Database::new()));
 options.dpi=72.0;
 let pdf=sdocx::render_svg_pages_pdf(&[page],&options).unwrap();
 std::fs::write(&a[2],pdf).unwrap();
}
