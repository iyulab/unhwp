//! BodyText section parsing for HWP 5.0.

use super::record::{Record, RecordIterator, TagId};
use crate::error::Result;
use crate::model::{
    ImageRef, InlineContent, Paragraph, ParagraphStyle, Section, StyleRegistry, Table, TableCell,
    TableRow, TextRun, TextStyle,
};

/// Control characters in HWP text.
/// Characters in range 0x0000-0x001F have special meanings.
///
/// According to HWP 5.0 specification (표 6):
/// - Char controls (1 WCHAR): 0x00, 0x0A, 0x0D, 0x1E, 0x1F
/// - Inline controls (8 WCHARs): 0x04, 0x05-0x08, 0x09, 0x0C, 0x13-0x14, 0x19-0x1D
/// - Extended controls (8 WCHARs): 0x01, 0x02, 0x03, 0x0B, 0x0E-0x12, 0x15-0x18
mod control_char {
    // === CHAR controls (size = 1 WCHAR) - just skip the control character ===
    /// Unusable character
    pub const UNUSABLE: u16 = 0x0000;
    /// Line break / soft return
    pub const LINE_BREAK: u16 = 0x000A;
    /// Paragraph break
    pub const PARA_BREAK: u16 = 0x000D;
    /// Non-breaking space
    pub const NBSP: u16 = 0x001E;
    /// Fixed-width space
    pub const FIXED_SPACE: u16 = 0x001F;

    // === INLINE controls (size = 8 WCHARs) - skip 14 more bytes after control ===
    /// Field end (hyperlink end, etc.)
    pub const FIELD_END: u16 = 0x0004;
    /// Reserved inline 1
    pub const INLINE_RESERVED_1: u16 = 0x0005;
    /// Reserved inline 2
    pub const INLINE_RESERVED_2: u16 = 0x0006;
    /// Reserved inline 3
    pub const INLINE_RESERVED_3: u16 = 0x0007;
    /// Title mark inline
    pub const INLINE_TITLE_MARK: u16 = 0x0008;
    /// Tab
    pub const TAB: u16 = 0x0009;
    /// Hyphen / reserved
    pub const HYPHEN: u16 = 0x000C;

    // === EXTENDED controls (size = 8 WCHARs) - skip 14 more bytes after control ===
    /// Reserved extended
    pub const RESERVED: u16 = 0x0001;
    /// Section/column definition
    pub const SECTION_DEF: u16 = 0x0002;
    /// Field start (hyperlink, etc.)
    pub const FIELD_START: u16 = 0x0003;
    /// Drawing object/table (GSO)
    pub const EXTENDED_CONTROL: u16 = 0x000B;
    /// Reserved extended
    pub const EXT_RESERVED_0E: u16 = 0x000E;
    /// Hidden comment
    pub const HIDDEN_COMMENT: u16 = 0x000F;
    /// Reserved
    pub const EXT_RESERVED_10: u16 = 0x0010;
    /// Footnote/endnote
    pub const FOOTNOTE: u16 = 0x0011;
    /// Auto numbering
    pub const AUTO_NUMBERING: u16 = 0x0012;
    /// Page control
    pub const PAGE_CTRL: u16 = 0x0015;
    /// Bookmark
    pub const BOOKMARK: u16 = 0x0016;
    /// OLE overlay / 덧말
    pub const OLE_OVERLAY: u16 = 0x0017;
    /// Title mark extended
    pub const TITLE_MARK: u16 = 0x0018;
}

