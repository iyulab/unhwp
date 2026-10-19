//! DocInfo stream parsing for HWP 5.0.
//!
//! DocInfo contains document-wide definitions for fonts, styles, and shapes.

use super::record::{Record, RecordIterator, TagId};
use crate::error::Result;
use crate::model::{
    Alignment, HeadKind, NumberLevel, NumberShape, Numbering, ParaHead, ParagraphStyle,
    StyleRegistry, TextStyle, NUMBERING_LEVELS,
};

// ============================================================================
// CharShape Record Byte Offsets (HWP 5.0 Spec 표 42)
//
// Exact layout:
//   Offset  0-13: FaceID[7]       (7 × u16 = 14 bytes)
//   Offset 14-20: CharRatio[7]    (7 × u8  =  7 bytes)
//   Offset 21-27: CharSpacing[7]  (7 × i8  =  7 bytes)
//   Offset 28-34: RelativeSize[7] (7 × u8  =  7 bytes)
//   Offset 35-41: CharPosition[7] (7 × i8  =  7 bytes)
//   Offset 42-45: BaseSize        (i32, hundredths of a point)
//   Offset 46-49: Properties      (u32 bitmask)
//   Offset 50:    ShadowGap1      (i8)
//   Offset 51:    ShadowGap2      (i8)
//   Offset 52-55: TextColor       (COLORREF)
// ============================================================================

/// Offset for face name ID (7 x u16 for different scripts)
const CHAR_SHAPE_FACE_NAME_OFFSET: usize = 0;
/// Offset for base font size (i32, in HWP units: hundredths of a point)
const CHAR_SHAPE_BASE_SIZE_OFFSET: usize = 42;
/// Offset for text properties (u32 bitmask)
const CHAR_SHAPE_PROPERTIES_OFFSET: usize = 46;
/// Offset for text color (COLORREF, 4 bytes)
const CHAR_SHAPE_TEXT_COLOR_OFFSET: usize = 52;

// CharShape property bit masks (HWP 5.0 스펙 Table 35: bit0=italic, bit1=bold)
const CHAR_PROP_ITALIC: u32 = 1 << 0;
const CHAR_PROP_BOLD: u32 = 1 << 1;
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
    let mut numberings: Vec<Numbering> = Vec::new();
    let mut bullets: Vec<Option<char>> = Vec::new();

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
            // Numbering and bullet definitions are named by their place among the records,
            // from 1. A record that cannot be read still takes its place, so the ones after
            // it keep their numbers.
            TagId::Numbering => {
                numberings.push(parse_numbering(&record).unwrap_or_default());
            }
            TagId::Bullet => {
                bullets.push(parse_bullet(&record));
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
            list_style: None,
            indent_level: shape.indent_level,
            line_spacing: shape.line_spacing,
            space_before: shape.space_before,
            space_after: shape.space_after,
            head: shape.head,
        };

        registry.register_para_style(index as u32, para_style);
    }

    for (index, numbering) in numberings.into_iter().enumerate() {
        registry.register_numbering(index as u32 + 1, numbering);
    }
    for (index, bullet) in bullets.into_iter().enumerate() {
        if let Some(bullet) = bullet {
            registry.register_bullet(index as u32 + 1, bullet);
        }
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
    head: Option<ParaHead>,
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
    if data.len() < 3 {
        return Err(crate::error::Error::InvalidData(
            "FaceName too small".into(),
        ));
    }

    // FaceName structure (HWP 5.0 Spec):
    // - Byte 0:   Property flags (u8)
    // - Byte 1-2: Name length in WCHARs (u16 LE)
    // - Byte 3+:  UTF-16LE name string

    let name_len = u16::from_le_bytes([data[1], data[2]]) as usize;
    let name_start = 3;
    let name_end = (name_start + name_len * 2).min(data.len());

    if name_end > name_start {
        decode_utf16le_string(&data[name_start..name_end])
    } else {
        Ok(String::new())
    }
}

