//! DocInfo stream parsing for HWP 5.0.
//!
//! DocInfo contains document-wide definitions for fonts, styles, and shapes.

use super::record::{Record, RecordIterator, TagId};
use crate::error::Result;
use crate::model::{Alignment, ListStyle, ParagraphStyle, StyleRegistry, TextStyle};

// ============================================================================
// CharShape Record Byte Offsets (HWP 5.0 Spec 표 42)
// ============================================================================

/// Offset for face name ID (7 x u16 for different scripts)
const CHAR_SHAPE_FACE_NAME_OFFSET: usize = 0;
/// Offset for base font size (i32, in HWP units: hundredths of a point)
const CHAR_SHAPE_BASE_SIZE_OFFSET: usize = 70;
/// Offset for text properties (u32 bitmask)
const CHAR_SHAPE_PROPERTIES_OFFSET: usize = 74;
/// Offset for text color (RGB, 3 bytes)
const CHAR_SHAPE_TEXT_COLOR_OFFSET: usize = 78;

// CharShape property bit masks
const CHAR_PROP_BOLD: u32 = 1 << 0;
const CHAR_PROP_ITALIC: u32 = 1 << 1;
const CHAR_PROP_UNDERLINE: u32 = 1 << 2;
const CHAR_PROP_SUPERSCRIPT: u32 = 1 << 11;
const CHAR_PROP_SUBSCRIPT: u32 = 1 << 12;
const CHAR_PROP_STRIKETHROUGH: u32 = 1 << 13;

// ============================================================================
// ParaShape Record Byte Offsets (HWP 5.0 Spec 표 43)
// ============================================================================

/// Offset for properties1 (u32)
const PARA_SHAPE_PROPERTIES1_OFFSET: usize = 0;
/// Offset for left margin (i32)
#[allow(dead_code)]
const PARA_SHAPE_LEFT_MARGIN_OFFSET: usize = 4;
/// Offset for right margin (i32)
#[allow(dead_code)]
const PARA_SHAPE_RIGHT_MARGIN_OFFSET: usize = 8;
/// Offset for indent (i32)
#[allow(dead_code)]
const PARA_SHAPE_INDENT_OFFSET: usize = 12;
/// Offset for space before (i32)
const PARA_SHAPE_SPACE_BEFORE_OFFSET: usize = 16;
/// Offset for space after (i32)
const PARA_SHAPE_SPACE_AFTER_OFFSET: usize = 20;
/// Offset for line spacing value (i32)
const PARA_SHAPE_LINE_SPACING_OFFSET: usize = 24;
/// Offset for numbering/bullet ID (u16)
const PARA_SHAPE_NUMBERING_ID_OFFSET: usize = 30;

// ParaShape property bit masks and shifts
const PARA_PROP1_LINE_SPACING_TYPE_MASK: u32 = 0x03;
const PARA_PROP1_ALIGNMENT_SHIFT: u32 = 2;
const PARA_PROP1_ALIGNMENT_MASK: u32 = 0x07;
const PARA_PROP1_HEAD_SHAPE_TYPE_SHIFT: u32 = 23;
const PARA_PROP1_HEAD_SHAPE_TYPE_MASK: u32 = 0x03;
const PARA_PROP1_OUTLINE_LEVEL_SHIFT: u32 = 25;
const PARA_PROP1_OUTLINE_LEVEL_MASK: u32 = 0x07;

/// HWP unit conversion: 1 HWP unit = 1/7200 inch
const HWP_UNITS_PER_INCH: f32 = 7200.0;
/// Points per inch
const POINTS_PER_INCH: f32 = 72.0;

