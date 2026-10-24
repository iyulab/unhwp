//! Style parsing for HWPX documents.
//!
//! Parses character and paragraph styles from HWPML header.xml.
//! Supports both HWPML 2011 elements (charShape, paraShape) and
//! common abbreviations (charPr, paraPr).

use crate::error::Result;
use crate::model::{
    Alignment, HeadKind, ListStyle, NumberLevel, NumberShape, Numbering, ParaHead, ParagraphStyle,
    StyleRegistry, TextStyle, NUMBERING_LEVELS,
};
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
    // A numbering definition being read (`hh:numbering`), and the level whose format text is
    // still to come (`hh:paraHead`).
    let mut current_numbering: Option<(u32, Vec<NumberLevel>)> = None;
    let mut pending_level: Option<(usize, NumberLevel)> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local_name = e.local_name();
                let name = local_name.as_ref();

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

                    // Numbering definitions: `hh:numbering id`, with a `hh:paraHead` per level
                    // whose text is the format (`^1.`).
                    "numbering" if !in_para_properties => {
                        finish_level(&mut current_numbering, &mut pending_level);
                        current_numbering = Some((get_id_attr(&e), Vec::new()));
                    }
                    "paraHead" if current_numbering.is_some() => {
                        finish_level(&mut current_numbering, &mut pending_level);
                        let level = get_int_attr(&e, "level").unwrap_or(0);
                        if (1..=NUMBERING_LEVELS as i32).contains(&level) {
                            pending_level = Some((
                                level as usize - 1,
                                NumberLevel {
                                    shape: NumberShape::from_name(
                                        &get_string_attr(&e, "numFormat").unwrap_or_default(),
                                    ),
                                    format: String::new(),
                                    start: get_int_attr(&e, "start")
                                        .and_then(|n| u32::try_from(n).ok())
                                        .unwrap_or(1),
                                },
                            ));
                        }
                    }
                    // Bullet definitions: `hh:bullet id char`.
                    "bullet" if !in_para_properties => {
                        if let Some(bullet) =
                            get_string_attr(&e, "char").and_then(|c| c.chars().next())
                        {
                            registry.register_bullet(get_id_attr(&e), bullet);
                        }
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
                            // OWPML underline types are NONE (the default), BOTTOM, CENTER and
                            // TOP. CENTER is the HWP 5.0 «middle» kind, which hwp2hwpx writes
                            // as no underline; the HWP 5.0 reader treats it the same way.
                            let utype = get_string_attr(&e, "type").unwrap_or_default();
                            style.underline = utype.eq_ignore_ascii_case("bottom")
                                || utype.eq_ignore_ascii_case("top");
                        }
                    }
                    "strikeout" | "strikethrough" if in_char_properties => {
                        if let Some((_, ref mut style)) = current_char_style {
                            let shape = get_string_attr(&e, "shape").unwrap_or_default();
                            style.strikethrough = strikeout_shape_draws(&shape);
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

                    // The head of a paragraph shape: `hh:heading type idRef level`, where the
                    // type says whether the paragraph is outlined, numbered or bulleted.
                    "heading" if in_para_properties => {
                        if let Some((_, ref mut style)) = current_para_style {
                            let kind = match get_string_attr(&e, "type").as_deref() {
                                Some("OUTLINE") => Some(HeadKind::Outline),
                                Some("NUMBER") => Some(HeadKind::Number),
                                Some("BULLET") => Some(HeadKind::Bullet),
                                _ => None,
                            };
                            let level = get_int_attr(&e, "level").unwrap_or(0);
                            style.head = kind.map(|kind| ParaHead {
                                kind,
                                level: level.clamp(0, i32::from(u8::MAX)) as u8,
                                id: get_int_attr(&e, "idRef")
                                    .and_then(|n| u32::try_from(n).ok())
                                    .unwrap_or(0),
                            });
                            // An outline level is a heading level; a number or a bullet is not.
                            if kind == Some(HeadKind::Outline) && level > 0 {
                                style.heading_level = (level as u8).min(6);
                            }
                        }
                    }

                    // Heading level / outline level
                    "outlineLevel" | "level" if in_para_properties => {
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
            Ok(Event::Text(t)) => {
                // The format of the level being read.
                if let Some((_, level)) = pending_level.as_mut() {
                    level.format.push_str(&super::xml::decode_text(&t));
                }
            }
            Ok(Event::End(e)) => {
                let local_name = e.local_name();
                let name = local_name.as_ref();

                match name {
                    "paraHead" => finish_level(&mut current_numbering, &mut pending_level),
                    "numbering" if current_numbering.is_some() => {
                        finish_level(&mut current_numbering, &mut pending_level);
                        if let Some((id, levels)) = current_numbering.take() {
                            registry.register_numbering(id, Numbering { levels });
                        }
                    }
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

/// Puts the level being read into the numbering being read, at its place.
fn finish_level(
    numbering: &mut Option<(u32, Vec<NumberLevel>)>,
    pending: &mut Option<(usize, NumberLevel)>,
) {
    let (Some((_, levels)), Some((index, level))) = (numbering.as_mut(), pending.take()) else {
        return;
    };
    if levels.len() <= index {
        levels.resize(index + 1, NumberLevel::default());
    }
    levels[index] = level;
}

/// Parses alignment string to Alignment enum.
/// Whether `<hh:strikeout shape="…">` draws a line through the text.
///
/// `shape` is an OWPML `LineType2`. The strikeout shapes a word processor offers are `SOLID`
/// through `SLIM_THICK_SLIM`; `WAVE`, `DOUBLEWAVE` and the 3D kinds are not, and Hancom writes
/// `3D` on the character properties of ordinary unstruck text (its printed copy has no line).
/// hwp2hwpx (`ValueConvertor.lineType2(BorderType2)`) maps those kinds to `NONE` the same way.
/// A missing or unknown shape is `NONE`, the attribute's default in Hancom's OWPML model.
fn strikeout_shape_draws(shape: &str) -> bool {
    matches!(
        shape.to_ascii_uppercase().as_str(),
        "SOLID"
            | "DOT"
            | "DASH"
            | "DASH_DOT"
            | "DASH_DOT_DOT"
            | "LONG_DASH"
            | "CIRCLE"
            | "DOUBLE_SLIM"
            | "SLIM_THICK"
            | "THICK_SLIM"
            | "SLIM_THICK_SLIM"
    )
}

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
        if attr.key.as_ref() == "id" {
            return attr.value.parse().unwrap_or(0);
        }
    }
    0
}

/// Gets a boolean attribute value.
fn get_bool_attr(e: &quick_xml::events::BytesStart, name: &str) -> Option<bool> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name {
            let val = attr.value.as_ref();
            return Some(val == "1" || val.to_lowercase() == "true");
        }
    }
    None
}

/// Gets a string attribute value.
fn get_string_attr(e: &quick_xml::events::BytesStart, name: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name {
            return Some(attr.value.to_string());
        }
    }
    None
}

