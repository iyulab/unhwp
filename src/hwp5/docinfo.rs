//! DocInfo stream parsing for HWP 5.0.
//!
//! DocInfo contains document-wide definitions for fonts, styles, and shapes.

use super::record::{Record, RecordIterator, TagId};
use crate::error::Result;
use crate::model::{ParagraphStyle, StyleRegistry, TextStyle, Alignment, ListStyle};

/// Parses DocInfo stream and populates the style registry.
pub fn parse_docinfo(data: &[u8], registry: &mut StyleRegistry) -> Result<()> {
    let mut face_names: Vec<String> = Vec::new();
    let mut char_shapes: Vec<CharShapeData> = Vec::new();
    let mut para_shapes: Vec<ParaShapeData> = Vec::new();

    for record in RecordIterator::new(data) {
        let record = record?;

        match record.tag() {
            TagId::FaceName => {
                if let Ok(name) = parse_face_name(&record) {
                    face_names.push(name);
                }
            }
            TagId::CharShape => {
                if let Ok(shape) = parse_char_shape(&record) {
                    char_shapes.push(shape);
                }
            }
            TagId::ParaShape => {
                if let Ok(shape) = parse_para_shape(&record) {
                    para_shapes.push(shape);
                }
            }
            TagId::Style => {
                // Style definitions reference CharShape and ParaShape by index
                // Will implement full style parsing later
            }
            _ => {
                // Skip other records
            }
        }
    }

    // Register character shapes
    for (index, shape) in char_shapes.iter().enumerate() {
        let font_name = shape
            .face_name_index
            .and_then(|i| face_names.get(i as usize))
            .cloned();

        let text_style = TextStyle {
            bold: shape.bold,
            italic: shape.italic,
            underline: shape.underline,
            strikethrough: shape.strikethrough,
            superscript: shape.superscript,
            subscript: shape.subscript,
            font_name,
            font_size: shape.font_size,
            color: shape.text_color.clone(),
            background_color: None,
        };

        registry.register_char_style(index as u32, text_style);
    }

    // Register paragraph shapes
    for (index, shape) in para_shapes.iter().enumerate() {
        let para_style = ParagraphStyle {
            heading_level: shape.outline_level.unwrap_or(0),
            alignment: shape.alignment,
            list_style: shape.list_style.clone(),
            indent_level: shape.indent_level,
            line_spacing: shape.line_spacing,
            space_before: shape.space_before,
            space_after: shape.space_after,
        };

        registry.register_para_style(index as u32, para_style);
    }

    Ok(())
}

/// Parsed character shape data.
#[derive(Debug, Default)]
struct CharShapeData {
    face_name_index: Option<u16>,
    font_size: Option<f32>,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    superscript: bool,
    subscript: bool,
    text_color: Option<String>,
}

/// Parsed paragraph shape data.
#[derive(Debug, Default)]
struct ParaShapeData {
    alignment: Alignment,
    outline_level: Option<u8>,
    list_style: Option<ListStyle>,
    indent_level: u8,
    line_spacing: Option<f32>,
    space_before: Option<f32>,
    space_after: Option<f32>,
}

/// Parses HWPTAG_FACE_NAME record.
fn parse_face_name(record: &Record) -> Result<String> {
    let data = record.data();
    if data.len() < 2 {
        return Err(crate::error::Error::InvalidData("FaceName too small".into()));
    }

    // FaceName structure:
    // - Property flags (1 byte)
    // - Name length in WCHARs (1 byte) - sometimes
    // - Name (UTF-16LE string)
    // The exact structure varies by version

    // Skip property byte and find the name
    // Name is typically at offset 2 or later, UTF-16LE encoded
    let name_start = 2;
    if data.len() > name_start {
        decode_utf16le_string(&data[name_start..])
    } else {
        Ok(String::new())
    }
}