/// Parses DocInfo stream and populates the style registry.
pub fn parse_docinfo(data: &[u8], registry: &mut StyleRegistry) -> Result<()> {
    // Pre-allocate with reasonable capacity based on typical HWP documents
    let mut face_names: Vec<String> = Vec::with_capacity(32);
    let mut char_shapes: Vec<CharShapeData> = Vec::with_capacity(64);
    let mut para_shapes: Vec<ParaShapeData> = Vec::with_capacity(64);
    let mut styles: Vec<StyleData> = Vec::with_capacity(32);

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
                if let Ok(style) = parse_style(&record) {
                    styles.push(style);
                }
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

    // Register named styles (referencing CharShape and ParaShape by index)
    // This allows resolving style names to their actual formatting definitions
    for (index, style) in styles.iter().enumerate() {
        registry.register_named_style(
            index as u32,
            style.name.clone(),
            style.para_shape_id as u32,
            style.char_shape_id as u32,
        );
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

/// Parsed style data (named style definition).
#[derive(Debug, Default)]
struct StyleData {
    /// Style name in Korean
    #[allow(dead_code)]
    name: String,
    /// Style name in English
    #[allow(dead_code)]
    name_en: String,
    /// Style type: 0=paragraph style, 1=character style
    #[allow(dead_code)]
    style_type: u8,
    /// Next style ID (for paragraph styles)
    #[allow(dead_code)]
    next_style_id: u16,
    /// Language ID
    #[allow(dead_code)]
    lang_id: u16,
    /// Reference to ParaShape by index
    para_shape_id: u16,
    /// Reference to CharShape by index
    char_shape_id: u16,
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

    // CharShape structure (HWP 5.0 Spec 표 42):
    // Offset 0-13: Face name IDs for different scripts (7 x u16)
    // Offset 14-27: Character ratios (7 x u8) for different scripts
    // Offset 28-41: Character spacings (7 x i8)
    // Offset 42-55: Relative sizes (7 x u8)
    // Offset 56-69: Position adjustments (7 x i8)
    // Offset 70-73: Base size (i32, in HWP units)
    // Offset 74-77: Properties (u32)
    // Offset 78-80: Text color (RGB, 3 bytes)

    // Need at least 2 bytes for face name index
    if data.len() < CHAR_SHAPE_FACE_NAME_OFFSET + 2 {
        return Err(crate::error::Error::InvalidData(
            "CharShape too small".into(),
        ));
    }

    let face_name_index = Some(u16::from_le_bytes([
        data[CHAR_SHAPE_FACE_NAME_OFFSET],
        data[CHAR_SHAPE_FACE_NAME_OFFSET + 1],
    ]));

    // Base size (in HWP units: hundredths of a point)
    let font_size = if data.len() >= CHAR_SHAPE_BASE_SIZE_OFFSET + 4 {
        let size = i32::from_le_bytes([
            data[CHAR_SHAPE_BASE_SIZE_OFFSET],
            data[CHAR_SHAPE_BASE_SIZE_OFFSET + 1],
            data[CHAR_SHAPE_BASE_SIZE_OFFSET + 2],
            data[CHAR_SHAPE_BASE_SIZE_OFFSET + 3],
        ]);
        Some((size as f32) / 100.0)
    } else {
        None
    };

    // Properties bitmask
    let properties = if data.len() >= CHAR_SHAPE_PROPERTIES_OFFSET + 4 {
        u32::from_le_bytes([
            data[CHAR_SHAPE_PROPERTIES_OFFSET],
            data[CHAR_SHAPE_PROPERTIES_OFFSET + 1],
            data[CHAR_SHAPE_PROPERTIES_OFFSET + 2],
            data[CHAR_SHAPE_PROPERTIES_OFFSET + 3],
        ])
    } else {
        0
    };

    let bold = properties & CHAR_PROP_BOLD != 0;
    let italic = properties & CHAR_PROP_ITALIC != 0;
    let underline = properties & CHAR_PROP_UNDERLINE != 0;
    let superscript = properties & CHAR_PROP_SUPERSCRIPT != 0;
    let subscript = properties & CHAR_PROP_SUBSCRIPT != 0;
    let strikethrough = properties & CHAR_PROP_STRIKETHROUGH != 0;

    // Text color (RGB, 3 bytes)
    let text_color = if data.len() > CHAR_SHAPE_TEXT_COLOR_OFFSET + 2 {
        Some(format!(
            "#{:02X}{:02X}{:02X}",
            data[CHAR_SHAPE_TEXT_COLOR_OFFSET],
            data[CHAR_SHAPE_TEXT_COLOR_OFFSET + 1],
            data[CHAR_SHAPE_TEXT_COLOR_OFFSET + 2]
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
        return Err(crate::error::Error::InvalidData(
            "ParaShape too small".into(),
        ));
    }

    // ParaShape structure (HWP 5.0 Spec 표 43)
    let properties1 = u32::from_le_bytes([
        data[PARA_SHAPE_PROPERTIES1_OFFSET],
        data[PARA_SHAPE_PROPERTIES1_OFFSET + 1],
        data[PARA_SHAPE_PROPERTIES1_OFFSET + 2],
        data[PARA_SHAPE_PROPERTIES1_OFFSET + 3],
    ]);

    // Alignment (bits 2-4)
    let alignment =
        match (properties1 >> PARA_PROP1_ALIGNMENT_SHIFT) & PARA_PROP1_ALIGNMENT_MASK {
            0 => Alignment::Justify,
            1 => Alignment::Left,
            2 => Alignment::Right,
            3 => Alignment::Center,
            4 | 5 => Alignment::Justify, // Distribute/Split -> Justify fallback
            _ => Alignment::Left,
        };

    // Space before (in HWP units -> points)
    let space_before_raw = i32::from_le_bytes([
        data[PARA_SHAPE_SPACE_BEFORE_OFFSET],
        data[PARA_SHAPE_SPACE_BEFORE_OFFSET + 1],
        data[PARA_SHAPE_SPACE_BEFORE_OFFSET + 2],
        data[PARA_SHAPE_SPACE_BEFORE_OFFSET + 3],
    ]);
    let space_before = Some((space_before_raw as f32) / HWP_UNITS_PER_INCH * POINTS_PER_INCH);

    // Space after (in HWP units -> points)
    let space_after_raw = i32::from_le_bytes([
        data[PARA_SHAPE_SPACE_AFTER_OFFSET],
        data[PARA_SHAPE_SPACE_AFTER_OFFSET + 1],
        data[PARA_SHAPE_SPACE_AFTER_OFFSET + 2],
        data[PARA_SHAPE_SPACE_AFTER_OFFSET + 3],
    ]);
    let space_after = Some((space_after_raw as f32) / HWP_UNITS_PER_INCH * POINTS_PER_INCH);

    // Line spacing
    let line_spacing_raw = i32::from_le_bytes([
        data[PARA_SHAPE_LINE_SPACING_OFFSET],
        data[PARA_SHAPE_LINE_SPACING_OFFSET + 1],
        data[PARA_SHAPE_LINE_SPACING_OFFSET + 2],
        data[PARA_SHAPE_LINE_SPACING_OFFSET + 3],
    ]);
    let line_spacing_type = properties1 & PARA_PROP1_LINE_SPACING_TYPE_MASK;
    let line_spacing = match line_spacing_type {
        0 => Some((line_spacing_raw as f32) / 100.0), // Percentage
        1 => Some((line_spacing_raw as f32) / HWP_UNITS_PER_INCH * POINTS_PER_INCH), // Fixed
        _ => None,
    };

    // Outline level from Properties1
    let head_shape_type =
        (properties1 >> PARA_PROP1_HEAD_SHAPE_TYPE_SHIFT) & PARA_PROP1_HEAD_SHAPE_TYPE_MASK;
    let outline_level = if head_shape_type == 1 {
        // Outline-styled paragraph
        let level =
            ((properties1 >> PARA_PROP1_OUTLINE_LEVEL_SHIFT) & PARA_PROP1_OUTLINE_LEVEL_MASK) as u8;
        if (1..=6).contains(&level) {
            Some(level) // Levels 1-6 are headings
        } else {
            None // Level 0 or 7 are normal text
        }
    } else {
        None // Not an outline style
    };

    // List style from numbering ID
    let numbering_id = u16::from_le_bytes([
        data[PARA_SHAPE_NUMBERING_ID_OFFSET],
        data[PARA_SHAPE_NUMBERING_ID_OFFSET + 1],
    ]);
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

/// Parses HWPTAG_STYLE record.
///
/// Style records define named styles that reference CharShape and ParaShape by index.
/// Structure (HWP 5.0 Spec):
/// - name_len (u16): Length of style name in WCHARs
/// - name (name_len * 2 bytes): Style name in UTF-16LE
/// - name_en_len (u16): Length of English style name
/// - name_en (name_en_len * 2 bytes): English style name
/// - properties (1 byte): Style type (bit 0: 0=paragraph, 1=character)
/// - next_style_id (u16): ID of next style
/// - lang_id (u16): Language ID
/// - para_shape_id (u16): Reference to ParaShape
/// - char_shape_id (u16): Reference to CharShape
fn parse_style(record: &Record) -> Result<StyleData> {
    let data = record.data();
    if data.len() < 2 {
        return Err(crate::error::Error::InvalidData("Style too small".into()));
    }

    let mut offset = 0;

    // Parse style name
    let name_len = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
    offset += 2;
    let name = if name_len > 0 && offset + name_len * 2 <= data.len() {
        let name_data = &data[offset..offset + name_len * 2];
        offset += name_len * 2;
        decode_utf16le_string(name_data).unwrap_or_default()
    } else {
        String::new()
    };

    // Parse English style name
    let name_en = if offset + 2 <= data.len() {
        let name_en_len = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;
        if name_en_len > 0 && offset + name_en_len * 2 <= data.len() {
            let name_en_data = &data[offset..offset + name_en_len * 2];
            offset += name_en_len * 2;
            decode_utf16le_string(name_en_data).unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // Parse properties and IDs
    let style_type = if offset < data.len() {
        let props = data[offset];
        offset += 1;
        props & 0x01 // bit 0: 0=paragraph style, 1=character style
    } else {
        0
    };

    let next_style_id = if offset + 2 <= data.len() {
        let id = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        id
    } else {
        0
    };

    let lang_id = if offset + 2 <= data.len() {
        let id = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        id
    } else {
        0
    };

    let para_shape_id = if offset + 2 <= data.len() {
        let id = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        id
    } else {
        0
    };

    let char_shape_id = if offset + 2 <= data.len() {
        u16::from_le_bytes([data[offset], data[offset + 1]])
    } else {
        0
    };

    Ok(StyleData {
        name,
        name_en,
        style_type,
        next_style_id,
        lang_id,
        para_shape_id,
        char_shape_id,
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
