//! Markdown renderer implementation.

use super::heading_analyzer::{HeadingAnalyzer, HeadingDecision};
use super::RenderOptions;
use crate::error::Result;
use crate::model::{
    Alignment, Block, Document, InlineContent, ListStyle, Paragraph, Table, TableCell, TextRun,
};

use std::collections::HashMap;

/// Maximum character length for a heading.
/// Text longer than this is unlikely to be a semantic heading.
const MAX_HEADING_TEXT_LENGTH: usize = 80;

/// Common list/bullet markers including Korean characters.
/// Used to detect paragraphs that should not be rendered as headings.
const LIST_MARKERS: &[char] = &[
    // ASCII markers
    '-', '*', '>', // Korean/Asian bullet markers (for list items)
    '•', '◦', '▪', '▫', '☐', '☑', '☒', '✓', '✗', 'ㅇ', // Korean jamo (circle)
    'ㆍ', // Korean middle dot (U+318D)
    '·',  // Middle dot (U+00B7)
    '∙',  // Bullet operator (U+2219)
    // Arrows (commonly used as list markers in Korean documents)
    '→', '←', '↔', '⇒', '⇐', '⇔', '►', '▶', '▷', '◀', '◁', '▻',
];

// NOTE: Symbol-based section marker detection has been removed.
// Heading detection now uses font-size-based statistical inference via HeadingAnalyzer.
// Symbols like ※, ◎, etc. are NOT reliable heading indicators without font context.

/// Markdown renderer.
#[derive(Debug)]
pub struct MarkdownRenderer {
    options: RenderOptions,
    /// Maps binaryItemIDRef (e.g., "image1") to actual filename (e.g., "image1.bmp")
    image_id_to_filename: HashMap<String, String>,
}

impl MarkdownRenderer {
    /// Creates a new renderer with the given options.
    pub fn new(options: RenderOptions) -> Self {
        Self {
            options,
            image_id_to_filename: HashMap::new(),
        }
    }

    /// Renders a document to Markdown string.
    pub fn render(&self, document: &Document) -> Result<String> {
        // Build image ID to filename mapping from resources
        let renderer = Self {
            options: self.options.clone(),
            image_id_to_filename: Self::build_image_mapping(document),
        };

        // If heading analysis is enabled, use the analyzer
        if let Some(ref config) = renderer.options.heading_config {
            return renderer.render_with_analyzer(document, config);
        }

        // Standard rendering without sophisticated heading analysis
        renderer.render_standard(document)
    }

    /// Standard rendering (legacy behavior without heading analyzer).
    fn render_standard(&self, document: &Document) -> Result<String> {
        let mut output = String::new();

        // Render frontmatter if enabled
        if self.options.include_frontmatter {
            self.render_frontmatter(document, &mut output);
        }

        // Render each section
        for section in &document.sections {
            for block in &section.content {
                match block {
                    Block::Paragraph(para) => {
                        self.render_paragraph(para, &mut output, None);
                    }
                    Block::Table(table) => {
                        self.render_table(table, &mut output);
                    }
                }
            }
        }

        // Apply cleanup pipeline if enabled
        if let Some(ref cleanup_options) = self.options.cleanup {
            output = crate::cleanup::cleanup(&output, cleanup_options);
        }

        Ok(output)
    }

    /// Rendering with sophisticated heading analysis.
    fn render_with_analyzer(
        &self,
        document: &Document,
        config: &super::heading_analyzer::HeadingConfig,
    ) -> Result<String> {
        // Run heading analysis
        let mut analyzer = HeadingAnalyzer::new(config.clone());
        let decisions = analyzer.analyze(document);

        let mut output = String::new();

        // Render frontmatter if enabled
        if self.options.include_frontmatter {
            self.render_frontmatter(document, &mut output);
        }

        // Track paragraph index across all sections
        let mut para_idx = 0;

        // Render each section with pre-computed heading decisions
        for section in &document.sections {
            for block in &section.content {
                match block {
                    Block::Paragraph(para) => {
                        let decision = decisions.get(para_idx).copied();
                        self.render_paragraph(para, &mut output, decision);
                        para_idx += 1;
                    }
                    Block::Table(table) => {
                        self.render_table(table, &mut output);
                    }
                }
            }
        }

        // Apply cleanup pipeline if enabled
        if let Some(ref cleanup_options) = self.options.cleanup {
            output = crate::cleanup::cleanup(&output, cleanup_options);
        }

        Ok(output)
    }

