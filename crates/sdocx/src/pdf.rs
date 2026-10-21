use crate::{Progress, ProgressStage};
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

mod ink_field;
mod svg;

#[cfg(test)]
#[path = "pdf/font_identity_tests.rs"]
mod font_identity_tests;
#[cfg(test)]
#[path = "pdf/paper.rs"]
mod paper;
#[cfg(test)]
#[path = "pdf/paper_segments.rs"]
mod paper_segments;

use crate::render::{
    DocumentTextCache, NativeInkPayload, NativeInkRegistry, NativePdfPainter, NativeTextRegistry,
};
use crate::{
    Document, GeometryDiagnostic, LayoutDocument, ObjectDiagnostic, PaintDiagnostic, RenderOptions,
    RenderedPage, TextDiagnostic,
    fonts::{FontBook, NativeFontNameConfig, SvgFontFamilies},
};

pub use usvg::fontdb;

pub(crate) fn validate_radial_ink(
    records: &[[f32; 9]],
    bounds: crate::BoundingBox,
    saved_alpha: f32,
    enhanced_antialias: bool,
) -> Result<(), String> {
    ink_field::validate_radial(
        records,
        saved_alpha,
        if enhanced_antialias {
            ink_field::PdfRadialCoverage::EnhancedMax
        } else {
            ink_field::PdfRadialCoverage::DirectSourceOver
        },
        bounds,
    )
}

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
    pub background_diagnostics: Vec<crate::BackgroundDiagnostic>,
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
            background_diagnostics: Vec::new(),
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
    #[error(
        "cannot export output page {output_page_index} (source page {source_page_index}): {record_count} retained PDF paper record(s) are not supported by this export route"
    )]
    UnsupportedPdfPaper {
        output_page_index: usize,
        source_page_index: usize,
        record_count: usize,
    },
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
    render_layout_pages_pdf_detailed_with_cache_and_progress(
        document,
        layout,
        page_indices,
        render_options,
        pdf_options,
        fonts,
        cache,
        &mut |_| {},
    )
}

/// Export the same selected-page PDF while reporting vector composition and writing.
#[allow(clippy::too_many_arguments)]
pub fn render_layout_pages_pdf_detailed_with_cache_and_progress(
    document: &Document,
    layout: &LayoutDocument,
    page_indices: &[usize],
    render_options: &RenderOptions,
    pdf_options: &PdfOptions,
    fonts: &crate::fonts::FontBook,
    cache: &mut DocumentTextCache,
    observer: &mut dyn FnMut(Progress),
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
    for (output_page_index, layout_page) in selected_pages.iter().enumerate() {
        let record_count = layout_page
            .page
            .background
            .pdf_paper
            .as_ref()
            .map_or(0, Vec::len);
        if record_count > 0 {
            return Err(PdfError::UnsupportedPdfPaper {
                output_page_index,
                source_page_index: layout_page.source_page_index,
                record_count,
            });
        }
    }
    let scenes =
        cache.render_layout_page_scenes(document, &selected_pages, render_options, fonts, observer);
    let bytes = render_scenes_pdf_with_progress(&scenes, &pdf_options, observer)?;
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
            background_diagnostics: scene.page.background_diagnostics,
        })
        .collect();
    Ok(PdfOutput { bytes, pages })
}

pub fn render_svg_pages_pdf(
    pages: &[RenderedPage],
    options: &PdfOptions,
) -> Result<Vec<u8>, PdfError> {
    for (output_page_index, page) in pages.iter().enumerate() {
        let record_count = page
            .background_diagnostics
            .iter()
            .map(|diagnostic| match diagnostic {
                crate::BackgroundDiagnostic::PdfPaperOmitted { record_count } => *record_count,
            })
            .fold(0_usize, usize::saturating_add);
        if record_count > 0 {
            return Err(PdfError::UnsupportedPdfPaper {
                output_page_index,
                source_page_index: page.source_page_index,
                record_count,
            });
        }
    }
    render_pages_pdf(
        pages.iter().map(|page| PdfPage {
            page,
            text: None,
            ink: None,
            text_error: None,
        }),
        options,
    )
}

struct PdfPage<'a> {
    page: &'a RenderedPage,
    text: Option<&'a NativeTextRegistry>,
    ink: Option<&'a NativeInkRegistry>,
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
            ink: Some(&scene.ink),
            text_error: scene.text_error.as_deref(),
        }],
        options,
    )
}

