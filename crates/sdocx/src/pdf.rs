use std::sync::{Arc, Mutex};

use krilla::{Document as PdfDocument, geom::Size, page::PageSettings};
use krilla_svg::{SurfaceExt, SvgSettings};

use crate::{Document, RenderOptions, RenderedPage, render_document_svg};

pub use usvg::fontdb;

const PDF_POINTS_PER_INCH: f32 = 72.0;
const MAX_PDF_PAGE_POINTS: f32 = 14_400.0;
const MAX_PNG_DECODED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PdfOptions {
    pub dpi: f32,
    pub font_database: Arc<fontdb::Database>,
}

impl PdfOptions {
    pub fn new(font_database: Arc<fontdb::Database>) -> Self {
        Self {
            dpi: 96.0,
            font_database,
        }
    }
}

impl Default for PdfOptions {
    fn default() -> Self {
        let mut fonts = fontdb::Database::new();
        fonts.load_system_fonts();
        fonts.set_sans_serif_family("DejaVu Sans");
        fonts.set_monospace_family("DejaVu Sans Mono");
        Self::new(Arc::new(fonts))
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PdfError {
    #[error("cannot export a PDF with no visible pages")]
    EmptyDocument,
    #[error("PDF DPI must be finite and greater than zero")]
    InvalidDpi,
    #[error("invalid PDF dimensions for page {page_index}")]
    InvalidPageSize { page_index: usize },
    #[error("invalid SVG on page {page_index}: {message}")]
    InvalidSvg { page_index: usize, message: String },
    #[error("invalid PNG on page {page_index}: {message}")]
    InvalidImage { page_index: usize, message: String },
    #[error("PDF export failed: {0}")]
    Conversion(String),
}

pub fn render_document_pdf(
    document: &Document,
    render_options: &RenderOptions,
    pdf_options: &PdfOptions,
) -> Result<Vec<u8>, PdfError> {
    render_svg_pages_pdf(&render_document_svg(document, render_options), pdf_options)
}

pub fn render_svg_pages_pdf(
    pages: &[RenderedPage],
    options: &PdfOptions,
) -> Result<Vec<u8>, PdfError> {
    if !options.dpi.is_finite() || options.dpi <= 0.0 {
        return Err(PdfError::InvalidDpi);
    }
    if pages.is_empty() {
        return Err(PdfError::EmptyDocument);
    }
    let image_error = Mutex::new(None);
    let data_resolver = usvg::ImageHrefResolver::default_data_resolver();
    let svg_options = usvg::Options {
        fontdb: options.font_database.clone(),
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|mime, data, options| {
                let image = data_resolver(mime, data, options)?;
                if let usvg::ImageKind::PNG(bytes) = &image
                    && let Err(error) = validate_png(bytes)
                {
                    *image_error.lock().unwrap() = Some(error);
                    return None;
                }
                Some(image)
            }),
            resolve_string: Box::new(|_, _| None),
        },
        ..Default::default()
    };
    let mut pdf = PdfDocument::new();
    for (page_index, rendered) in pages.iter().enumerate() {
        let width = rendered.width as f32 * (PDF_POINTS_PER_INCH / options.dpi);
        let height = rendered.height as f32 * (PDF_POINTS_PER_INCH / options.dpi);
        if width > MAX_PDF_PAGE_POINTS || height > MAX_PDF_PAGE_POINTS {
            return Err(PdfError::InvalidPageSize { page_index });
        }
        let size = Size::from_wh(width, height).ok_or(PdfError::InvalidPageSize { page_index })?;
        let tree = usvg::Tree::from_str(&rendered.svg, &svg_options).map_err(|error| {
            PdfError::InvalidSvg {
                page_index,
                message: error.to_string(),
            }
        })?;
        if let Some(message) = image_error.lock().unwrap().take() {
            return Err(PdfError::InvalidImage {
                page_index,
                message,
            });
        }
        if tree.size().width() != rendered.width as f32
            || tree.size().height() != rendered.height as f32
        {
            return Err(PdfError::InvalidPageSize { page_index });
        }
        let mut page = pdf.start_page_with(PageSettings::new(size));
        let mut surface = page.surface();
        surface
            .draw_svg(&tree, size, SvgSettings::default())
            .ok_or_else(|| PdfError::Conversion(format!("cannot draw page {page_index}")))?;
        surface.finish();
        page.finish();
    }
    pdf.finish()
        .map_err(|error| PdfError::Conversion(error.to_string()))
}

fn validate_png(bytes: &[u8]) -> Result<(), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let buffer_size = reader
        .output_buffer_size()
        .ok_or("PNG dimensions overflow")?;
    if buffer_size > MAX_PNG_DECODED_BYTES {
        return Err("decoded PNG exceeds the 64 MiB buffer limit".into());
    }
    let mut buffer = vec![0; buffer_size];
    reader
        .next_frame(&mut buffer)
        .map_err(|error| error.to_string())?;
    Ok(())
}