    /// Builds a mapping from binaryItemIDRef to actual filename.
    fn build_image_mapping(document: &Document) -> HashMap<String, String> {
        let mut mapping = HashMap::new();

        for filename in document.resources.keys() {
            // Extract base name without extension (e.g., "image1.bmp" -> "image1")
            if let Some(dot_pos) = filename.rfind('.') {
                let base_name = &filename[..dot_pos];
                mapping.insert(base_name.to_string(), filename.clone());
            } else {
                // No extension - use as-is
                mapping.insert(filename.clone(), filename.clone());
            }
        }

        mapping
    }

    /// Renders YAML frontmatter.
    fn render_frontmatter(&self, document: &Document, output: &mut String) {
        output.push_str("---\n");

        if let Some(ref title) = document.metadata.title {
            output.push_str(&format!("title: \"{}\"\n", escape_yaml(title)));
        }
        if let Some(ref author) = document.metadata.author {
            output.push_str(&format!("author: \"{}\"\n", escape_yaml(author)));
        }
        if let Some(ref subject) = document.metadata.subject {
            output.push_str(&format!("description: \"{}\"\n", escape_yaml(subject)));
        }
        if let Some(ref created) = document.metadata.created {
            output.push_str(&format!("date: \"{}\"\n", created));
        }
        if let Some(ref modified) = document.metadata.modified {
            output.push_str(&format!("lastmod: \"{}\"\n", modified));
        }
        if !document.metadata.keywords.is_empty() {
            output.push_str("tags:\n");
            for keyword in &document.metadata.keywords {
                output.push_str(&format!("  - \"{}\"\n", escape_yaml(keyword)));
            }
        }
        if let Some(ref app) = document.metadata.creator_app {
            output.push_str(&format!("generator: \"{}\"\n", escape_yaml(app)));
        }
        if let Some(ref format) = document.metadata.format_version {
            output.push_str(&format!("format: \"{}\"\n", escape_yaml(format)));
        }

        output.push_str("---\n\n");
    }

    /// Renders a paragraph.
    ///
    /// If `heading_decision` is provided (from HeadingAnalyzer), it takes precedence
    /// over the legacy inline heading detection logic.
    fn render_paragraph(
        &self,
        para: &Paragraph,
        output: &mut String,
        heading_decision: Option<HeadingDecision>,
    ) {
        if para.is_empty() && !self.options.include_empty_paragraphs {
            return;
        }

        let style = &para.style;
        let plain_text = para.plain_text();
        let trimmed_text = plain_text.trim();
        let first_char = trimmed_text.chars().next();

        // Determine heading behavior based on whether we have a pre-computed decision
        let (should_apply_heading, heading_level) = if let Some(decision) = heading_decision {
            // Use pre-computed decision from HeadingAnalyzer
            match decision {
                HeadingDecision::Explicit(level) | HeadingDecision::Inferred(level) => {
                    // Check additional conditions that analyzer doesn't know about
                    let valid = para.has_text_content()
                        && !para.is_image_only()
                        && style.list_style.is_none();
                    (valid, level)
                }
                HeadingDecision::Demoted | HeadingDecision::None => (false, 0),
            }
        } else {
            // Legacy inline heading detection
            self.compute_heading_inline(para, trimmed_text, first_char)
        };

        // Skip empty headings entirely - don't output anything
        if style.heading_level > 0 && !para.has_text_content() && !para.is_image_only() {
            return;
        }

        // Heading prefix (only for valid headings)
        if should_apply_heading {
            output.push_str(&"#".repeat(heading_level as usize));
            output.push(' ');
        }

        // List prefix
        let is_list_item = style.list_style.is_some();
        if let Some(ref list_style) = style.list_style {
            let indent = "  ".repeat(style.indent_level as usize);
            output.push_str(&indent);

            match list_style {
                ListStyle::Ordered => output.push_str("1. "),
                ListStyle::Unordered => {
                    output.push(self.options.list_marker);
                    output.push(' ');
                }
                ListStyle::CustomBullet(c) => {
                    output.push(*c);
                    output.push(' ');
                }
            }
        }

        // Render inline content
        for item in &para.content {
            self.render_inline(item, output);
        }

        // End paragraph
        output.push('\n');

        // Add blank line between paragraphs for proper Markdown separation
        // But NOT after list items (consecutive list items should be adjacent)
        // and NOT after headings (blank line added before next content is enough)
        let is_heading = should_apply_heading || style.heading_level > 0;
        if self.options.paragraph_spacing && !is_heading && !is_list_item {
            output.push('\n');
        }
    }

