mod container;
mod decode;
mod error;
mod page;
mod types;

pub use error::{Error, Result};
pub use types::*;

use std::fs::File;
use std::io::Cursor;
use std::path::Path;

/// Parse a `.sdocx` file from a filesystem path.
pub fn parse(path: impl AsRef<Path>) -> Result<Document> {
    let file = File::open(path)?;
    container::parse_from_reader(file)
}

/// Parse a `.sdocx` file from in-memory bytes.
pub fn parse_bytes(bytes: &[u8]) -> Result<Document> {
    let cursor = Cursor::new(bytes);
    container::parse_from_reader(cursor)
}
