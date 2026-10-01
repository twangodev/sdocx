mod debugger;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

const MAX_BROWSER_INPUT_SIZE: usize = 250 * 1024 * 1024;
const MAX_BROWSER_ENTRY_SIZE: u64 = 256 * 1024 * 1024;
const MAX_BROWSER_TOTAL_UNCOMPRESSED_SIZE: u64 = 1024 * 1024 * 1024;

/// Parse a `.sdocx` file from bytes.
///
/// Accepts a `Uint8Array` and returns a `Document` object.
#[wasm_bindgen]
pub fn parse(bytes: &[u8]) -> Result<JsValue, JsError> {
    let doc = sdocx::parse_bytes(bytes).map_err(|e| JsError::new(&e.to_string()))?;
    serde_wasm_bindgen::to_value(&doc).map_err(|e| JsError::new(&e.to_string()))
}

/// A parse-once document session for browser inspection and rendering.
#[wasm_bindgen]
pub struct DocumentSession {
    parsed: Option<sdocx::ParsedDocument>,
    layout: Option<sdocx::LayoutDocument>,
    page_count: usize,
    fonts: sdocx::fonts::FontBook,
    text_cache: RefCell<sdocx::DocumentTextCache>,
    debugger: Option<debugger::Source>,
}

#[wasm_bindgen]
impl DocumentSession {
    /// Parse a `.sdocx` document using browser-specific resource limits.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<DocumentSession, JsError> {
        if bytes.len() > MAX_BROWSER_INPUT_SIZE {
            return Err(JsError::new("input exceeds the browser limit of 250 MiB"));
        }

        let options = browser_parse_options();
        let parsed = sdocx::parse_bytes_detailed_with_options(bytes, &options)
            .map_err(|error| JsError::new(&error.to_string()))?;
        let layout = sdocx::layout_document(&parsed.document);
        let page_count = layout.pages.len();
        Ok(Self {
            debugger: Some(debugger::Source::new(bytes)?),
            parsed: Some(parsed),
            layout: Some(layout),
            page_count,
            fonts: sdocx::fonts::FontBook::default(),
            text_cache: RefCell::new(sdocx::DocumentTextCache::default()),
        })
    }

    /// Number of visible pages available for preview or export.
    #[wasm_bindgen(getter)]
    pub fn page_count(&self) -> usize {
        self.page_count
    }

    /// Return the parsed document, visible layout, media summaries, and diagnostics.
    pub fn inspection(&self) -> Result<JsValue, JsError> {
        let parsed = self.parsed()?;
        inspection_value(parsed, self.layout()?).map_err(|error| JsError::new(&error.to_string()))
    }

    /// Render one visible page as a standalone SVG document.
    pub fn render_svg(&self, page_index: usize, color_mode: &str) -> Result<String, JsError> {
        let parsed = self.parsed()?;
        let mut options = sdocx::RenderOptions::default();
        options.color_mode = parse_render_color_mode(color_mode)?;
        self.text_cache
            .borrow_mut()
            .render_layout_page_svg(
                &parsed.document,
                self.layout()?,
                page_index,
                &options,
                &self.fonts,
            )
            .map(|page| page.svg)
            .ok_or_else(|| JsError::new("page index is out of bounds"))
    }

    /// Add a TTF/OTF font for preview, replay, and PDF export.
    pub fn add_pdf_font(&mut self, bytes: &[u8]) -> Result<(), JsError> {
        self.parsed()?;
        self.add_font(bytes)
            .map_err(|message| JsError::new(&message))
    }

    /// Export all visible pages, or one zero-based page, using the CLI's PDF engine.
    /// Returns PDF bytes with bundled fonts and any supplied additional faces.
    pub fn render_pdf(
        &self,
        page_index: Option<usize>,
        color_mode: &str,
    ) -> Result<Vec<u8>, JsError> {
        let indices = match page_index {
            Some(index) => vec![
                u32::try_from(index).map_err(|_| JsError::new("page index is out of bounds"))?,
            ],
            None => (0..self.page_count as u32).collect(),
        };
        self.render_pdf_pages(&indices, color_mode)
    }

    /// Resolve a one-based range expression into sorted, unique zero-based indices.
    pub fn resolve_pages(&self, selection: &str) -> Result<Vec<u32>, JsError> {
        self.parsed()?;
        sdocx::parse_page_selection(selection, self.page_count)
            .map(|indices| indices.into_iter().map(|index| index as u32).collect())
            .map_err(|error| JsError::new(&error.to_string()))
    }

    /// Export explicit zero-based visible pages in the supplied order.
    pub fn render_pdf_pages(
        &self,
        page_indices: &[u32],
        color_mode: &str,
    ) -> Result<Vec<u8>, JsError> {
        let parsed = self.parsed()?;
        let layout = self.layout()?;
        let mut options = sdocx::RenderOptions::default();
        options.color_mode = parse_render_color_mode(color_mode)?;
        let page_indices = page_indices
            .iter()
            .map(|&index| index as usize)
            .collect::<Vec<_>>();
        if page_indices
            .iter()
            .any(|&index| index >= layout.pages.len())
        {
            return Err(JsError::new("page index is out of bounds"));
        }
        let pdf_options = sdocx::PdfOptions::new(self.fonts.database());
        sdocx::render_layout_pages_pdf_detailed_with_cache(
            &parsed.document,
            layout,
            &page_indices,
            &options,
            &pdf_options,
            &self.fonts,
            &mut self.text_cache.borrow_mut(),
        )
        .map(|output| output.bytes)
        .map_err(|error| JsError::new(&error.to_string()))
    }

    /// Lazy debugger request. Large integers are returned as decimal strings.
    pub fn debug(&mut self, request: &str) -> Result<String, JsError> {
        let parsed = self
            .parsed
            .as_ref()
            .ok_or_else(|| JsError::new("session disposed"))?;
        let layout = self
            .layout
            .as_ref()
            .ok_or_else(|| JsError::new("session disposed"))?;
        self.debugger
            .as_mut()
            .ok_or_else(|| JsError::new("session disposed"))?
            .request_with_text_cache(
                parsed,
                layout,
                request,
                &self.fonts,
                self.text_cache.get_mut(),
            )
            .map_err(|e| JsError::new(&e))
    }

    /// Release the parsed document before the JavaScript wrapper is collected.
    pub fn dispose(&mut self) {
        self.fonts =
            sdocx::fonts::FontBook::new(std::sync::Arc::new(sdocx::fonts::fontdb::Database::new()));
        self.debugger = None;
        self.text_cache.get_mut().clear();
        self.parsed = None;
        self.layout = None;
        self.page_count = 0;
    }
}