    /// Legacy inline heading detection (used when heading_config is None).
    ///
    /// NOTE: This is a fallback path. The recommended approach is to use
    /// `heading_config` which enables statistical font-size-based heading detection.
    fn compute_heading_inline(
        &self,
        para: &Paragraph,
        trimmed_text: &str,
        first_char: Option<char>,
    ) -> (bool, u8) {
        let style = &para.style;

        // Check if paragraph content looks like a list item (starts with list-like markers)
        let looks_like_list_item = first_char
            .map(|c| LIST_MARKERS.contains(&c))
            .unwrap_or(false);

        // Check if text is too long to be a meaningful heading
        let text_length = trimmed_text.chars().count();
        let text_too_long = text_length > MAX_HEADING_TEXT_LENGTH;

        // Only use explicit heading_level from document styles
        // Do NOT auto-detect headings based on symbols (※, ◎, etc.)
        // Symbol-based detection is unreliable without font size context
        let should_apply_heading = style.heading_level > 0
            && para.has_text_content()
            && !para.is_image_only()
            && style.list_style.is_none()
            && !looks_like_list_item
            && !text_too_long;

        // Calculate heading level
        let level = if should_apply_heading {
            style.heading_level.min(self.options.max_heading_level)
        } else {
            0
        };

        (should_apply_heading, level)
    }

    /// Renders inline content.
    fn render_inline(&self, item: &InlineContent, output: &mut String) {
        match item {
            InlineContent::Text(run) => {
                self.render_text_run(run, output);
            }
            InlineContent::LineBreak => {
                if self.options.preserve_line_breaks {
                    output.push_str("  \n"); // Two spaces + newline for Markdown line break
                } else {
                    output.push(' ');
                }
            }
            InlineContent::Image(img) => {
                let alt = img.alt_text.as_deref().unwrap_or("image");
                // Look up the actual filename from binaryItemIDRef
                let filename = self
                    .image_id_to_filename
                    .get(&img.id)
                    .cloned()
                    .unwrap_or_else(|| img.id.clone());
                let path = format!("{}{}", self.options.image_path_prefix, filename);
                output.push_str(&format!("![{}]({})", alt, path));
            }
            InlineContent::Equation(eq) => {
                // Prefer LaTeX if available, otherwise convert from HWP script
                if let Some(ref latex) = eq.latex {
                    output.push_str(&format!("${}$", latex));
                } else if !eq.script.is_empty() {
                    // Convert HWP equation script to LaTeX
                    let latex = crate::equation::to_latex(&eq.script);
                    if latex.is_empty() {
                        output.push_str(&format!("`{}`", eq.script));
                    } else {
                        output.push_str(&format!("${}$", latex));
                    }
                }
            }
            InlineContent::Footnote(text) => {
                output.push_str(&format!("[^{}]", text));
            }
            InlineContent::Link { text, url } => {
                output.push_str(&format!("[{}]({})", text, url));
            }
        }
    }

