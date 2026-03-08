use std::io;

/// Errors that can occur when parsing an `.sdocx` file.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An I/O error occurred while reading the file.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// The file is not a valid ZIP archive.
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// The file contents do not match the expected `.sdocx` format.
    #[error("format error: {0}")]
    Format(String),
}

/// A specialized `Result` type for sdocx operations.
pub type Result<T> = std::result::Result<T, Error>;