fn render_pages_pdf<'a>(
    pages: impl IntoIterator<Item = PdfPage<'a>>,
    options: &PdfOptions,
) -> Result<Vec<u8>, PdfError> {
    render_pages_pdf_with_progress(pages, options, &mut |_| {})
}

pub(crate) fn render_scenes_pdf_with_progress(
    scenes: &[crate::render::RenderedScene],
    options: &PdfOptions,
    observer: &mut dyn FnMut(Progress),
) -> Result<Vec<u8>, PdfError> {
    render_pages_pdf_with_progress(
        scenes.iter().map(|scene| PdfPage {
            page: &scene.page,
            text: Some(&scene.text),
            ink: Some(&scene.ink),
            text_error: scene.text_error.as_deref(),
        }),
        options,
        observer,
    )
}

fn render_pages_pdf_with_progress<'a>(
    pages: impl IntoIterator<Item = PdfPage<'a>>,
    options: &PdfOptions,
    observer: &mut dyn FnMut(Progress),
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
    let mut ink_fields = Vec::new();
    let pages = pages.into_iter();
    let (lower, upper) = pages.size_hint();
    let total = (upper == Some(lower)).then_some(lower);
    observer(Progress::new(ProgressStage::WritingPdf, 0, total));
    for (page_index, source) in pages.enumerate() {
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
            let ink = source.ink.ok_or_else(|| {
                PdfError::Conversion("retained PDF scene has no native ink registry".into())
            })?;
            let page_tags = draw_retained_page(
                &mut surface,
                &tree,
                size,
                registry,
                ink,
                page_index,
                &mut painter,
            )
            .map_err(|error| match error {
                SvgError::Text(message) => PdfError::UnsupportedText {
                    page_index,
                    message,
                },
                SvgError::Image(message) => PdfError::InvalidImage {
                    page_index,
                    message,
                },
                SvgError::Filter(message) => {
                    PdfError::Conversion(format!("page {page_index}: {message}"))
                }
                SvgError::Ink(message) => PdfError::Conversion(format!(
                    "page {page_index}: native ink lowering failed: {message}"
                )),
            })?;
            for tag in page_tags {
                tags.push(tag);
            }
            for (id, entry) in ink.iter() {
                let marker_id = id.marker_id(page_index).ok_or_else(|| {
                    PdfError::Conversion("native ink marker ID exceeds supported range".into())
                })?;
                ink_fields.push((marker_id, entry.payload.clone(), entry.bounds));
            }
        } else {
            surface
                .draw_svg(&tree, size, SvgSettings::default())
                .ok_or_else(|| PdfError::Conversion(format!("cannot draw page {page_index}")))?;
        }
        surface.finish();
        page.finish();
        observer(Progress::new(ProgressStage::WritingPdf, page_count, total));
    }
    if page_count == 0 {
        return Err(PdfError::EmptyDocument);
    }
    if retained_text {
        pdf.set_tag_tree(tags);
    }
    observer(Progress::pending(ProgressStage::Finalizing));
    let bytes = pdf
        .finish()
        .map_err(|error| PdfError::Conversion(error.to_string()))?;
    if ink_fields.is_empty() {
        // Keep ordinary SVG/text exports byte-for-byte as Krilla serialized
        // them; only PDFs with registered marker images need lopdf rewriting.
        return Ok(bytes);
    }
    let fields = ink_fields
        .iter()
        .map(|(marker_id, payload, bounds)| match payload {
            NativeInkPayload::Mesh { mesh, color } => {
                ink_field::PdfInkField::mesh(*marker_id, mesh, *color, *bounds)
            }
            NativeInkPayload::Radial {
                records,
                saved_alpha,
                enhanced_antialias,
            } => ink_field::PdfInkField::radial(
                *marker_id,
                records,
                *saved_alpha,
                if *enhanced_antialias {
                    ink_field::PdfRadialCoverage::EnhancedMax
                } else {
                    ink_field::PdfRadialCoverage::DirectSourceOver
                },
                *bounds,
            ),
        })
        .collect::<Vec<_>>();
    ink_field::replace_markers(&bytes, &fields).map_err(|error| {
        PdfError::Conversion(format!("native ink PDF replacement failed: {error}"))
    })
}

