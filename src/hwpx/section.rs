//! Section parsing for HWPX documents.

use super::xml::{decode_text, resolve_general_ref};
use crate::error::Result;
use crate::model::{
    Block, Equation, ImageRef, InlineContent, NumberingState, Paragraph, ParagraphStyle, Section,
    StyleRegistry, Table, TableCell, TableRow, TextRun, MAX_TABLE_NESTING,
};
use quick_xml::events::Event;
use quick_xml::Reader;
use unparser_shared::grid::{MAX_COL_SPAN, MAX_ROW_SPAN};

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
    /// How many tables deep the parser is (0 outside any table).
    table_depth: usize,
    /// Counts the section's numbered paragraphs.
    numbering: NumberingState,
}

impl<'a> SectionParser<'a> {
    fn new(xml: &'a str, styles: &'a StyleRegistry) -> Self {
        let mut reader = Reader::from_str(xml);
        // Keep text verbatim. `<hp:t>` content (the body text) is only captured
        // inside the element's Start/End (the `in_text_element` guard in
        // `parse_run`), so inter-element indentation whitespace is naturally
        // ignored, while meaningful leading/trailing spaces inside `<hp:t>`
        // (e.g. a TOC entry "1. ") are preserved. Trimming here would drop them
        // and glue adjacent runs together (`**1.**바코드`). The two capture sites
        // that are NOT `<hp:t>`-scoped (equation script, footnote text) trim
        // explicitly below.
        reader.config_mut().trim_text(false);

        Self {
            reader,
            styles,
            table_depth: 0,
            numbering: NumberingState::new(),
        }
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
                            let (para, tables) = self.parse_paragraph(attrs)?;
                            // Don't filter empty paragraphs here — let the renderer
                            // decide based on include_empty_paragraphs option.
                            // This matches HWP5 behavior for consistency.
                            section.content.push(Block::Paragraph(para));
                            // A table anchored in the paragraph follows it.
                            section.content.extend(tables.into_iter().map(Block::Table));
                        }
                        "tbl" => {
                            buf.clear();
                            if let Some(table) = self.parse_table()? {
                                section.content.push(Block::Table(table));
                            }
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

    /// Parses a <hp:p> paragraph element, returning it with the tables anchored in it.
    fn parse_paragraph(&mut self, attrs: ParaAttrs) -> Result<(Paragraph, Vec<Table>)> {
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

        // Counted when the paragraph starts, so a table anchored in it is counted after it.
        let label = para_style
            .head
            .as_ref()
            .and_then(|head| self.numbering.label(head, self.styles));
        let mut paragraph = Paragraph::with_style(para_style);
        let mut buf = Vec::new();

        // Tables found in runs and controls; the caller places them
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
                            let run =
                                self.parse_run(run_attrs, &mut paragraph, &mut tables_found)?;
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

        // The number is not in the paragraph's text; it is written in front of text that has
        // some (an empty numbered paragraph is counted but says nothing).
        if let Some(label) = label {
            let has_text = paragraph.content.iter().any(
                |item| matches!(item, InlineContent::Text(run) if !run.text.trim().is_empty()),
            );
            if has_text {
                paragraph
                    .content
                    .insert(0, InlineContent::Text(TextRun::new(format!("{label} "))));
            }
        }

        Ok((paragraph, tables_found))
    }

    /// Parses a <hp:run> text run element, returning the text run and any images found.
    fn parse_run(
        &mut self,
        attrs: RunAttrs,
        paragraph: &mut Paragraph,
        tables: &mut Vec<Table>,
    ) -> Result<TextRun> {
        let text_style = attrs
            .char_pr_id
            .and_then(|id| self.styles.get_char_style(id))
            .cloned()
            .unwrap_or_default();

        let mut text = String::new();
        let mut buf = Vec::new();
        let mut in_pic = false;
        let mut pic_floating = false; // Whether the current <hp:pic> floats over text
        let mut in_ctrl = false; // Track if we're inside a control element (to skip formula fields etc.)
        let mut in_text_element = false; // Track if we're inside <hp:t> element

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
                        pic_floating = is_floating_pic(&e);
                    } else if in_pic && name == "img" {
                        // Look for binaryItemIDRef in <hc:img> element
                        if let Some(id) = get_attr_string(&e, "binaryItemIDRef") {
                            paragraph.content.push(InlineContent::Image(
                                ImageRef::new(id).floating(pic_floating),
                            ));
                        }
                    } else if name == "secPr" {
                        self.note_section_properties(&e);
                    } else if name == "tbl" {
                        // Table inside run - parse and collect
                        buf.clear();
                        tables.extend(self.parse_table()?);
                    }
                }
                Ok(Event::Empty(e)) => {
                    let name = get_local_name(&e);

                    if name == "t" {
                        // Empty text element - skip
                    } else if name == "secPr" {
                        self.note_section_properties(&e);
                    } else if name == "tab" && !in_ctrl {
                        // <hp:tab/> is a layout tab (often a TOC leader between a title
                        // and its page number). Markdown has no tab concept, so emit a
                        // single space to preserve the word boundary instead of letting
                        // the surrounding text collapse together (e.g. `재고관리1`).
                        text.push(' ');
                    } else if in_pic && name == "img" {
                        // Look for binaryItemIDRef in <hc:img/> element
                        if let Some(id) = get_attr_string(&e, "binaryItemIDRef") {
                            paragraph.content.push(InlineContent::Image(
                                ImageRef::new(id).floating(pic_floating),
                            ));
                        }
                    }
                }
                Ok(Event::Text(t)) if in_text_element && !in_ctrl => {
                    // Only capture text inside <hp:t> elements, not inside control elements
                    text.push_str(&decode_text(&t));
                }
                // Entity references (`&amp;`, `&#NN;`) are separate events in
                // quick-xml 0.40+; resolve them into the same text buffer.
                Ok(Event::GeneralRef(r)) if in_text_element && !in_ctrl => {
                    text.push_str(&resolve_general_ref(&r));
                }
                Ok(Event::End(e)) => {
                    let name = get_local_name_end(&e);
                    if name == "t" {
                        in_text_element = false;
                    } else if name == "ctrl" {
                        in_ctrl = false;
                    } else if name == "pic" {
                        in_pic = false;
                        pic_floating = false;
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

    /// Reads the section properties (`hp:secPr`): the numbering its outline paragraphs are
    /// counted by.
    fn note_section_properties(&mut self, e: &quick_xml::events::BytesStart) {
        if let Some(id) = get_attr_u32(e, "outlineShapeIDRef") {
            self.numbering.set_outline_id(id);
        }
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
        let mut pic_floating = false;
        let mut in_equation = false;
        let mut in_footnote = false;
        let mut equation_script = String::new();
        let mut footnote_text = String::new();

        loop {
            match self.reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = get_local_name(&e);

                    match name.as_str() {
                        "pic" => {
                            in_pic = true;
                            pic_floating = is_floating_pic(&e);
                        }
                        "eqEdit" | "equation" => in_equation = true,
                        "fn" | "footnote" => in_footnote = true,
                        "en" | "endnote" => in_footnote = true, // Treat endnotes like footnotes
                        "tbl" => {
                            // Table inside control - parse and collect
                            buf.clear();
                            tables.extend(self.parse_table()?);
                        }
                        "img" if in_pic => {
                            // Look for binaryItemIDRef attribute in <hc:img> element
                            if let Some(id) = get_attr_string(&e, "binaryItemIDRef") {
                                paragraph.content.push(InlineContent::Image(
                                    ImageRef::new(id).floating(pic_floating),
                                ));
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
                            paragraph.content.push(InlineContent::Image(
                                ImageRef::new(id).floating(pic_floating),
                            ));
                        }
                    }
                }
                Ok(Event::Text(t)) if in_equation => {
                    // Collect equation script text
                    equation_script.push_str(&decode_text(&t));
                }
                Ok(Event::GeneralRef(r)) if in_equation => {
                    equation_script.push_str(&resolve_general_ref(&r));
                }
                Ok(Event::End(e)) => {
                    let name = get_local_name_end(&e);
                    match name.as_str() {
                        "pic" => {
                            in_pic = false;
                            pic_floating = false;
                        }
                        "eqEdit" | "equation" => {
                            // Equation text is captured for the whole `in_equation`
                            // span, so with trim_text(false) it can pick up
                            // inter-element indentation; trim the assembled script.
                            let script = std::mem::take(&mut equation_script);
                            let script = script.trim();
                            if !script.is_empty() {
                                let eq = Equation::new(script.to_string());
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
                    let s = decode_text(&t);
                    // This site is not `<hp:t>`-scoped, so with trim_text(false)
                    // it also receives inter-element indentation. Skip
                    // whitespace-only events; join real fragments with a single
                    // space (footnote paragraphs flatten to one line).
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        if !text.is_empty() && !text.ends_with(' ') {
                            text.push(' ');
                        }
                        text.push_str(trimmed);
                    }
                }
                // An entity reference is real content, not inter-element
                // whitespace, so append it directly (no trim/space-join).
                Ok(Event::GeneralRef(r)) => {
                    text.push_str(&resolve_general_ref(&r));
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
    fn parse_table(&mut self) -> Result<Option<Table>> {
        // A table nested past the bound is skipped, not read: the recursion over a hostile
        // document's element nesting stays finite.
        if self.table_depth >= MAX_TABLE_NESTING {
            skip_element(&mut self.reader)?;
            return Ok(None);
        }
        self.table_depth += 1;
        let table = self.parse_table_inner();
        self.table_depth -= 1;
        table.map(Some)
    }

    fn parse_table_inner(&mut self) -> Result<Table> {
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
        let mut nested_tables: Vec<Table> = Vec::new();
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
                        let (para, tables) = self.parse_paragraph(para_attrs)?;
                        // Don't filter empty paragraphs here — renderer handles it.
                        // This matches HWP5 cell parsing behavior.
                        paragraphs.push(para);
                        // A table anchored in a cell's paragraph is nested in the cell.
                        nested_tables.extend(tables);
                    }
                }
                Ok(Event::Empty(e)) => {
                    let name = get_local_name(&e);

                    // cellSpan element contains colspan/rowspan as attributes
                    if name == "cellSpan" {
                        // A declared span is read the way HTML reads one: past the bound,
                        // as the bound (`unparser_shared::grid`).
                        if let Some(cs) = get_attr_u32(&e, "colSpan") {
                            colspan = cs.min(MAX_COL_SPAN);
                        }
                        if let Some(rs) = get_attr_u32(&e, "rowSpan") {
                            rowspan = rs.min(MAX_ROW_SPAN);
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
        cell.nested_tables = nested_tables;
        Ok(cell)
    }
}

/// Gets the local name of an element (without namespace prefix).
fn get_local_name(e: &quick_xml::events::BytesStart) -> String {
    e.local_name().as_ref().to_string()
}

fn get_local_name_end(e: &quick_xml::events::BytesEnd) -> String {
    e.local_name().as_ref().to_string()
}

/// Gets a u32 attribute value.
fn get_attr_u32(e: &quick_xml::events::BytesStart, name: &str) -> Option<u32> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name {
            return attr.value.parse().ok();
        }
    }
    None
}

/// Gets a string attribute value.
fn get_attr_string(e: &quick_xml::events::BytesStart, name: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name {
            return Some(attr.value.to_string());
        }
    }
    None
}

/// Returns true if a `<hp:pic>` `textWrap` value denotes a floating object that
/// is layered over/under the text rather than embedded in the text flow.
///
/// `IN_FRONT_OF_TEXT`/`BEHIND_TEXT` are stamps, signatures, and watermarks that
/// must not glue to adjacent text. Other wraps (e.g. `SQUARE`, default) keep the
/// existing inline behaviour to avoid regressing embedded illustrations.
fn is_floating_pic(e: &quick_xml::events::BytesStart) -> bool {
    matches!(
        get_attr_string(e, "textWrap").as_deref(),
        Some("IN_FRONT_OF_TEXT") | Some("BEHIND_TEXT")
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::InlineContent;

    /// Collects the text of every `InlineContent::Text` run in the first
    /// paragraph of a parsed section, preserving order.
    fn run_texts(xml: &str) -> Vec<String> {
        let styles = StyleRegistry::new();
        let section = parse_section(xml, 0, &styles).expect("section must parse");
        let para = section
            .content
            .iter()
            .find_map(|b| match b {
                Block::Paragraph(p) => Some(p),
                _ => None,
            })
            .expect("a paragraph");
        para.content
            .iter()
            .filter_map(|c| match c {
                InlineContent::Text(run) => Some(run.text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_declared_cell_span_past_the_bound_is_read_as_the_bound() {
        let xml = r#"<hs:sec xmlns:hp="x"><hp:p><hp:run><hp:tbl><hp:tr><hp:tc><hp:cellSpan colSpan="4294967295" rowSpan="4294967295"/><hp:subList><hp:p><hp:run><hp:t>a</hp:t></hp:run></hp:p></hp:subList></hp:tc></hp:tr></hp:tbl></hp:run></hp:p></hs:sec>"#;
        let section = parse_section(xml, 0, &StyleRegistry::new()).expect("section must parse");
        let cell = section
            .content
            .iter()
            .find_map(|b| match b {
                Block::Table(t) => t.rows.first().and_then(|r| r.cells.first()),
                _ => None,
            })
            .expect("a table cell");
        assert_eq!((cell.colspan, cell.rowspan), (MAX_COL_SPAN, MAX_ROW_SPAN));
    }

    /// A cell `text` with, after it, a paragraph anchoring `inner` (a `<hp:tbl>` as XML).
    fn cell_xml(text: &str, inner: &str) -> String {
        format!(
            "<hp:tc><hp:subList><hp:p><hp:run><hp:t>{text}</hp:t></hp:run></hp:p>\
             <hp:p><hp:run>{inner}</hp:run></hp:p></hp:subList></hp:tc>"
        )
    }

    fn table_of(section: &Section) -> &Table {
        section
            .content
            .iter()
            .find_map(|b| match b {
                Block::Table(t) => Some(t),
                _ => None,
            })
            .expect("a table")
    }

    #[test]
    fn a_table_nested_in_a_cell_is_kept_as_a_table_of_the_cell() {
        let inner = "<hp:tbl><hp:tr><hp:tc><hp:subList><hp:p><hp:run><hp:t>inner</hp:t></hp:run></hp:p></hp:subList></hp:tc></hp:tr></hp:tbl>";
        let xml = format!(
            "<hs:sec xmlns:hp=\"x\"><hp:p><hp:run><hp:tbl><hp:tr>{}<hp:tc><hp:subList><hp:p><hp:run><hp:t>beside</hp:t></hp:run></hp:p></hp:subList></hp:tc></hp:tr></hp:tbl></hp:run></hp:p></hs:sec>",
            cell_xml("outer", inner)
        );
        let section = parse_section(&xml, 0, &StyleRegistry::new()).expect("section must parse");
        let table = table_of(&section);
        // Only the outer table is a block of the section: the nested one belongs to its cell.
        let tables = section
            .content
            .iter()
            .filter(|b| matches!(b, Block::Table(_)))
            .count();
        assert_eq!(tables, 1);
        assert_eq!(table.rows[0].cells.len(), 2);
        assert_eq!(table.rows[0].cells[0].plain_text().trim(), "outer");
        let nested = &table.rows[0].cells[0].nested_tables;
        assert_eq!(nested.len(), 1, "the nested table must not be dropped");
        assert_eq!(nested[0].rows[0].cells[0].plain_text(), "inner");
    }

    #[test]
    fn tables_nested_past_the_bound_are_skipped_not_overflowed() {
        // MAX_TABLE_NESTING + 3 tables, each in the cell of the one above.
        let levels = MAX_TABLE_NESTING + 3;
        let mut xml = String::from("<hp:tbl><hp:tr><hp:tc><hp:subList><hp:p><hp:run><hp:t>leaf</hp:t></hp:run></hp:p></hp:subList></hp:tc></hp:tr></hp:tbl>");
        for n in 0..levels {
            xml = format!(
                "<hp:tbl><hp:tr>{}</hp:tr></hp:tbl>",
                cell_xml(&format!("level {n}"), &xml)
            );
        }
        let xml = format!("<hs:sec xmlns:hp=\"x\"><hp:p><hp:run>{xml}</hp:run></hp:p></hs:sec>");
        let section = parse_section(&xml, 0, &StyleRegistry::new()).expect("section must parse");
        let mut depth = 0;
        let mut table = table_of(&section);
        while let Some(next) = table.rows[0].cells[0].nested_tables.first() {
            table = next;
            depth += 1;
        }
        // The outermost table is at depth 0; the bound is how many tables deep the parser goes.
        assert_eq!(depth + 1, MAX_TABLE_NESTING);
    }

    // === Paragraph numbers ===

    /// A header holding one numbering (`1.` `가.`), one bullet, and paragraph shapes 0 to 4:
    /// plain, first and second level of the numbering, the bullet, and an outline.
    const HEADER: &str = r#"<hh:head xmlns:hh="x"><hh:refList>
        <hh:numberings itemCnt="1"><hh:numbering id="1" start="0">
          <hh:paraHead start="1" level="1" align="LEFT" numFormat="DIGIT" charPrIDRef="4294967295" checkable="0">^1.</hh:paraHead>
          <hh:paraHead start="1" level="2" align="LEFT" numFormat="HANGUL_SYLLABLE" charPrIDRef="4294967295" checkable="0">^2.</hh:paraHead>
        </hh:numbering></hh:numberings>
        <hh:bullets itemCnt="1"><hh:bullet id="1" char="●" useImage="0">
          <hh:paraHead level="0" align="LEFT" charPrIDRef="4294967295"/>
        </hh:bullet></hh:bullets>
        <hh:paraProperties itemCnt="5">
          <hh:paraPr id="0"><hh:heading type="NONE" idRef="0" level="0"/></hh:paraPr>
          <hh:paraPr id="1"><hh:heading type="NUMBER" idRef="1" level="0"/></hh:paraPr>
          <hh:paraPr id="2"><hh:heading type="NUMBER" idRef="1" level="1"/></hh:paraPr>
          <hh:paraPr id="3"><hh:heading type="BULLET" idRef="1" level="0"/></hh:paraPr>
          <hh:paraPr id="4"><hh:heading type="OUTLINE" idRef="0" level="0"/></hh:paraPr>
        </hh:paraProperties></hh:refList></hh:head>"#;

    fn styles_of(header: &str) -> StyleRegistry {
        let mut registry = StyleRegistry::new();
        crate::hwpx::styles::parse_styles(header, &mut registry).expect("header parses");
        registry
    }

    fn paragraph_xml(shape: u32, text: &str) -> String {
        format!("<hp:p paraPrIDRef=\"{shape}\"><hp:run><hp:t>{text}</hp:t></hp:run></hp:p>")
    }

    fn section_texts(xml: &str, styles: &StyleRegistry) -> Vec<String> {
        let section = parse_section(xml, 0, styles).expect("section parses");
        section
            .content
            .iter()
            .filter_map(|b| match b {
                Block::Paragraph(p) => Some(p.plain_text()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_header_gives_the_numbering_and_the_bullet_and_the_heads() {
        let styles = styles_of(HEADER);
        let numbering = styles.numbering(1).expect("numbering 1");
        assert_eq!(numbering.levels.len(), 2);
        assert_eq!(numbering.levels[0].format, "^1.");
        assert_eq!(
            numbering.levels[1].shape,
            crate::model::NumberShape::HangulSyllable
        );
        assert_eq!(styles.bullet(1), Some('●'));
        let head = |id| styles.get_para_style(id).unwrap().head;
        assert_eq!(head(0), None);
        assert_eq!(head(2).map(|h| (h.level, h.id)), Some((1, 1)));
        // A numbered paragraph is not a heading; an outline of the first level is not either.
        assert_eq!(styles.get_para_style(2).unwrap().heading_level, 0);
    }

    #[test]
    fn numbered_paragraphs_are_written_with_their_numbers() {
        let styles = styles_of(HEADER);
        let body: String = [
            (1, "first"),
            (2, "under"),
            (1, "second"),
            (0, "plain"),
            (3, "bullet"),
        ]
        .iter()
        .map(|(shape, text)| paragraph_xml(*shape, text))
        .collect();
        let xml = format!("<hs:sec xmlns:hp=\"x\">{body}</hs:sec>");
        assert_eq!(
            section_texts(&xml, &styles),
            ["1. first", "가. under", "2. second", "plain", "● bullet"]
        );
    }

    #[test]
    fn outline_paragraphs_are_counted_by_the_sections_outline_numbering() {
        let styles = styles_of(HEADER);
        let xml = format!(
            "<hs:sec xmlns:hp=\"x\"><hp:p paraPrIDRef=\"0\"><hp:run><hp:secPr outlineShapeIDRef=\"1\"/><hp:t>title</hp:t></hp:run></hp:p>{}{}</hs:sec>",
            paragraph_xml(4, "chapter"),
            paragraph_xml(4, "next")
        );
        assert_eq!(
            section_texts(&xml, &styles),
            ["title", "1. chapter", "2. next"]
        );
    }

    #[test]
    fn the_count_runs_on_through_a_table_cell() {
        let styles = styles_of(HEADER);
        let cell = format!(
            "<hp:tbl><hp:tr><hp:tc><hp:subList>{}</hp:subList></hp:tc></hp:tr></hp:tbl>",
            paragraph_xml(1, "in the cell")
        );
        let xml = format!(
            "<hs:sec xmlns:hp=\"x\">{}<hp:p><hp:run>{cell}</hp:run></hp:p>{}</hs:sec>",
            paragraph_xml(1, "body"),
            paragraph_xml(1, "after")
        );
        let section = parse_section(&xml, 0, &styles).expect("section parses");
        let mut seen: Vec<String> = Vec::new();
        for block in &section.content {
            match block {
                Block::Paragraph(p) => seen.push(p.plain_text()),
                Block::Table(t) => seen.push(t.rows[0].cells[0].plain_text()),
            }
        }
        seen.retain(|t| !t.is_empty());
        assert_eq!(seen, ["1. body", "2. in the cell", "3. after"]);
    }

    #[test]
    fn test_entity_references_resolved_in_hp_t() {
        // quick-xml 0.40+ splits `&amp;` / `&lt;` / `&#NN;` out of the text node
        // into separate `GeneralRef` events. The `<hp:t>` loop must fold them
        // back in, or extracted text silently loses every entity. A single run
        // interleaves literal text with predefined, decimal, and hex refs.
        let xml = r#"<hs:sec xmlns:hp="x"><hp:p><hp:run><hp:t>R&amp;D &lt;a&gt; &#48;&#x31;</hp:t></hp:run></hp:p></hs:sec>"#;
        assert_eq!(run_texts(xml), vec!["R&D <a> 01".to_string()]);
    }

    #[test]
    fn test_hp_t_trailing_space_preserved() {
        // `<hp:t>1. </hp:t>` followed by a plain run must keep the separating
        // space so the renderer emits `**1.** 바코드`, not `**1.**바코드`.
        // Regression for D8: trim_text(true) used to strip it.
        let xml = r#"<hs:sec xmlns:hp="x"><hp:p><hp:run><hp:t>1. </hp:t></hp:run><hp:run><hp:t>바코드</hp:t></hp:run></hp:p></hs:sec>"#;
        assert_eq!(
            run_texts(xml),
            vec!["1. ".to_string(), "바코드".to_string()]
        );
    }

    #[test]
    fn test_hp_t_leading_space_preserved() {
        // Leading whitespace inside <hp:t> is meaningful content and must
        // survive parsing.
        let xml = r#"<hs:sec xmlns:hp="x"><hp:p><hp:run><hp:t>가</hp:t></hp:run><hp:run><hp:t> 나</hp:t></hp:run></hp:p></hs:sec>"#;
        assert_eq!(run_texts(xml), vec!["가".to_string(), " 나".to_string()]);
    }

    #[test]
    fn test_interelement_indentation_not_captured() {
        // Pretty-printed XML with newline+indent whitespace between elements
        // must NOT leak into run text — only `<hp:t>` content is captured.
        let xml = "<hs:sec xmlns:hp=\"x\">\n  <hp:p>\n    <hp:run>\n      <hp:t>foo</hp:t>\n    </hp:run>\n  </hp:p>\n</hs:sec>";
        assert_eq!(run_texts(xml), vec!["foo".to_string()]);
    }

    /// Collects equation scripts from the first paragraph of a parsed section.
    fn equation_scripts(xml: &str) -> Vec<String> {
        let styles = StyleRegistry::new();
        let section = parse_section(xml, 0, &styles).expect("section must parse");
        let para = section
            .content
            .iter()
            .find_map(|b| match b {
                Block::Paragraph(p) => Some(p),
                _ => None,
            })
            .expect("a paragraph");
        para.content
            .iter()
            .filter_map(|c| match c {
                InlineContent::Equation(e) => Some(e.script.clone()),
                _ => None,
            })
            .collect()
    }

    /// Collects footnote texts from the first paragraph of a parsed section.
    fn footnote_texts(xml: &str) -> Vec<String> {
        let styles = StyleRegistry::new();
        let section = parse_section(xml, 0, &styles).expect("section must parse");
        let para = section
            .content
            .iter()
            .find_map(|b| match b {
                Block::Paragraph(p) => Some(p),
                _ => None,
            })
            .expect("a paragraph");
        para.content
            .iter()
            .filter_map(|c| match c {
                InlineContent::Footnote(t) => Some(t.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn test_equation_script_trimmed_under_no_trim_reader() {
        // With trim_text(false) the equation capture spans the whole eqEdit and
        // would otherwise swallow inter-element indentation ("\n  x + y\n").
        // The explicit trim must strip the surrounding whitespace while keeping
        // internal spacing.
        let xml = "<hs:sec xmlns:hp=\"x\"><hp:p><hp:ctrl><hp:equation>\n      <hp:script>x + y</hp:script>\n    </hp:equation></hp:ctrl></hp:p></hs:sec>";
        assert_eq!(equation_scripts(xml), vec!["x + y".to_string()]);
    }

    #[test]
    fn test_footnote_text_clean_under_no_trim_reader() {
        // Pretty-printed footnote subList: with trim_text(false) every newline+
        // indent is now a Text event. The whitespace-only-skip guard must drop
        // them, and real fragments across runs join with a single space.
        let xml = "<hs:sec xmlns:hp=\"x\"><hp:p><hp:ctrl><hp:fn>\n  <hp:subList>\n    <hp:p>\n      <hp:run><hp:t>각주</hp:t></hp:run>\n      <hp:run><hp:t>내용</hp:t></hp:run>\n    </hp:p>\n  </hp:subList>\n</hp:fn></hp:ctrl></hp:p></hs:sec>";
        assert_eq!(footnote_texts(xml), vec!["각주 내용".to_string()]);
    }

    #[test]
    fn test_whitespace_only_run_preserved_between_words() {
        // A run whose only content is a space (`<hp:t> </hp:t>`) is a real
        // inter-word space; it must not be dropped.
        let xml = r#"<hs:sec xmlns:hp="x"><hp:p><hp:run><hp:t>가</hp:t></hp:run><hp:run><hp:t> </hp:t></hp:run><hp:run><hp:t>나</hp:t></hp:run></hp:p></hs:sec>"#;
        assert_eq!(
            run_texts(xml),
            vec!["가".to_string(), " ".to_string(), "나".to_string()]
        );
    }
}