impl DocumentSession {
    fn add_font(&mut self, bytes: &[u8]) -> Result<(), String> {
        let mut database = self.fonts.database();
        let before = database.faces().count();
        std::sync::Arc::make_mut(&mut database).load_font_data(bytes.to_vec());
        if database.faces().count() == before {
            return Err("no usable PDF font faces in the supplied data".into());
        }
        self.fonts = sdocx::fonts::FontBook::new(database);
        self.text_cache.get_mut().clear();
        Ok(())
    }

    fn parsed(&self) -> Result<&sdocx::ParsedDocument, JsError> {
        self.parsed
            .as_ref()
            .ok_or_else(|| JsError::new("document session has been disposed"))
    }

    fn layout(&self) -> Result<&sdocx::LayoutDocument, JsError> {
        self.layout
            .as_ref()
            .ok_or_else(|| JsError::new("document session has been disposed"))
    }
}

fn browser_parse_options() -> sdocx::ParseOptions {
    let limits = sdocx::ParseLimits {
        max_entry_size: MAX_BROWSER_ENTRY_SIZE,
        max_total_uncompressed_size: MAX_BROWSER_TOTAL_UNCOMPRESSED_SIZE,
        ..sdocx::ParseLimits::default()
    };
    sdocx::ParseOptions {
        limits,
        ..Default::default()
    }
}

fn parse_render_color_mode(value: &str) -> Result<sdocx::RenderColorMode, JsError> {
    match value {
        "auto" => Ok(sdocx::RenderColorMode::Auto),
        "light" => Ok(sdocx::RenderColorMode::Light),
        "dark" => Ok(sdocx::RenderColorMode::Dark),
        _ => Err(JsError::new("color mode must be one of: auto, light, dark")),
    }
}

#[derive(Serialize)]
struct Inspection<'a> {
    document: InspectionDocument<'a>,
    layout: &'a sdocx::LayoutDocument,
    stored_page_count: usize,
    page_manifest: &'a Option<sdocx::PageManifest>,
    report: &'a sdocx::ParseReport,
}

