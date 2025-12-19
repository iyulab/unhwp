//! OLE container wrapper for HWP 5.0 documents.

use crate::error::{Error, Result};
use cfb::CompoundFile;
use flate2::read::DeflateDecoder;
use std::cell::RefCell;
use std::io::{Cursor, Read, Seek};
use std::path::Path;

/// OLE container wrapper for HWP 5.0 documents.
pub struct Hwp5Container {
    cfb: RefCell<CompoundFile<Cursor<Vec<u8>>>>,
}

impl Hwp5Container {
    /// Opens an HWP 5.0 container from a file path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_bytes(data)
    }

    /// Opens an HWP 5.0 container from a reader.
    pub fn from_reader<R: Read + Seek>(mut reader: R) -> Result<Self> {
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        Self::from_bytes(data)
    }

    /// Opens an HWP 5.0 container from bytes.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let cursor = Cursor::new(data);
        let cfb = CompoundFile::open(cursor)?;
        Ok(Self {
            cfb: RefCell::new(cfb),
        })
    }

    /// Reads the FileHeader stream (always uncompressed).
    pub fn read_file_header(&self) -> Result<super::FileHeader> {
        let data = self.read_stream_raw("FileHeader")?;
        super::FileHeader::parse(&data)
    }

    /// Reads a raw stream without decompression.
    pub fn read_stream_raw(&self, name: &str) -> Result<Vec<u8>> {
        let mut cfb = self.cfb.borrow_mut();

        let mut stream = cfb
            .open_stream(name)
            .map_err(|_| Error::MissingComponent(name.to_string()))?;

        let mut data = Vec::new();
        stream.read_to_end(&mut data)?;
        Ok(data)
    }

    /// Reads a stream with optional decompression.
    pub fn read_stream_decompressed(&self, name: &str, compressed: bool) -> Result<Vec<u8>> {
        let raw = self.read_stream_raw(name)?;

        if compressed {
            decompress_stream(&raw)
        } else {
            Ok(raw)
        }
    }

    /// Lists all BodyText section streams.
    pub fn list_bodytext_sections(&self) -> Result<Vec<String>> {
        let mut sections = Vec::new();
        let mut index = 0;

        loop {
            let name = format!("BodyText/Section{}", index);
            // Check if stream exists by trying to read it
            if self.read_stream_raw(&name).is_ok() {
                sections.push(name);
                index += 1;
            } else {
                break;
            }
        }

        if sections.is_empty() {
            return Err(Error::MissingComponent("BodyText".into()));
        }

        Ok(sections)
    }

    /// Lists all BinData entries.
    pub fn list_bindata(&self) -> Result<Vec<String>> {
        // BinData entries are typically named BIN0001.xxx, BIN0002.xxx, etc.
        // For now, return empty - will implement when iterating storage
        Ok(Vec::new())
    }

    /// Reads a BinData entry.
    pub fn read_bindata(&self, name: &str, compressed: bool) -> Result<Vec<u8>> {
        let full_path = format!("BinData/{}", name);
        self.read_stream_decompressed(&full_path, compressed)
    }

    /// Checks if a stream exists.
    pub fn stream_exists(&self, name: &str) -> bool {
        self.read_stream_raw(name).is_ok()
    }

    /// Reads the preview text (PrvText) if available.
    pub fn read_preview_text(&self) -> Result<String> {
        let data = self.read_stream_raw("PrvText")?;
        // PrvText is UTF-16LE encoded
        decode_utf16le(&data)
    }
}

/// Decompresses a stream using raw deflate.
fn decompress_stream(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = DeflateDecoder::new(data);
    let mut output = Vec::new();

    decoder
        .read_to_end(&mut output)
        .map_err(|e| Error::Decompression(e.to_string()))?;

    Ok(output)
}

/// Decodes UTF-16LE bytes to a String.
fn decode_utf16le(data: &[u8]) -> Result<String> {
    if !data.len().is_multiple_of(2) {
        return Err(Error::Encoding("Invalid UTF-16LE data length".into()));
    }

    let u16_iter = data
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]));

    String::from_utf16(&u16_iter.collect::<Vec<_>>()).map_err(|e| Error::Encoding(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_utf16le() {
        // "Hello" in UTF-16LE
        let data = [0x48, 0x00, 0x65, 0x00, 0x6C, 0x00, 0x6C, 0x00, 0x6F, 0x00];
        assert_eq!(decode_utf16le(&data).unwrap(), "Hello");
    }

    #[test]
    fn test_decode_utf16le_korean() {
        // "안녕" in UTF-16LE
        let data = [0x48, 0xC5, 0x55, 0xB1]; // 안(0xC548), 녕(0xB155)
        assert_eq!(decode_utf16le(&data).unwrap(), "안녕");
    }
}