    /// Renders a text run with formatting.
    fn render_text_run(&self, run: &TextRun, output: &mut String) {
        let style = &run.style;
        let text = if self.options.escape_special_chars {
            escape_markdown(&run.text)
        } else {
            run.text.clone()
        };

        // Apply formatting markers
        let mut prefix = String::new();
        let mut suffix = String::new();

        if style.bold {
            prefix.push_str("**");
            suffix.insert_str(0, "**");
        }
        if style.italic {
            prefix.push('*');
            suffix.insert(0, '*');
        }
        // Note: Markdown doesn't support underline natively.
        // We skip underline formatting as <u> tags are not standard markdown.
        if style.strikethrough {
            prefix.push_str("~~");
            suffix.insert_str(0, "~~");
        }
        if style.superscript {
            prefix.push_str("<sup>");
            suffix.insert_str(0, "</sup>");
        }
        if style.subscript {
            prefix.push_str("<sub>");
            suffix.insert_str(0, "</sub>");
        }

        output.push_str(&prefix);
        output.push_str(&text);
        output.push_str(&suffix);
    }

    /// Renders a table.
    fn render_table(&self, table: &Table, output: &mut String) {
        if table.rows.is_empty() {
            return;
        }

        // Single-row tables are typically used as decorative boxes, not actual tables.
        // Render them as plain text instead.
        if table.rows.len() == 1 {
            let row = &table.rows[0];
            let texts: Vec<String> = row
                .cells
                .iter()
                .map(|cell| self.render_cell_content(cell))
                .filter(|s| !s.is_empty())
                .collect();
            if !texts.is_empty() {
                output.push_str(&texts.join(" "));
                output.push_str("\n\n");
            }
            return;
        }

        // Render all tables as markdown, including those with rowspan
        // Rowspan cells are expanded: content appears in first row, subsequent rows get empty cells
        self.render_table_markdown_with_rowspan(table, output);
    }

    /// Renders a simple table as Markdown (without colspan handling).
    #[allow(dead_code)]
    fn render_table_markdown(&self, table: &Table, output: &mut String) {
        let _col_count = table.column_count();

        for (row_idx, row) in table.rows.iter().enumerate() {
            output.push('|');

            for cell in &row.cells {
                let text = cell.plain_text().replace('\n', " ");
                output.push_str(&format!(" {} |", text.trim()));
            }

            output.push('\n');

            // Add separator after header row
            if row_idx == 0 {
                output.push('|');
                for cell in &row.cells {
                    let sep = match cell.alignment {
                        Alignment::Left => ":---",
                        Alignment::Center => ":---:",
                        Alignment::Right => "---:",
                        Alignment::Justify => "---",
                    };
                    output.push_str(&format!(" {} |", sep));
                }
                output.push('\n');
            }
        }

        output.push('\n');
    }

    /// Renders a table as plain text (for complex tables with rowspan).
    #[allow(dead_code)]
    fn render_table_as_text(&self, table: &Table, output: &mut String) {
        for row in &table.rows {
            let texts: Vec<String> = row
                .cells
                .iter()
                .map(|cell| self.render_cell_content(cell))
                .filter(|s| !s.is_empty())
                .collect();
            if !texts.is_empty() {
                output.push_str(&texts.join(" | "));
                output.push_str("\n\n");
            }
        }
    }

