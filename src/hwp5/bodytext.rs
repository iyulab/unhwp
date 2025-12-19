//! BodyText section parsing for HWP 5.0.

use super::record::{Record, RecordIterator, TagId};
use crate::error::Result;
use crate::model::{
    ImageRef, InlineContent, Paragraph, ParagraphStyle, Section, StyleRegistry, Table, TableCell,
    TableRow, TextRun, TextStyle,
};

/// Control characters in HWP text.
/// Characters in range 0x0001-0x001F have special meanings.
///
/// According to HWP 5.0 specification:
/// - Char controls (1 WCHAR): 0x0001, 0x0004, 0x0009-0x000A, 0x000D-0x0018, 0x001E-0x001F
/// - Inline controls (8 WCHARs): 0x0002-0x0003, 0x000B, 0x000C
/// - Extended controls (8 WCHARs): 0x0005-0x0008
mod control_char {
    /// Unusable character (char, 1 WCHAR)
    pub const RESERVED: u16 = 0x0001;
    /// Section/column definition (inline, 8 WCHARs)
    pub const SECTION_DEF: u16 = 0x0002;
    /// Field start (inline, 8 WCHARs)
    pub const FIELD_START: u16 = 0x0003;
    /// Field end (char, 1 WCHAR)
    pub const FIELD_END: u16 = 0x0004;
    /// Title mark / control inline (extended, 8 WCHARs)
    pub const INLINE_CTRL_1: u16 = 0x0005;
    /// Tab definition (extended, 8 WCHARs)
    pub const INLINE_CTRL_2: u16 = 0x0006;
    /// Drawing object (extended, 8 WCHARs)
    pub const INLINE_CTRL_3: u16 = 0x0007;
    /// Reserved (extended, 8 WCHARs)
    pub const INLINE_CTRL_4: u16 = 0x0008;
    /// Tab (char, 1 WCHAR)
    pub const TAB: u16 = 0x0009;
    /// Line break / soft return (char, 1 WCHAR)
    pub const LINE_BREAK: u16 = 0x000A;
    /// Extended control (table, image, equation, etc.) - (inline, 8 WCHARs)
    pub const EXTENDED_CONTROL: u16 = 0x000B;
    /// Hyphen (inline, 8 WCHARs)
    pub const HYPHEN: u16 = 0x000C;
    /// Paragraph break (char, 1 WCHAR)
    pub const PARA_BREAK: u16 = 0x000D;
    /// Page break in column (char, 1 WCHAR)
    pub const PAGE_BREAK_COL: u16 = 0x000E;
    /// Page break in box (char, 1 WCHAR)
    pub const PAGE_BREAK_BOX: u16 = 0x000F;
    /// Hidden comment (char, 1 WCHAR)
    pub const HIDDEN_COMMENT: u16 = 0x0010;
    /// Footnote/endnote (char, 1 WCHAR)
    pub const FOOTNOTE: u16 = 0x0011;
    /// Auto numbering (char, 1 WCHAR)
    pub const AUTO_NUMBERING: u16 = 0x0012;
    /// Page control (char, 1 WCHAR)
    pub const PAGE_CTRL: u16 = 0x0015;
    /// Bookmark (char, 1 WCHAR)
    pub const BOOKMARK: u16 = 0x0016;
    /// OLE overlay/underlay (char, 1 WCHAR)
    pub const OLE_OVERLAY: u16 = 0x0017;
    /// Title mark (char, 1 WCHAR)
    pub const TITLE_MARK: u16 = 0x0018;
    /// Non-breaking space (char, 1 WCHAR)
    pub const NBSP: u16 = 0x001E;
    /// Fixed-width space (char, 1 WCHAR)
    pub const FIXED_SPACE: u16 = 0x001F;
}

