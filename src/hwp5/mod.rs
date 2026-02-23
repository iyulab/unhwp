//! HWP 5.0 binary format parser.
//!
//! This module handles parsing of HWP 5.0 documents stored in OLE containers.

mod bodytext;
mod container;
mod control;
mod docinfo;
mod header;
mod record;

pub use container::Hwp5Container;
pub use header::FileHeader;
pub use record::{Record, RecordHeader, RecordIterator, TagId};

use crate::error::Result;
use crate::model::Document;
use std::io::{Read, Seek};
use std::path::Path;

/// HWP 5.0 document parser.
pub struct Hwp5Parser {
    container: Hwp5Container,
    header: FileHeader,
}

impl Hwp5Parser {
    /// Opens an HWP 5.0 document from a file path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let container = Hwp5Container::open(path)?;
        let header = container.read_file_header()?;
        Ok(Self { container, header })
    }

    /// Opens an HWP 5.0 document from a reader.
    pub fn from_reader<R: Read + Seek>(reader: R) -> Result<Self> {
        let container = Hwp5Container::from_reader(reader)?;
        let header = container.read_file_header()?;
        Ok(Self { container, header })
    }

    /// Returns the file header.
    pub fn header(&self) -> &FileHeader {
        &self.header
    }

    /// Returns true if the document is compressed.
    pub fn is_compressed(&self) -> bool {
        self.header.is_compressed()
    }

    /// Returns true if the document is encrypted.
    pub fn is_encrypted(&self) -> bool {
        self.header.is_encrypted()
    }

    /// Parses the document into the unified document model.
    pub fn parse(&mut self) -> Result<Document> {
        if self.is_encrypted() {
            return Err(crate::error::Error::Encrypted);
        }

        let mut document = Document::new();

        // Set format version
        document.metadata.format_version = Some(self.header.version_string());

        // Set distribution flag
        document.metadata.is_distribution = self.header.is_distribution();

        // Parse metadata from SummaryInformation (best-effort, ignore errors)
        let _ = self.parse_metadata(&mut document);

        // Parse DocInfo for styles
        self.parse_docinfo(&mut document)?;

        // Parse BodyText sections
        self.parse_bodytext(&mut document)?;

        // Extract BinData resources
        self.extract_bindata(&mut document)?;

        Ok(document)
    }

    /// Parses DocInfo stream for style definitions.
    fn parse_docinfo(&self, document: &mut Document) -> Result<()> {
        let data = self
            .container
            .read_stream_decompressed("DocInfo", self.is_compressed())?;

        docinfo::parse_docinfo(&data, &mut document.styles)?;
        Ok(())
    }

    /// Parses BodyText sections.
    /// Parses BodyText sections sequentially to share picture counter across sections.
    fn parse_bodytext(&self, document: &mut Document) -> Result<()> {
        let section_names = self.container.list_bodytext_sections()?;
        let is_compressed = self.is_compressed();

        // Read all section data first
        let section_data: Vec<(usize, Vec<u8>)> = section_names
            .iter()
            .enumerate()
            .filter_map(|(index, name)| {
                self.container
                    .read_stream_decompressed(name, is_compressed)
                    .ok()
                    .map(|data| (index, data))
            })
            .collect();

        let styles = document.styles.clone();

        // Parse sections sequentially to share picture_counter across sections.
        // This ensures BinId references remain correct even in multi-section documents.
        let mut picture_counter: u32 = 0;
        let mut sections: Vec<_> = section_data
            .iter()
            .filter_map(|(index, data)| {
                bodytext::parse_section(data, *index, &styles, &mut picture_counter).ok()
            })
            .collect();

        // Sort by index to maintain order
        sections.sort_by_key(|s| s.index);

        document.sections = sections;
        Ok(())
    }

    /// Extracts binary resources from BinData storage.
    fn extract_bindata(&self, document: &mut Document) -> Result<()> {
        let resources = self.container.list_bindata()?;

        for name in resources {
            if let Ok(data) = self.container.read_bindata(&name, self.is_compressed()) {
                let mime_type = guess_mime_type(&name);
                let size = data.len();
                let resource = crate::model::Resource {
                    resource_type: crate::model::ResourceType::Image,
                    filename: Some(name.clone()),
                    mime_type,
                    data,
                    size,
                };
                document.resources.insert(name, resource);
            }
        }

        Ok(())
    }

    /// Extracts document metadata from the `\x05HwpSummaryInformation` OLE stream.
    ///
    /// The stream follows the Microsoft OLE Property Set format (FMTID_SummaryInformation).
    /// Properties of interest:
    ///   PID_TITLE    = 2  → title
    ///   PID_SUBJECT  = 3  → subject
    ///   PID_AUTHOR   = 4  → author
    ///   PID_KEYWORDS = 5  → keywords (comma-separated)
    ///   PID_CREATE   = 12 → created date (FILETIME)
    ///   PID_LASTSAVE = 13 → modified date (FILETIME)
    fn parse_metadata(&self, document: &mut Document) -> Result<()> {
        // The OLE compound file stores it with the special prefix byte 0x05
        let data = self
            .container
            .read_stream_raw("\x05HwpSummaryInformation")
            .or_else(|_| self.container.read_stream_raw("SummaryInformation"))?;

        // OLE Property Set header:
        // 0x00-0x01: byte order (FE FF = little endian)
        // 0x02-0x03: version
        // 0x04-0x07: OS version
        // 0x08-0x17: FMTID (16 bytes)
        // 0x18-0x1B: section offset count (u32)
        // Then for each section: FMTID (16 bytes) + offset (u32)
        // At the section offset: size (u32) + property count (u32) + [propId (u32) + offset (u32)]*
        if data.len() < 0x30 {
            return Ok(());
        }

        // Byte order mark: FE FF = little endian
        if data[0] != 0xFE || data[1] != 0xFF {
            return Ok(());
        }

        // Number of property sets (typically 1 or 2)
        let num_sets = u32::from_le_bytes(get4(&data, 0x18)) as usize;
        if num_sets == 0 {
            return Ok(());
        }

        // First set FMTID is at 0x1C, offset at 0x2C
        let section_offset = u32::from_le_bytes(get4(&data, 0x2C)) as usize;
        if section_offset + 8 > data.len() {
            return Ok(());
        }

        let section_data = &data[section_offset..];
        let prop_count = u32::from_le_bytes(get4(section_data, 4)) as usize;

        if section_data.len() < 8 + prop_count * 8 {
            return Ok(());
        }

        // Build property ID → offset map
        for i in 0..prop_count {
            let entry_off = 8 + i * 8;
            let prop_id = u32::from_le_bytes(get4(section_data, entry_off));
            let prop_off = u32::from_le_bytes(get4(section_data, entry_off + 4)) as usize;

            if prop_off + 4 > section_data.len() {
                continue;
            }

            let vt_type = u16::from_le_bytes([section_data[prop_off], section_data[prop_off + 1]]);

            match prop_id {
                2..=5 => {
                    // VT_LPSTR = 0x001E, VT_LPWSTR = 0x001F
                    if let Some(s) = read_ole_string(section_data, prop_off, vt_type) {
                        match prop_id {
                            2 => document.metadata.title = Some(s),
                            3 => document.metadata.subject = Some(s),
                            4 => document.metadata.author = Some(s),
                            5 => {
                                document.metadata.keywords = s
                                    .split(',')
                                    .map(|k| k.trim().to_string())
                                    .filter(|k| !k.is_empty())
                                    .collect();
                            }
                            _ => {}
                        }
                    }
                }
                12 | 13 => {
                    // VT_FILETIME = 0x0040
                    if vt_type == 0x0040 && prop_off + 12 <= section_data.len() {
                        let lo = u32::from_le_bytes(get4(section_data, prop_off + 4)) as u64;
                        let hi = u32::from_le_bytes(get4(section_data, prop_off + 8)) as u64;
                        let filetime = (hi << 32) | lo;
                        let iso = filetime_to_iso8601(filetime);
                        match prop_id {
                            12 => document.metadata.created = Some(iso),
                            13 => document.metadata.modified = Some(iso),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

/// Reads 4 bytes as a little-endian u32 from a slice at the given offset.
#[inline]
fn get4(data: &[u8], offset: usize) -> [u8; 4] {
    if offset + 4 <= data.len() {
        [data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]
    } else {
        [0u8; 4]
    }
}

/// Reads a string property from an OLE Property Set section.
///
/// Supports VT_LPSTR (0x001E, CP_ACP 8-bit) and VT_LPWSTR (0x001F, UTF-16LE).
fn read_ole_string(section_data: &[u8], prop_off: usize, vt_type: u16) -> Option<String> {
    match vt_type {
        0x001E => {
            // VT_LPSTR: 2-byte type + 2-byte padding + 4-byte count + bytes
            let count = u32::from_le_bytes(get4(section_data, prop_off + 4)) as usize;
            let start = prop_off + 8;
            let end = start.checked_add(count)?;
            if end > section_data.len() {
                return None;
            }
            // Trim trailing NUL bytes
            let bytes = &section_data[start..end];
            let trimmed = bytes.iter().position(|&b| b == 0).map_or(bytes, |n| &bytes[..n]);
            // Attempt Windows-1252 (most common for Korean HWP metadata)
            Some(String::from_utf8_lossy(trimmed).into_owned())
        }
        0x001F => {
            // VT_LPWSTR: 2-byte type + 2-byte padding + 4-byte count (in WCHARs, incl. NUL) + UTF-16LE
            let wchar_count = u32::from_le_bytes(get4(section_data, prop_off + 4)) as usize;
            let start = prop_off + 8;
            let byte_count = wchar_count.saturating_sub(1) * 2; // exclude trailing NUL
            let end = start.checked_add(byte_count)?;
            if end > section_data.len() {
                return None;
            }
            let u16_vals: Vec<u16> = section_data[start..end]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            Some(String::from_utf16_lossy(&u16_vals))
        }
        _ => None,
    }
}

/// Converts a Windows FILETIME (100-nanosecond intervals since 1601-01-01) to ISO 8601.
fn filetime_to_iso8601(filetime: u64) -> String {
    // Convert to Unix epoch: FILETIME epoch is 1601-01-01, Unix is 1970-01-01
    // Difference = 116444736000000000 × 100ns intervals
    const EPOCH_DIFF: u64 = 116_444_736_000_000_000;
    if filetime < EPOCH_DIFF {
        return String::from("1601-01-01T00:00:00Z");
    }
    let unix_100ns = filetime - EPOCH_DIFF;
    let unix_secs = unix_100ns / 10_000_000;

    // Simple date computation (no external crate)
    let secs = unix_secs;
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let h = time_secs / 3600;
    let m = (time_secs % 3600) / 60;
    let s = time_secs % 60;

    // Compute year/month/day from days since 1970-01-01
    let (year, month, day) = days_to_ymd(days);

    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, h, m, s)
}

/// Converts days since Unix epoch to (year, month, day).
fn days_to_ymd(mut days: u64) -> (u32, u32, u32) {
    let mut year = 1970u32;
    loop {
        let leap = is_leap(year);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days: [u64; 12] = [
        31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    let mut month = 1u32;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days as u32 + 1)
}

#[inline]
fn is_leap(year: u32) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

/// Guesses MIME type from filename extension.
fn guess_mime_type(filename: &str) -> Option<String> {
    let ext = filename.rsplit('.').next()?.to_lowercase();
    match ext.as_str() {
        "bmp" => Some("image/bmp".to_string()),
        "jpg" | "jpeg" => Some("image/jpeg".to_string()),
        "png" => Some("image/png".to_string()),
        "gif" => Some("image/gif".to_string()),
        "tif" | "tiff" => Some("image/tiff".to_string()),
        "wmf" => Some("image/x-wmf".to_string()),
        "emf" => Some("image/x-emf".to_string()),
        _ => None,
    }
}