    /// Renders a table as Markdown, handling both rowspan and colspan.
    /// Rowspan cells have content in first row only; subsequent rows get empty cells.
    fn render_table_markdown_with_rowspan(&self, table: &Table, output: &mut String) {
        // First, build a grid that expands rowspan/colspan
        // Each cell in grid: (content_string, alignment, is_continuation)
        // is_continuation = true means this cell is part of a rowspan from above

        // Calculate total columns considering colspan
        let total_cols = table
            .rows
            .iter()
            .map(|row| row.cells.iter().map(|c| c.colspan as usize).sum::<usize>())
            .max()
            .unwrap_or(0);

        if total_cols == 0 {
            return;
        }

        // Build expanded grid
        // Track which cells are "occupied" by rowspan from previous rows
        // rowspan_remaining[col] = (remaining_rows, content, alignment)
        let mut rowspan_remaining: Vec<Option<(u32, String, Alignment)>> = vec![None; total_cols];

        for (row_idx, row) in table.rows.iter().enumerate() {
            let mut col_idx = 0;
            let mut cell_iter = row.cells.iter();
            let mut row_output = String::from("|");
            let mut alignments: Vec<Alignment> = Vec::new();

            while col_idx < total_cols {
                // Check if this column is occupied by rowspan from above
                if let Some((remaining, _, align)) = &rowspan_remaining[col_idx] {
                    if *remaining > 0 {
                        // This cell is a continuation of rowspan - render empty
                        row_output.push_str(" |");
                        alignments.push(*align);
                        rowspan_remaining[col_idx] = if *remaining > 1 {
                            Some((*remaining - 1, String::new(), *align))
                        } else {
                            None
                        };
                        col_idx += 1;
                        continue;
                    }
                }

                // Get next cell from the row
                if let Some(cell) = cell_iter.next() {
                    let text = self.render_cell_content(cell);
                    row_output.push_str(&format!(" {} |", text.trim()));
                    alignments.push(cell.alignment);

                    // Handle colspan - add empty cells
                    for _ in 1..cell.colspan {
                        row_output.push_str(" |");
                        alignments.push(cell.alignment);
                    }

                    // Handle rowspan - mark columns as occupied for future rows
                    if cell.rowspan > 1 {
                        for slot in rowspan_remaining
                            .iter_mut()
                            .take((col_idx + cell.colspan as usize).min(total_cols))
                            .skip(col_idx)
                        {
                            *slot = Some((cell.rowspan - 1, String::new(), cell.alignment));
                        }
                    }

                    col_idx += cell.colspan as usize;
                } else {
                    // No more cells in row, fill with empty
                    row_output.push_str(" |");
                    alignments.push(Alignment::Left);
                    col_idx += 1;
                }
            }

            output.push_str(&row_output);
            output.push('\n');

            // Add separator after header row
            if row_idx == 0 {
                output.push('|');
                for align in &alignments {
                    let sep = match align {
                        Alignment::Left => " :--- |",
                        Alignment::Center => " :---: |",
                        Alignment::Right => " ---: |",
                        Alignment::Justify => " --- |",
                    };
                    output.push_str(sep);
                }
                output.push('\n');
            }
        }

        output.push('\n');
    }

    /// Renders a table as Markdown, handling colspan by adding empty cells.
    #[allow(dead_code)]
    fn render_table_markdown_with_colspan(&self, table: &Table, output: &mut String) {
        for (row_idx, row) in table.rows.iter().enumerate() {
            output.push('|');

            for cell in &row.cells {
                let text = self.render_cell_content(cell);
                output.push_str(&format!(" {} |", text.trim()));

                // Add empty cells for colspan > 1
                for _ in 1..cell.colspan {
                    output.push_str(" |");
                }
            }

            output.push('\n');

            // Add separator after header row
            if row_idx == 0 {
                output.push('|');
                for cell in &row.cells {
                    let sep = match cell.alignment {
                        Alignment::Left => ":---",
                        Alignment::Center => ":---:",
                        Alignment::Right => "---:",
                        Alignment::Justify => "---",
                    };
                    output.push_str(&format!(" {} |", sep));

                    // Add separators for colspan cells
                    for _ in 1..cell.colspan {
                        output.push_str(&format!(" {} |", sep));
                    }
                }
                output.push('\n');
            }
        }

        output.push('\n');
    }

    /// Renders the content of a table cell, including text and images.
    fn render_cell_content(&self, cell: &TableCell) -> String {
        let mut content = String::new();

        for para in &cell.content {
            for item in &para.content {
                match item {
                    InlineContent::Text(run) => {
                        content.push_str(&run.text);
                    }
                    InlineContent::Image(img) => {
                        let alt = img.alt_text.as_deref().unwrap_or("image");
                        let filename = self
                            .image_id_to_filename
                            .get(&img.id)
                            .cloned()
                            .unwrap_or_else(|| img.id.clone());
                        let path = format!("{}{}", self.options.image_path_prefix, filename);
                        content.push_str(&format!("![{}]({})", alt, path));
                    }
                    InlineContent::LineBreak => {
                        content.push(' ');
                    }
                    _ => {}
                }
            }
            content.push(' '); // Space between paragraphs in cell
        }

        // Normalize whitespace
        content.replace('\n', " ").trim().to_string()
    }

