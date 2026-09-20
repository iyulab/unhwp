//! OLE container wrapper for HWP 5.0 documents.
//!
//! # Memory Usage
//!
//! The current implementation loads the entire OLE container into memory.
//! This is a limitation of the underlying `cfb` crate which requires the
//! full file to be available for parsing the compound file structure.
//!
//! For very large files (100MB+), consider:
//! - Processing documents in batches
//! - Using the BinData lazy loading methods to avoid loading all resources
//! - Extracting only the required sections/resources
//!
//! # Future Improvements
//!
//! Memory-mapped file support could reduce memory usage for large files
//! by allowing the OS to manage page loading.

use crate::error::{Error, Result};
use cfb::CompoundFile;
use flate2::read::DeflateDecoder;
use std::cell::RefCell;
use std::io::{self, Cursor, Read, Seek};
use std::path::Path;

/// OLE container wrapper for HWP 5.0 documents.
///
/// # Memory Model
///
/// The container loads the full OLE file into memory on open.
/// Individual streams (DocInfo, BodyText, BinData) are read on-demand.
/// Use `list_bindata()` and `read_bindata()` for lazy resource loading.
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
        // Not `?`: every failure here is the container refusing to parse, which is what
        // `Error::OleContainer` exists for. The whole file is already in memory, so an
        // `io::Error` from this layer never means an I/O problem -- reporting it as
        // `Error::Io` would tell the caller to check their disk.
        let cfb = CompoundFile::open(cursor)
            .map_err(|e| Error::OleContainer(format!("cannot read the OLE container: {e}")))?;
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

        // A stream that is absent and a stream the container cannot deliver are different
        // answers: the first says this document does not have that part, the second says
        // the container is damaged. Collapsing both into `MissingComponent` reported a
        // corrupt file as an incomplete one.
        let mut stream = cfb.open_stream(name).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => Error::MissingComponent(name.to_string()),
            _ => Error::OleContainer(format!("cannot open stream {name}: {e}")),
        })?;

        let mut data = Vec::new();
        stream
            .read_to_end(&mut data)
            .map_err(|e| Error::OleContainer(format!("cannot read stream {name}: {e}")))?;
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

    pub fn list_bindata(&self) -> Result<Vec<String>> {
        let cfb = self.cfb.borrow_mut();

        // Check if BinData storage exists
        if !cfb.is_storage("/BinData") {
            return Ok(Vec::new());
        }

        let mut resources = Vec::new();
        for entry in cfb
            // Absence was already ruled out by `is_storage` above, so a failure here is
            // the container, not a missing part.
            .read_storage("/BinData")
            .map_err(|e| Error::OleContainer(format!("cannot read the BinData storage: {e}")))?
        {
            if entry.is_stream() {
                resources.push(entry.name().to_string());
            }
        }

        // Sort for consistent ordering (BIN0001.xxx, BIN0002.xxx, etc.)
        resources.sort();

        Ok(resources)
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
        .as_chunks::<2>()
        .0
        .iter()
        .copied()
        .map(u16::from_le_bytes);

    String::from_utf16(&u16_iter.collect::<Vec<_>>()).map_err(|e| Error::Encoding(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal CFB container holding the named streams.
    fn cfb_with_streams(names: &[&str]) -> Vec<u8> {
        let mut container =
            CompoundFile::create(Cursor::new(Vec::new())).expect("create container");
        for name in names {
            container.create_stream(name).expect("create stream");
        }
        container.flush().expect("flush");
        container.into_inner().into_inner()
    }

    /// The same container with its directory sector overwritten: the header still says
    /// "OLE container", and nothing past it can be read.
    fn cfb_with_unreadable_directory() -> Vec<u8> {
        let mut data = cfb_with_streams(&["FileHeader"]);
        let sector_size = 1usize << u16::from_le_bytes(data[0x1E..0x20].try_into().unwrap());
        let first_dir_sector = u32::from_le_bytes(data[0x30..0x34].try_into().unwrap()) as usize;
        // The header occupies sector 0, so sector N begins one sector later.
        let start = sector_size * (first_dir_sector + 1);
        data[start..start + sector_size].fill(0xFF);
        data
    }

    /// `ErrorKind::OleContainer` is this crate's own discriminant for exactly this case,
    /// and it is exposed to C, C# and Python. Reporting a damaged container as `Io` told
    /// those callers to check their disk for a file that was already fully in memory.
    #[test]
    fn a_damaged_container_is_an_ole_container_error_not_an_io_error() {
        // `Hwp5Container` is not `Debug` (it owns the open container), so no `expect_err`.
        let Err(err) = Hwp5Container::from_bytes(cfb_with_unreadable_directory()) else {
            panic!("a container whose directory cannot be read must not open");
        };

        assert_eq!(err.kind(), crate::ErrorKind::OleContainer, "got: {err}");
    }

    /// The distinction the stream reader has to keep: a part this document does not have,
    /// versus a part the container cannot deliver.
    #[test]
    fn an_absent_stream_is_missing_not_a_container_error() {
        let container =
            Hwp5Container::from_bytes(cfb_with_streams(&["FileHeader"])).expect("opens");

        let err = container
            .read_stream_raw("DocInfo")
            .expect_err("DocInfo is not in this container");

        assert_eq!(err.kind(), crate::ErrorKind::MissingComponent, "got: {err}");
    }

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