fn draw_retained_page(
    surface: &mut krilla::surface::Surface<'_>,
    tree: &usvg::Tree,
    size: Size,
    registry: &NativeTextRegistry,
    ink_registry: &NativeInkRegistry,
    page_index: usize,
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
        nodes.insert(
            text.as_ref() as *const usvg::Text,
            (id, block, registry.visual_transform(id)),
        );
    }
    let mut ink_entries = HashMap::with_capacity(ink_registry.len());
    for (id, entry) in ink_registry.iter() {
        let count = count_node_ids(tree.root(), &id.svg_id());
        if count != 1 {
            return Err(SvgError::Ink(if count == 0 {
                format!("retained native ink {} was pruned from SVG", id.svg_id())
            } else {
                format!("retained native ink {} has duplicate SVG IDs", id.svg_id())
            }));
        }
        let Some(usvg::Node::Group(group)) = tree.node_by_id(&id.svg_id()) else {
            return Err(SvgError::Ink(format!(
                "retained native ink {} was lost during SVG parsing",
                id.svg_id()
            )));
        };
        ink_entries.insert(group.id().to_owned(), (id, entry));
    }
    let ink_ids = ink_entries
        .keys()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    let mut painted_ink = std::collections::HashSet::with_capacity(ink_entries.len());
    let mut tags = HashMap::with_capacity(registry.len());
    let mut ink_hook = |group: &usvg::Group, surface: &mut krilla::surface::Surface<'_>| {
        let Some((id, entry)) = ink_entries.get(group.id()) else {
            return Err("native ink group identity disappeared during traversal".into());
        };
        if group.opacity().get().to_bits() != (entry.opacity as f32).to_bits() {
            return Err("native ink SVG opacity no longer matches its typed record".into());
        }
        if !painted_ink.insert(id.svg_id()) {
            return Err("native ink group was encountered more than once".into());
        }
        let marker_id = id
            .marker_id(page_index)
            .ok_or("native ink marker ID exceeds the supported page or entry range")?;
        ink_field::emit_marker(surface, marker_id, entry.bounds)
    };
    surface.draw_svg_with_text_and_ink(
        tree,
        size,
        SvgSettings {
            reject_images: true,
            reject_rasterized_filters: true,
            ..Default::default()
        },
        &mut |text, surface| {
            let Some(&(id, block, visual_transform)) = nodes.get(&(text as *const usvg::Text))
            else {
                return Ok(false);
            };
            if tags.contains_key(&id) {
                return Err("retained text was painted more than once".into());
            }
            if let Some(transform) = visual_transform {
                let [a, b, c, d, e, f] = transform.inverse();
                surface.push_transform(&krilla::geom::Transform::from_row(
                    a as f32, b as f32, c as f32, d as f32, e as f32, f as f32,
                ));
                let result = painter.paint(block, surface);
                surface.pop();
                let tag = result.map_err(|error| error.to_string())?;
                tags.insert(id, tag);
                return Ok(true);
            }
            let tag = painter
                .paint(block, surface)
                .map_err(|error| error.to_string())?;
            tags.insert(id, tag);
            Ok(true)
        },
        &ink_ids,
        &mut ink_hook,
    )?;
    if painted_ink.len() != ink_registry.len() {
        return Err(SvgError::Ink(
            "a registered native ink group was skipped by SVG traversal".into(),
        ));
    }
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

