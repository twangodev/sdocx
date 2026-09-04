//! Reverse-engineered parser for Samsung Notes `.sdocx` documents.
//!
//! # Accuracy and compatibility
//!
//! Archive structure, format versions, page ordering, and supported packed
//! stroke channels are decoded from observed S Pen SDK contracts. Higher-level
//! page objects, rich text, templates, and media associations remain
//! best-effort. A successful parse can omit unsupported content and does not
//! indicate a lossless decode. Preserve the source document when fidelity is
//! important.

mod binary;
mod container;
mod decode;
mod error;
mod frame;
mod layout;
mod note;
mod object;
mod page;
#[cfg(feature = "render")]
mod render;
mod report;
mod storage;
mod types;

pub use error::{Error, Result};
pub use layout::{LayoutDocument, LayoutPage, layout_document};
pub use note::{StoredNote, StoredNoteHeader, parse_note_bytes, parse_note_bytes_with_limits};
pub use object::ObjectMetadata;
#[cfg(feature = "render")]
pub use render::{
    RenderColorMode, RenderOptions, RenderedPage, render_document_svg, render_layout_page_svg,
    render_page_svg,
};
pub use report::{DiagnosticCode, DiagnosticSeverity, ParseDiagnostic, ParseReport};
pub use storage::{
    PageManifest, PageManifestEntry, ParsedDocument, StoredArchivePage, StoredLayer, StoredObject,
    StoredPage, StoredPageHeader, StoredPageLayers, parse_page_manifest_bytes,
    parse_page_manifest_bytes_with_limits, parse_stored_page_bytes,
    parse_stored_page_bytes_with_limits,
};
pub use types::*;

use std::fs::File;
use std::io::Cursor;
use std::path::Path;

/// Resource limits applied while parsing untrusted `.sdocx` input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    /// Maximum number of entries in the ZIP archive.
    pub max_archive_entries: usize,
    /// Maximum declared or decoded size of one archive entry.
    pub max_entry_size: u64,
    /// Maximum total declared uncompressed size across archive entries.
    pub max_total_uncompressed_size: u64,
    /// Maximum number of pages.
    pub max_pages: usize,
    /// Maximum number of strokes declared by one page.
    pub max_strokes_per_page: usize,
    /// Maximum number of points declared by one stroke.
    pub max_points_per_stroke: usize,
    /// Maximum number of non-stroke objects detected on one page.
    pub max_objects_per_page: usize,
    /// Maximum number of physical layers declared by one page.
    pub max_layers_per_page: usize,
    /// Maximum nesting depth for child object records.
    pub max_object_nesting_depth: usize,
    /// Maximum UTF-16 code units in one rich-text object.
    pub max_text_characters: usize,
    /// Maximum style spans in one rich-text object.
    pub max_text_spans: usize,
    /// Maximum paragraph records in one rich-text object.
    pub max_text_paragraphs: usize,
    /// Maximum embedded object spans in one rich-text object.
    pub max_text_object_spans: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_archive_entries: 65_536,
            max_entry_size: 512 * 1024 * 1024,
            max_total_uncompressed_size: 2 * 1024 * 1024 * 1024,
            max_pages: 10_000,
            max_strokes_per_page: 100_000,
            max_points_per_stroke: u16::MAX as usize,
            max_objects_per_page: 100_000,
            max_layers_per_page: 64,
            max_object_nesting_depth: 64,
            max_text_characters: 250_000,
            max_text_spans: 10_000,
            max_text_paragraphs: 10_000,
            max_text_object_spans: 10_000,
        }
    }
}

/// Options controlling `.sdocx` parsing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParseOptions {
    /// Resource limits for untrusted input.
    pub limits: ParseLimits,
}

/// Parse a `.sdocx` file from a filesystem path.
pub fn parse(path: impl AsRef<Path>) -> Result<Document> {
    parse_with_options(path, &ParseOptions::default())
}

/// Parse a `.sdocx` file from a filesystem path with explicit options.
pub fn parse_with_options(path: impl AsRef<Path>, options: &ParseOptions) -> Result<Document> {
    let file = File::open(path)?;
    container::parse_from_reader(file, options)
}

/// Parse a `.sdocx` file and retain its physical page structure and diagnostics.
pub fn parse_detailed(path: impl AsRef<Path>) -> Result<ParsedDocument> {
    parse_detailed_with_options(path, &ParseOptions::default())
}

/// Parse a `.sdocx` file in detail with explicit options.
pub fn parse_detailed_with_options(
    path: impl AsRef<Path>,
    options: &ParseOptions,
) -> Result<ParsedDocument> {
    let file = File::open(path)?;
    container::parse_detailed_from_reader(file, options)
}

/// Parse a `.sdocx` file from in-memory bytes.
pub fn parse_bytes(bytes: &[u8]) -> Result<Document> {
    parse_bytes_with_options(bytes, &ParseOptions::default())
}

/// Parse in-memory `.sdocx` bytes with explicit options.
pub fn parse_bytes_with_options(bytes: &[u8], options: &ParseOptions) -> Result<Document> {
    let cursor = Cursor::new(bytes);
    container::parse_from_reader(cursor, options)
}

/// Parse in-memory `.sdocx` bytes and retain physical structure and diagnostics.
pub fn parse_bytes_detailed(bytes: &[u8]) -> Result<ParsedDocument> {
    parse_bytes_detailed_with_options(bytes, &ParseOptions::default())
}

/// Parse in-memory `.sdocx` bytes in detail with explicit options.
pub fn parse_bytes_detailed_with_options(
    bytes: &[u8],
    options: &ParseOptions,
) -> Result<ParsedDocument> {
    let cursor = Cursor::new(bytes);
    container::parse_detailed_from_reader(cursor, options)
}
