//! Record parsing for HWP 5.0 streams.
//!
//! HWP 5.0 uses a TLV (Tag-Length-Value) record format with 4-byte headers.

use crate::error::{Error, Result};

/// Tag IDs for HWP 5.0 records.
/// Based on HWPTAG_BEGIN = 0x10 (16)
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagId {
    // DocInfo tags (0x10 - 0x31)
    DocumentProperties = 16,
    IdMappings = 17,
    BinData = 18,
    FaceName = 19,
    BorderFill = 20,
    CharShape = 21,
    TabDef = 22,
    Numbering = 23,
    Bullet = 24,
    ParaShape = 25,
    Style = 26,
    DocData = 27,
    DistributeDocData = 28,
    CompatibleDocument = 30,
    LayoutCompatibility = 31,

    // BodyText tags (0x32 - 0x50+)
    ParaHeader = 50,
    ParaText = 51,
    ParaCharShape = 52,
    ParaLineSeg = 53,
    ParaRangeTag = 54,
    CtrlHeader = 55,
    ListHeader = 56,
    PageDef = 57,
    FootnoteShape = 58,
    PageBorderFill = 59,

    // Extended control tags
    ShapeComponent = 60,
    Table = 61,
    ShapeComponentLine = 62,
    ShapeComponentRectangle = 63,
    ShapeComponentEllipse = 64,
    ShapeComponentArc = 65,
    ShapeComponentPolygon = 66,
    ShapeComponentCurve = 67,
    ShapeComponentOle = 68,
    ShapeComponentPicture = 69,
    ShapeComponentContainer = 70,
    CtrlData = 71,
    EqEdit = 72,

    // Unknown tag
    Unknown = 0xFFFF,
}

impl From<u16> for TagId {
    fn from(value: u16) -> Self {
        match value {
            16 => TagId::DocumentProperties,
            17 => TagId::IdMappings,
            18 => TagId::BinData,
            19 => TagId::FaceName,
            20 => TagId::BorderFill,
            21 => TagId::CharShape,
            22 => TagId::TabDef,
            23 => TagId::Numbering,
            24 => TagId::Bullet,
            25 => TagId::ParaShape,
            26 => TagId::Style,
            27 => TagId::DocData,
            28 => TagId::DistributeDocData,
            30 => TagId::CompatibleDocument,
            31 => TagId::LayoutCompatibility,
            50 => TagId::ParaHeader,
            51 => TagId::ParaText,
            52 => TagId::ParaCharShape,
            53 => TagId::ParaLineSeg,
            54 => TagId::ParaRangeTag,
            55 => TagId::CtrlHeader,
            56 => TagId::ListHeader,
            57 => TagId::PageDef,
            58 => TagId::FootnoteShape,
            59 => TagId::PageBorderFill,
            60 => TagId::ShapeComponent,
            61 => TagId::Table,
            62 => TagId::ShapeComponentLine,
            63 => TagId::ShapeComponentRectangle,
            64 => TagId::ShapeComponentEllipse,
            65 => TagId::ShapeComponentArc,
            66 => TagId::ShapeComponentPolygon,
            67 => TagId::ShapeComponentCurve,
            68 => TagId::ShapeComponentOle,
            69 => TagId::ShapeComponentPicture,
            70 => TagId::ShapeComponentContainer,
            71 => TagId::CtrlData,
            72 => TagId::EqEdit,
            _ => TagId::Unknown,
        }
    }
}

/// Record header structure.
///
/// Layout (32 bits little-endian):
/// - Bits 0-9: Tag ID (0-1023)
/// - Bits 10-19: Level (nesting depth)
/// - Bits 20-31: Size (0-4095, or 0xFFF for extended)
#[derive(Debug, Clone, Copy)]
pub struct RecordHeader {
    /// Tag ID identifying the record type
    pub tag_id: u16,
    /// Nesting level
    pub level: u16,
    /// Data size in bytes
    pub size: u32,
}

impl RecordHeader {
    /// Size of a standard record header in bytes.
    pub const SIZE: usize = 4;
    /// Extended size sentinel value.
    pub const EXTENDED_SIZE_SENTINEL: u32 = 0xFFF;

    /// Parses a record header from bytes.
    ///
    /// Returns the header and the number of bytes consumed (4 or 8 for extended).
    pub fn parse(data: &[u8]) -> Result<(Self, usize)> {
        if data.len() < 4 {
            return Err(Error::InvalidData("Record header too small".into()));
        }

        let header_value = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        let tag_id = (header_value & 0x3FF) as u16;
        let level = ((header_value >> 10) & 0x3FF) as u16;
        let size_field = (header_value >> 20) & 0xFFF;

        let (size, consumed) = if size_field == Self::EXTENDED_SIZE_SENTINEL {
            // Extended size: next 4 bytes contain actual size
            if data.len() < 8 {
                return Err(Error::InvalidData("Extended record header too small".into()));
            }
            let extended_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
            (extended_size, 8)
        } else {
            (size_field, 4)
        };

        Ok((
            Self {
                tag_id,
                level,
                size,
            },
            consumed,
        ))
    }

    /// Returns the tag ID as an enum.
    pub fn tag(&self) -> TagId {
        TagId::from(self.tag_id)
    }
}

