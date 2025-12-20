//! Markdown renderer implementation.

use super::{RenderOptions, TableFallback};
use crate::error::Result;
use crate::model::{
    Alignment, Block, Document, InlineContent, ListStyle, Paragraph, Table, TextRun,
};

use std::collections::HashMap;

/// Maximum character length for a heading.
/// Text longer than this is unlikely to be a semantic heading.
const MAX_HEADING_TEXT_LENGTH: usize = 80;

/// Common list/bullet markers including Korean characters.
/// Used to detect paragraphs that should not be rendered as headings.
const LIST_MARKERS: &[char] = &[
    // ASCII markers
    '-', '*', '>',
    // Korean/Asian markers
    '※', '○', '•', '●', '◦', '◎',
    '□', '■', '▪', '▫', '◇', '◆',
    '☐', '☑', '☒', '✓', '✗',
    'ㅇ',  // Korean jamo (circle)
    'ㆍ',  // Korean middle dot (U+318D)
    '·',   // Middle dot (U+00B7)
    '∙',   // Bullet operator (U+2219)
    // Arrows (commonly used as list markers in Korean documents)
    '→', '←', '↔', '⇒', '⇐', '⇔',
    '►', '▶', '▷', '◀', '◁', '▻',
];

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

        let mut output = String::new();

        // Render frontmatter if enabled
        if renderer.options.include_frontmatter {
            renderer.render_frontmatter(document, &mut output);
        }

        // Render each section
        for section in &document.sections {
            for block in &section.content {
                match block {
                    Block::Paragraph(para) => {
                        renderer.render_paragraph(para, &mut output);
                    }
                    Block::Table(table) => {
                        renderer.render_table(table, &mut output);
                    }
                }
            }
        }

        // Apply cleanup pipeline if enabled
        if let Some(ref cleanup_options) = renderer.options.cleanup {
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
    fn render_paragraph(&self, para: &Paragraph, output: &mut String) {
        if para.is_empty() && !self.options.include_empty_paragraphs {
            return;
        }

        let style = &para.style;
        let plain_text = para.plain_text();
        let trimmed_text = plain_text.trim();

        // Check if paragraph content looks like a list item (starts with list-like markers)
        let looks_like_list_item = trimmed_text
            .chars()
            .next()
            .map(|c| LIST_MARKERS.contains(&c))
            .unwrap_or(false);

        // Check if text is too long to be a meaningful heading
        let text_too_long = trimmed_text.chars().count() > MAX_HEADING_TEXT_LENGTH;

        // Determine if heading should be applied
        // Skip heading markers for:
        // 1. Empty headings (no text content at all)
        // 2. Image-only paragraphs (images should not have heading markers)
        // 3. Paragraphs with list styles set
        // 4. Paragraphs that look like list items (start with list markers)
        // 5. Paragraphs with text too long to be semantic headings
        let should_apply_heading = style.heading_level > 0
            && para.has_text_content()
            && !para.is_image_only()
            && style.list_style.is_none()
            && !looks_like_list_item
            && !text_too_long;

        // Skip empty headings entirely - don't output anything
        if style.heading_level > 0 && !para.has_text_content() && !para.is_image_only() {
            return;
        }

        // Heading prefix (only for valid headings)
        if should_apply_heading {
            let level = style.heading_level.min(self.options.max_heading_level);
            output.push_str(&"#".repeat(level as usize));
            output.push(' ');
        }

        // List prefix
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
        if self.options.paragraph_spacing && style.heading_level == 0 {
            output.push('\n');
        }
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
        if style.underline {
            prefix.push_str("<u>");
            suffix.insert_str(0, "</u>");
        }
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

        // Check if table has merged cells
        if table.has_merged_cells() {
            match self.options.table_fallback {
                TableFallback::Html => {
                    self.render_table_html(table, output);
                    return;
                }
                TableFallback::Skip => {
                    output.push_str("<!-- Complex table omitted -->\n\n");
                    return;
                }
                TableFallback::SimplifiedMarkdown => {
                    // Continue with simplified rendering
                }
            }
        }

        self.render_table_markdown(table, output);
    }

    /// Renders a simple table as Markdown.
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

    /// Renders a table with merged cells as HTML.
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

/// Escapes special Markdown characters.
fn escape_markdown(text: &str) -> String {
    let mut result = String::with_capacity(text.len());

    for ch in text.chars() {
        match ch {
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#' | '+' | '-' | '.'
            | '!' | '|' => {
                result.push('\\');
                result.push(ch);
            }
            _ => result.push(ch),
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
    use crate::model::{ImageRef, Section, TextStyle};

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
        assert_eq!(escape_markdown("[link]"), "\\[link\\]");
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
}
