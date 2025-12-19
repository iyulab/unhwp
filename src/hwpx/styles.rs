//! Style parsing for HWPX documents.
//!
//! Parses character and paragraph styles from HWPML header.xml.
//! Supports both HWPML 2011 elements (charShape, paraShape) and
//! common abbreviations (charPr, paraPr).

use crate::error::Result;
use crate::model::{Alignment, ListStyle, ParagraphStyle, StyleRegistry, TextStyle};
use quick_xml::events::Event;
use quick_xml::Reader;

/// Parses styles from header.xml or styles definition.
///
/// Recognizes both full HWPML element names (charShape, paraShape) and
/// abbreviated forms (charPr, paraPr) for broader compatibility.
pub fn parse_styles(xml: &str, registry: &mut StyleRegistry) -> Result<()> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut current_char_style: Option<(u32, TextStyle)> = None;
    let mut current_para_style: Option<(u32, ParagraphStyle)> = None;
    let mut in_char_properties = false;
    let mut in_para_properties = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");

                match name {
                    // Character shape/properties - HWPML uses "charShape" or "charPr"
                    "charShape" | "charPr" | "charProperties" => {
                        let id = get_id_attr(&e);
                        current_char_style = Some((id, TextStyle::default()));
                        in_char_properties = true;
                    }
                    // Paragraph shape/properties - HWPML uses "paraShape" or "paraPr"
                    "paraShape" | "paraPr" | "paraProperties" => {
                        let id = get_id_attr(&e);
                        current_para_style = Some((id, ParagraphStyle::default()));
                        in_para_properties = true;
                    }

                    // Character formatting elements
                    "bold" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            style.bold = get_bool_attr(&e, "val").unwrap_or(true);
                        }
                    }
                    "italic" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            style.italic = get_bool_attr(&e, "val").unwrap_or(true);
                        }
                    }
                    "underline" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            // Check underline type - any non-none value means underlined
                            let utype = get_string_attr(&e, "type").unwrap_or_default();
                            style.underline = utype != "none" && utype != "0";
                            if utype.is_empty() {
                                style.underline = true; // Default if just <underline/>
                            }
                        }
                    }
                    "strikeout" | "strikethrough" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            style.strikethrough = true;
                        }
                    }
                    // Superscript/subscript (HWPML uses supscript/subscript in charShape)
                    "supscript" | "superscript" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            style.superscript = true;
                        }
                    }
                    "subscript" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            style.subscript = true;
                        }
                    }

                    // Font face information
                    "fontRef" | "font" | "fontface" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            // Try multiple attribute names for font face
                            if let Some(face) = get_string_attr(&e, "face")
                                .or_else(|| get_string_attr(&e, "hangul"))
                                .or_else(|| get_string_attr(&e, "latin"))
                            {
                                style.font_name = Some(face);
                            }
                        }
                    }

                    // Font size - HWPML uses "height" in charShape (in hwpunit = 1/7200 inch)
                    "sz" | "size" | "height" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            if let Some(size) =
                                get_float_attr(&e, "val").or_else(|| get_float_attr(&e, "height"))
                            {
                                // HWPML height is in hwpunit (1/7200 inch)
                                // 1 point = 100 hwpunit, so divide by 100
                                style.font_size = Some(size / 100.0);
                            }
                        }
                    }

                    // Text color
                    "color" | "textColor" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            if let Some(color) = get_string_attr(&e, "val")
                                .or_else(|| get_string_attr(&e, "textColor"))
                            {
                                // Handle various color formats
                                let color_str = if color.starts_with('#') {
                                    color
                                } else if color.len() == 6 || color.len() == 8 {
                                    format!("#{}", color)
                                } else {
                                    color
                                };
                                style.color = Some(color_str);
                            }
                        }
                    }

                    // Highlight/shading (background color)
                    "highlight" | "shd" | "shading" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            if let Some(color) = get_string_attr(&e, "val")
                                .or_else(|| get_string_attr(&e, "backColor"))
                            {
                                style.background_color = Some(format!("#{}", color));
                            }
                        }
                    }

                    // Paragraph alignment
                    "align" | "alignment" if in_para_properties => {
                        if let Some((_, ref mut style)) = current_para_style {
                            if let Some(align) = get_string_attr(&e, "val")
                                .or_else(|| get_string_attr(&e, "horizontal"))
                            {
                                style.alignment = parse_alignment(&align);
                            }
                        }
                    }

                    // Heading level / outline level
                    "outlineLevel" | "heading" | "level" if in_para_properties => {
                        if let Some((_, ref mut style)) = current_para_style {
                            if let Some(level) =
                                get_int_attr(&e, "val").or_else(|| get_int_attr(&e, "level"))
                            {
                                // Level 1-6 for headings, 0 means not a heading
                                if level > 0 {
                                    style.heading_level = (level as u8).min(6);
                                }
                            }
                        }
                    }

                    // Indent (HWPML uses indent element with left/right/firstLine)
                    "indent" | "margin" if in_para_properties => {
                        if let Some((_, ref mut style)) = current_para_style {
                            if let Some(level) = get_int_attr(&e, "level")
                                .or_else(|| get_int_attr(&e, "left").map(|v| v / 850))
                            {
                                style.indent_level = level.max(0) as u8;
                            }
                        }
                    }

                    // Line spacing
                    "lineSpacing" | "spacing" | "lnSpc" if in_para_properties => {
                        if let Some((_, ref mut style)) = current_para_style {
                            if let Some(spacing) =
                                get_float_attr(&e, "val").or_else(|| get_float_attr(&e, "line"))
                            {
                                // HWPML line spacing is in percentage (e.g., 160 = 160%)
                                style.line_spacing = Some(spacing / 100.0);
                            }
                        }
                    }

                    // Numbered/bulleted lists
                    "numbering" if in_para_properties => {
                        if let Some((_, ref mut style)) = current_para_style {
                            style.list_style = Some(ListStyle::Ordered);
                        }
                    }
                    "bullet" if in_para_properties => {
                        if let Some((_, ref mut style)) = current_para_style {
                            if let Some(char_val) = get_string_attr(&e, "char") {
                                if let Some(ch) = char_val.chars().next() {
                                    style.list_style = Some(ListStyle::CustomBullet(ch));
                                }
                            } else {
                                style.list_style = Some(ListStyle::Unordered);
                            }
                        }
                    }

                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");

                match name {
                    "charShape" | "charPr" | "charProperties" => {
                        if let Some((id, style)) = current_char_style.take() {
                            registry.register_char_style(id, style);
                        }
                        in_char_properties = false;
                    }
                    "paraShape" | "paraPr" | "paraProperties" => {
                        if let Some((id, style)) = current_para_style.take() {
                            registry.register_para_style(id, style);
                        }
                        in_para_properties = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(crate::error::Error::XmlParse(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(())
}

/// Parses alignment string to Alignment enum.
fn parse_alignment(align: &str) -> Alignment {
    match align.to_lowercase().as_str() {
        "left" | "0" => Alignment::Left,
        "center" | "1" => Alignment::Center,
        "right" | "2" => Alignment::Right,
        "justify" | "both" | "3" => Alignment::Justify,
        _ => Alignment::Left,
    }
}

/// Gets the 'id' attribute as u32.
fn get_id_attr(e: &quick_xml::events::BytesStart) -> u32 {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"id" {
            if let Ok(val) = std::str::from_utf8(&attr.value) {
                return val.parse().unwrap_or(0);
            }
        }
    }
    0
}

/// Gets a boolean attribute value.
fn get_bool_attr(e: &quick_xml::events::BytesStart, name: &str) -> Option<bool> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name.as_bytes() {
            if let Ok(val) = std::str::from_utf8(&attr.value) {
                return Some(val == "1" || val.to_lowercase() == "true");
            }
        }
    }
    None
}

/// Gets a string attribute value.
fn get_string_attr(e: &quick_xml::events::BytesStart, name: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name.as_bytes() {
            if let Ok(val) = std::str::from_utf8(&attr.value) {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Gets a float attribute value.
fn get_float_attr(e: &quick_xml::events::BytesStart, name: &str) -> Option<f32> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name.as_bytes() {
            if let Ok(val) = std::str::from_utf8(&attr.value) {
                return val.parse().ok();
            }
        }
    }
    None
}

/// Gets an integer attribute value.
fn get_int_attr(e: &quick_xml::events::BytesStart, name: &str) -> Option<i32> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name.as_bytes() {
            if let Ok(val) = std::str::from_utf8(&attr.value) {
                return val.parse().ok();
            }
        }
    }
    None
}
