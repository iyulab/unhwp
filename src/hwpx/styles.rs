//! Style parsing for HWPX documents.

use crate::error::Result;
use crate::model::{Alignment, ListStyle, ParagraphStyle, StyleRegistry, TextStyle};
use quick_xml::events::Event;
use quick_xml::Reader;

/// Parses styles from header.xml or styles definition.
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
                    "charPr" | "charProperties" => {
                        let id = get_id_attr(&e);
                        current_char_style = Some((id, TextStyle::default()));
                        in_char_properties = true;
                    }
                    "paraPr" | "paraProperties" => {
                        let id = get_id_attr(&e);
                        current_para_style = Some((id, ParagraphStyle::default()));
                        in_para_properties = true;
                    }
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
                            style.underline = true;
                        }
                    }
                    "strikeout" | "strikethrough" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            style.strikethrough = true;
                        }
                    }
                    "fontRef" | "font" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            if let Some(face) = get_string_attr(&e, "face") {
                                style.font_name = Some(face);
                            }
                        }
                    }
                    "sz" | "size" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            if let Some(size) = get_float_attr(&e, "val") {
                                // Size might be in half-points or points
                                style.font_size = Some(size / 100.0);
                            }
                        }
                    }
                    "color" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            if let Some(color) = get_string_attr(&e, "val") {
                                style.color = Some(format!("#{}", color));
                            }
                        }
                    }
                    "align" | "alignment" if in_para_properties => {
                        if let Some((_, ref mut style)) = current_para_style {
                            if let Some(align) = get_string_attr(&e, "val") {
                                style.alignment = match align.to_lowercase().as_str() {
                                    "left" => Alignment::Left,
                                    "center" => Alignment::Center,
                                    "right" => Alignment::Right,
                                    "justify" | "both" => Alignment::Justify,
                                    _ => Alignment::Left,
                                };
                            }
                        }
                    }
                    "outlineLevel" | "heading" if in_para_properties => {
                        if let Some((_, ref mut style)) = current_para_style {
                            if let Some(level) = get_int_attr(&e, "val") {
                                style.heading_level = (level as u8).min(6);
                            }
                        }
                    }
                    "numbering" if in_para_properties => {
                        if let Some((_, ref mut style)) = current_para_style {
                            style.list_style = Some(ListStyle::Ordered);
                        }
                    }
                    "bullet" if in_para_properties => {
                        if let Some((_, ref mut style)) = current_para_style {
                            style.list_style = Some(ListStyle::Unordered);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");

                match name {
                    "charPr" | "charProperties" => {
                        if let Some((id, style)) = current_char_style.take() {
                            registry.register_char_style(id, style);
                        }
                        in_char_properties = false;
                    }
                    "paraPr" | "paraProperties" => {
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