/// Parses HWPTAG_CHAR_SHAPE record.
fn parse_char_shape(record: &Record) -> Result<CharShapeData> {
    let data = record.data();

    // CharShape structure (simplified):
    // Offset 0-13: Face name IDs for different scripts (7 x u16)
    // Offset 14-27: Character ratios (7 x u8) for different scripts
    // Offset 28-41: Character spacings (7 x i8)
    // Offset 42-55: Relative sizes (7 x u8)
    // Offset 56-69: Position adjustments (7 x i8)
    // Offset 70-73: Base size (i32, in HWP units)
    // Offset 74-77: Properties (u32)
    // Offset 78-81: Shadow gap X (i8), shadow gap Y (i8), text color (3 bytes)
    // ...

    // Need at least 2 bytes for face name index
    if data.len() < 2 {
        return Err(crate::error::Error::InvalidData("CharShape too small".into()));
    }

    let face_name_index = Some(u16::from_le_bytes([data[0], data[1]]));

    // Base size at offset 70 (in HWP units, 1/7200 inch)
    let font_size = if data.len() >= 74 {
        let size = i32::from_le_bytes([data[70], data[71], data[72], data[73]]);
        Some((size as f32) / 100.0)
    } else {
        None
    };

    // Properties at offset 74
    let properties = if data.len() >= 78 {
        u32::from_le_bytes([data[74], data[75], data[76], data[77]])
    } else {
        0
    };

    let bold = properties & (1 << 0) != 0;
    let italic = properties & (1 << 1) != 0;
    let underline = properties & (1 << 2) != 0;
    // Underline shape is in bits 3-6
    // Outline is bit 7
    // Shadow is bit 8
    // Emboss is bit 9
    // Engrave is bit 10
    let superscript = properties & (1 << 11) != 0;
    let subscript = properties & (1 << 12) != 0;
    let strikethrough = properties & (1 << 13) != 0;

    // Text color at offset 78 (RGB, 3 bytes)
    let text_color = if data.len() > 80 {
        Some(format!(
            "#{:02X}{:02X}{:02X}",
            data[78], data[79], data[80]
        ))
    } else {
        None
    };

    Ok(CharShapeData {
        face_name_index,
        font_size,
        bold,
        italic,
        underline,
        strikethrough,
        superscript,
        subscript,
        text_color,
    })
}

/// Parses HWPTAG_PARA_SHAPE record.
fn parse_para_shape(record: &Record) -> Result<ParaShapeData> {
    let data = record.data();
    if data.len() < 54 {
        return Err(crate::error::Error::InvalidData("ParaShape too small".into()));
    }

    // ParaShape structure (simplified):
    // Offset 0-3: Properties 1 (u32) - alignment, etc.
    // Offset 4-7: Left margin
    // Offset 8-11: Right margin
    // Offset 12-15: Indent
    // Offset 16-19: Top margin (space before)
    // Offset 20-23: Bottom margin (space after)
    // Offset 24-27: Line spacing
    // Offset 28-29: Tab definition ID
    // Offset 30-31: Numbering/bullet ID
    // Offset 32-33: Border fill ID
    // Offset 34-35: Border offset (left)
    // ...
    // Offset 50-53: Properties 3 (u32) - outline level, etc.

    let properties1 = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

    // Alignment is in bits 0-1
    let alignment = match properties1 & 0x03 {
        0 => Alignment::Justify,
        1 => Alignment::Left,
        2 => Alignment::Right,
        3 => Alignment::Center,
        _ => Alignment::Left,
    };

    // Space before (in HWP units)
    let space_before_raw = i32::from_le_bytes([data[16], data[17], data[18], data[19]]);
    let space_before = Some((space_before_raw as f32) / 7200.0 * 72.0); // Convert to points

    // Space after (in HWP units)
    let space_after_raw = i32::from_le_bytes([data[20], data[21], data[22], data[23]]);
    let space_after = Some((space_after_raw as f32) / 7200.0 * 72.0);

    // Line spacing
    let line_spacing_raw = i32::from_le_bytes([data[24], data[25], data[26], data[27]]);
    // Line spacing type is in properties1 bits 4-5
    let line_spacing_type = (properties1 >> 4) & 0x03;
    let line_spacing = match line_spacing_type {
        0 => Some((line_spacing_raw as f32) / 100.0), // Percentage
        1 => Some((line_spacing_raw as f32) / 7200.0 * 72.0), // Fixed (points)
        _ => None,
    };

    // Outline level from Properties3 (offset 50)
    let outline_level = if data.len() >= 54 {
        let props3 = u32::from_le_bytes([data[50], data[51], data[52], data[53]]);
        let level = (props3 & 0x07) as u8;
        if level > 0 && level <= 6 {
            Some(level)
        } else {
            None
        }
    } else {
        None
    };

    // List style from numbering ID
    let numbering_id = u16::from_le_bytes([data[30], data[31]]);
    let list_style = if numbering_id > 0 {
        Some(ListStyle::Ordered) // Simplified - would need to look up actual style
    } else {
        None
    };

    Ok(ParaShapeData {
        alignment,
        outline_level,
        list_style,
        indent_level: 0,
        line_spacing,
        space_before,
        space_after,
    })
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
