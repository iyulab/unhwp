//! DocInfo stream parsing for HWP 5.0.
//!
//! DocInfo contains document-wide definitions for fonts, styles, and shapes.

use super::record::{Record, RecordIterator, TagId};
use crate::error::Result;
use crate::model::{Alignment, ListStyle, ParagraphStyle, StyleRegistry, TextStyle};

/// Parses DocInfo stream and populates the style registry.
pub fn parse_docinfo(data: &[u8], registry: &mut StyleRegistry) -> Result<()> {
    let mut face_names: Vec<String> = Vec::new();
    let mut char_shapes: Vec<CharShapeData> = Vec::new();
    let mut para_shapes: Vec<ParaShapeData> = Vec::new();
    for record in RecordIterator::new(data) {
        let record = record?;

        match record.tag() {
            TagId::BinData => {
                // Parse BinData record to get binId → filename mapping
                if let Some((bin_id, filename)) = parse_bindata_record(&record) {
                    registry.register_bindata(bin_id as u32, filename);
                }
            }
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
        return Err(crate::error::Error::InvalidData(
            "FaceName too small".into(),
        ));
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
        return Err(crate::error::Error::InvalidData(
            "CharShape too small".into(),
        ));
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
        Some(format!("#{:02X}{:02X}{:02X}", data[78], data[79], data[80]))
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
        return Err(crate::error::Error::InvalidData(
            "ParaShape too small".into(),
        ));
    }

    // ParaShape structure (HWP 5.0 spec 표 43):
    // Offset 0-3: Properties 1 (u32)
    //   - bit 0-1: Line spacing type (0=percent, 1=fixed, 2=margin-only)
    //   - bit 2-4: Alignment (0=justify, 1=left, 2=right, 3=center, 4=distribute, 5=split)
    //   - bit 23-24: Head shape type (0=none, 1=outline, 2=numbering, 3=bullet)
    //   - bit 25-27: Paragraph level (1-7 for outline/numbering)
    // Offset 4-7: Left margin
    // Offset 8-11: Right margin
    // Offset 12-15: Indent
    // Offset 16-19: Top margin (space before)
    // Offset 20-23: Bottom margin (space after)
    // Offset 24-27: Line spacing value
    // Offset 28-29: Tab definition ID
    // Offset 30-31: Numbering/bullet ID
    // Offset 32-33: Border fill ID
    // ...
    // Offset 50-53: Properties 3 (u32) - extended properties (5.0.2.5+)

    let properties1 = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

    // Alignment is in bits 2-4 (0=justify, 1=left, 2=right, 3=center, 4=distribute, 5=split)
    let alignment = match (properties1 >> 2) & 0x07 {
        0 => Alignment::Justify,
        1 => Alignment::Left,
        2 => Alignment::Right,
        3 => Alignment::Center,
        4 => Alignment::Justify, // Distribute -> Justify fallback
        5 => Alignment::Justify, // Split -> Justify fallback
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
    // Line spacing type is in properties1 bits 0-1 (for newer versions)
    let line_spacing_type = properties1 & 0x03;
    let line_spacing = match line_spacing_type {
        0 => Some((line_spacing_raw as f32) / 100.0), // Percentage
        1 => Some((line_spacing_raw as f32) / 7200.0 * 72.0), // Fixed (points)
        _ => None,
    };

    // Outline level from Properties1:
    // - bit 23-24: Paragraph head shape type (0=none, 1=outline, 2=numbering, 3=bullet)
    // - bit 25-27: Paragraph level (0-7)
    //   - Level 0: 바탕글 (normal text)
    //   - Level 1-6: 개요 1-6 (heading 1-6)
    //   - Level 7: 바탕글 변형 (normal text variant)
    //
    // Only treat as heading if head_shape_type == 1 (outline) AND level is 1-6
    let head_shape_type = (properties1 >> 23) & 0x03;
    let outline_level = if head_shape_type == 1 {
        // This is an outline-styled paragraph, extract the level (bits 25-27)
        let level = ((properties1 >> 25) & 0x07) as u8;
        if level >= 1 && level <= 6 {
            // Levels 1-6 are actual headings
            Some(level)
        } else {
            // Level 0 (바탕글) or 7 (바탕글 변형) are normal text
            None
        }
    } else {
        // Not an outline style (none, numbering, or bullet)
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

/// Parses a BinData record to extract the binId and filename.
///
/// BinData structure for embedded files:
/// - type (2 bytes): 0=link, 1=embedding, 2=storage
/// - binId (2 bytes): ID used in filename (BINxxxx)
/// - extLen (2 bytes): length of extension string
/// - ext (extLen * 2 bytes): extension in UTF-16LE
///
/// Returns (binId, filename) tuple if successful.
fn parse_bindata_record(record: &Record) -> Option<(u16, String)> {
    let data = record.data();

    if data.len() < 6 {
        return None;
    }

    let type_info = u16::from_le_bytes([data[0], data[1]]);
    let bin_type = type_info & 0x0F;

    // Only process embedded (1) or storage (2) types
    if bin_type != 1 && bin_type != 2 {
        return None;
    }

    let bin_id = u16::from_le_bytes([data[2], data[3]]);
    let ext_len = u16::from_le_bytes([data[4], data[5]]) as usize;

    // Extract extension
    let ext = if ext_len > 0 && 6 + ext_len * 2 <= data.len() {
        let ext_data = &data[6..6 + ext_len * 2];
        let u16_values: Vec<u16> = ext_data
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .take_while(|&v| v != 0)
            .collect();
        String::from_utf16_lossy(&u16_values)
    } else {
        String::from("bin")
    };

    // Format filename: BIN + 4-digit hex + extension
    Some((bin_id, format!("BIN{:04X}.{}", bin_id, ext)))
}
