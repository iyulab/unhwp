//! Control parsing for HWP 5.0 (tables, images, equations).

use super::record::Record;
use crate::error::Result;
use crate::model::{Table, TableRow, TableCell, Paragraph};

/// Control types identified by 4-character codes.
pub mod ctrl_id {
    /// Section definition
    pub const SECD: u32 = u32::from_le_bytes(*b"secd");
    /// Column definition
    pub const COLD: u32 = u32::from_le_bytes(*b"cold");
    /// Table
    pub const TBL: u32 = u32::from_le_bytes(*b"tbl ");
    /// Generic drawing object
    pub const GSO: u32 = u32::from_le_bytes(*b"gso ");
    /// Equation editor
    pub const EQED: u32 = u32::from_le_bytes(*b"eqed");
    /// Header
    pub const HEADER: u32 = u32::from_le_bytes(*b"head");
    /// Footer
    pub const FOOTER: u32 = u32::from_le_bytes(*b"foot");
    /// Footnote
    pub const FN: u32 = u32::from_le_bytes(*b"fn  ");
    /// Endnote
    pub const EN: u32 = u32::from_le_bytes(*b"en  ");
    /// Auto number
    pub const ATNO: u32 = u32::from_le_bytes(*b"atno");
    /// New number
    pub const NWNO: u32 = u32::from_le_bytes(*b"nwno");
    /// Page number
    pub const PGCT: u32 = u32::from_le_bytes(*b"pgct");
    /// Hidden comment
    pub const TCMT: u32 = u32::from_le_bytes(*b"tcmt");
    /// Field (bookmark, hyperlink, etc.)
    pub const FIELD: u32 = u32::from_le_bytes(*b"fld ");
}

/// Parser for control elements.
pub struct ControlParser;

impl ControlParser {
    /// Parses a control header record to determine control type.
    pub fn parse_ctrl_type(record: &Record) -> Result<u32> {
        if record.data().len() < 4 {
            return Err(crate::error::Error::InvalidData(
                "Control header too small".into(),
            ));
        }

        Ok(u32::from_le_bytes([
            record.data()[0],
            record.data()[1],
            record.data()[2],
            record.data()[3],
        ]))
    }

    /// Parses a table control.
    pub fn parse_table(
        ctrl_header: &Record,
        list_header: &Record,
        cell_records: &[Record],
    ) -> Result<Table> {
        // ListHeader contains row/column info
        let data = list_header.data();
        if data.len() < 30 {
            return Err(crate::error::Error::InvalidData(
                "ListHeader too small for table".into(),
            ));
        }

        // Parse table dimensions
        // Offset 0-1: Number of rows
        // Offset 2-3: Number of columns
        let row_count = u16::from_le_bytes([data[0], data[1]]) as usize;
        let col_count = u16::from_le_bytes([data[2], data[3]]) as usize;

        let mut table = Table::new();
        table.has_header = true; // Assume first row is header

        // Create empty table structure
        for _ in 0..row_count {
            let mut row = TableRow::new();
            for _ in 0..col_count {
                row.cells.push(TableCell::new());
            }
            table.rows.push(row);
        }

        // Parse cell properties from subsequent records
        // This is simplified - full implementation would parse each cell's
        // ListHeader, rowspan/colspan, and nested paragraphs

        Ok(table)
    }

    /// Parses an equation control.
    pub fn parse_equation(record: &Record) -> Result<String> {
        let data = record.data();

        // EQEdit script is stored after the control header properties
        // The exact offset depends on version, typically starts around offset 16

        // Find the script by looking for recognizable EQEdit syntax
        // Scripts contain commands like "over", "sqrt", "pile", etc.

        // For now, try to extract as UTF-16LE string
        if data.len() > 16 {
            let script_data = &data[16..];
            if let Ok(script) = decode_utf16le_string(script_data) {
                return Ok(script);
            }
        }

        Ok(String::new())
    }

    /// Parses a drawing object (GSO) to extract image reference.
    pub fn parse_image(record: &Record) -> Result<Option<String>> {
        let data = record.data();

        // GSO contains various shape components
        // Picture component has BinData reference

        // This is simplified - actual implementation would:
        // 1. Check shape type is picture
        // 2. Parse ShapeComponentPicture
        // 3. Extract BinData ID

        // For now, return None
        Ok(None)
    }
}

/// Decodes a null-terminated UTF-16LE string.
fn decode_utf16le_string(data: &[u8]) -> Result<String> {
    let mut u16_values = Vec::new();

    for chunk in data.chunks_exact(2) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        if value == 0 {
            break;
        }
        u16_values.push(value);
    }

    String::from_utf16(&u16_values).map_err(|e| crate::error::Error::Encoding(e.to_string()))
}

/// Cell information for table parsing.
#[derive(Debug, Default)]
pub struct CellInfo {
    /// Row index
    pub row: usize,
    /// Column index
    pub col: usize,
    /// Row span
    pub rowspan: u32,
    /// Column span
    pub colspan: u32,
    /// Cell content paragraphs
    pub content: Vec<Paragraph>,
}