    /// Renders a table with merged cells as HTML (for future use).
    #[allow(dead_code)]
    fn render_table_html(&self, table: &Table, output: &mut String) {
        output.push_str("<table>\n");

        for (row_idx, row) in table.rows.iter().enumerate() {
            let tag = if row_idx == 0 && table.has_header {
                "th"
            } else {
                "td"
            };

            output.push_str("  <tr>\n");

            for cell in &row.cells {
                let mut attrs = String::new();

                if cell.rowspan > 1 {
                    attrs.push_str(&format!(" rowspan=\"{}\"", cell.rowspan));
                }
                if cell.colspan > 1 {
                    attrs.push_str(&format!(" colspan=\"{}\"", cell.colspan));
                }

                let text = cell.plain_text();
                output.push_str(&format!(
                    "    <{}{}>{}</{}>\n",
                    tag,
                    attrs,
                    text.trim(),
                    tag
                ));
            }

            output.push_str("  </tr>\n");
        }

        output.push_str("</table>\n\n");
    }
}

/// Escape Markdown special characters.
///
/// Only escapes characters that are **always** special in Markdown regardless of position:
/// - `\` - escape character
/// - `` ` `` - inline code
/// - `*` and `_` - emphasis/bold
/// - `|` - table delimiter
///
/// Characters that are NOT escaped (only special in specific contexts):
/// - `()`, `[]`, `{}` - only special in link/image syntax `[text](url)`
/// - `#` - only special at start of line (headings)
/// - `+`, `-` - only special at start of line (lists) or `---` (rules)
/// - `!` - only special before `[` (images)
/// - `.` - only special in ordered lists at line start (e.g., "1.")
fn escape_markdown(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            // Only escape characters that are ALWAYS special regardless of position
            '\\' | '`' | '*' | '_' | '|' => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result
}