#[derive(Serialize)]
struct InspectionDocument<'a> {
    pages: &'a [sdocx::Page],
    metadata: InspectionMetadata<'a>,
}

#[derive(Serialize)]
struct InspectionMetadata<'a> {
    format_version: Option<sdocx::FormatVersion>,
    created_ms: Option<i64>,
    modified_ms: Option<i64>,
    background_color: Option<sdocx::Color>,
    dark_mode_compatibility: Option<bool>,
    page_dimensions: Option<(u32, u32)>,
    default_page_dimensions: Option<(u32, u32)>,
    page_mode: Option<u16>,
    orientation: Option<i32>,
    flow_dimensions: Option<(u32, u32)>,
    flow_page_padding: Option<(u32, u32)>,
    page_ids: &'a [String],
    media_assets: Vec<MediaAssetSummary<'a>>,
    note_text: &'a Option<sdocx::RichTextBox>,
    note_title: &'a Option<sdocx::RichTextBox>,
}

#[derive(Serialize)]
struct MediaAssetSummary<'a> {
    name: &'a str,
    archive_id: Option<u32>,
    mime_type: &'a str,
    byte_length: usize,
    sha256: String,
}

fn inspection_value(
    parsed: &sdocx::ParsedDocument,
    layout: &sdocx::LayoutDocument,
) -> Result<JsValue, serde_wasm_bindgen::Error> {
    let document = &parsed.document;
    let metadata = &document.metadata;
    let media_assets = metadata
        .media_assets
        .iter()
        .map(|asset| {
            let digest = Sha256::digest(&asset.data);
            MediaAssetSummary {
                name: &asset.name,
                archive_id: asset.archive_id,
                mime_type: &asset.mime_type,
                byte_length: asset.data.len(),
                sha256: format!("{digest:x}"),
            }
        })
        .collect::<Vec<_>>();
    let inspection = Inspection {
        document: InspectionDocument {
            pages: &document.pages,
            metadata: InspectionMetadata {
                format_version: metadata.format_version,
                created_ms: metadata.created_ms,
                modified_ms: metadata.modified_ms,
                background_color: metadata.background_color,
                dark_mode_compatibility: metadata.dark_mode_compatibility,
                page_dimensions: metadata.page_dimensions,
                default_page_dimensions: metadata.default_page_dimensions,
                page_mode: metadata.page_mode,
                orientation: metadata.orientation,
                flow_dimensions: metadata.flow_dimensions,
                flow_page_padding: metadata.flow_page_padding,
                page_ids: &metadata.page_ids,
                media_assets,
                note_text: &metadata.note_text,
                note_title: &metadata.note_title,
            },
        },
        layout,
        stored_page_count: parsed.stored_pages.len(),
        page_manifest: &parsed.page_manifest,
        report: &parsed.report,
    };
    serde_wasm_bindgen::to_value(&inspection)
}

#[cfg(test)]
mod tests {
    use super::{
        DocumentSession, MAX_BROWSER_ENTRY_SIZE, MAX_BROWSER_TOTAL_UNCOMPRESSED_SIZE,
        browser_parse_options, parse_render_color_mode,
    };
    use sdocx::fonts::{FontBook, fontdb};
    use std::sync::Arc;

    #[test]
    fn browser_options_bound_archive_expansion() {
        let options = browser_parse_options();
        assert_eq!(options.limits.max_entry_size, MAX_BROWSER_ENTRY_SIZE);
        assert_eq!(
            options.limits.max_total_uncompressed_size,
            MAX_BROWSER_TOTAL_UNCOMPRESSED_SIZE
        );
        assert_eq!(
            options.limits.max_pages,
            sdocx::ParseLimits::default().max_pages
        );
    }

    #[test]
    fn render_color_modes_use_stable_string_names() {
        assert_eq!(
            parse_render_color_mode("auto").unwrap(),
            sdocx::RenderColorMode::Auto
        );
        assert_eq!(
            parse_render_color_mode("light").unwrap(),
            sdocx::RenderColorMode::Light
        );
        assert_eq!(
            parse_render_color_mode("dark").unwrap(),
            sdocx::RenderColorMode::Dark
        );
    }