/// Parses a BodyText section stream into a Section.
pub fn parse_section(data: &[u8], section_index: usize, styles: &StyleRegistry) -> Result<Section> {
    // First, collect all records into a Vec for indexed access
    let records: Vec<Record> = RecordIterator::new(data)
        .filter_map(|r| r.ok())
        .collect();

    let mut section = Section::new(section_index);
    let mut paragraph_context = ParagraphContext::new();
    // Section-wide picture counter (1-based to match BinId)
    let mut picture_counter: u32 = 0;
    // Track which records to skip (already processed as part of table)
    let mut skip_until_idx: usize = 0;

    let mut idx = 0;
    while idx < records.len() {
        if idx < skip_until_idx {
            idx += 1;
            continue;
        }

        let record = &records[idx];

        match record.tag() {
            TagId::ParaHeader => {
                // Check if this paragraph is at base level (not inside a table)
                let base_level = record.level();

                // Finish previous paragraph if any
                if let Some(para) = paragraph_context.finish() {
                    section.content.push(crate::model::Block::Paragraph(para));
                }

                // Start new paragraph
                let para_shape_id = record.read_u32(0).unwrap_or(0);
                let style_id = record.read_u16(4).unwrap_or(0) as u32;

                let mut style = styles
                    .get_para_style(para_shape_id)
                    .cloned()
                    .unwrap_or_default();

                // Override with explicit style if different
                if let Some(named_style) = styles.get_para_style(style_id) {
                    if named_style.heading_level > 0 {
                        style.heading_level = named_style.heading_level;
                    }
                }

                paragraph_context.start(style);
                paragraph_context.base_level = base_level;
            }

            TagId::ParaText => {
                let text_data = record.data();
                parse_para_text(text_data, &mut paragraph_context, &mut picture_counter, styles)?;
            }

            TagId::ParaCharShape => {
                parse_char_shape_positions(record, &mut paragraph_context, styles)?;
            }

            TagId::Table => {
                // Table record - parse the full table including nested cells
                let table_level = record.level();

                // Finish any pending paragraph
                if let Some(para) = paragraph_context.finish() {
                    section.content.push(crate::model::Block::Paragraph(para));
                }

                // Find all records belonging to this table
                let table_end = find_block_end(&records, idx, table_level);

                // Parse table from the collected records
                if let Some(table) = parse_table_records(&records[idx..table_end], styles) {
                    section.content.push(crate::model::Block::Table(table));
                }

                // Skip all records that were part of the table
                skip_until_idx = table_end;
            }

            _ => {
                // Skip other records (CtrlHeader, ShapeComponent, etc.)
            }
        }

        idx += 1;
    }

    // Don't forget the last paragraph
    if let Some(para) = paragraph_context.finish() {
        section.content.push(crate::model::Block::Paragraph(para));
    }

    Ok(section)
}

/// Finds the end index of a block (table, cell, etc.)
/// Returns the index of the first record that drops BELOW the base level.
/// In HWP, table cells (ListHeader) are at the SAME level as the Table record,
/// so we look for records with level < base_level (not <=).
fn find_block_end(records: &[Record], start_idx: usize, base_level: u16) -> usize {
    for i in (start_idx + 1)..records.len() {
        if records[i].level() < base_level {
            return i;
        }
    }
    records.len()
}

/// Parses table records into a Table structure.
fn parse_table_records(records: &[Record], styles: &StyleRegistry) -> Option<Table> {
    if records.is_empty() {
        return None;
    }

    // First record should be Table tag with table properties
    let table_record = &records[0];
    if table_record.tag() != TagId::Table {
        return None;
    }

    let data = table_record.data();
    if data.len() < 14 {
        return None;
    }

    // Table record structure:
    // Offset 0-3: CtrlId (should be "tbl ")
    // Offset 4-5: Number of rows
    // Offset 6-7: Number of columns
    // Offset 8-9: Cell spacing
    // Offset 10-13: Table left margin, etc.
    let row_count = u16::from_le_bytes([data[4], data[5]]) as usize;
    let col_count = u16::from_le_bytes([data[6], data[7]]) as usize;

    if row_count == 0 || col_count == 0 {
        return None;
    }

    // Find all cell ListHeaders and their content
    // In HWP, cells are represented by ListHeader records
    // The structure is: Table -> (ListHeader -> ParaHeader -> ParaText)*
    let mut cells_data: Vec<CellData> = Vec::new();

    // Find all ListHeader records that belong to this table
    let mut i = 1; // Skip the Table record itself

    while i < records.len() {
        let record = &records[i];

        // ListHeader marks the beginning of a cell
        if record.tag() == TagId::ListHeader {
            // Find all records belonging to this cell
            let cell_end = find_cell_end(records, i, record.level());

            let cell_content = parse_cell_content(&records[i..cell_end], styles);
            cells_data.push(cell_content);
            i = cell_end;
        } else {
            i += 1;
        }
    }

    // Build the table from collected cells
    let mut table = Table::new();
    let total_cells = row_count * col_count;

    for row_idx in 0..row_count {
        let mut row = TableRow::new();
        row.is_header = row_idx == 0;

        for col_idx in 0..col_count {
            let cell_idx = row_idx * col_count + col_idx;
            let cell = if cell_idx < cells_data.len() {
                let cell_data = &cells_data[cell_idx];
                TableCell {
                    content: cell_data.paragraphs.clone(),
                    rowspan: cell_data.rowspan,
                    colspan: cell_data.colspan,
                    ..Default::default()
                }
            } else {
                TableCell::new()
            };
            row.cells.push(cell);
        }
        table.rows.push(row);
    }

    // Set header flag if we have at least one row
    table.has_header = !table.rows.is_empty();

    // If we couldn't parse cells properly but have cell data,
    // try to infer structure from actual cell count
    if cells_data.len() > total_cells && total_cells > 0 {
        // Cells might include merged cell placeholders, keep as is
    }

    Some(table)
}