/// Escapes special characters for YAML strings.
fn escape_yaml(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Block, ImageRef, Section, Table, TableCell, TableRow, TextStyle};

    #[test]
    fn test_render_simple_paragraph() {
        let mut doc = Document::new();
        let mut section = Section::new(0);
        section.push_paragraph(Paragraph::text("Hello, world!"));
        doc.sections.push(section);

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        assert!(result.contains("Hello, world!"));
    }

    #[test]
    fn test_render_heading() {
        let mut doc = Document::new();
        let mut section = Section::new(0);

        let mut para = Paragraph::with_style(crate::model::ParagraphStyle::heading(2));
        para.content
            .push(InlineContent::Text(TextRun::new("Section Title")));
        section.push_paragraph(para);
        doc.sections.push(section);

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        assert!(result.contains("## Section Title"));
    }

    #[test]
    fn test_render_bold_text() {
        let mut doc = Document::new();
        let mut section = Section::new(0);

        let mut para = Paragraph::new();
        para.content.push(InlineContent::Text(TextRun::with_style(
            "bold text",
            TextStyle::bold(),
        )));
        section.push_paragraph(para);
        doc.sections.push(section);

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        assert!(result.contains("**bold text**"));
    }

    #[test]
    fn test_escape_markdown() {
        assert_eq!(escape_markdown("*bold*"), "\\*bold\\*");
        // [] are not escaped - only special in link/image syntax context
        assert_eq!(escape_markdown("[link]"), "[link]");
        assert_eq!(escape_markdown("a|b|c"), "a\\|b\\|c");
    }

    #[test]
    fn test_empty_heading_skipped() {
        // Empty headings should not be output
        let mut doc = Document::new();
        let mut section = Section::new(0);

        // Create an empty heading (heading_level > 0 but no content)
        let para = Paragraph::with_style(crate::model::ParagraphStyle::heading(3));
        section.push_paragraph(para);

        // Also add a normal paragraph for output
        section.push_paragraph(Paragraph::text("Normal content"));
        doc.sections.push(section);

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        // Should NOT contain empty heading markers
        assert!(
            !result.contains("###"),
            "Empty heading should be skipped: {}",
            result
        );
        // Should still contain normal content
        assert!(result.contains("Normal content"));
    }

    #[test]
    fn test_image_only_heading_no_markers() {
        // Image-only paragraphs should not have heading markers
        let mut doc = Document::new();
        let mut section = Section::new(0);

        // Create a paragraph with heading style but only an image
        let mut para = Paragraph::with_style(crate::model::ParagraphStyle::heading(2));
        para.content
            .push(InlineContent::Image(ImageRef::new("test_image")));
        section.push_paragraph(para);
        doc.sections.push(section);

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        // Should contain the image but NOT heading markers
        assert!(
            result.contains("![image]"),
            "Image should be rendered: {}",
            result
        );
        assert!(
            !result.contains("##"),
            "Image-only paragraph should not have heading markers: {}",
            result
        );
    }

    #[test]
    fn test_heading_with_text_works() {
        // Normal headings with text should still work
        let mut doc = Document::new();
        let mut section = Section::new(0);

        let mut para = Paragraph::with_style(crate::model::ParagraphStyle::heading(4));
        para.content
            .push(InlineContent::Text(TextRun::new("Real Heading")));
        section.push_paragraph(para);
        doc.sections.push(section);

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        assert!(
            result.contains("#### Real Heading"),
            "Normal heading should work: {}",
            result
        );
    }

    #[test]
    fn test_korean_bullet_marker_not_heading() {
        // Paragraphs starting with Korean bullet markers should not be headings
        let mut doc = Document::new();
        let mut section = Section::new(0);

        // Korean jamo 'ㅇ' as bullet marker
        let mut para = Paragraph::with_style(crate::model::ParagraphStyle::heading(2));
        para.content
            .push(InlineContent::Text(TextRun::new("ㅇ항목 내용입니다")));
        section.push_paragraph(para);
        doc.sections.push(section);

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        assert!(
            !result.contains("##"),
            "Korean bullet marker should not be heading: {}",
            result
        );
        assert!(
            result.contains("ㅇ항목"),
            "Content should still be present: {}",
            result
        );
    }

    #[test]
    fn test_long_text_not_heading() {
        // Very long text (>80 chars) should not be treated as a heading
        let mut doc = Document::new();
        let mut section = Section::new(0);

        // This text is exactly 100+ characters to ensure it exceeds MAX_HEADING_TEXT_LENGTH (80)
        let long_text = "이것은 매우 긴 문장입니다. 제목으로 사용하기에는 너무 길어서 본문으로 처리되어야 합니다. 일반적인 제목은 짧고 간결해야 하며, 본문과 구분되어야 합니다.";
        assert!(
            long_text.chars().count() > 80,
            "Test text should be longer than 80 chars"
        );

        let mut para = Paragraph::with_style(crate::model::ParagraphStyle::heading(3));
        para.content
            .push(InlineContent::Text(TextRun::new(long_text)));
        section.push_paragraph(para);
        doc.sections.push(section);

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        assert!(
            !result.contains("###"),
            "Long text should not have heading markers: {}",
            result
        );
        assert!(
            result.contains("이것은 매우"),
            "Content should still be present: {}",
            result
        );
    }

    #[test]
    fn test_max_heading_level_capped() {
        // Heading levels beyond max should be capped
        let mut doc = Document::new();
        let mut section = Section::new(0);

        let mut para = Paragraph::with_style(crate::model::ParagraphStyle::heading(6));
        para.content
            .push(InlineContent::Text(TextRun::new("Deep Heading")));
        section.push_paragraph(para);
        doc.sections.push(section);

        // Default max_heading_level is now 4
        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        assert!(
            result.contains("#### Deep Heading"),
            "Heading level 6 should be capped to 4: {}",
            result
        );
        assert!(
            !result.contains("######"),
            "Should not have 6 hash marks: {}",
            result
        );
    }

    #[test]
    fn test_single_row_table_as_plain_text() {
        // Single-row tables (used as decorative boxes) should render as plain text
        let mut doc = Document::new();
        let mut section = Section::new(0);

        let table = Table {
            rows: vec![TableRow {
                cells: vec![TableCell::text("박스 안의"), TableCell::text("텍스트")],
                is_header: false,
            }],
            column_widths: vec![],
            has_header: false,
        };
        section.content.push(Block::Table(table));
        doc.sections.push(section);

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        // Should NOT contain table markers
        assert!(
            !result.contains("|"),
            "Single-row table should not use pipe syntax: {}",
            result
        );
        assert!(
            !result.contains(":---"),
            "Single-row table should not have separator: {}",
            result
        );

        // Should contain the text
        assert!(
            result.contains("박스 안의"),
            "Text content should be present: {}",
            result
        );
        assert!(
            result.contains("텍스트"),
            "Text content should be present: {}",
            result
        );
    }

    #[test]
    fn test_multi_row_table_renders_as_table() {
        // Multi-row tables should render as proper markdown tables
        let mut doc = Document::new();
        let mut section = Section::new(0);

        let table = Table {
            rows: vec![
                TableRow {
                    cells: vec![TableCell::text("Header 1"), TableCell::text("Header 2")],
                    is_header: true,
                },
                TableRow {
                    cells: vec![TableCell::text("Data 1"), TableCell::text("Data 2")],
                    is_header: false,
                },
            ],
            column_widths: vec![],
            has_header: true,
        };
        section.content.push(Block::Table(table));
        doc.sections.push(section);

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        // Should contain table markers
        assert!(
            result.contains("|"),
            "Multi-row table should use pipe syntax: {}",
            result
        );
        assert!(
            result.contains(":---"),
            "Multi-row table should have separator: {}",
            result
        );
    }
}

