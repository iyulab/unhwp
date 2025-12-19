//! Format detection for HWP/HWPX documents.

use crate::error::{Error, Result};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Magic bytes for OLE Compound File (HWP 5.x)
const OLE_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// Magic bytes for ZIP archive (HWPX)
const ZIP_MAGIC: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];

/// ASCII signature for HWP 3.x
const HWP3_SIGNATURE: &[u8] = b"HWP Document File V";

/// ASCII signature for HWP 5.x (inside OLE container)
const HWP5_SIGNATURE: &[u8] = b"HWP Document File";

/// Supported document format types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatType {
    /// HWP 5.0+ binary format (OLE container)
    Hwp5,
    /// HWPX XML-based format (ZIP container)
    Hwpx,
    /// Legacy HWP 3.x format
    Hwp3,
}

impl std::fmt::Display for FormatType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatType::Hwp5 => write!(f, "HWP 5.0"),
            FormatType::Hwpx => write!(f, "HWPX"),
            FormatType::Hwp3 => write!(f, "HWP 3.x"),
        }
    }
}

/// Detect document format from a file path.
pub fn detect_format_from_path(path: impl AsRef<Path>) -> Result<FormatType> {
    let mut file = std::fs::File::open(path)?;
    detect_format(&mut file)
}

/// Detect document format from a reader.
pub fn detect_format<R: Read + Seek>(reader: &mut R) -> Result<FormatType> {
    let mut buffer = [0u8; 32];

    // Read first 32 bytes for detection
    reader.seek(SeekFrom::Start(0))?;
    let bytes_read = reader.read(&mut buffer)?;

    if bytes_read < 8 {
        return Err(Error::InvalidData("File too small".into()));
    }

    // Reset reader position
    reader.seek(SeekFrom::Start(0))?;

    // Check OLE container (HWP 5.x)
    if buffer[..8] == OLE_MAGIC {
        return Ok(FormatType::Hwp5);
    }

    // Check ZIP archive (HWPX)
    if buffer[..4] == ZIP_MAGIC {
        return Ok(FormatType::Hwpx);
    }

    // Check HWP 3.x signature
    if buffer.starts_with(HWP3_SIGNATURE) {
        return Ok(FormatType::Hwp3);
    }

    // Check for HWP 5.x without OLE (rare variant)
    if buffer.starts_with(HWP5_SIGNATURE) {
        return Ok(FormatType::Hwp5);
    }

    Err(Error::UnknownFormat)
}

/// Detect document format from bytes.
pub fn detect_format_from_bytes(data: &[u8]) -> Result<FormatType> {
    if data.len() < 8 {
        return Err(Error::InvalidData("Data too small".into()));
    }

    // Check OLE container (HWP 5.x)
    if data[..8] == OLE_MAGIC {
        return Ok(FormatType::Hwp5);
    }

    // Check ZIP archive (HWPX)
    if data.len() >= 4 && data[..4] == ZIP_MAGIC {
        return Ok(FormatType::Hwpx);
    }

    // Check HWP 3.x signature
    if data.starts_with(HWP3_SIGNATURE) {
        return Ok(FormatType::Hwp3);
    }

    // Check for HWP 5.x without OLE (rare variant)
    if data.starts_with(HWP5_SIGNATURE) {
        return Ok(FormatType::Hwp5);
    }

    Err(Error::UnknownFormat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_ole_magic() {
        let data = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0x00, 0x00];
        assert_eq!(detect_format_from_bytes(&data).unwrap(), FormatType::Hwp5);
    }

    #[test]
    fn test_detect_zip_magic() {
        let data = [0x50, 0x4B, 0x03, 0x04, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(detect_format_from_bytes(&data).unwrap(), FormatType::Hwpx);
    }

    #[test]
    fn test_detect_hwp3_signature() {
        let mut data = Vec::from(b"HWP Document File V3.0");
        data.resize(32, 0);
        assert_eq!(detect_format_from_bytes(&data).unwrap(), FormatType::Hwp3);
    }

    #[test]
    fn test_detect_unknown() {
        let data = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        assert!(matches!(
            detect_format_from_bytes(&data),
            Err(Error::UnknownFormat)
        ));
    }
}