/// Data for a single table cell
struct CellData {
    paragraphs: Vec<Paragraph>,
    rowspan: u32,
    colspan: u32,
}

/// Finds the end of a cell (next ListHeader at same level or lower level record)
fn find_cell_end(records: &[Record], start_idx: usize, cell_level: u16) -> usize {
    for i in (start_idx + 1)..records.len() {
        let record = &records[i];
        // End when we hit another ListHeader at the same level (next cell)
        // or when we drop below the cell level (end of table)
        if record.level() < cell_level {
            return i;
        }
        if record.level() == cell_level && record.tag() == TagId::ListHeader {
            return i;
        }
    }
    records.len()
}

/// Parses cell content from a slice of records starting with ListHeader
fn parse_cell_content(records: &[Record], styles: &StyleRegistry) -> CellData {
    let mut paragraphs = Vec::new();
    let rowspan = 1u32;
    let colspan = 1u32;

    if records.is_empty() {
        return CellData {
            paragraphs,
            rowspan,
            colspan,
        };
    }

    // First record is ListHeader with cell properties
    let list_header = &records[0];
    let _data = list_header.data();

    // ListHeader structure for table cells:
    // Offset 0-1: Number of paragraphs
    // Offset 2-5: TextWidth
    // For cells, colspan/rowspan is stored in a separate CellSplit record (not ListHeader)
    // We leave colspan/rowspan as 1 for now - merged cells require additional parsing
    // TODO: Parse CELL_SPLIT record to get actual rowspan/colspan

    // Parse paragraphs within the cell
    // Process all remaining records - they belong to this cell
    let mut para_context = ParagraphContext::new();
    let mut picture_counter = 0u32;

    for record in records.iter().skip(1) {
        match record.tag() {
            TagId::ParaHeader => {
                if let Some(para) = para_context.finish() {
                    paragraphs.push(para);
                }

                let para_shape_id = record.read_u32(0).unwrap_or(0);
                let style = styles
                    .get_para_style(para_shape_id)
                    .cloned()
                    .unwrap_or_default();
                para_context.start(style);
            }

            TagId::ParaText => {
                let _ = parse_para_text(record.data(), &mut para_context, &mut picture_counter, styles);
            }

            TagId::ParaCharShape => {
                let _ = parse_char_shape_positions(record, &mut para_context, styles);
            }

            _ => {}
        }
    }

    // Don't forget the last paragraph
    if let Some(para) = para_context.finish() {
        paragraphs.push(para);
    }

    CellData {
        paragraphs,
        rowspan,
        colspan,
    }
}

/// Context for building a paragraph.
struct ParagraphContext {
    style: ParagraphStyle,
    content: Vec<InlineContent>,
    current_text: String,
    current_style: TextStyle,
    char_shape_positions: Vec<(usize, u32)>,
    in_paragraph: bool,
    base_level: u16,
}

impl ParagraphContext {
    fn new() -> Self {
        Self {
            style: ParagraphStyle::default(),
            content: Vec::new(),
            current_text: String::new(),
            current_style: TextStyle::default(),
            char_shape_positions: Vec::new(),
            in_paragraph: false,
            base_level: 0,
        }
    }

    fn start(&mut self, style: ParagraphStyle) {
        self.style = style;
        self.content.clear();
        self.current_text.clear();
        self.current_style = TextStyle::default();
        self.char_shape_positions.clear();
        self.in_paragraph = true;
    }

    fn push_char(&mut self, ch: char) {
        self.current_text.push(ch);
    }

    fn push_line_break(&mut self) {
        self.flush_text();
        self.content.push(InlineContent::LineBreak);
    }

    fn push_image(&mut self, filename: &str) {
        self.flush_text();
        self.content
            .push(InlineContent::Image(ImageRef::new(filename)));
    }

    fn flush_text(&mut self) {
        if !self.current_text.is_empty() {
            let text = std::mem::take(&mut self.current_text);
            let style = self.current_style.clone();
            self.content
                .push(InlineContent::Text(TextRun::with_style(text, style)));
        }
    }

