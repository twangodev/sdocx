use serde::Serialize;
use sha2::{Digest, Sha256};
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
            parsed: Some(parsed),
            layout: Some(layout),
            page_count,
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
        sdocx::render_layout_page_svg(&parsed.document, self.layout()?, page_index, &options)
            .map(|page| page.svg)
            .ok_or_else(|| JsError::new("page index is out of bounds"))
    }

    /// Release the parsed document before the JavaScript wrapper is collected.
    pub fn dispose(&mut self) {
        self.parsed = None;
        self.layout = None;
        self.page_count = 0;
    }
}

impl DocumentSession {
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
    sdocx::ParseOptions { limits }
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
        MAX_BROWSER_ENTRY_SIZE, MAX_BROWSER_TOTAL_UNCOMPRESSED_SIZE, browser_parse_options,
        parse_render_color_mode,
    };

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
}
