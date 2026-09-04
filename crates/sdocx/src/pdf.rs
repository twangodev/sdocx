//! Multipage PDF export through the shared SVG renderer (feature `pdf`).
//!
//! Text uses the supplied font database and is embedded as selectable text.
//! SVG rendering limitations also apply to PDF. Hyperlink annotations and
//! document structure tags are not exported; SVG filters can become bitmaps.

use std::sync::{Arc, Mutex};

use krilla::{Document as PdfDocument, geom::Size, page::PageSettings};
use krilla_svg::{SurfaceExt, SvgSettings};

use crate::{Document, RenderOptions, RenderedPage, render_document_svg};

/// Font database types used to configure PDF text rendering.
pub use usvg::fontdb;

/// PDF-specific settings, reusable across exports.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PdfOptions {
    /// SVG coordinate units per inch. Defaults to 96; PDF uses 72 points/inch.
    /// This controls physical page size, not vector rendering resolution.
    pub dpi: f32,
    /// Fonts available for shaping and embedding. Missing families use the
    /// database's fallback faces; missing glyphs can still be omitted.
    pub font_database: Arc<fontdb::Database>,
}

impl PdfOptions {
    /// Use a caller-supplied font database without discovering system fonts.
    pub fn new(font_database: Arc<fontdb::Database>) -> Self {
        Self {
            dpi: 96.0,
            font_database,
        }
    }
}

impl Default for PdfOptions {
    /// Discover system fonts once. For reproducible exports, use [`Self::new`]
    /// with a database populated from known font files instead.
    fn default() -> Self {
        let mut fonts = fontdb::Database::new();
        fonts.load_system_fonts();
        fonts.set_sans_serif_family("DejaVu Sans");
        fonts.set_monospace_family("DejaVu Sans Mono");
        Self::new(Arc::new(fonts))
    }
}

/// A failure to convert a document or its SVG pages into a PDF.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PdfError {
    /// A PDF requires at least one visible page.
    #[error("cannot export a PDF with no visible pages")]
    EmptyDocument,
    /// The requested physical scale must be finite and positive.
    #[error("PDF DPI must be finite and greater than zero")]
    InvalidDpi,
    /// Page geometry cannot be represented at the requested scale.
    #[error("invalid PDF dimensions for page {page_index}")]
    InvalidPageSize {
        /// Zero-based presentation index.
        page_index: usize,
    },
    /// An SVG page could not be parsed.
    #[error("invalid SVG on page {page_index}: {message}")]
    InvalidSvg {
        /// Zero-based presentation index.
        page_index: usize,
        /// Converter error detail.
        message: String,
    },
    /// A PNG cannot be decoded within the 64 MiB image buffer limit.
    #[error("invalid PNG on page {page_index}: {message}")]
    InvalidImage {
        /// Zero-based presentation index.
        page_index: usize,
        /// Decoder error or resource limit detail.
        message: String,
    },
    /// Conversion or font/image embedding failed.
    #[error("PDF export failed: {0}")]
    Conversion(String),
}

/// Render every visible page, in presentation order, into one PDF.
///
/// Uses exactly the same page layout and SVG markup as [`render_document_svg`].
/// Bytes are returned only after every page and its resources are serialized.
pub fn render_document_pdf(
    document: &Document,
    render_options: &RenderOptions,
    pdf_options: &PdfOptions,
) -> Result<Vec<u8>, PdfError> {
    render_svg_pages_pdf(&render_document_svg(document, render_options), pdf_options)
}

/// Combine already-rendered SVG pages into one PDF, preserving slice order.
///
/// Each page's `width` and `height` define its physical size at [`PdfOptions::dpi`].
/// SVG intrinsic dimensions must match those values. Only in-memory data images
/// are resolved; external image paths are deliberately not read.
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
                // krilla 0.5 assumes valid PNGs and can panic while drawing a
                // corrupt image. Decode before entering its drawing surface.
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
        let width = rendered.width as f32 * (72.0 / options.dpi);
        let height = rendered.height as f32 * (72.0 / options.dpi);
        // PDF's default user space supports pages up to 14,400 points per side.
        // Larger pages would require explicit UserUnit support.
        if width > 14_400.0 || height > 14_400.0 {
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
        let mut page = pdf.start_page_with(PageSettings::new(width, height));
        let mut surface = page.surface();
        surface
            .draw_svg(&tree, size, SvgSettings::default())
            .ok_or_else(|| PdfError::Conversion(format!("cannot draw page {page_index}")))?;
        surface.finish();
        page.finish();
    }
    pdf.finish()
        .map_err(|error| PdfError::Conversion(format!("{error:?}")))
}

fn validate_png(bytes: &[u8]) -> Result<(), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let buffer_size = reader.output_buffer_size();
    if buffer_size > 64 * 1024 * 1024 {
        return Err("decoded PNG exceeds the 64 MiB buffer limit".into());
    }
    let mut buffer = vec![0; buffer_size];
    reader
        .next_frame(&mut buffer)
        .map_err(|error| error.to_string())?;
    Ok(())
}