/// Parses a BodyText section stream into a Section.
pub fn parse_section(data: &[u8], section_index: usize, styles: &StyleRegistry) -> Result<Section> {
    // First, collect all records into a Vec for indexed access
    let records: Vec<Record> = RecordIterator::new(data).filter_map(|r| r.ok()).collect();

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
                parse_para_text(
                    text_data,
                    &mut paragraph_context,
                    &mut picture_counter,
                    styles,
                )?;
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
    for (i, record) in records.iter().enumerate().skip(start_idx + 1) {
        if record.level() < base_level {
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

    // Build a 2D grid initialized with None
    let mut grid: Vec<Vec<Option<usize>>> = vec![vec![None; col_count]; row_count];

    // Place each cell at its row/col position
    for (cell_idx, cell_data) in cells_data.iter().enumerate() {
        let r = cell_data.row as usize;
        let c = cell_data.col as usize;

        // Check bounds
        if r < row_count && c < col_count {
            grid[r][c] = Some(cell_idx);

            // Mark cells covered by rowspan/colspan as occupied (with None)
            // These will remain None, indicating they're part of a merged cell
            for dr in 0..cell_data.rowspan as usize {
                for dc in 0..cell_data.colspan as usize {
                    if dr == 0 && dc == 0 {
                        continue; // Skip the cell itself
                    }
                    let nr = r + dr;
                    let nc = c + dc;
                    if nr < row_count && nc < col_count {
                        // Leave as None (already initialized)
                    }
                }
            }
        }
    }

    // Build the table from the grid
    let mut table = Table::new();

    for (row_idx, grid_row) in grid.iter().enumerate() {
        let mut row = TableRow::new();
        row.is_header = row_idx == 0;

        for &cell_idx_opt in grid_row {
            if let Some(cell_idx) = cell_idx_opt {
                let cell_data = &cells_data[cell_idx];
                let cell = TableCell {
                    content: cell_data.paragraphs.clone(),
                    rowspan: cell_data.rowspan,
                    colspan: cell_data.colspan,
                    ..Default::default()
                };
                row.cells.push(cell);
            } else {
                // Empty cell (part of a merged region or missing data)
                // Skip it - the colspan/rowspan from the parent cell handles this
            }
        }
        table.rows.push(row);
    }

    // Set header flag if we have at least one row
    table.has_header = !table.rows.is_empty();

    Some(table)
}

/// Data for a single table cell
struct CellData {
    paragraphs: Vec<Paragraph>,
    rowspan: u32,
    colspan: u32,
    row: u16,
    col: u16,
}

/// Finds the end of a cell (next ListHeader at same level or lower level record)
fn find_cell_end(records: &[Record], start_idx: usize, cell_level: u16) -> usize {
    for (i, record) in records.iter().enumerate().skip(start_idx + 1) {
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
    let mut rowspan = 1u32;
    let mut colspan = 1u32;
    let mut row = 0u16;
    let mut col = 0u16;

    if records.is_empty() {
        return CellData {
            paragraphs,
            rowspan,
            colspan,
            row,
            col,
        };
    }

    // First record is ListHeader with cell properties
    let list_header = &records[0];
    let data = list_header.data();

    // ListHeader structure for table cells (based on pyhwp reference):
    // Offset 0-1: Number of paragraphs (UINT16)
    // Offset 2-3: unknown1 (UINT16)
    // Offset 4-7: listflags (UINT32)
    // Offset 8-9: col - column address (UINT16)
    // Offset 10-11: row - row address (UINT16)
    // Offset 12-13: colspan (UINT16)
    // Offset 14-15: rowspan (UINT16)
    // Offset 16+: width, height, padding, etc.
    if data.len() >= 16 {
        // Parse col and row addresses
        col = u16::from_le_bytes([data[8], data[9]]);
        row = u16::from_le_bytes([data[10], data[11]]);

        // Parse colspan and rowspan
        colspan = u16::from_le_bytes([data[12], data[13]]) as u32;
        rowspan = u16::from_le_bytes([data[14], data[15]]) as u32;

        // Ensure minimum values of 1
        if colspan == 0 {
            colspan = 1;
        }
        if rowspan == 0 {
            rowspan = 1;
        }
    }

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
                let _ = parse_para_text(
                    record.data(),
                    &mut para_context,
                    &mut picture_counter,
                    styles,
                );
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
        row,
        col,
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
///
/// Control character handling per HWP 5.0 specification (표 6):
/// - Char controls (size=1): 0x00, 0x0A, 0x0D, 0x1E, 0x1F
/// - Inline/Extended controls (size=8): All others in 0x01-0x1D range
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
            // === CHAR controls (size = 1 WCHAR) ===
            control_char::UNUSABLE => {
                // Null/unusable - end of meaningful text
                break;
            }

            control_char::LINE_BREAK => {
                context.push_line_break();
            }

            control_char::PARA_BREAK => {
                // End of paragraph text
                break;
            }

            control_char::NBSP | control_char::FIXED_SPACE => {
                context.push_char(' ');
            }

            // === EXTENDED CONTROL (0x0B) - special handling for GSO ===
            control_char::EXTENDED_CONTROL => {
                // Extended control is followed by 7 more WCHARs (14 bytes) of inline data
                // Structure: [ctrl_type(4)] [instance_id(4)] [reserved(6)]
                if i + 14 > data.len() {
                    break;
                }

                context.flush_text();

                // Check control type at offset 0-3 of the inline data
                // GSO identifier: " osg" = [0x20, 0x6F, 0x73, 0x67] or "gso "
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

            // === TAB control (0x09) - inline but we render it ===
            control_char::TAB => {
                // Tab is inline control but we just render it as tab character
                // Skip the 14 bytes of inline data after the tab
                if i + 14 <= data.len() {
                    i += 14;
                }
                context.push_char('\t');
            }

            // === INLINE controls (size = 8 WCHARs) - skip 14 bytes ===
            control_char::FIELD_END // 0x04 - hyperlink end, etc.
            | control_char::INLINE_RESERVED_1 // 0x05
            | control_char::INLINE_RESERVED_2 // 0x06
            | control_char::INLINE_RESERVED_3 // 0x07
            | control_char::INLINE_TITLE_MARK // 0x08
            | control_char::HYPHEN // 0x0C
            | 0x0013 // reserved inline
            | 0x0014 // reserved inline
            | 0x0019..=0x001D // reserved inline range
            => {
                // Inline controls: skip next 7 WCHARs (14 bytes)
                if i + 14 <= data.len() {
                    i += 14;
                }
            }

            // === EXTENDED controls (size = 8 WCHARs) - skip 14 bytes ===
            control_char::RESERVED // 0x01
            | control_char::SECTION_DEF // 0x02
            | control_char::FIELD_START // 0x03
            | control_char::EXT_RESERVED_0E // 0x0E
            | control_char::HIDDEN_COMMENT // 0x0F
            | control_char::EXT_RESERVED_10 // 0x10
            | control_char::FOOTNOTE // 0x11
            | control_char::AUTO_NUMBERING // 0x12
            | control_char::PAGE_CTRL // 0x15
            | control_char::BOOKMARK // 0x16
            | control_char::OLE_OVERLAY // 0x17
            | control_char::TITLE_MARK // 0x18
            => {
                // Extended controls: skip next 7 WCHARs (14 bytes)
                if i + 14 <= data.len() {
                    i += 14;
                }
            }

            _ => {
                // Regular character (code >= 0x20)
                if let Some(c) = char::from_u32(ch as u32) {
                    context.push_char(c);
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
