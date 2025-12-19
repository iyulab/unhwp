//! HWP 3.x header parsing.
//!
//! The HWP 3.x header is a fixed 128-byte structure at the start of the file.

use crate::error::{Error, Result};
use std::io::{Read, Seek, SeekFrom};

/// HWP 3.x file signature.
pub const HWP3_SIGNATURE: &[u8] = b"HWP Document File V";

/// Header size in bytes.
pub const HEADER_SIZE: usize = 128;

/// HWP 3.x document version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hwp3Version {
    /// Major version (e.g., 3 for 3.0)
    pub major: u8,
    /// Minor version (e.g., 0 for 3.0)
    pub minor: u8,
}

impl std::fmt::Display for Hwp3Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// HWP 3.x document header.
#[derive(Debug, Clone)]
pub struct Hwp3Header {
    /// Document version
    pub version: Hwp3Version,
    /// Document compressed flag
    pub compressed: bool,
    /// Document encrypted flag
    pub encrypted: bool,
    /// Number of pages
    pub page_count: u16,
    /// Document info block offset
    pub info_block_offset: u32,
    /// Document info block size
    pub info_block_size: u32,
    /// Summary info offset
    pub summary_offset: u32,
    /// Summary info size
    pub summary_size: u32,
    /// Body text offset
    pub body_offset: u32,
    /// Body text size
    pub body_size: u32,
}

impl Default for Hwp3Header {
    fn default() -> Self {
        Self {
            version: Hwp3Version { major: 3, minor: 0 },
            compressed: false,
            encrypted: false,
            page_count: 0,
            info_block_offset: 0,
            info_block_size: 0,
            summary_offset: 0,
            summary_size: 0,
            body_offset: 0,
            body_size: 0,
        }
    }
}

/// Parses the HWP 3.x header from a reader.
pub fn parse_header<R: Read + Seek>(reader: &mut R) -> Result<Hwp3Header> {
    reader.seek(SeekFrom::Start(0))?;

    let mut buffer = [0u8; HEADER_SIZE];
    reader.read_exact(&mut buffer)?;

    // Check signature
    if !buffer.starts_with(HWP3_SIGNATURE) {
        return Err(Error::InvalidData(
            "Invalid HWP 3.x signature".into(),
        ));
    }

    // Parse version from signature (e.g., "HWP Document File V3.0")
    let version = parse_version(&buffer[0..30])?;

    // Parse header fields (offsets are approximate based on format)
    // Offset 30: Flags byte
    let flags = buffer[30];
    let compressed = (flags & 0x01) != 0;
    let encrypted = (flags & 0x02) != 0;

    if encrypted {
        return Err(Error::UnsupportedFormat(
            "Encrypted HWP 3.x files are not supported".into(),
        ));
    }

    // Read field offsets and sizes from header
    // These offsets are based on HWP 3.0 format specification
    let info_block_offset = read_u32_le(&buffer[32..36]);
    let info_block_size = read_u32_le(&buffer[36..40]);
    let summary_offset = read_u32_le(&buffer[40..44]);
    let summary_size = read_u32_le(&buffer[44..48]);
    let body_offset = read_u32_le(&buffer[96..100]);
    let body_size = read_u32_le(&buffer[100..104]);
    let page_count = read_u16_le(&buffer[48..50]);

    Ok(Hwp3Header {
        version,
        compressed,
        encrypted,
        page_count,
        info_block_offset,
        info_block_size,
        summary_offset,
        summary_size,
        body_offset,
        body_size,
    })
}

/// Parses version from signature string.
fn parse_version(signature: &[u8]) -> Result<Hwp3Version> {
    // Find version number after 'V'
    let sig_str = std::str::from_utf8(signature)
        .map_err(|_| Error::InvalidData("Invalid signature encoding".into()))?;

    if let Some(v_pos) = sig_str.find('V') {
        let version_part = &sig_str[v_pos + 1..];
        let version_str: String = version_part
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();

        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() >= 2 {
            let major = parts[0].parse().unwrap_or(3);
            let minor = parts[1].parse().unwrap_or(0);
            return Ok(Hwp3Version { major, minor });
        }
    }

    // Default to 3.0 if parsing fails
    Ok(Hwp3Version { major: 3, minor: 0 })
}

/// Reads a little-endian u16.
fn read_u16_le(data: &[u8]) -> u16 {
    u16::from_le_bytes([data[0], data[1]])
}

/// Reads a little-endian u32.
fn read_u32_le(data: &[u8]) -> u32 {
    u32::from_le_bytes([data[0], data[1], data[2], data[3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        let sig = b"HWP Document File V3.0\x00\x00\x00\x00\x00\x00\x00\x00";
        let version = parse_version(sig).unwrap();
        assert_eq!(version.major, 3);
        assert_eq!(version.minor, 0);
    }

    #[test]
    fn test_version_display() {
        let version = Hwp3Version { major: 3, minor: 1 };
        assert_eq!(format!("{}", version), "3.1");
    }

    #[test]
    fn test_read_u16_le() {
        assert_eq!(read_u16_le(&[0x01, 0x02]), 0x0201);
    }

    #[test]
    fn test_read_u32_le() {
        assert_eq!(read_u32_le(&[0x01, 0x02, 0x03, 0x04]), 0x04030201);
    }
}