/// A parsed record with header and data.
#[derive(Debug, Clone)]
pub struct Record {
    /// Record header
    pub header: RecordHeader,
    /// Record data (payload)
    pub data: Vec<u8>,
    /// Offset in the stream where this record starts
    pub offset: u64,
}

impl Record {
    /// Returns the tag ID.
    pub fn tag(&self) -> TagId {
        self.header.tag()
    }

    /// Returns the raw tag ID value.
    pub fn tag_id(&self) -> u16 {
        self.header.tag_id
    }

    /// Returns the nesting level.
    pub fn level(&self) -> u16 {
        self.header.level
    }

    /// Returns the data size.
    pub fn size(&self) -> u32 {
        self.header.size
    }

    /// Returns the record data.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Reads a u8 at the specified offset.
    pub fn read_u8(&self, offset: usize) -> Result<u8> {
        self.data
            .get(offset)
            .copied()
            .ok_or_else(|| Error::InvalidData("Read past end of record".into()))
    }

    /// Reads a u16 (little-endian) at the specified offset.
    pub fn read_u16(&self, offset: usize) -> Result<u16> {
        if offset + 2 > self.data.len() {
            return Err(Error::InvalidData("Read past end of record".into()));
        }
        Ok(u16::from_le_bytes([self.data[offset], self.data[offset + 1]]))
    }

    /// Reads a u32 (little-endian) at the specified offset.
    pub fn read_u32(&self, offset: usize) -> Result<u32> {
        if offset + 4 > self.data.len() {
            return Err(Error::InvalidData("Read past end of record".into()));
        }
        Ok(u32::from_le_bytes([
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ]))
    }

    /// Reads an i32 (little-endian) at the specified offset.
    pub fn read_i32(&self, offset: usize) -> Result<i32> {
        if offset + 4 > self.data.len() {
            return Err(Error::InvalidData("Read past end of record".into()));
        }
        Ok(i32::from_le_bytes([
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ]))
    }
}

/// Iterator over records in a stream.
pub struct RecordIterator<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> RecordIterator<'a> {
    /// Creates a new record iterator.
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    /// Returns the current position in the stream.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Returns true if there are more records to read.
    pub fn has_more(&self) -> bool {
        self.position + 4 <= self.data.len()
    }
}

impl<'a> Iterator for RecordIterator<'a> {
    type Item = Result<Record>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.has_more() {
            return None;
        }

        let offset = self.position as u64;

        // Parse header
        let (header, header_size) = match RecordHeader::parse(&self.data[self.position..]) {
            Ok(h) => h,
            Err(e) => return Some(Err(e)),
        };

        self.position += header_size;

        // Read data
        let data_end = self.position + header.size as usize;
        if data_end > self.data.len() {
            return Some(Err(Error::RecordParse {
                offset,
                message: format!(
                    "Record data exceeds stream bounds: {} + {} > {}",
                    self.position,
                    header.size,
                    self.data.len()
                ),
            }));
        }

        let data = self.data[self.position..data_end].to_vec();
        self.position = data_end;

        Some(Ok(Record {
            header,
            data,
            offset,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_record_header() {
        // Tag ID: 50 (PARA_HEADER), Level: 0, Size: 10
        // header = 50 | (0 << 10) | (10 << 20) = 0x00A00032
        let data = [0x32, 0x00, 0xA0, 0x00];
        let (header, consumed) = RecordHeader::parse(&data).unwrap();

        assert_eq!(consumed, 4);
        assert_eq!(header.tag_id, 50);
        assert_eq!(header.level, 0);
        assert_eq!(header.size, 10);
    }

    #[test]
    fn test_parse_extended_record_header() {
        // Tag ID: 51 (PARA_TEXT), Level: 0, Size: 0xFFF (extended)
        // header = 51 | (0 << 10) | (0xFFF << 20) = 0xFFF00033
        // Extended size: 5000
        let data = [0x33, 0x00, 0xF0, 0xFF, 0x88, 0x13, 0x00, 0x00];
        let (header, consumed) = RecordHeader::parse(&data).unwrap();

        assert_eq!(consumed, 8);
        assert_eq!(header.tag_id, 51);
        assert_eq!(header.size, 5000);
    }

    #[test]
    fn test_record_iterator() {
        // Two records: one with size 2, one with size 3
        let mut data = Vec::new();

        // Record 1: Tag 50, Level 0, Size 2
        data.extend_from_slice(&[0x32, 0x00, 0x20, 0x00]); // header
        data.extend_from_slice(&[0xAA, 0xBB]); // data

        // Record 2: Tag 51, Level 0, Size 3
        data.extend_from_slice(&[0x33, 0x00, 0x30, 0x00]); // header
        data.extend_from_slice(&[0xCC, 0xDD, 0xEE]); // data

        let records: Vec<_> = RecordIterator::new(&data).collect();
        assert_eq!(records.len(), 2);

        let r1 = records[0].as_ref().unwrap();
        assert_eq!(r1.tag_id(), 50);
        assert_eq!(r1.data(), &[0xAA, 0xBB]);

        let r2 = records[1].as_ref().unwrap();
        assert_eq!(r2.tag_id(), 51);
        assert_eq!(r2.data(), &[0xCC, 0xDD, 0xEE]);
    }
}
