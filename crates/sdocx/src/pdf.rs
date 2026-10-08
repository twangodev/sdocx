use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use self::svg::{SurfaceExt, SvgError, SvgSettings};
use krilla::{
    Document as PdfDocument,
    geom::Size,
    page::PageSettings,
    tagging::{TagGroup, TagTree},
};

mod svg;

#[cfg(test)]
#[path = "pdf/font_identity_tests.rs"]
mod font_identity_tests;

use crate::render::{DocumentTextCache, NativePdfPainter, NativeTextRegistry};
use crate::{
    Document, GeometryDiagnostic, LayoutDocument, ObjectDiagnostic, PaintDiagnostic, RenderOptions,
    RenderedPage, TextDiagnostic,
    fonts::{FontBook, NativeFontNameConfig, SvgFontFamilies},
};

pub use usvg::fontdb;

const PDF_POINTS_PER_INCH: f32 = 72.0;
const MAX_PDF_PAGE_POINTS: f32 = 14_400.0;
const MAX_PNG_DECODED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PdfOptions {
    pub dpi: f32,
    pub font_database: Arc<fontdb::Database>,
    pub native_font_names: Option<NativeFontNameConfig>,
    retained_fonts: Option<FontBook>,
}

impl PdfOptions {
    pub fn new(font_database: Arc<fontdb::Database>) -> Self {
        Self {
            dpi: 96.0,
            font_database,
            native_font_names: None,
            retained_fonts: None,
        }
    }

    pub fn from_font_book(fonts: &FontBook) -> Self {
        Self {
            dpi: 96.0,
            font_database: fonts.database(),
            native_font_names: fonts.native_name_config().cloned(),
            retained_fonts: Some(fonts.clone()),
        }
    }