    #[test]
    fn added_fonts_replace_cached_fallbacks_for_preview_replay_and_pdf() {
        let page = crate::debugger::support::page(&[Vec::new()], 0, &[]);
        let archive = crate::debugger::support::archive(&page);
        let mut session = DocumentSession::new(&archive).unwrap();
        let mut database = fontdb::Database::new();
        database
            .load_font_data(include_bytes!("../../sdocx/assets/fonts/Roboto-Regular.ttf").to_vec());
        database.set_sans_serif_family("Roboto");
        session.fonts = FontBook::new(Arc::new(database));
        let original_database = session.fonts.database();

        let text = "iiiiiiii iiiiiiii";
        let mut payload = vec![0; 8];
        payload.extend(12_u16.to_le_bytes());
        payload.extend(b"Roboto Mono\0");
        let text_box = sdocx::RichTextBox {
            text_area_type: None,
            bbox: Default::default(),
            rotation_degrees: None,
            text: text.into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: Some(20.0),
            runs: Vec::new(),
            spans: vec![sdocx::RichTextSpan {
                kind: sdocx::RichTextSpanType::FontName,
                start_utf16: 0,
                end_utf16: text.encode_utf16().count() as u32,
                interval_type: sdocx::SpanIntervalType::from(0),
                payload,
            }],
            paragraphs: Vec::new(),
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        };
        let parsed = session.parsed.as_mut().unwrap();
        parsed.document.pages[0].width = 160;
        parsed.document.metadata.flow_page_padding = Some((10, 0));
        parsed.document.pages[0]
            .objects
            .push(sdocx::PageElement::TextBox(text_box).into());
        session.layout = Some(sdocx::layout_document(&parsed.document));

        let before = session.render_svg(0, "light").unwrap();
        assert!(before.contains("@font-face{font-family:\"Roboto\""));
        assert!(!before.contains("Roboto Mono"));
        session
            .add_pdf_font(include_bytes!(
                "../../sdocx/assets/fonts/RobotoMono-Regular.ttf"
            ))
            .unwrap();
        assert_eq!(original_database.faces().count(), 1);
        assert_eq!(session.fonts.database().faces().count(), 2);
        assert!(!Arc::ptr_eq(&original_database, &session.fonts.database()));
        assert_eq!(
            session
                .fonts
                .resolve("Roboto Mono", false, false)
                .unwrap()
                .family,
            "Roboto Mono"
        );

        let preview = session.render_svg(0, "light").unwrap();
        assert!(preview.contains("@font-face{font-family:\"Roboto Mono\""));
        assert_ne!(preview, before);
        for kind in ["replay-svg", "background"] {
            let response = session
                .debug(&format!(
                    r#"{{"kind":"{kind}","page":0,"colorMode":"light"}}"#
                ))
                .unwrap();
            let response: serde_json::Value = serde_json::from_str(&response).unwrap();
            assert_eq!(response["svg"].as_str().unwrap(), preview, "{kind}");
        }
        let pdf = session.render_pdf(Some(0), "light").unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        assert!(
            pdf.windows(b"RobotoMono-Regular".len())
                .any(|window| window == b"RobotoMono-Regular")
        );
        let repeated_pdf = session.render_pdf_pages(&[0], "light").unwrap();
        assert_eq!(repeated_pdf, pdf);
        session.dispose();
        assert_eq!(session.fonts.database().faces().count(), 0);
    }

    fn body_session(text: &str, family: &str) -> DocumentSession {
        let page = crate::debugger::support::page(&[Vec::new()], 0, &[]);
        let archive = crate::debugger::support::archive(&page);
        let mut session = DocumentSession::new(&archive).unwrap();
        let mut database = fontdb::Database::new();
        database
            .load_font_data(include_bytes!("../../sdocx/assets/fonts/Roboto-Regular.ttf").to_vec());
        database.set_sans_serif_family("Roboto");
        session.fonts = FontBook::new(Arc::new(database));
        let mut payload = vec![0; 8];
        payload.extend(((family.len() + 1) as u16).to_le_bytes());
        payload.extend(family.as_bytes());
        payload.push(0);
        let parsed = session.parsed.as_mut().unwrap();
        parsed.document.pages[0].width = 260;
        parsed.document.pages[0].height = 120;
        parsed.document.pages = vec![parsed.document.pages[0].clone(); 3];
        parsed.document.metadata.flow_page_padding = Some((10, 0));
        parsed.document.metadata.note_text = Some(sdocx::RichTextBox {
            text_area_type: None,
            bbox: Default::default(),
            rotation_degrees: None,
            text: text.into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: Some(20.0),
            runs: Vec::new(),
            spans: vec![sdocx::RichTextSpan {
                kind: sdocx::RichTextSpanType::FontName,
                start_utf16: 0,
                end_utf16: text.encode_utf16().count() as u32,
                interval_type: sdocx::SpanIntervalType::from(0),
                payload,
            }],
            paragraphs: Vec::new(),
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        });
        session.layout = Some(sdocx::layout_document(&parsed.document));
        session.page_count = session.layout.as_ref().unwrap().pages.len();
        session
    }