    fn finish(&mut self) -> Option<Paragraph> {
        if !self.in_paragraph {
            return None;
        }

        self.flush_text();
        self.in_paragraph = false;

        if self.content.is_empty() {
            return None;
        }

        Some(Paragraph {
            style: std::mem::take(&mut self.style),
            content: std::mem::take(&mut self.content),
        })
    }
}

/// Parses PARA_TEXT record content.
fn parse_para_text(
    data: &[u8],
    context: &mut ParagraphContext,
    picture_counter: &mut u32,
    styles: &StyleRegistry,
) -> Result<()> {
    if !data.len().is_multiple_of(2) {
        return Err(crate::error::Error::InvalidData(
            "PARA_TEXT data must be even length".into(),
        ));
    }

    let mut i = 0;

    while i + 1 < data.len() {
        let ch = u16::from_le_bytes([data[i], data[i + 1]]);
        i += 2;

        match ch {
            control_char::LINE_BREAK => {
                context.push_line_break();
            }

            control_char::EXTENDED_CONTROL => {
                // EXTENDED_CONTROL is followed by 7 more WCHARs (14 bytes) of inline data
                // Structure: [instance_id(4)] [ctrl_type(4)] [reserved(6)]
                if i + 14 > data.len() {
                    break;
                }

                context.flush_text();

                // Check control type at offset 0-3 of the inline data
                // GSO identifier: " osg" = [0x20, 0x6F, 0x73, 0x67]
                let ctrl_type = &data[i..i + 4];
                let is_gso = ctrl_type == b" osg" || ctrl_type == b"gso ";

                if is_gso {
                    *picture_counter += 1;
                    // Only add image if bindata exists in registry
                    if let Some(filename) = styles.get_bindata_filename(*picture_counter) {
                        context.push_image(filename);
                    }
                    // Skip GSO controls without bindata (equations, OLE, etc.)
                }

                i += 14; // Skip remaining 7 WCHARs
            }

            // All controls that consume 8 WCHARs total (including the control char itself)
            // Inline controls: 0x0002, 0x0003, 0x000B, 0x000C
            // Extended controls: 0x0005-0x0008
            control_char::SECTION_DEF
            | control_char::FIELD_START
            | control_char::INLINE_CTRL_1
            | control_char::INLINE_CTRL_2
            | control_char::INLINE_CTRL_3
            | control_char::INLINE_CTRL_4
            | control_char::HYPHEN => {
                // Skip next 7 WCHARs (14 bytes)
                i += 14;
            }

            control_char::PARA_BREAK => {
                // End of paragraph text
                break;
            }

            control_char::TAB => {
                context.push_char('\t');
            }

            control_char::NBSP | control_char::FIXED_SPACE => {
                context.push_char(' ');
            }

            // Char controls (1 WCHAR) - just skip the control character
            // Includes: 0x0001, 0x0004, 0x000E-0x0018, 0x001A-0x001D
            control_char::RESERVED
            | control_char::FIELD_END
            | control_char::PAGE_BREAK_COL
            | control_char::PAGE_BREAK_BOX
            | control_char::HIDDEN_COMMENT
            | control_char::FOOTNOTE
            | control_char::AUTO_NUMBERING
            | 0x0013
            | 0x0014
            | control_char::PAGE_CTRL
            | control_char::BOOKMARK
            | control_char::OLE_OVERLAY
            | control_char::TITLE_MARK
            | 0x0019..=0x001D => {
                // Skip silently - these are 1 WCHAR char controls
            }

            0x0000 => {
                // Null terminator - end of meaningful text
                break;
            }

            _ => {
                // Regular character
                if let Some(ch) = char::from_u32(ch as u32) {
                    context.push_char(ch);
                }
            }
        }
    }

    Ok(())
}

/// Parses PARA_CHAR_SHAPE record for style positions.
fn parse_char_shape_positions(
    record: &Record,
    context: &mut ParagraphContext,
    styles: &StyleRegistry,
) -> Result<()> {
    let data = record.data();

    // Format: pairs of (position: u32, char_shape_id: u32)
    let mut offset = 0;
    while offset + 8 <= data.len() {
        let position = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;

        let shape_id = u32::from_le_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);

        context.char_shape_positions.push((position, shape_id));

        // Update current style if this is the first position
        if position == 0 {
            if let Some(style) = styles.get_char_style(shape_id) {
                context.current_style = style.clone();
            }
        }

        offset += 8;
    }

    Ok(())
}
