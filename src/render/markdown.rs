//! Markdown renderer implementation.

use super::{RenderOptions, TableFallback};
use crate::error::Result;
use crate::model::{
    Alignment, Block, Document, InlineContent, ListStyle, Paragraph, Table, TextRun,
};

/// Markdown renderer.
#[derive(Debug)]
pub struct MarkdownRenderer {
    options: RenderOptions,
}

impl MarkdownRenderer {
    /// Creates a new renderer with the given options.
    pub fn new(options: RenderOptions) -> Self {
        Self { options }
    }

    /// Renders a document to Markdown string.
    pub fn render(&self, document: &Document) -> Result<String> {
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
                        self.render_paragraph(para, &mut output);
                    }
                    Block::Table(table) => {
                        self.render_table(table, &mut output);
                    }
                }
            }
        }

        Ok(output)
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
        if let Some(ref created) = document.metadata.created {
            output.push_str(&format!("date: \"{}\"\n", created));
        }

        output.push_str("---\n\n");
    }

    /// Renders a paragraph.
    fn render_paragraph(&self, para: &Paragraph, output: &mut String) {
        if para.is_empty() && !self.options.include_empty_paragraphs {
            return;
        }

        let style = &para.style;

        // Heading prefix
        if style.heading_level > 0 {
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
                let path = format!("{}{}", self.options.image_path_prefix, img.id);
                output.push_str(&format!("![{}]({})", alt, path));
            }
            InlineContent::Equation(eq) => {
                // Prefer LaTeX if available, otherwise use code block
                if let Some(ref latex) = eq.latex {
                    output.push_str(&format!("${}$", latex));
                } else {
                    output.push_str(&format!("`{}`", eq.script));
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
                output.push_str(&format!("    <{}{}>{}</{}>\n", tag, attrs, text.trim(), tag));
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
    use crate::model::{Section, TextStyle};

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
        para.content.push(InlineContent::Text(TextRun::new("Section Title")));
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
}