fn count_node_ids(group: &usvg::Group, id: &str) -> usize {
    group
        .children()
        .iter()
        .fold(usize::from(group.id() == id), |count, child| {
            count
                + match child {
                    usvg::Node::Group(child_group) => count_node_ids(child_group, id),
                    _ => usize::from(child.id() == id),
                }
        })
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
    use crate::{BoundingBox, CoverageMesh};

    fn native_ink_registry() -> NativeInkRegistry {
        let mesh = CoverageMesh {
            triangles: vec![crate::CoverageTriangle {
                vertices: [
                    crate::CoverageVertex {
                        x: 10.0,
                        y: 10.0,
                        coverage: 0.0,
                    },
                    crate::CoverageVertex {
                        x: 30.0,
                        y: 10.0,
                        coverage: 0.5,
                    },
                    crate::CoverageVertex {
                        x: 10.0,
                        y: 30.0,
                        coverage: 1.0,
                    },
                ],
            }],
            sample_ends: vec![1],
            terminal_start: 1,
            terminal_end: 1,
            bounds: Some(BoundingBox {
                x_min: 10.0,
                y_min: 10.0,
                x_max: 30.0,
                y_max: 30.0,
            }),
        };
        let mut registry = NativeInkRegistry::default();
        registry.register(&mesh, [0x33, 0x66, 0x99], 0.5).unwrap();
        registry
    }

    #[test]
    fn retained_pdf_rejects_a_registered_but_pruned_native_ink_id() {
        let tree = usvg::Tree::from_str(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><g id="other"/></svg>"#,
            &usvg::Options::default(),
        )
        .unwrap();
        let mut document = PdfDocument::new();
        let mut page =
            document.start_page_with(PageSettings::new(Size::from_wh(100.0, 100.0).unwrap()));
        let mut surface = page.surface();
        let result = draw_retained_page(
            &mut surface,
            &tree,
            Size::from_wh(100.0, 100.0).unwrap(),
            &NativeTextRegistry::default(),
            &native_ink_registry(),
            0,
            &mut NativePdfPainter::new(72.0),
        );
        assert!(
            matches!(result, Err(SvgError::Ink(message)) if message.contains("pruned from SVG"))
        );
        surface.finish();
        page.finish();
        document.finish().unwrap();
    }

    #[test]
    fn retained_pdf_rejects_duplicate_native_ink_svg_ids() {
        let id = "sdocx-native-ink-0";
        let tree = usvg::Tree::from_str(
            &format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><g id="{id}"/><g id="{id}"/></svg>"#
            ),
            &usvg::Options::default(),
        )
        .unwrap();
        assert_eq!(count_node_ids(tree.root(), id), 2);
        let mut document = PdfDocument::new();
        let mut page =
            document.start_page_with(PageSettings::new(Size::from_wh(100.0, 100.0).unwrap()));
        let mut surface = page.surface();
        let result = draw_retained_page(
            &mut surface,
            &tree,
            Size::from_wh(100.0, 100.0).unwrap(),
            &NativeTextRegistry::default(),
            &native_ink_registry(),
            0,
            &mut NativePdfPainter::new(72.0),
        );
        assert!(
            matches!(result, Err(SvgError::Ink(message)) if message.contains("duplicate SVG IDs"))
        );
        surface.finish();
        page.finish();
        document.finish().unwrap();
    }

    #[test]
    fn retained_pdf_replaces_only_typed_ink_marker_and_keeps_real_images() {
        // The typed field is wholly outside the 100x100 SVG viewport. usvg
        // retains it, and the root clip must continue to hide the replacement
        // Form at the same invocation site.
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><filter id="fx"><feGaussianBlur stdDeviation="1"/></filter></defs><image x="1" y="1" width="4" height="3" href="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAQAAAADCAYAAAC09K7GAAAAGElEQVR4nGP4z8DwH4SRIJoAlA8EDGAMAGPrGef+nm3iAAAAAElFTkSuQmCC"/><g id="sdocx-native-ink-0" transform="translate(200 0)" opacity="0.5" filter="url(#fx)"><path d="M10 10L30 10L10 30Z" fill="#808080"/></g></svg>"##;
        let page = RenderedPage {
            source_page_index: 0,
            width: 100,
            height: 100,
            svg: svg.into(),
            text_diagnostics: Vec::new(),
            object_diagnostics: Vec::new(),
            geometry_diagnostics: Vec::new(),
            paint_diagnostics: Vec::new(),
            background_diagnostics: Vec::new(),
        };
        let ink = native_ink_registry();
        let bytes = render_pages_pdf(
            [PdfPage {
                page: &page,
                text: Some(&NativeTextRegistry::default()),
                ink: Some(&ink),
                text_error: None,
            }],
            &PdfOptions::default(),
        )
        .unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let (images, forms) = pdf
            .objects
            .values()
            .fold((0, 0), |(images, forms), object| {
                let Ok(stream) = object.as_stream() else {
                    return (images, forms);
                };
                match stream.dict.get(b"Subtype").and_then(lopdf::Object::as_name) {
                    Ok(b"Image") => (images + 1, forms),
                    Ok(b"Form") => (images, forms + 1),
                    _ => (images, forms),
                }
            });
        assert_eq!(images, 1, "only the original source image remains");
        assert!(
            forms >= 1,
            "the typed ink marker was replaced by a PDF Form"
        );
    }

    #[test]
    fn retained_pdf_qualifies_local_ink_ids_across_pages() {
        let svg = |color: &str| {
            format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><filter id="fx"><feGaussianBlur stdDeviation="1"/></filter></defs><g id="sdocx-native-ink-0" opacity="0.5" filter="url(#fx)"><path d="M10 10L30 10L10 30Z" fill="{color}"/></g></svg>"##
            )
        };
        let page = |svg: String| RenderedPage {
            source_page_index: 0,
            width: 100,
            height: 100,
            svg,
            text_diagnostics: Vec::new(),
            object_diagnostics: Vec::new(),
            geometry_diagnostics: Vec::new(),
            paint_diagnostics: Vec::new(),
            background_diagnostics: Vec::new(),
        };
        let red_page = page(svg("#ff0000"));
        let blue_page = page(svg("#0000ff"));
        // Both pages deliberately register local ID 0 with the same geometry,
        // but their distinct colors must survive marker replacement separately.
        let source_ink = native_ink_registry();
        let NativeInkPayload::Mesh { mesh, .. } = &source_ink.iter().next().unwrap().1.payload
        else {
            panic!("expected source mesh");
        };
        let mut red_ink = NativeInkRegistry::default();
        red_ink.register(mesh, [255, 0, 0], 0.5).unwrap();
        let mut blue_ink = NativeInkRegistry::default();
        blue_ink.register(mesh, [0, 0, 255], 0.5).unwrap();
        assert_eq!(
            red_ink.iter().next().unwrap().0.svg_id(),
            "sdocx-native-ink-0"
        );
        assert_eq!(
            blue_ink.iter().next().unwrap().0.svg_id(),
            "sdocx-native-ink-0"
        );
        assert_ne!(
            red_ink.iter().next().unwrap().0.marker_id(0),
            blue_ink.iter().next().unwrap().0.marker_id(1)
        );

        let text = NativeTextRegistry::default();
        let bytes = render_pages_pdf(
            [
                PdfPage {
                    page: &red_page,
                    text: Some(&text),
                    ink: Some(&red_ink),
                    text_error: None,
                },
                PdfPage {
                    page: &blue_page,
                    text: Some(&text),
                    ink: Some(&blue_ink),
                    text_error: None,
                },
            ],
            &PdfOptions::default(),
        )
        .unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let pages = pdf.get_pages();
        assert_eq!(pages.len(), 2);

        fn collect_form_colors(
            pdf: &lopdf::Document,
            xobjects: &lopdf::Dictionary,
            visited: &mut std::collections::HashSet<lopdf::ObjectId>,
            colors: &mut Vec<String>,
        ) {
            for object in xobjects.iter().map(|(_, object)| object) {
                let (id, object) = pdf.dereference(object).unwrap();
                if let Some(id) = id
                    && !visited.insert(id)
                {
                    continue;
                }
                let Ok(stream) = object.as_stream() else {
                    continue;
                };
                if stream.dict.get(b"Subtype").unwrap().as_name().unwrap() != b"Form" {
                    continue;
                }
                let content = String::from_utf8(stream.decompressed_content().unwrap()).unwrap();
                if content.contains(" rg ") {
                    colors.push(content);
                }
                if let Ok(resources) = stream.dict.get(b"Resources") {
                    let (_, resources) = pdf.dereference(resources).unwrap();
                    if let Ok(xobjects) = resources.as_dict().and_then(|r| r.get(b"XObject")) {
                        let (_, xobjects) = pdf.dereference(xobjects).unwrap();
                        collect_form_colors(pdf, xobjects.as_dict().unwrap(), visited, colors);
                    }
                }
            }
        }
        let page_form_color = |page_number: u32| {
            let page_dict = pdf.get_dictionary(pages[&page_number]).unwrap();
            let (_, resources) = pdf
                .dereference(page_dict.get(b"Resources").unwrap())
                .unwrap();
            let resources = resources.as_dict().unwrap();
            let (_, xobjects) = pdf.dereference(resources.get(b"XObject").unwrap()).unwrap();
            let mut colors = Vec::new();
            collect_form_colors(
                &pdf,
                xobjects.as_dict().unwrap(),
                &mut std::collections::HashSet::new(),
                &mut colors,
            );
            assert_eq!(colors.len(), 1, "each page must reference one field Form");
            colors.pop().unwrap()
        };
        let red = page_form_color(1);
        let blue = page_form_color(2);
        assert!(
            red.contains("1 0 0 rg"),
            "first page has its red field form: {red}"
        );
        assert!(
            blue.contains("0 0 1 rg"),
            "second page has its blue field form: {blue}"
        );
        assert_ne!(
            red, blue,
            "page resource targets must not alias by local ID"
        );
        assert!(
            pdf.objects.values().all(|object| object
                .as_stream()
                .ok()
                .and_then(|stream| stream.dict.get(b"Subtype").ok())
                .and_then(|subtype| subtype.as_name().ok())
                != Some(b"Image")),
            "all per-page markers must be removed after replacement"
        );
    }

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