/// Parses HWPTAG_CHAR_SHAPE record.
fn parse_char_shape(record: &Record) -> Result<CharShapeData> {
    let data = record.data();

    // CharShape structure (HWP 5.0 Spec 표 42):
    // Offset  0-13: FaceID[7]       (7 × u16 = 14 bytes)
    // Offset 14-20: CharRatio[7]    (7 × u8  =  7 bytes)
    // Offset 21-27: CharSpacing[7]  (7 × i8  =  7 bytes)
    // Offset 28-34: RelativeSize[7] (7 × u8  =  7 bytes)
    // Offset 35-41: CharPosition[7] (7 × i8  =  7 bytes)
    // Offset 42-45: BaseSize        (i32, hundredths of a point)
    // Offset 46-49: Properties      (u32 bitmask)
    // Offset 50:    ShadowGap1      (i8)
    // Offset 51:    ShadowGap2      (i8)
    // Offset 52-55: TextColor       (COLORREF)

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
    let alignment = match (properties1 >> PARA_PROP1_ALIGNMENT_SHIFT) & PARA_PROP1_ALIGNMENT_MASK {
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

    // The head the shape gives its paragraphs: an outline, a number or a bullet, at a level,
    // from the definition the shape names (from 1; an outline paragraph is counted by the
    // section's outline numbering instead).
    let numbering_id = u16::from_le_bytes([
        data[PARA_SHAPE_NUMBERING_ID_OFFSET],
        data[PARA_SHAPE_NUMBERING_ID_OFFSET + 1],
    ]);
    let head_level =
        ((properties1 >> PARA_PROP1_OUTLINE_LEVEL_SHIFT) & PARA_PROP1_OUTLINE_LEVEL_MASK) as u8;
    let head = match head_shape_type {
        1 => Some(HeadKind::Outline),
        2 => Some(HeadKind::Number),
        3 => Some(HeadKind::Bullet),
        _ => None,
    }
    .map(|kind| ParaHead {
        kind,
        level: head_level,
        id: u32::from(numbering_id),
    });

    Ok(ParaShapeData {
        alignment,
        outline_level,
        head,
        indent_level: 0,
        line_spacing,
        space_before,
        space_after,
    })
}

/// Parses HWPTAG_NUMBERING record: seven levels, each a 12-byte paragraph head info (its
/// attributes hold the number shape in bits 5-8) and a length-prefixed format string, then the
/// first number (`u16`) and, from version 5.0.3.0, one first number per level (`u32`).
fn parse_numbering(record: &Record) -> Option<Numbering> {
    const HEAD_INFO_LEN: usize = 12;
    const SHAPE_SHIFT: u32 = 5;
    const SHAPE_MASK: u32 = 0x0F;

    let data = record.data();
    let mut at = 0usize;
    let mut levels = Vec::with_capacity(NUMBERING_LEVELS);
    for _ in 0..NUMBERING_LEVELS {
        let head = data.get(at..at + HEAD_INFO_LEN)?;
        let attributes = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
        at += HEAD_INFO_LEN;
        let length = usize::from(u16::from_le_bytes([*data.get(at)?, *data.get(at + 1)?]));
        at += 2;
        let text = data.get(at..at + length * 2)?;
        at += length * 2;
        let units: Vec<u16> = text
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(u16::from_le_bytes)
            .collect();
        levels.push(NumberLevel {
            shape: NumberShape::from_code((attributes >> SHAPE_SHIFT) & SHAPE_MASK),
            format: String::from_utf16_lossy(&units),
            start: 1,
        });
    }

    // The first number of the definition, and of each level where the record gives it.
    let first = data
        .get(at..at + 2)
        .map(|b| u32::from(u16::from_le_bytes([b[0], b[1]])))
        .filter(|&n| n > 0)
        .unwrap_or(1);
    at += 2;
    for (index, level) in levels.iter_mut().enumerate() {
        level.start = data
            .get(at + index * 4..at + index * 4 + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .unwrap_or(first);
    }
    Some(Numbering { levels })
}

/// Parses HWPTAG_BULLET record: a 12-byte paragraph head info, then the bullet character.
/// A character from the private-use area is a glyph of a symbol font this reader does not
/// have, so it is written as a plain bullet.
fn parse_bullet(record: &Record) -> Option<char> {
    let data = record.data();
    let unit = u16::from_le_bytes([*data.get(12)?, *data.get(13)?]);
    if (0xE000..=0xF8FF).contains(&unit) {
        return Some('•');
    }
    char::from_u32(u32::from(unit))
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

    for &chunk in data.as_chunks::<2>().0 {
        let value = u16::from_le_bytes(chunk);
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
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(u16::from_le_bytes)
            .take_while(|&v| v != 0)
            .collect();
        String::from_utf16_lossy(&u16_values)
    } else {
        String::from("bin")
    };

    // Format filename: BIN + 4-digit hex + extension
    Some((bin_id, format!("BIN{:04X}.{}", bin_id, ext)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hwp5::record::{Record, RecordHeader, TagId};

    /// Builds a minimal CharShape binary payload with the given properties bitmask.
    ///
    /// Layout (HWP 5.0 Spec Table 35):
    ///   0-13:  FaceID[7]      (7 × u16 = 14 bytes, zeroed)
    ///   14-20: CharRatio[7]   (7 × u8  =  7 bytes, 100 each)
    ///   21-27: CharSpacing[7] (7 × i8  =  7 bytes, zeroed)
    ///   28-34: RelativeSize[7](7 × u8  =  7 bytes, 100 each)
    ///   35-41: CharPosition[7](7 × i8  =  7 bytes, zeroed)
    ///   42-45: BaseSize       (i32 LE, 1000 = 10pt in hundredths of a point)
    ///   46-49: Properties     (u32 LE bitmask)
    ///   50-55: ShadowGap + TextColor (zeroed)
    fn make_char_shape_record(properties: u32) -> Record {
        let mut data = vec![0u8; 56];
        // CharRatio and RelativeSize fields default to 100
        data[14..21].fill(100);
        data[28..35].fill(100);
        // BaseSize = 1000 (10pt)
        let base_size: i32 = 1000;
        data[42..46].copy_from_slice(&base_size.to_le_bytes());
        // Properties bitmask
        data[46..50].copy_from_slice(&properties.to_le_bytes());

        Record {
            header: RecordHeader {
                tag_id: TagId::CharShape as u16,
                level: 0,
                size: data.len() as u32,
            },
            data,
            offset: 0,
        }
    }

    #[test]
    fn test_char_shape_bit0_is_italic_not_bold() {
        // HWP 5.0 Spec Table 35: bit0 = italic (기울임), bit1 = bold (진하게)
        let record = make_char_shape_record(0b01); // bit0 set
        let shape = parse_char_shape(&record).expect("parse should succeed");
        assert!(shape.italic, "bit0 must be italic");
        assert!(!shape.bold, "bit0 must NOT be bold");
    }

    #[test]
    fn test_char_shape_bit1_is_bold_not_italic() {
        let record = make_char_shape_record(0b10); // bit1 set
        let shape = parse_char_shape(&record).expect("parse should succeed");
        assert!(shape.bold, "bit1 must be bold");
        assert!(!shape.italic, "bit1 must NOT be italic");
    }

    #[test]
    fn test_char_shape_both_bits() {
        let record = make_char_shape_record(0b11); // both bits set
        let shape = parse_char_shape(&record).expect("parse should succeed");
        assert!(shape.bold, "bit1 must be bold");
        assert!(shape.italic, "bit0 must be italic");
    }

    /// A record of `tag` holding `data`.
    fn record_of(tag: TagId, data: Vec<u8>) -> Record {
        Record {
            header: RecordHeader {
                tag_id: tag as u16,
                level: 1,
                size: data.len() as u32,
            },
            data,
            offset: 0,
        }
    }

    fn unhex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    /// A numbering record as the editor writes its default definition: `^1.` `^2.` `^3)`
    /// `^4)` `(^5)` `(^6)` `^7`, the second, fourth and sixth levels in Hangul syllables
    /// (attribute bits 5-8 = 8) and the seventh circled (= 1).
    const DEFAULT_NUMBERING: &str = concat!(
        "0c00000000003200ffffffff03005e0031002e000c01000000003200ffffffff",
        "03005e0032002e000c00000000003200ffffffff03005e00330029000c010000",
        "00003200ffffffff03005e00340029000c00000000003200ffffffff04002800",
        "5e00350029000c01000000003200ffffffff040028005e00360029002c000000",
        "00003200ffffffff02005e003700010001000000010000000100000001000000",
        "0100000001000000010000000800000000003200ffffffff0000080000000000",
        "3200ffffffff00000800000000003200ffffffff000001000000010000000100",
        "0000",
    );

    #[test]
    fn a_numbering_record_gives_the_shape_format_and_start_of_each_level() {
        let numbering = parse_numbering(&record_of(TagId::Numbering, unhex(DEFAULT_NUMBERING)))
            .expect("a whole record parses");
        let levels: Vec<(NumberShape, &str, u32)> = numbering
            .levels
            .iter()
            .map(|l| (l.shape, l.format.as_str(), l.start))
            .collect();
        assert_eq!(
            levels,
            [
                (NumberShape::Digit, "^1.", 1),
                (NumberShape::HangulSyllable, "^2.", 1),
                (NumberShape::Digit, "^3)", 1),
                (NumberShape::HangulSyllable, "^4)", 1),
                (NumberShape::Digit, "(^5)", 1),
                (NumberShape::HangulSyllable, "(^6)", 1),
                (NumberShape::CircledDigit, "^7", 1),
            ]
        );
    }

    #[test]
    fn a_truncated_numbering_record_is_not_a_definition() {
        let mut data = unhex(DEFAULT_NUMBERING);
        data.truncate(40);
        assert!(parse_numbering(&record_of(TagId::Numbering, data)).is_none());
    }

    /// A paragraph shape record: `properties` then zeros, with the numbering ID at offset 30.
    fn para_shape_record(properties: u32, numbering_id: u16) -> Record {
        let mut data = vec![0u8; 58];
        data[..4].copy_from_slice(&properties.to_le_bytes());
        data[30..32].copy_from_slice(&numbering_id.to_le_bytes());
        record_of(TagId::ParaShape, data)
    }

    #[test]
    fn a_paragraph_shape_names_its_head_kind_level_and_definition() {
        // Bits 23-24 = 2 (number), bits 25-27 = 1 (second level), numbering 3.
        let shape = parse_para_shape(&para_shape_record((2 << 23) | (1 << 25), 3)).unwrap();
        assert_eq!(
            shape.head,
            Some(ParaHead {
                kind: HeadKind::Number,
                level: 1,
                id: 3
            })
        );
        // Bullet and outline.
        let bullet = parse_para_shape(&para_shape_record(3 << 23, 2)).unwrap();
        assert_eq!(
            bullet.head.map(|h| (h.kind, h.id)),
            Some((HeadKind::Bullet, 2))
        );
        let outline = parse_para_shape(&para_shape_record(1 << 23, 0)).unwrap();
        assert_eq!(outline.head.map(|h| h.kind), Some(HeadKind::Outline));
        // No head.
        assert_eq!(
            parse_para_shape(&para_shape_record(0, 7)).unwrap().head,
            None
        );
    }

    #[test]
    fn a_bullet_record_gives_its_character_and_a_symbol_glyph_a_plain_bullet() {
        let bullet = |unit: u16| {
            let mut data = vec![0u8; 25];
            data[12..14].copy_from_slice(&unit.to_le_bytes());
            parse_bullet(&record_of(TagId::Bullet, data))
        };
        assert_eq!(bullet(0x25CF), Some('●'));
        assert_eq!(bullet(0xF0A7), Some('•'));
        assert_eq!(parse_bullet(&record_of(TagId::Bullet, vec![0u8; 8])), None);
    }

    #[test]
    fn the_definitions_are_named_by_their_place_among_the_records() {
        let mut stream = Vec::new();
        let mut push = |tag: u32, data: &[u8]| {
            let header = tag | (1 << 10) | ((data.len() as u32) << 20);
            stream.extend_from_slice(&header.to_le_bytes());
            stream.extend_from_slice(data);
        };
        push(TagId::Numbering as u32, &unhex(DEFAULT_NUMBERING));
        // A damaged record still takes its place, so the next one is the third.
        push(TagId::Numbering as u32, &[1, 2, 3]);
        push(TagId::Numbering as u32, &unhex(DEFAULT_NUMBERING));
        let mut bullet = vec![0u8; 25];
        bullet[12..14].copy_from_slice(&0x25A0u16.to_le_bytes());
        push(TagId::Bullet as u32, &bullet);

        let mut registry = StyleRegistry::new();
        parse_docinfo(&stream, &mut registry).unwrap();
        assert_eq!(registry.numbering(1).unwrap().levels.len(), 7);
        assert!(registry.numbering(2).unwrap().levels.is_empty());
        assert_eq!(registry.numbering(3).unwrap().levels.len(), 7);
        assert_eq!(registry.bullet(1), Some('■'));
    }
}