#[cfg(test)]
mod font_size_heading_tests {
    use super::*;

    #[test]
    fn test_symbol_without_large_font_is_not_heading() {
        use crate::model::{
            Block, Document, InlineContent, Paragraph, ParagraphStyle, Section, TextRun, TextStyle,
        };

        // Symbol markers like ◎, ※ should NOT be headings without larger font size
        let doc = Document {
            sections: vec![Section {
                content: vec![Block::Paragraph(Paragraph {
                    style: ParagraphStyle {
                        heading_level: 0,
                        ..Default::default()
                    },
                    content: vec![InlineContent::Text(TextRun {
                        text: "※ This is a note, not a heading".to_string(),
                        style: TextStyle {
                            font_size: Some(12.0), // Same as body text
                            ..Default::default()
                        },
                    })],
                })],
                ..Default::default()
            }],
            ..Default::default()
        };

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        // Should NOT be a heading (no ## prefix)
        assert!(
            !result.contains("## ※"),
            "Symbol with normal font size should not be heading, got: {}",
            result
        );
    }

    #[test]
    fn test_large_font_becomes_heading() {
        use crate::model::{
            Block, Document, InlineContent, Paragraph, ParagraphStyle, Section, TextRun, TextStyle,
        };

        // Create document with body text (12pt) and a larger title (16pt)
        let doc = Document {
            sections: vec![Section {
                content: vec![
                    // Body text to establish baseline
                    Block::Paragraph(Paragraph {
                        style: ParagraphStyle::default(),
                        content: vec![InlineContent::Text(TextRun {
                            text: "This is body text with normal font size.".to_string(),
                            style: TextStyle {
                                font_size: Some(12.0),
                                ..Default::default()
                            },
                        })],
                    }),
                    // Larger font text should become heading
                    Block::Paragraph(Paragraph {
                        style: ParagraphStyle::default(),
                        content: vec![InlineContent::Text(TextRun {
                            text: "Section Title".to_string(),
                            style: TextStyle {
                                font_size: Some(16.0), // 133% of 12pt
                                ..Default::default()
                            },
                        })],
                    }),
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        // Should be a heading (has ## or ### prefix)
        assert!(
            result.contains("# Section Title") || result.contains("## Section Title") || result.contains("### Section Title"),
            "Large font text should become heading, got: {}",
            result
        );
    }

    #[test]
    fn test_list_markers_not_heading() {
        // List markers should never be headings regardless of font size
        let marker = '•';
        assert!(
            LIST_MARKERS.contains(&marker),
            "• should be in LIST_MARKERS"
        );
    }
}
