use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use self::svg::{SurfaceExt, SvgSettings};
use krilla::{Document as PdfDocument, geom::Size, page::PageSettings};

mod svg;

use crate::render::{NativePdfPainter, NativeTextRegistry, render_layout_page_scene};
use crate::{Document, LayoutDocument, RenderOptions, RenderedPage};

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
        Self::new(crate::fonts::FontBook::default().database())
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
    #[error("visible page index {page_index} is out of bounds")]
    InvalidPageIndex { page_index: usize },
    #[error("unsupported retained text on page {page_index}: {message}")]
    UnsupportedText { page_index: usize, message: String },
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
    let fonts = crate::fonts::FontBook::new(pdf_options.font_database.clone());
    let layout = crate::layout_document(document);
    let indices = (0..layout.pages.len()).collect::<Vec<_>>();
    render_layout_pages_pdf_with_fonts(
        document,
        &layout,
        &indices,
        render_options,
        pdf_options,
        &fonts,
    )
}

/// Export selected visible pages using the authoritative Rust text and font plans.
/// The supplied font book controls both measurement and SVG carrier parsing.
pub fn render_layout_pages_pdf_with_fonts(
    document: &Document,
    layout: &LayoutDocument,
    page_indices: &[usize],
    render_options: &RenderOptions,
    pdf_options: &PdfOptions,
    fonts: &crate::fonts::FontBook,
) -> Result<Vec<u8>, PdfError> {
    let mut pdf_options = pdf_options.clone();
    pdf_options.font_database = fonts.database();
    let scenes = page_indices
        .iter()
        .map(|&page_index| {
            let page = layout
                .pages
                .get(page_index)
                .ok_or(PdfError::InvalidPageIndex { page_index })?;
            Ok(render_layout_page_scene(
                document,
                page,
                render_options,
                fonts,
            ))
        })
        .collect::<Result<Vec<_>, PdfError>>()?;
    render_pages_pdf(
        scenes.iter().map(|scene| PdfPage {
            page: &scene.page,
            text: Some(&scene.text),
            text_error: scene.text_error.as_deref(),
        }),
        &pdf_options,
    )
}

pub fn render_svg_pages_pdf(
    pages: &[RenderedPage],
    options: &PdfOptions,
) -> Result<Vec<u8>, PdfError> {
    render_pages_pdf(
        pages.iter().map(|page| PdfPage {
            page,
            text: None,
            text_error: None,
        }),
        options,
    )
}

struct PdfPage<'a> {
    page: &'a RenderedPage,
    text: Option<&'a NativeTextRegistry>,
    text_error: Option<&'a str>,
}

fn render_pages_pdf<'a>(
    pages: impl IntoIterator<Item = PdfPage<'a>>,
    options: &PdfOptions,
) -> Result<Vec<u8>, PdfError> {
    if !options.dpi.is_finite() || options.dpi <= 0.0 {
        return Err(PdfError::InvalidDpi);
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
    let mut painter = NativePdfPainter::default();
    let mut page_count = 0;
    let mut retained_text = false;
    for (page_index, source) in pages.into_iter().enumerate() {
        page_count += 1;
        let rendered = source.page;
        if let Some(message) = source.text_error {
            return Err(PdfError::UnsupportedText {
                page_index,
                message: message.into(),
            });
        }
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
        if let Some(registry) = source.text {
            retained_text = true;
            draw_retained_page(&mut surface, &tree, size, registry, &mut painter).map_err(
                |message| PdfError::UnsupportedText {
                    page_index,
                    message,
                },
            )?;
        } else {
            surface
                .draw_svg(&tree, size, SvgSettings::default())
                .ok_or_else(|| PdfError::Conversion(format!("cannot draw page {page_index}")))?;
        }
        surface.finish();
        page.finish();
    }
    if page_count == 0 {
        return Err(PdfError::EmptyDocument);
    }
    if retained_text {
        pdf.set_tag_tree(painter.take_tag_tree());
    }
    pdf.finish()
        .map_err(|error| PdfError::Conversion(error.to_string()))
}

fn draw_retained_page(
    surface: &mut krilla::surface::Surface<'_>,
    tree: &usvg::Tree,
    size: Size,
    registry: &NativeTextRegistry,
    painter: &mut NativePdfPainter,
) -> Result<(), String> {
    let mut nodes = HashMap::with_capacity(registry.len());
    for (index, (id, block)) in registry.iter().enumerate() {
        let Some(usvg::Node::Text(text)) = tree.node_by_id(&id.svg_id()) else {
            return Err(format!(
                "retained text {} was lost during SVG parsing",
                id.svg_id()
            ));
        };
        nodes.insert(text.as_ref() as *const usvg::Text, (index, block));
    }
    let mut handled = vec![false; registry.len()];
    surface.draw_svg_with_text(tree, size, SvgSettings::default(), &mut |text, surface| {
        let Some(&(index, block)) = nodes.get(&(text as *const usvg::Text)) else {
            return Ok(false);
        };
        if handled[index] {
            return Err("retained text was painted more than once".into());
        }
        painter
            .paint(block, surface)
            .map_err(|error| error.to_string())?;
        handled[index] = true;
        Ok(true)
    })?;
    if handled.iter().any(|&painted| !painted) {
        return Err("retained text was skipped by an unsupported SVG effect".into());
    }
    Ok(())
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
