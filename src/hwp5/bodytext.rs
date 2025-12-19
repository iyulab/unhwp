//! BodyText section parsing for HWP 5.0.

use super::control::ControlParser;
use super::record::{Record, RecordIterator, TagId};
use crate::error::Result;
use crate::model::{
    InlineContent, Paragraph, ParagraphStyle, Section, StyleRegistry, Table, TextRun, TextStyle,
};

/// Control characters in HWP text.
mod control_char {
    /// Line break (soft return)
    pub const LINE_BREAK: u16 = 0x000A;
    /// Extended control (table, image, etc.) - consumes 8 WCHARs total
    pub const EXTENDED_CONTROL: u16 = 0x000B;
    /// Paragraph break
    pub const PARA_BREAK: u16 = 0x000D;
    /// Section definition - consumes 8 WCHARs total
    pub const SECTION_DEF: u16 = 0x0002;
    /// Field start - consumes 8 WCHARs total
    pub const FIELD_START: u16 = 0x0003;
    /// Field end
    pub const FIELD_END: u16 = 0x0004;
    /// Footnote/endnote - consumes 8 WCHARs total
    pub const FOOTNOTE: u16 = 0x0011;
    /// Tab
    pub const TAB: u16 = 0x0009;
    /// Non-breaking space
    pub const NBSP: u16 = 0x001E;
    /// Fixed-width space
    pub const FIXED_SPACE: u16 = 0x001F;
}

/// Parses a BodyText section stream into a Section.
pub fn parse_section(
    data: &[u8],
    section_index: usize,
    styles: &StyleRegistry,
) -> Result<Section> {
    let mut section = Section::new(section_index);
    let mut paragraph_context = ParagraphContext::new();
    let mut pending_controls: Vec<Record> = Vec::new();

    for record in RecordIterator::new(data) {
        let record = record?;

        match record.tag() {
            TagId::ParaHeader => {
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
                pending_controls.clear();
            }

            TagId::ParaText => {
                let text_data = record.data();
                parse_para_text(text_data, &mut paragraph_context, &mut pending_controls)?;
            }

            TagId::ParaCharShape => {
                // Maps positions to character shape IDs
                parse_char_shape_positions(&record, &mut paragraph_context, styles)?;
            }

            TagId::CtrlHeader => {
                // Store control for later processing
                pending_controls.push(record);
            }

            TagId::Table | TagId::ListHeader => {
                // Table encountered - parse and add to section
                if let Some(table) = parse_table_from_records(&record, &pending_controls, styles)? {
                    // If we have a pending paragraph, add it first
                    if let Some(para) = paragraph_context.finish() {
                        section.content.push(crate::model::Block::Paragraph(para));
                    }
                    section.content.push(crate::model::Block::Table(table));
                }
            }

            _ => {
                // Skip other records
            }
        }
    }

    // Don't forget the last paragraph
    if let Some(para) = paragraph_context.finish() {
        section.content.push(crate::model::Block::Paragraph(para));
    }

    Ok(section)
}

/// Context for building a paragraph.
struct ParagraphContext {
    style: ParagraphStyle,
    content: Vec<InlineContent>,
    current_text: String,
    current_style: TextStyle,
    char_shape_positions: Vec<(usize, u32)>,
    in_paragraph: bool,
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

    fn flush_text(&mut self) {
        if !self.current_text.is_empty() {
            let text = std::mem::take(&mut self.current_text);
            let style = self.current_style.clone();
            self.content.push(InlineContent::Text(TextRun::with_style(text, style)));
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
    pending_controls: &mut Vec<Record>,
) -> Result<()> {
    if data.len() % 2 != 0 {
        return Err(crate::error::Error::InvalidData(
            "PARA_TEXT data must be even length".into(),
        ));
    }

    let mut i = 0;
    let mut control_index = 0;

    while i + 1 < data.len() {
        let ch = u16::from_le_bytes([data[i], data[i + 1]]);
        i += 2;

        match ch {
            control_char::LINE_BREAK => {
                context.push_line_break();
            }

            control_char::EXTENDED_CONTROL => {
                // Skip next 7 WCHARs (14 bytes) - extended control data
                context.flush_text();

                // Process the control if we have one pending
                if control_index < pending_controls.len() {
                    // Control processing would go here
                    control_index += 1;
                }

                i += 14; // Skip remaining 7 WCHARs
            }

            control_char::SECTION_DEF | control_char::FIELD_START | control_char::FOOTNOTE => {
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

            0x0000..=0x001F => {
                // Other control characters - skip
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

/// Attempts to parse a table from control records.
fn parse_table_from_records(
    _table_record: &Record,
    _controls: &[Record],
    _styles: &StyleRegistry,
) -> Result<Option<Table>> {
    // Table parsing is complex - implement basic structure
    // Full implementation would recursively parse cell paragraphs

    // For now, return None to skip tables
    // TODO: Implement full table parsing
    Ok(None)
}
