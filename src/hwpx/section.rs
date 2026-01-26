//! Section parsing for HWPX documents.

use crate::error::Result;
use crate::model::{
    Block, Equation, ImageRef, InlineContent, Paragraph, ParagraphStyle, Section, StyleRegistry,
    Table, TableCell, TableRow, TextRun,
};
use quick_xml::events::Event;
use quick_xml::Reader;

/// Parses a section XML file.
pub fn parse_section(xml: &str, section_index: usize, styles: &StyleRegistry) -> Result<Section> {
    let mut section = Section::new(section_index);
    let mut parser = SectionParser::new(xml, styles);
    parser.parse(&mut section)?;
    Ok(section)
}

/// Extracted paragraph attributes from XML.
#[derive(Default)]
struct ParaAttrs {
    para_pr_id: Option<u32>,
    style_id: Option<u32>,
}

/// Extracted run attributes from XML.
#[derive(Default)]
struct RunAttrs {
    char_pr_id: Option<u32>,
}

/// Marker type for cell attributes (colspan/rowspan parsed from child elements).
#[derive(Default)]
struct CellAttrs;

/// Section parser state machine.
struct SectionParser<'a> {
    reader: Reader<&'a [u8]>,
    styles: &'a StyleRegistry,
}

impl<'a> SectionParser<'a> {
    fn new(xml: &'a str, styles: &'a StyleRegistry) -> Self {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        Self { reader, styles }
    }

