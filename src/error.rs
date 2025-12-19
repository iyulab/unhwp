//! Error types for unhwp library.

use std::io;
use thiserror::Error;

/// Result type alias for unhwp operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for unhwp library.
#[derive(Error, Debug)]
pub enum Error {
    /// I/O error during file operations.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// The file format is not recognized.
    #[error("Unknown file format")]
    UnknownFormat,

    /// The file format is not supported (e.g., HWP 2.x).
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    /// The document is encrypted and cannot be parsed.
    #[error("Document is encrypted")]
    Encrypted,

    /// The document is a distribution document with restrictions.
    #[error("Document is a restricted distribution document")]
    DistributionRestricted,

    /// OLE container parsing error.
    #[error("OLE container error: {0}")]
    OleContainer(String),

    /// ZIP archive parsing error.
    #[error("ZIP archive error: {0}")]
    ZipArchive(String),

    /// Decompression error.
    #[error("Decompression error: {0}")]
    Decompression(String),

    /// Record parsing error in HWP 5.0 format.
    #[error("Record parsing error at offset {offset}: {message}")]
    RecordParse { offset: u64, message: String },

    /// XML parsing error in HWPX format.
    #[error("XML parsing error: {0}")]
    XmlParse(String),

    /// Invalid or malformed data.
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Required stream or file is missing.
    #[error("Missing required component: {0}")]
    MissingComponent(String),

    /// Text encoding error.
    #[error("Text encoding error: {0}")]
    Encoding(String),

    /// Style reference not found.
    #[error("Style reference not found: {0}")]
    StyleNotFound(u32),

    /// Resource (image, etc.) not found.
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),
}

// Note: cfb crate returns io::Error, not a custom error type

impl From<zip::result::ZipError> for Error {
    fn from(err: zip::result::ZipError) -> Self {
        Error::ZipArchive(err.to_string())
    }
}

impl From<quick_xml::Error> for Error {
    fn from(err: quick_xml::Error) -> Self {
        Error::XmlParse(err.to_string())
    }
}

// Note: quick_xml::DeError requires the "serialize" feature, which is not enabled

impl From<std::string::FromUtf16Error> for Error {
    fn from(err: std::string::FromUtf16Error) -> Self {
        Error::Encoding(err.to_string())
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(err: std::str::Utf8Error) -> Self {
        Error::Encoding(err.to_string())
    }
}
