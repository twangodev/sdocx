use std::io;

/// Errors that can occur when parsing an `.sdocx` file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
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

    /// The file uses Samsung Notes document protection and must be unlocked first.
    #[error("protected Samsung Notes document; unlock or export it before parsing")]
    ProtectedDocument,

    /// A configurable parser resource limit was exceeded.
    #[error("{resource} limit exceeded: {actual} > {limit}")]
    LimitExceeded {
        /// Resource whose limit was exceeded.
        resource: &'static str,
        /// Configured maximum.
        limit: u64,
        /// Value found in the input.
        actual: u64,
    },
}

/// A specialized `Result` type for sdocx operations.
pub type Result<T> = std::result::Result<T, Error>;