    fn font_book(&self) -> FontBook {
        if let Some(fonts) = &self.retained_fonts
            && Arc::ptr_eq(&self.font_database, &fonts.database())
            && self.native_font_names.as_ref() == fonts.native_name_config()
        {
            return fonts.clone();
        }
        let fonts = FontBook::new(self.font_database.clone());
        match &self.native_font_names {
            Some(configuration) => fonts.with_native_name_config(configuration.clone()),
            None => fonts,
        }
    }
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self::from_font_book(&FontBook::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PdfOutput {
    pub bytes: Vec<u8>,
    /// Render diagnostics in exported page order, including repeated selections.
    pub pages: Vec<PdfPageDiagnostics>,
}

impl PdfOutput {
    /// Construct an export with diagnostics in output page order.
    pub fn new(bytes: Vec<u8>, pages: Vec<PdfPageDiagnostics>) -> Self {
        Self { bytes, pages }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct PdfPageDiagnostics {
    /// The selected page's index in the supplied visible layout.
    pub page_index: usize,
    /// Index of the backing page in the parsed document.
    pub source_page_index: usize,
    pub text_diagnostics: Vec<TextDiagnostic>,
    pub object_diagnostics: Vec<ObjectDiagnostic>,
    pub geometry_diagnostics: Vec<GeometryDiagnostic>,
    pub paint_diagnostics: Vec<PaintDiagnostic>,
}

impl PdfPageDiagnostics {
    /// Construct an empty report for one selected page and its backing source.
    pub fn new(page_index: usize, source_page_index: usize) -> Self {
        Self {
            page_index,
            source_page_index,
            text_diagnostics: Vec::new(),
            object_diagnostics: Vec::new(),
            geometry_diagnostics: Vec::new(),
            paint_diagnostics: Vec::new(),
        }
    }
}

/// Rendering error page indices are zero-based output ordinals, including repeated selections.
/// `InvalidPageIndex` instead reports the requested index in the supplied visible layout.
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
    #[error("invalid image on page {page_index}: {message}")]
    InvalidImage { page_index: usize, message: String },
    #[error("PDF export failed: {0}")]
    Conversion(String),
}

pub fn render_document_pdf(
    document: &Document,
    render_options: &RenderOptions,
    pdf_options: &PdfOptions,
) -> Result<Vec<u8>, PdfError> {
    let fonts = pdf_options.font_book();
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
    render_layout_pages_pdf_detailed_with_fonts(
        document,
        layout,
        page_indices,
        render_options,
        pdf_options,
        fonts,
    )
    .map(|output| output.bytes)
}

/// Export selected pages and retain nonfatal diagnostics from the same render plans.
pub fn render_layout_pages_pdf_detailed_with_fonts(
    document: &Document,
    layout: &LayoutDocument,
    page_indices: &[usize],
    render_options: &RenderOptions,
    pdf_options: &PdfOptions,
    fonts: &crate::fonts::FontBook,
) -> Result<PdfOutput, PdfError> {
    render_layout_pages_pdf_detailed_with_cache(
        document,
        layout,
        page_indices,
        render_options,
        pdf_options,
        fonts,
        &mut DocumentTextCache::default(),
    )
}

/// Reuse document text preparation across previews and selected-page PDF exports.
pub fn render_layout_pages_pdf_detailed_with_cache(
    document: &Document,
    layout: &LayoutDocument,
    page_indices: &[usize],
    render_options: &RenderOptions,
    pdf_options: &PdfOptions,
    fonts: &crate::fonts::FontBook,
    cache: &mut DocumentTextCache,
) -> Result<PdfOutput, PdfError> {
    let mut pdf_options = pdf_options.clone();
    pdf_options.font_database = fonts.database();
    let selected_pages = page_indices
        .iter()
        .map(|&page_index| {
            layout
                .pages
                .get(page_index)
                .ok_or(PdfError::InvalidPageIndex { page_index })
        })
        .collect::<Result<Vec<_>, PdfError>>()?;
    let scenes = cache.render_layout_page_scenes(document, &selected_pages, render_options, fonts);
    let bytes = render_pages_pdf(
        scenes.iter().map(|scene| PdfPage {
            page: &scene.page,
            text: Some(&scene.text),
            text_error: scene.text_error.as_deref(),
        }),
        &pdf_options,
    )?;
    let pages = page_indices
        .iter()
        .copied()
        .zip(scenes)
        .map(|(page_index, scene)| PdfPageDiagnostics {
            page_index,
            source_page_index: scene.page.source_page_index,
            text_diagnostics: scene.page.text_diagnostics,
            object_diagnostics: scene.page.object_diagnostics,
            geometry_diagnostics: scene.page.geometry_diagnostics,
            paint_diagnostics: scene.page.paint_diagnostics,
        })
        .collect();
    Ok(PdfOutput { bytes, pages })
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

#[cfg(test)]
pub(crate) fn render_native_scene_pdf_for_test(
    scene: &crate::render::RenderedScene,
    options: &PdfOptions,
) -> Result<Vec<u8>, PdfError> {
    render_pages_pdf(
        [PdfPage {
            page: &scene.page,
            text: Some(&scene.text),
            text_error: scene.text_error.as_deref(),
        }],
        options,
    )
}

fn render_pages_pdf<'a>(
    pages: impl IntoIterator<Item = PdfPage<'a>>,
    options: &PdfOptions,
) -> Result<Vec<u8>, PdfError> {
    if !options.dpi.is_finite() || options.dpi <= 0.0 {
        return Err(PdfError::InvalidDpi);
    }
    let image_error = Mutex::new(None);
    let font_error = Mutex::new(false);
    let physical_families = SvgFontFamilies::new(&options.font_database);
    let usvg::FontResolver {
        select_font,
        select_fallback,
    } = physical_families.usvg_resolver();
    let data_resolver = usvg::ImageHrefResolver::default_data_resolver();
    let mut pdf = PdfDocument::new();
    let mut painter = NativePdfPainter::new(options.dpi);
    let mut tags = TagTree::default();
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
        *font_error.lock().unwrap() = false;
        let strict_images = source.text.is_some();
        let svg_options = usvg::Options {
            fontdb: options.font_database.clone(),
            font_resolver: usvg::FontResolver {
                select_font: Box::new(|font, database| {
                    if let Some(usvg::FontFamily::Named(family)) = font.families().first()
                        && physical_families.is_unavailable(family)
                    {
                        *font_error.lock().unwrap() = true;
                    }
                    select_font(font, database)
                }),
                select_fallback: Box::new(|character, used_fonts, database| {
                    select_fallback(character, used_fonts, database)
                }),
            },
            image_href_resolver: usvg::ImageHrefResolver {
                resolve_data: Box::new(|mime, data, options| {
                    let Some(image) = data_resolver(mime, data, options) else {
                        if strict_images {
                            image_error.lock().unwrap().get_or_insert_with(|| {
                                format!("cannot resolve embedded {mime} image")
                            });
                        }
                        return None;
                    };
                    if let usvg::ImageKind::PNG(bytes) = &image
                        && let Err(error) = validate_png(bytes)
                    {
                        image_error.lock().unwrap().get_or_insert(error);
                        return None;
                    }
                    if strict_images {
                        let admission = svg::raster_image(&image).and_then(|image| {
                            if let Some(image) = image {
                                let (width, height) = image.size();
                                if width == 0 || height == 0 {
                                    return Err("embedded image has zero dimensions".into());
                                }
                            }
                            Ok(())
                        });
                        if let Err(error) = admission {
                            image_error.lock().unwrap().get_or_insert(error);
                            return None;
                        }
                    }
                    Some(image)
                }),
                resolve_string: Box::new(|_, _| None),
            },
            ..Default::default()
        };
        let tree = usvg::Tree::from_str(&rendered.svg, &svg_options).map_err(|error| {
            PdfError::InvalidSvg {
                page_index,
                message: error.to_string(),
            }
        })?;
        if source.text.is_none() && *font_error.lock().unwrap() {
            return Err(PdfError::UnsupportedText {
                page_index,
                message: "the SVG's selected physical font is absent from the supplied database"
                    .into(),
            });
        }
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
            let page_tags = draw_retained_page(&mut surface, &tree, size, registry, &mut painter)
                .map_err(|error| match error {
                SvgError::Text(message) => PdfError::UnsupportedText {
                    page_index,
                    message,
                },
                SvgError::Image(message) => PdfError::InvalidImage {
                    page_index,
                    message,
                },
            })?;
            for tag in page_tags {
                tags.push(tag);
            }
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
        pdf.set_tag_tree(tags);
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
) -> Result<Vec<TagGroup>, SvgError> {
    let mut nodes = HashMap::with_capacity(registry.len());
    for (id, block) in registry.iter() {
        let Some(usvg::Node::Text(text)) = tree.node_by_id(&id.svg_id()) else {
            return Err(SvgError::Text(format!(
                "retained text {} was lost during SVG parsing",
                id.svg_id()
            )));
        };
        nodes.insert(text.as_ref() as *const usvg::Text, (id, block));
    }
    let mut tags = HashMap::with_capacity(registry.len());
    surface.draw_svg_with_text(
        tree,
        size,
        SvgSettings {
            reject_images: true,
            ..Default::default()
        },
        &mut |text, surface| {
            let Some(&(id, block)) = nodes.get(&(text as *const usvg::Text)) else {
                return Ok(false);
            };
            if tags.contains_key(&id) {
                return Err("retained text was painted more than once".into());
            }
            let tag = painter
                .paint(block, surface)
                .map_err(|error| error.to_string())?;
            tags.insert(id, tag);
            Ok(true)
        },
    )?;
    registry
        .ordered_ids()
        .into_iter()
        .map(|id| {
            tags.remove(&id).ok_or_else(|| {
                SvgError::Text("retained text was skipped by an unsupported SVG effect".into())
            })
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_preserve_native_font_configuration() {
        let fonts = FontBook::default();
        let options = PdfOptions::default();
        assert!(options.native_font_names.is_some());
        assert_eq!(
            options.native_font_names.as_ref(),
            fonts.native_name_config()
        );
        assert_eq!(
            options.font_book().native_name_config(),
            fonts.native_name_config()
        );
        let face = fonts.resolve("Roboto", false, false).unwrap();
        assert_eq!(
            options.font_book().registered_source(&face),
            fonts.registered_source(&face)
        );
    }

    #[test]
    fn database_constructor_keeps_native_font_configuration_explicit() {
        let database = FontBook::default().database();
        let options = PdfOptions::new(database.clone());
        assert!(Arc::ptr_eq(&options.font_database, &database));
        assert!(options.native_font_names.is_none());
        assert!(options.font_book().native_name_config().is_none());
        let fonts = options.font_book();
        let face = fonts.resolve("Roboto", false, false).unwrap();
        assert!(fonts.registered_source(&face).is_none());
    }

    #[test]
    fn font_book_constructor_preserves_caller_configuration() {
        let configuration = NativeFontNameConfig::new("custom-family")
            .unwrap()
            .with_family_alias("custom-family", "Roboto")
            .unwrap()
            .with_font_file("Custom-Regular.ttf", "custom-family")
            .unwrap();
        let database = FontBook::default().database();
        let fonts = FontBook::new(database.clone()).with_native_name_config(configuration.clone());
        let options = PdfOptions::from_font_book(&fonts);
        assert!(Arc::ptr_eq(&options.font_database, &database));
        assert_eq!(options.native_font_names.as_ref(), Some(&configuration));
        let restored = options.font_book();
        assert!(Arc::ptr_eq(&restored.database(), &database));
        assert_eq!(restored.native_name_config(), Some(&configuration));
    }

    #[test]
    fn font_book_constructor_and_option_clones_retain_registered_sources() {
        let fonts = FontBook::default();
        let face = fonts.resolve("Roboto", false, false).unwrap();
        let source = fonts.registered_source(&face).unwrap();
        let options = PdfOptions::from_font_book(&fonts);
        for options in [options.clone(), options] {
            assert_eq!(
                options.font_book().registered_source(&face),
                Some(source.clone())
            );
        }
    }

    #[test]
    fn changed_database_invalidates_retained_registered_sources() {
        let mut options = PdfOptions::default();
        options.font_database = Arc::new(options.font_database.as_ref().clone());
        let fonts = options.font_book();
        assert!(Arc::ptr_eq(&fonts.database(), &options.font_database));
        assert_eq!(
            fonts.native_name_config(),
            options.native_font_names.as_ref()
        );
        let face = fonts.resolve("Roboto", false, false).unwrap();
        assert!(fonts.registered_source(&face).is_none());
    }

    #[test]
    fn changed_native_configuration_invalidates_retained_registered_sources() {
        let options = PdfOptions {
            native_font_names: Some(
                NativeFontNameConfig::new("custom-family")
                    .unwrap()
                    .with_family_alias("custom-family", "Roboto")
                    .unwrap(),
            ),
            ..Default::default()
        };
        let fonts = options.font_book();
        assert_eq!(
            fonts.native_name_config(),
            options.native_font_names.as_ref()
        );
        let face = fonts.resolve("Roboto", false, false).unwrap();
        assert!(fonts.registered_source(&face).is_none());
    }
}