/// Gets a float attribute value.
fn get_float_attr(e: &quick_xml::events::BytesStart, name: &str) -> Option<f32> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name {
            return attr.value.parse().ok();
        }
    }
    None
}

/// Gets an integer attribute value.
fn get_int_attr(e: &quick_xml::events::BytesStart, name: &str) -> Option<i32> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name {
            return attr.value.parse().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The character styles of a header holding one `charPr` per entry of `elements`, each
    /// `charPr` carrying that entry as its child elements.
    fn styles_of(elements: &[String]) -> Vec<TextStyle> {
        let char_prs: String = elements
            .iter()
            .enumerate()
            .map(|(id, inner)| {
                format!(r##"<hh:charPr id="{id}" height="1000" textColor="#000000">{inner}</hh:charPr>"##)
            })
            .collect();
        let xml = format!(
            r#"<hh:head xmlns:hh="http://www.hancom.co.kr/hwpml/2011/head"><hh:refList><hh:charProperties itemCnt="{}">{char_prs}</hh:charProperties></hh:refList></hh:head>"#,
            elements.len()
        );
        let mut registry = StyleRegistry::new();
        parse_styles(&xml, &mut registry).unwrap();
        (0..elements.len() as u32)
            .map(|id| registry.get_char_style(id).cloned().unwrap_or_default())
            .collect()
    }

    #[test]
    fn only_the_line_shapes_a_strikeout_offers_strike_the_text() {
        // `NONE` and `3D` are the two shapes of a public press release's header; its printed copy
        // shows no line through the `3D` runs.
        let shapes = [
            "NONE",
            "3D",
            "SOLID",
            "DASH_DOT",
            "SLIM_THICK_SLIM",
            "WAVE",
            "REV3D",
            "",
        ];
        let elements: Vec<String> = shapes
            .iter()
            .map(|shape| match *shape {
                "" => r##"<hh:strikeout color="#000000"/>"##.to_string(),
                shape => format!(r##"<hh:strikeout shape="{shape}" color="#000000"/>"##),
            })
            .collect();
        let struck: Vec<bool> = styles_of(&elements)
            .iter()
            .map(|s| s.strikethrough)
            .collect();
        assert_eq!(
            struck,
            [false, false, true, true, true, false, false, false]
        );
    }

    #[test]
    fn an_underline_is_one_below_or_above_the_text() {
        let elements: Vec<String> = ["NONE", "BOTTOM", "CENTER", "TOP"]
            .iter()
            .map(|t| format!(r##"<hh:underline type="{t}" shape="SOLID" color="#000000"/>"##))
            .chain([r##"<hh:underline shape="SOLID" color="#000000"/>"##.to_string()])
            .collect();
        let underlined: Vec<bool> = styles_of(&elements).iter().map(|s| s.underline).collect();
        assert_eq!(underlined, [false, true, false, true, false]);
    }
}
