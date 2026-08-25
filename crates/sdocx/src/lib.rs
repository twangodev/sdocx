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
mod page;
mod storage;
mod types;

pub use error::{Error, Result};
pub use storage::{
    StoredLayer, StoredObject, StoredPage, StoredPageHeader, StoredPageLayers,
    parse_stored_page_bytes, parse_stored_page_bytes_with_limits,
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

/// Parse a `.sdocx` file from in-memory bytes.
pub fn parse_bytes(bytes: &[u8]) -> Result<Document> {
    parse_bytes_with_options(bytes, &ParseOptions::default())
}

/// Parse in-memory `.sdocx` bytes with explicit options.
pub fn parse_bytes_with_options(bytes: &[u8], options: &ParseOptions) -> Result<Document> {
    let cursor = Cursor::new(bytes);
    container::parse_from_reader(cursor, options)
}