    #[test]
    fn body_preparation_preserves_cold_and_warm_pages_across_themes() {
        let mut session = body_session(&"alpha beta gamma delta epsilon ".repeat(80), "Roboto");
        assert_eq!(session.page_count(), 3);
        for color_mode in ["light", "dark", "auto", "light"] {
            let mut options = sdocx::RenderOptions::default();
            options.color_mode = parse_render_color_mode(color_mode).unwrap();
            for page_index in [2, 0, 1, 2] {
                let expected = sdocx::render_layout_page_svg_with_fonts(
                    &session.parsed.as_ref().unwrap().document,
                    session.layout.as_ref().unwrap(),
                    page_index,
                    &options,
                    &session.fonts,
                )
                .unwrap();
                assert_eq!(
                    session.render_svg(page_index, color_mode).unwrap(),
                    expected.svg
                );
                assert_eq!(
                    session.render_svg(page_index, color_mode).unwrap(),
                    expected.svg
                );
            }
            let preview = session.render_svg(0, color_mode).unwrap();
            for kind in ["replay-svg", "background"] {
                let response = session
                    .debug(&format!(
                        r#"{{"kind":"{kind}","page":0,"colorMode":"{color_mode}"}}"#,
                    ))
                    .unwrap();
                let response: serde_json::Value = serde_json::from_str(&response).unwrap();
                assert_eq!(response["svg"].as_str().unwrap(), preview);
            }
        }
        let warm_pdf = session.render_pdf_pages(&[2, 0, 2], "light").unwrap();
        let cold_session = body_session(&"alpha beta gamma delta epsilon ".repeat(80), "Roboto");
        assert_eq!(
            cold_session.render_pdf_pages(&[2, 0, 2], "light").unwrap(),
            warm_pdf
        );
    }

    #[test]
    fn body_preparation_is_isolated_and_invalidated_only_after_successful_font_import() {
        let text = "iiiiiiii iiiiiiii ".repeat(60);
        let mut first = body_session(&text, "Roboto Mono");
        let second = body_session(&text, "Roboto Mono");
        let before = first.render_svg(1, "light").unwrap();
        assert_eq!(second.render_svg(1, "light").unwrap(), before);
        let original_database = first.fonts.database();
        let warm_references = Arc::strong_count(&original_database);
        assert!(first.add_font(b"invalid font bytes").is_err());
        assert!(Arc::ptr_eq(&original_database, &first.fonts.database()));
        assert_eq!(Arc::strong_count(&original_database), warm_references);
        assert_eq!(first.render_svg(1, "light").unwrap(), before);
        first
            .add_pdf_font(include_bytes!(
                "../../sdocx/assets/fonts/RobotoMono-Regular.ttf"
            ))
            .unwrap();
        assert_eq!(Arc::strong_count(&original_database), 1);
        let after = first.render_svg(1, "light").unwrap();
        assert_ne!(after, before);
        assert!(after.contains("@font-face{font-family:\"Roboto Mono\""));
        assert_eq!(second.render_svg(1, "light").unwrap(), before);
        let imported_database = first.fonts.database();
        first.dispose();
        assert_eq!(first.page_count(), 0);
        assert!(first.parsed.is_none());
        assert!(first.layout.is_none());
        assert!(first.debugger.is_none());
        assert_eq!(first.fonts.database().faces().count(), 0);
        assert_eq!(Arc::strong_count(&imported_database), 1);
        assert_eq!(second.render_svg(1, "light").unwrap(), before);
    }
}
