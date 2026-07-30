//! Error types for unhwp library.

use std::io;
use thiserror::Error;

/// Result type alias for unhwp operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Stable classification of an [`Error`], for consumers that must branch on the
/// *reason* a call failed rather than match on its message text.
///
/// Every [`Error`] variant maps onto one of these, so the mapping needs no judgement
/// and stays obvious as the error type grows. The discriminants are explicit and part
/// of the public contract: they cross the C-ABI boundary as `unhwp_last_error_kind`
/// return values, so **existing values must never be renumbered** — a new failure
/// reason takes the next free number instead. Treat an unrecognised value as a
/// generic failure rather than as an error.
///
/// Values are drawn from the `un*` family's shared numbering
/// ([`uncore::kind`] docs): `1..=12` mirror [`undoc`](https://docs.rs/undoc)'s
/// discriminants of the same name (the closest sibling — nine of them are shared),
/// and `400..=403` are unhwp's own band. Values `100` and above are reserved for
/// FFI-boundary reasons that have no core `Error` counterpart (null arguments, caught
/// panics, output that cannot cross the ABI); see the `UNHWP_ERROR_*` constants in the
/// `ffi` module.
///
/// This enum is `#[non_exhaustive]`: match it with a `_ =>` arm and treat an unfamiliar
/// reason as a generic failure. That is the same contract the C, C# and Python surfaces
/// document, and it is what lets a later release name a new reason without breaking
/// callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[repr(i32)]
pub enum ErrorKind {
    /// A failure with no more specific classification.
    ///
    /// [`Error::kind`] never returns this — no core variant maps to it. It exists so
    /// that bindings and consumers have a value for a failure that did not come from
    /// this library and therefore carries no classification. It must not be confused
    /// with success, which is the absence of an error (`0`).
    Other = 1,
    /// [`Error::Io`]
    Io = 2,
    /// [`Error::UnknownFormat`]
    UnknownFormat = 3,
    /// [`Error::UnsupportedFormat`]
    UnsupportedFormat = 4,
    /// [`Error::ZipArchive`]
    ZipArchive = 5,
    /// [`Error::XmlParse`]
    XmlParse = 6,
    /// [`Error::InvalidData`]
    InvalidData = 7,
    /// [`Error::MissingComponent`]
    MissingComponent = 8,
    /// [`Error::Encoding`]
    Encoding = 9,
    /// [`Error::StyleNotFound`]
    StyleNotFound = 10,
    /// [`Error::ResourceNotFound`]
    ResourceNotFound = 11,
    /// [`Error::Encrypted`]
    Encrypted = 12,
    /// [`Error::Decompression`] — unhwp's own band ([`uncore::kind::library_band`]`(2)`).
    Decompression = 400,
    /// [`Error::OleContainer`] — HWP 5.0's CFB container is unique to this format.
    OleContainer = 401,
    /// [`Error::RecordParse`] — HWP 5.0's binary record structure is unique to this format.
    RecordParse = 402,
    /// [`Error::DistributionRestricted`] — HWP's distribution-restriction flag has no
    /// counterpart in the sibling formats.
    DistributionRestricted = 403,
}

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

impl Error {
    /// Classify this error into a stable [`ErrorKind`].
    ///
    /// Lets a caller branch on *why* an operation failed without matching on the
    /// message text. The message stays the human-readable diagnostic; the kind is
    /// additive.
    ///
    /// # Example
    /// ```
    /// use unhwp::{Error, ErrorKind};
    ///
    /// let err = Error::UnknownFormat;
    /// assert_eq!(err.kind(), ErrorKind::UnknownFormat);
    /// ```
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::Io(_) => ErrorKind::Io,
            Error::UnknownFormat => ErrorKind::UnknownFormat,
            Error::UnsupportedFormat(_) => ErrorKind::UnsupportedFormat,
            Error::Encrypted => ErrorKind::Encrypted,
            Error::DistributionRestricted => ErrorKind::DistributionRestricted,
            Error::OleContainer(_) => ErrorKind::OleContainer,
            Error::ZipArchive(_) => ErrorKind::ZipArchive,
            Error::Decompression(_) => ErrorKind::Decompression,
            Error::RecordParse { .. } => ErrorKind::RecordParse,
            Error::XmlParse(_) => ErrorKind::XmlParse,
            Error::InvalidData(_) => ErrorKind::InvalidData,
            Error::MissingComponent(_) => ErrorKind::MissingComponent,
            Error::Encoding(_) => ErrorKind::Encoding,
            Error::StyleNotFound(_) => ErrorKind::StyleNotFound,
            Error::ResourceNotFound(_) => ErrorKind::ResourceNotFound,
        }
    }
}

#[cfg(test)]
mod kind_tests {
    use super::*;

    #[test]
    fn every_variant_maps_to_its_named_kind() {
        assert_eq!(Error::UnknownFormat.kind(), ErrorKind::UnknownFormat);
        assert_eq!(
            Error::UnsupportedFormat("HWP 2.x".into()).kind(),
            ErrorKind::UnsupportedFormat
        );
        assert_eq!(Error::Encrypted.kind(), ErrorKind::Encrypted);
        assert_eq!(
            Error::DistributionRestricted.kind(),
            ErrorKind::DistributionRestricted
        );
        assert_eq!(
            Error::OleContainer("bad sector chain".into()).kind(),
            ErrorKind::OleContainer
        );
        assert_eq!(
            Error::ZipArchive("bad central directory".into()).kind(),
            ErrorKind::ZipArchive
        );
        assert_eq!(
            Error::Decompression("inflate failed".into()).kind(),
            ErrorKind::Decompression
        );
        assert_eq!(
            Error::RecordParse {
                offset: 0,
                message: "truncated".into()
            }
            .kind(),
            ErrorKind::RecordParse
        );
        assert_eq!(
            Error::XmlParse("unexpected eof".into()).kind(),
            ErrorKind::XmlParse
        );
        assert_eq!(
            Error::InvalidData("bad".into()).kind(),
            ErrorKind::InvalidData
        );
        assert_eq!(
            Error::MissingComponent("Contents/header.xml".into()).kind(),
            ErrorKind::MissingComponent
        );
        assert_eq!(
            Error::Encoding("bad utf-16".into()).kind(),
            ErrorKind::Encoding
        );
        assert_eq!(Error::StyleNotFound(7).kind(), ErrorKind::StyleNotFound);
        assert_eq!(
            Error::ResourceNotFound("BIN0001.jpg".into()).kind(),
            ErrorKind::ResourceNotFound
        );
    }

    #[test]
    fn io_error_maps_to_io_kind() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert_eq!(err.kind(), ErrorKind::Io);
    }

    // The discriminants are a public ABI contract: they cross the C boundary as
    // `unhwp_last_error_kind` values. Pinning every one of them here — via the same
    // macro `undoc` and `unpdf` will adopt — is what makes an accidental renumbering
    // a test failure instead of a silent consumer break.
    uncore::assert_stable_kinds! {
        ErrorKind, error_kind_discriminants_are_stable,
        Other = 1,
        Io = 2,
        UnknownFormat = 3,
        UnsupportedFormat = 4,
        ZipArchive = 5,
        XmlParse = 6,
        InvalidData = 7,
        MissingComponent = 8,
        Encoding = 9,
        StyleNotFound = 10,
        ResourceNotFound = 11,
        Encrypted = 12,
        Decompression = 400,
        OleContainer = 401,
        RecordParse = 402,
        DistributionRestricted = 403,
    }
}