    fn parse(&mut self, section: &mut Section) -> Result<()> {
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = get_local_name(&e);

                    match name.as_str() {
                        "p" => {
                            let attrs = ParaAttrs {
                                para_pr_id: get_attr_u32(&e, "paraPrIDRef"),
                                style_id: get_attr_u32(&e, "styleIDRef"),
                            };
                            buf.clear();
                            let para = self.parse_paragraph(attrs, Some(section))?;
                            if !para.is_empty() {
                                section.content.push(Block::Paragraph(para));
                            }
                        }
                        "tbl" => {
                            buf.clear();
                            let table = self.parse_table()?;
                            section.content.push(Block::Table(table));
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

    /// Parses a <hp:p> paragraph element.
    fn parse_paragraph(&mut self, attrs: ParaAttrs, section: Option<&mut Section>) -> Result<Paragraph> {
        // Build paragraph style from references
        let mut para_style = ParagraphStyle::default();

        if let Some(id) = attrs.para_pr_id {
            if let Some(style) = self.styles.get_para_style(id) {
                para_style = style.clone();
            }
        }

        if let Some(id) = attrs.style_id {
            if let Some(style) = self.styles.get_para_style(id) {
                if style.heading_level > 0 {
                    para_style.heading_level = style.heading_level;
                }
            }
        }

        let mut paragraph = Paragraph::with_style(para_style);
        let mut buf = Vec::new();
        
        // Collect tables found in runs (to add to section after paragraph)
        let mut tables_found: Vec<Table> = Vec::new();

        // Parse paragraph content
        loop {
            match self.reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = get_local_name(&e);

                    match name.as_str() {
                        "run" => {
                            let run_attrs = RunAttrs {
                                char_pr_id: get_attr_u32(&e, "charPrIDRef"),
                            };
                            buf.clear();
                            let run = self.parse_run(run_attrs, &mut paragraph, &mut tables_found)?;
                            if !run.text.is_empty() {
                                paragraph.content.push(InlineContent::Text(run));
                            }
                        }
                        "linesegarray" | "lineSeg" => {
                            // Line segment info - skip
                            buf.clear();
                            skip_element(&mut self.reader)?;
                        }
                        "ctrl" => {
                            // Control element (table, image, etc.)
                            buf.clear();
                            self.parse_control(&mut paragraph, &mut tables_found)?;
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(e)) => {
                    let name = get_local_name_end(&e);
                    if name == "p" {
                        break;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(crate::error::Error::XmlParse(e.to_string())),
                _ => {}
            }
            buf.clear();
        }
        
        // Add any tables found to section
        if let Some(sec) = section {
            for table in tables_found {
                sec.content.push(Block::Table(table));
            }
        }

        Ok(paragraph)
    }

    /// Parses a <hp:run> text run element, returning the text run and any images found.
    fn parse_run(&mut self, attrs: RunAttrs, paragraph: &mut Paragraph, tables: &mut Vec<Table>) -> Result<TextRun> {
        let text_style = attrs
            .char_pr_id
            .and_then(|id| self.styles.get_char_style(id))
            .cloned()
            .unwrap_or_default();

        let mut text = String::new();
        let mut buf = Vec::new();
        let mut in_pic = false;
        let mut in_ctrl = false;  // Track if we're inside a control element (to skip formula fields etc.)
        let mut in_text_element = false;  // Track if we're inside <hp:t> element

        loop {
            match self.reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = get_local_name(&e);

                    if name == "t" {
                        // Text content element - mark we're inside <hp:t>
                        in_text_element = true;
                    } else if name == "ctrl" {
                        // Control element (fieldBegin, fieldEnd, etc.) - skip its content
                        in_ctrl = true;
                    } else if name == "pic" {
                        // Start of picture element
                        in_pic = true;
                    } else if in_pic && name == "img" {
                        // Look for binaryItemIDRef in <hc:img> element
                        if let Some(id) = get_attr_string(&e, "binaryItemIDRef") {
                            paragraph
                                .content
                                .push(InlineContent::Image(ImageRef::new(id)));
                        }
                    } else if name == "tbl" {
                        // Table inside run - parse and collect
                        buf.clear();
                        let table = self.parse_table()?;
                        tables.push(table);
                    }
                }
                Ok(Event::Empty(e)) => {
                    let name = get_local_name(&e);

                    if name == "t" {
                        // Empty text element - skip
                    } else if in_pic && name == "img" {
                        // Look for binaryItemIDRef in <hc:img/> element
                        if let Some(id) = get_attr_string(&e, "binaryItemIDRef") {
                            paragraph
                                .content
                                .push(InlineContent::Image(ImageRef::new(id)));
                        }
                    }
                }
                Ok(Event::Text(t)) => {
                    // Only capture text inside <hp:t> elements, not inside control elements
                    if in_text_element && !in_ctrl {
                        if let Ok(s) = t.unescape() {
                            text.push_str(&s);
                        }
                    }
                }
                Ok(Event::End(e)) => {
                    let name = get_local_name_end(&e);
                    if name == "t" {
                        in_text_element = false;
                    } else if name == "ctrl" {
                        in_ctrl = false;
                    } else if name == "pic" {
                        in_pic = false;
                    } else if name == "run" {
                        break;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(crate::error::Error::XmlParse(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        Ok(TextRun::with_style(text, text_style))
    }

    /// Parses a <hp:ctrl> control element.
    ///
    /// Handles various control types:
    /// - pic: Pictures/images
    /// - eqEdit: Equations (mathematical formulas)
    /// - fn: Footnotes
    /// - en: Endnotes
    /// - tbl: Tables
    fn parse_control(&mut self, paragraph: &mut Paragraph, tables: &mut Vec<Table>) -> Result<()> {
        let mut buf = Vec::new();
        let mut in_pic = false;
        let mut in_equation = false;
        let mut in_footnote = false;
        let mut equation_script = String::new();
        let mut footnote_text = String::new();

        loop {
            match self.reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = get_local_name(&e);

                    match name.as_str() {
                        "pic" => in_pic = true,
                        "eqEdit" | "equation" => in_equation = true,
                        "fn" | "footnote" => in_footnote = true,
                        "en" | "endnote" => in_footnote = true, // Treat endnotes like footnotes
                        "tbl" => {
                            // Table inside control - parse and collect
                            buf.clear();
                            let table = self.parse_table()?;
                            tables.push(table);
                        }
                        "img" if in_pic => {
                            // Look for binaryItemIDRef attribute in <hc:img> element
                            if let Some(id) = get_attr_string(&e, "binaryItemIDRef") {
                                paragraph
                                    .content
                                    .push(InlineContent::Image(ImageRef::new(id)));
                            }
                        }
                        "script" if in_equation => {
                            // Equation script content will be in text node
                        }
                        "subList" if in_footnote => {
                            // Footnote content is in subList > p elements
                            buf.clear();
                            footnote_text = self.parse_footnote_content()?;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(e)) => {
                    let name = get_local_name(&e);

                    if in_pic && name == "img" {
                        // Look for binaryItemIDRef attribute in <hc:img/> element
                        if let Some(id) = get_attr_string(&e, "binaryItemIDRef") {
                            paragraph
                                .content
                                .push(InlineContent::Image(ImageRef::new(id)));
                        }
                    }
                }
                Ok(Event::Text(t)) if in_equation => {
                    // Collect equation script text
                    if let Ok(s) = t.unescape() {
                        equation_script.push_str(&s);
                    }
                }
                Ok(Event::End(e)) => {
                    let name = get_local_name_end(&e);
                    match name.as_str() {
                        "pic" => in_pic = false,
                        "eqEdit" | "equation" => {
                            if !equation_script.is_empty() {
                                let script = std::mem::take(&mut equation_script);
                                let eq = Equation::new(script);
                                paragraph.content.push(InlineContent::Equation(eq));
                            }
                            in_equation = false;
                        }
                        "fn" | "footnote" | "en" | "endnote" => {
                            if !footnote_text.is_empty() {
                                paragraph
                                    .content
                                    .push(InlineContent::Footnote(std::mem::take(
                                        &mut footnote_text,
                                    )));
                            }
                            in_footnote = false;
                        }
                        "ctrl" => break,
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

    /// Parses footnote content from subList element.
    fn parse_footnote_content(&mut self) -> Result<String> {
        let mut text = String::new();
        let mut buf = Vec::new();
        let mut depth = 1;

        loop {
            match self.reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = get_local_name(&e);
                    if name == "t" {
                        // Text element inside footnote paragraph
                    } else {
                        depth += 1;
                    }
                }
                Ok(Event::Text(t)) => {
                    if let Ok(s) = t.unescape() {
                        if !text.is_empty() && !text.ends_with(' ') {
                            text.push(' ');
                        }
                        text.push_str(s.trim());
                    }
                }
                Ok(Event::End(e)) => {
                    let name = get_local_name_end(&e);
                    if name == "subList" {
                        break;
                    }
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(crate::error::Error::XmlParse(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        Ok(text.trim().to_string())
    }

    /// Parses a <hp:tbl> table element.
    fn parse_table(&mut self) -> Result<Table> {
        let mut table = Table::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = get_local_name(&e);

                    if name == "tr" {
                        buf.clear();
                        let row = self.parse_table_row()?;
                        table.rows.push(row);
                    }
                }
                Ok(Event::End(e)) => {
                    let name = get_local_name_end(&e);
                    if name == "tbl" {
                        break;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(crate::error::Error::XmlParse(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        // Mark first row as header if it exists
        if !table.rows.is_empty() {
            table.has_header = true;
        }

        Ok(table)
    }

    /// Parses a <hp:tr> table row element.
    fn parse_table_row(&mut self) -> Result<TableRow> {
        let mut row = TableRow::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = get_local_name(&e);

                    if name == "tc" {
                        buf.clear();
                        let cell = self.parse_table_cell(CellAttrs)?;
                        row.cells.push(cell);
                    }
                }
                Ok(Event::End(e)) => {
                    let name = get_local_name_end(&e);
                    if name == "tr" {
                        break;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(crate::error::Error::XmlParse(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        Ok(row)
    }

    /// Parses a <hp:tc> table cell element.
    fn parse_table_cell(&mut self, _attrs: CellAttrs) -> Result<TableCell> {
        let mut rowspan: u32 = 1;
        let mut colspan: u32 = 1;
        let mut paragraphs: Vec<Paragraph> = Vec::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = get_local_name(&e);

                    // Cell content is in subList > p
                    if name == "p" {
                        let para_attrs = ParaAttrs {
                            para_pr_id: get_attr_u32(&e, "paraPrIDRef"),
                            style_id: get_attr_u32(&e, "styleIDRef"),
                        };
                        buf.clear();
                        // Pass None for section since we're inside a table cell
                        let para = self.parse_paragraph(para_attrs, None)?;
                        if !para.is_empty() {
                            paragraphs.push(para);
                        }
                    }
                }
                Ok(Event::Empty(e)) => {
                    let name = get_local_name(&e);
                    
                    // cellSpan element contains colspan/rowspan as attributes
                    if name == "cellSpan" {
                        if let Some(cs) = get_attr_u32(&e, "colSpan") {
                            colspan = cs;
                        }
                        if let Some(rs) = get_attr_u32(&e, "rowSpan") {
                            rowspan = rs;
                        }
                    }
                }
                Ok(Event::End(e)) => {
                    let name = get_local_name_end(&e);
                    if name == "tc" {
                        break;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(crate::error::Error::XmlParse(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        let mut cell = TableCell::merged(rowspan, colspan);
        cell.content = paragraphs;
        Ok(cell)
    }
}

/// Gets the local name of an element (without namespace prefix).
fn get_local_name(e: &quick_xml::events::BytesStart) -> String {
    std::str::from_utf8(e.local_name().as_ref())
        .unwrap_or("")
        .to_string()
}

fn get_local_name_end(e: &quick_xml::events::BytesEnd) -> String {
    std::str::from_utf8(e.local_name().as_ref())
        .unwrap_or("")
        .to_string()
}

/// Gets a u32 attribute value.
fn get_attr_u32(e: &quick_xml::events::BytesStart, name: &str) -> Option<u32> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name.as_bytes() {
            if let Ok(val) = std::str::from_utf8(&attr.value) {
                return val.parse().ok();
            }
        }
    }
    None
}

/// Gets a string attribute value.
fn get_attr_string(e: &quick_xml::events::BytesStart, name: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name.as_bytes() {
            if let Ok(val) = std::str::from_utf8(&attr.value) {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Skips an element and all its children.
fn skip_element(reader: &mut Reader<&[u8]>) -> Result<()> {
    let mut depth = 1;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    break;
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
