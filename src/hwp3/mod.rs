//! HWP 3.x format parser.
//!
//! HWP 3.x is a legacy binary format used by Hangul Word Processor 3.x.
//! It uses a simple binary structure without OLE containers.
//!
//! ## Format Overview
//!
//! - **Header**: Fixed 128-byte header with signature "HWP Document File V3.0"
//! - **Document Info**: Document properties and settings
//! - **Font Table**: Font definitions
//! - **Style Table**: Paragraph and character styles
//! - **Body**: Document content with control codes
//!
//! ## Text Encoding
//!
//! HWP 3.x files use EUC-KR (or CP949) encoding for Korean text.

mod header;
mod body;

pub use header::{Hwp3Header, Hwp3Version};
pub use body::BodyParser;

use crate::error::Result;
use crate::model::Document;
use encoding_rs::EUC_KR;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// HWP 3.x document parser.
pub struct Hwp3Parser<R> {
    reader: R,
    header: Hwp3Header,
}

impl Hwp3Parser<std::io::BufReader<std::fs::File>> {
    /// Opens an HWP 3.x file for parsing.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        Self::from_reader(reader)
    }
}

impl<R: Read + Seek> Hwp3Parser<R> {
    /// Creates a parser from a reader.
    pub fn from_reader(mut reader: R) -> Result<Self> {
        reader.seek(SeekFrom::Start(0))?;
        let header = header::parse_header(&mut reader)?;
        Ok(Self { reader, header })
    }

    /// Parses the document and returns the IR model.
    pub fn parse(&mut self) -> Result<Document> {
        let mut document = Document::new();

        // Parse document content
        let body_parser = BodyParser::new(&self.header);
        body_parser.parse(&mut self.reader, &mut document)?;

        Ok(document)
    }

    /// Returns the document header.
    pub fn header(&self) -> &Hwp3Header {
        &self.header
    }
}

/// Decodes EUC-KR/CP949 bytes to UTF-8 string.
pub fn decode_euckr(data: &[u8]) -> String {
    let (decoded, _, _) = EUC_KR.decode(data);
    decoded.into_owned()
}

/// Decodes a null-terminated EUC-KR string.
pub fn decode_euckr_cstr(data: &[u8]) -> String {
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    decode_euckr(&data[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_euckr() {
        // "한글" in EUC-KR: 0xC7, 0xD1, 0xB1, 0xDB
        let data = [0xC7, 0xD1, 0xB1, 0xDB];
        let decoded = decode_euckr(&data);
        assert_eq!(decoded, "한글");
    }

    #[test]
    fn test_decode_euckr_cstr() {
        // "한글" with null terminator
        let data = [0xC7, 0xD1, 0xB1, 0xDB, 0x00, 0xFF, 0xFF];
        let decoded = decode_euckr_cstr(&data);
        assert_eq!(decoded, "한글");
    }

    #[test]
    fn test_decode_ascii() {
        let data = b"Hello World";
        let decoded = decode_euckr(data);
        assert_eq!(decoded, "Hello World");
    }
}
