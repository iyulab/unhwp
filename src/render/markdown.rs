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
    // Filled/hollow shape bullets
    '●', '○', '■', '□', '◆', '◇', '★', '☆', '◼', '◾',
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
        let mut mapping = HashMap::with_capacity(document.resources.len());

        for filename in document.resources.keys() {
            // Extract base name without extension (e.g., "image1.bmp" -> "image1")
            if let Some(dot_pos) = filename.rfind('.') {
                let base_name = &filename[..dot_pos];
                mapping.insert(base_name.to_string(), filename.clone());
            } else {
                // No extension - use filename as both key and value
                let filename_owned = filename.clone();
                mapping.insert(filename_owned.clone(), filename_owned);
            }
        }

        mapping
    }

    /// Renders YAML frontmatter.
    /// Only outputs if there's meaningful metadata to include.
    fn render_frontmatter(&self, document: &Document, output: &mut String) {
        let mut content = String::new();

        if let Some(ref title) = document.metadata.title {
            content.push_str(&format!("title: \"{}\"\n", escape_yaml(title)));
        }
        if let Some(ref author) = document.metadata.author {
            content.push_str(&format!("author: \"{}\"\n", escape_yaml(author)));
        }
        if let Some(ref subject) = document.metadata.subject {
            content.push_str(&format!("description: \"{}\"\n", escape_yaml(subject)));
        }
        if let Some(ref created) = document.metadata.created {
            content.push_str(&format!("date: \"{}\"\n", created));
        }
        if let Some(ref modified) = document.metadata.modified {
            content.push_str(&format!("lastmod: \"{}\"\n", modified));
        }
        if !document.metadata.keywords.is_empty() {
            content.push_str("tags:\n");
            for keyword in &document.metadata.keywords {
                content.push_str(&format!("  - \"{}\"\n", escape_yaml(keyword)));
            }
        }
        if let Some(ref app) = document.metadata.creator_app {
            content.push_str(&format!("generator: \"{}\"\n", escape_yaml(app)));
        }
        // Note: format_version (e.g., "5.0.3.0") is HWP internal version, not useful for end users
        // Omitted from frontmatter for cleaner output

        // Only output frontmatter if there's content
        if !content.is_empty() {
            output.push_str("---\n");
            output.push_str(&content);
            output.push_str("---\n\n");
        }
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
        let mut is_list_item = style.list_style.is_some();
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

        // Bullet/blockquote mapping: convert leading bullet characters to
        // markdown list markers or blockquote prefixes.
        // Applied at render time regardless of cleanup settings.
        let mut strip_bullet = false;
        let mut is_blockquote = false;
        if !should_apply_heading && !is_list_item {
            if let Some((prefix, strip)) = detect_bullet_prefix(trimmed_text) {
                output.push_str(prefix);
                strip_bullet = strip;
                if prefix.starts_with('>') {
                    is_blockquote = true;
                } else {
                    is_list_item = true; // treat as list item for spacing
                }
            }
        }

        // Render inline content
        let mut need_strip = strip_bullet;
        for item in &para.content {
            if need_strip {
                if let InlineContent::Text(run) = item {
                    let stripped = strip_leading_bullet_char(&run.text);
                    need_strip = false;
                    if !stripped.is_empty() {
                        let modified = TextRun::with_style(stripped, run.style.clone());
                        self.render_text_run(&modified, output);
                    }
                    continue;
                }
            }
            self.render_inline(item, output);
        }

        // End paragraph
        output.push('\n');

        // Add blank line for proper Markdown separation:
        // - After headings: ALWAYS add blank line (Markdown convention)
        // - After list items: NO blank line (consecutive items should be adjacent)
        // - After regular paragraphs: add blank line for readability
        let is_heading = should_apply_heading || style.heading_level > 0;
        if is_heading {
            // Headings always followed by blank line (Markdown convention)
            output.push('\n');
        } else if is_blockquote {
            // Blockquotes followed by blank line
            output.push('\n');
        } else if self.options.paragraph_spacing && !is_list_item {
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

    /// Renders inline content in paragraph context.
    ///
    /// Differs from `render_inline_to_string` only in LineBreak handling:
    /// paragraphs respect `preserve_line_breaks` option.
    fn render_inline(&self, item: &InlineContent, output: &mut String) {
        if let InlineContent::LineBreak = item {
            if self.options.preserve_line_breaks {
                output.push_str("  \n"); // Two spaces + newline for Markdown line break
            } else {
                output.push(' ');
            }
        } else {
            self.render_inline_to_string(item, output);
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

        // Ensure blank line before table (Markdown convention)
        // But avoid double blank lines if output already ends with blank line
        if !output.is_empty() && !output.ends_with("\n\n") {
            if output.ends_with('\n') {
                output.push('\n');
            } else {
                output.push_str("\n\n");
            }
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

        // Calculate total columns considering colspan
        let total_cols = table
            .rows
            .iter()
            .map(|row| row.cells.iter().map(|c| c.colspan as usize).sum::<usize>())
            .max()
            .unwrap_or(0);

        // Optimization: extract full-span rows (colspan == total_cols) as plain text blocks
        // and check empty cell ratio for the remaining rows.
        if total_cols > 0 {
            let (full_span_rows, normal_rows): (Vec<_>, Vec<_>) =
                table.rows.iter().enumerate().partition(|(_, row)| {
                    row.cells.len() == 1 && row.cells[0].colspan as usize >= total_cols
                });

            // If all rows are full-span, render as plain text
            if normal_rows.is_empty() {
                for (_, row) in &full_span_rows {
                    let text = self.render_cell_content(&row.cells[0]);
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        output.push_str(trimmed);
                        output.push_str("\n\n");
                    }
                }
                return;
            }

            // Calculate empty cell ratio in normal (non-full-span) rows
            let mut total_cells = 0usize;
            let mut empty_cells = 0usize;
            for (_, row) in &normal_rows {
                for cell in &row.cells {
                    let expanded = cell.colspan as usize;
                    total_cells += expanded;
                    let text = cell.plain_text();
                    if text.trim().is_empty() {
                        empty_cells += expanded;
                    }
                }
            }

            // If empty cell ratio >= 60%, fall back to structured text rendering
            // This typically indicates form-style tables (신청서 양식) where
            // markdown table format produces excessive empty pipe columns
            let empty_ratio = if total_cells > 0 {
                empty_cells as f64 / total_cells as f64
            } else {
                0.0
            };

            if empty_ratio >= 0.6 {
                self.render_table_structured_text(table, total_cols, output);
                return;
            }
        }

        // Render all tables as markdown, including those with rowspan
        // Rowspan cells are expanded: content appears in first row, subsequent rows get empty cells
        self.render_table_markdown_with_rowspan(table, output);
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

    /// Renders the content of a table cell, including all inline content types
    /// with formatting (bold, italic, etc.).
    ///
    /// When a cell contains multiple non-empty paragraphs, uses `<br>` to
    /// preserve the multi-line structure (HTML-compatible Markdown).
    fn render_cell_content(&self, cell: &TableCell) -> String {
        // Collect paragraph contents, filtering out empty paragraphs
        let para_texts: Vec<String> = cell
            .content
            .iter()
            .map(|para| {
                let mut para_content = String::new();
                for item in &para.content {
                    self.render_inline_to_string(item, &mut para_content);
                }
                para_content.replace('\n', " ").trim().to_string()
            })
            .filter(|s| !s.is_empty())
            .collect();

        if para_texts.len() <= 1 {
            // Single paragraph or empty: simple join
            para_texts.into_iter().next().unwrap_or_default()
        } else {
            // Multiple paragraphs: use <br> for line separation
            para_texts.join("<br>")
        }
    }

    /// Renders a single inline content item to a string buffer.
    /// Reusable for both paragraph rendering and table cell rendering.
    fn render_inline_to_string(&self, item: &InlineContent, output: &mut String) {
        match item {
            InlineContent::Text(run) => {
                self.render_text_run(run, output);
            }
            InlineContent::LineBreak => {
                output.push(' ');
            }
            InlineContent::Image(img) => {
                let alt = img.alt_text.as_deref().unwrap_or("image");
                let filename = self
                    .image_id_to_filename
                    .get(&img.id)
                    .cloned()
                    .unwrap_or_else(|| img.id.clone());
                let path = format!("{}{}", self.options.image_path_prefix, filename);
                output.push_str(&format!("![{}]({})", alt, path));
            }
            InlineContent::Equation(eq) => {
                if let Some(ref latex) = eq.latex {
                    output.push_str(&format!("${}$", latex));
                } else if !eq.script.is_empty() {
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

    /// Renders a sparse table (high empty cell ratio) as structured text.
    ///
    /// Full-span rows (colspan == total_cols) are rendered as standalone text blocks.
    /// Normal rows render non-empty cells as "key: value" or comma-separated items.
    fn render_table_structured_text(&self, table: &Table, total_cols: usize, output: &mut String) {
        for row in &table.rows {
            let is_full_span = row.cells.len() == 1 && row.cells[0].colspan as usize >= total_cols;

            if is_full_span {
                let text = self.render_cell_content(&row.cells[0]);
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    output.push_str(trimmed);
                    output.push_str("\n\n");
                }
            } else {
                // Collect non-empty cells
                let texts: Vec<String> = row
                    .cells
                    .iter()
                    .map(|cell| self.render_cell_content(cell))
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                if !texts.is_empty() {
                    output.push_str(&texts.join(" | "));
                    output.push_str("\n\n");
                }
            }
        }
    }
}

/// Detects if text starts with a bullet or blockquote marker character.
///
/// Returns `(markdown_prefix, should_strip_char)`:
/// - For bullets: ("- ", true) — add list marker, strip the bullet char
/// - For sub-item bullets: ("  - ", true) — indented list marker
/// - For blockquote markers: ("> ", false) — add blockquote prefix, keep the char
/// - For checkmarks: ("- [x] ", true) / ("- [ ] ", true)
fn detect_bullet_prefix(text: &str) -> Option<(&'static str, bool)> {
    let first_char = text.chars().next()?;

    match first_char {
        // Filled bullets → list marker
        '●' | '■' | '◆' | '▶' | '►' | '•' | '◼' | '◾' => Some(("- ", true)),
        // Middle dot variants
        '·' | '∙' | 'ㆍ' => Some(("- ", true)),
        // Hollow bullets → list marker
        '○' | '□' | '◇' | '▷' => Some(("- ", true)),
        // White bullet → indented sub-item
        '◦' => Some(("  - ", true)),
        // Arrows
        '→' | '⇒' | '➔' | '➢' | '➤' => Some(("- ", true)),
        // Stars
        '★' | '☆' => Some(("- ", true)),
        // Checkmarks
        '✓' | '✔' => Some(("- [x] ", true)),
        '✗' | '✘' => Some(("- [ ] ", true)),
        // Note marker → blockquote (keep the ※ in text)
        '※' => Some(("> ", false)),
        _ => None,
    }
}

/// Strips the leading bullet character and any following whitespace from text.
///
/// Given "□ (지원목적)", returns "(지원목적)".
/// Given "●항목", returns "항목".
fn strip_leading_bullet_char(text: &str) -> String {
    let mut chars = text.chars();
    chars.next(); // skip the bullet character
    let remaining: String = chars.collect();
    // Trim leading whitespace after the bullet
    remaining.trim_start().to_string()
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
        // Note: With normalize_levels=true (default), H4 is normalized to H2
        let mut doc = Document::new();
        let mut section = Section::new(0);

        let mut para = Paragraph::with_style(crate::model::ParagraphStyle::heading(4));
        para.content
            .push(InlineContent::Text(TextRun::new("Real Heading")));
        section.push_paragraph(para);
        doc.sections.push(section);

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        // H4 is normalized to H2 (normalize_min_level=2)
        assert!(
            result.contains("## Real Heading"),
            "Normal heading should work (normalized to H2): {}",
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
        // With normalize_levels=true: H6 → H4 (capped) → H2 (normalized)
        let mut doc = Document::new();
        let mut section = Section::new(0);

        let mut para = Paragraph::with_style(crate::model::ParagraphStyle::heading(6));
        para.content
            .push(InlineContent::Text(TextRun::new("Deep Heading")));
        section.push_paragraph(para);
        doc.sections.push(section);

        // Default max_heading_level is 4, normalize_min_level is 2
        // H6 → capped to H4 → normalized to H2
        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        assert!(
            result.contains("## Deep Heading"),
            "Heading level 6 should be capped and normalized to 2: {}",
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
            result.contains("# Section Title")
                || result.contains("## Section Title")
                || result.contains("### Section Title"),
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

#[cfg(test)]
mod bullet_blockquote_tests {
    use super::*;
    use crate::model::{
        Block, Document, InlineContent, Paragraph, ParagraphStyle, Section, TextRun,
    };

    fn render_single_paragraph(text: &str) -> String {
        let doc = Document {
            sections: vec![Section {
                content: vec![Block::Paragraph(Paragraph {
                    style: ParagraphStyle::default(),
                    content: vec![InlineContent::Text(TextRun::new(text.to_string()))],
                })],
                ..Default::default()
            }],
            ..Default::default()
        };
        let renderer = MarkdownRenderer::new(RenderOptions::default());
        renderer.render(&doc).unwrap()
    }

    #[test]
    fn test_detect_bullet_prefix_filled() {
        assert_eq!(detect_bullet_prefix("● 항목"), Some(("- ", true)));
        assert_eq!(detect_bullet_prefix("■ 항목"), Some(("- ", true)));
        assert_eq!(detect_bullet_prefix("◆ 항목"), Some(("- ", true)));
        assert_eq!(detect_bullet_prefix("◼ 항목"), Some(("- ", true)));
        assert_eq!(detect_bullet_prefix("◾ 항목"), Some(("- ", true)));
    }

    #[test]
    fn test_detect_bullet_prefix_hollow() {
        assert_eq!(detect_bullet_prefix("○ 항목"), Some(("- ", true)));
        assert_eq!(detect_bullet_prefix("□ 항목"), Some(("- ", true)));
        assert_eq!(detect_bullet_prefix("◇ 항목"), Some(("- ", true)));
    }

    #[test]
    fn test_detect_bullet_prefix_sub_item() {
        // ◦ should produce indented sub-item
        assert_eq!(detect_bullet_prefix("◦ 하위항목"), Some(("  - ", true)));
    }

    #[test]
    fn test_detect_bullet_prefix_blockquote() {
        // ※ should produce blockquote, NOT strip the char
        assert_eq!(detect_bullet_prefix("※ 참고사항"), Some(("> ", false)));
    }

    #[test]
    fn test_detect_bullet_prefix_checkmarks() {
        assert_eq!(detect_bullet_prefix("✓ 완료"), Some(("- [x] ", true)));
        assert_eq!(detect_bullet_prefix("✗ 미완료"), Some(("- [ ] ", true)));
    }

    #[test]
    fn test_detect_bullet_prefix_none() {
        assert_eq!(detect_bullet_prefix("일반 텍스트"), None);
        assert_eq!(detect_bullet_prefix("Hello world"), None);
    }

    #[test]
    fn test_strip_leading_bullet_char() {
        assert_eq!(strip_leading_bullet_char("● 항목"), "항목");
        assert_eq!(strip_leading_bullet_char("●항목"), "항목");
        assert_eq!(strip_leading_bullet_char("□ (지원목적)"), "(지원목적)");
    }

    #[test]
    fn test_bullet_renders_as_list() {
        let result = render_single_paragraph("● 첫 번째 항목");
        assert!(
            result.contains("- 첫 번째 항목"),
            "Bullet should render as list item, got: {}",
            result
        );
        assert!(
            !result.contains('●'),
            "Bullet char should be stripped, got: {}",
            result
        );
    }

    #[test]
    fn test_sub_bullet_renders_indented() {
        let result = render_single_paragraph("◦ 하위 항목");
        assert!(
            result.contains("  - 하위 항목"),
            "Sub-bullet should render as indented list item, got: {}",
            result
        );
    }

    #[test]
    fn test_blockquote_renders_with_note_char() {
        let result = render_single_paragraph("※ 참고사항입니다");
        assert!(
            result.contains("> ※ 참고사항입니다"),
            "※ should render as blockquote keeping the char, got: {}",
            result
        );
    }

    #[test]
    fn test_new_bullet_chars_in_list_markers() {
        // Ensure newly added bullet chars are in LIST_MARKERS
        assert!(LIST_MARKERS.contains(&'◼'));
        assert!(LIST_MARKERS.contains(&'◾'));
        assert!(LIST_MARKERS.contains(&'◦'));
    }
}

#[cfg(test)]
mod table_cell_content_tests {
    use super::*;
    use crate::model::{Block, Document, Paragraph, Section, Table, TableCell, TableRow};

    #[test]
    fn test_single_paragraph_cell_no_br() {
        // Cell with single paragraph should not have <br>
        let mut doc = Document::new();
        let mut section = Section::new(0);

        let table = Table {
            rows: vec![
                TableRow {
                    cells: vec![TableCell::text("Header")],
                    is_header: true,
                },
                TableRow {
                    cells: vec![TableCell::text("Simple text")],
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

        assert!(
            !result.contains("<br>"),
            "Single paragraph cell should not have <br>, got: {}",
            result
        );
    }

    #[test]
    fn test_multi_paragraph_cell_uses_br() {
        // Cell with multiple paragraphs should use <br>
        let cell = TableCell {
            content: vec![
                Paragraph::text("첫 번째 항목"),
                Paragraph::text("두 번째 항목"),
                Paragraph::text("세 번째 항목"),
            ],
            rowspan: 1,
            colspan: 1,
            ..Default::default()
        };

        let mut doc = Document::new();
        let mut section = Section::new(0);

        let table = Table {
            rows: vec![
                TableRow {
                    cells: vec![TableCell::text("내용")],
                    is_header: true,
                },
                TableRow {
                    cells: vec![cell],
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

        assert!(
            result.contains("<br>"),
            "Multi-paragraph cell should use <br>, got: {}",
            result
        );
        assert!(
            result.contains("첫 번째 항목<br>두 번째 항목<br>세 번째 항목"),
            "Paragraphs should be joined with <br>, got: {}",
            result
        );
    }

    #[test]
    fn test_empty_paragraphs_filtered() {
        // Empty paragraphs in cell should be filtered out
        let cell = TableCell {
            content: vec![
                Paragraph::text("유효한 텍스트"),
                Paragraph::new(), // empty
                Paragraph::text("또 다른 텍스트"),
            ],
            rowspan: 1,
            colspan: 1,
            ..Default::default()
        };

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render_cell_content(&cell);

        assert_eq!(result, "유효한 텍스트<br>또 다른 텍스트");
    }

    #[test]
    fn test_sparse_table_falls_back_to_text() {
        // Table with >60% empty cells should fall back to structured text
        let mut doc = Document::new();
        let mut section = Section::new(0);

        // 4-column table where most cells are empty (form-style)
        let table = Table {
            rows: vec![
                TableRow {
                    cells: vec![
                        TableCell::text("이름"),
                        TableCell::text("홍길동"),
                        TableCell::new(), // empty
                        TableCell::new(), // empty
                    ],
                    is_header: false,
                },
                TableRow {
                    cells: vec![
                        TableCell::text("연락처"),
                        TableCell::new(), // empty
                        TableCell::new(), // empty
                        TableCell::new(), // empty
                    ],
                    is_header: false,
                },
            ],
            column_widths: vec![],
            has_header: false,
        };
        section.content.push(Block::Table(table));
        doc.sections.push(section);

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        // Should NOT contain pipe-style table (too sparse)
        assert!(
            !result.contains(":---"),
            "Sparse table should not use markdown table format, got: {}",
            result
        );
        // Should contain the text content
        assert!(
            result.contains("홍길동"),
            "Text content should be preserved, got: {}",
            result
        );
    }

    #[test]
    fn test_full_span_rows_as_text() {
        // Table where all rows have full-width colspan should render as text
        let mut doc = Document::new();
        let mut section = Section::new(0);

        let full_span_cell = |text: &str| -> TableCell {
            TableCell {
                content: vec![Paragraph::text(text)],
                rowspan: 1,
                colspan: 3, // full span (total_cols = 3)
                ..Default::default()
            }
        };

        let table = Table {
            rows: vec![
                TableRow {
                    cells: vec![full_span_cell("첫째 행 전체 텍스트")],
                    is_header: false,
                },
                TableRow {
                    cells: vec![full_span_cell("둘째 행 전체 텍스트")],
                    is_header: false,
                },
            ],
            column_widths: vec![],
            has_header: false,
        };
        section.content.push(Block::Table(table));
        doc.sections.push(section);

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        // Full-span table should be rendered as plain text, not table
        assert!(
            !result.contains("|"),
            "Full-span table should be plain text, got: {}",
            result
        );
        assert!(result.contains("첫째 행 전체 텍스트"));
        assert!(result.contains("둘째 행 전체 텍스트"));
    }

    #[test]
    fn test_normal_table_still_uses_markdown() {
        // Table with low empty cell ratio should still use markdown format
        let mut doc = Document::new();
        let mut section = Section::new(0);

        let table = Table {
            rows: vec![
                TableRow {
                    cells: vec![TableCell::text("항목"), TableCell::text("내용")],
                    is_header: true,
                },
                TableRow {
                    cells: vec![TableCell::text("A"), TableCell::text("설명 A")],
                    is_header: false,
                },
                TableRow {
                    cells: vec![TableCell::text("B"), TableCell::text("설명 B")],
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

        // Should use pipe-style table (not sparse)
        assert!(
            result.contains("|"),
            "Normal table should use markdown format, got: {}",
            result
        );
        assert!(
            result.contains(":---"),
            "Normal table should have separator, got: {}",
            result
        );
    }

    #[test]
    fn test_cell_with_bold_text() {
        use crate::model::{InlineContent, TextRun, TextStyle};

        // Cell with bold text should render with ** markers
        let cell = TableCell {
            content: vec![Paragraph {
                style: Default::default(),
                content: vec![InlineContent::Text(TextRun::with_style(
                    "강조 텍스트",
                    TextStyle {
                        bold: true,
                        ..Default::default()
                    },
                ))],
            }],
            rowspan: 1,
            colspan: 1,
            ..Default::default()
        };

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render_cell_content(&cell);

        assert!(
            result.contains("**강조 텍스트**"),
            "Bold text in cell should have ** markers, got: {}",
            result
        );
    }

    #[test]
    fn test_cell_with_equation() {
        use crate::model::{Equation, InlineContent};

        // Cell with equation should render as LaTeX or code
        let cell = TableCell {
            content: vec![Paragraph {
                style: Default::default(),
                content: vec![InlineContent::Equation(Equation {
                    script: "x^2".to_string(),
                    latex: Some("x^2".to_string()),
                })],
            }],
            rowspan: 1,
            colspan: 1,
            ..Default::default()
        };

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render_cell_content(&cell);

        assert!(
            result.contains("$x^2$"),
            "Equation in cell should render as LaTeX, got: {}",
            result
        );
    }

    #[test]
    fn test_cell_with_link() {
        use crate::model::InlineContent;

        // Cell with hyperlink should render as markdown link
        let cell = TableCell {
            content: vec![Paragraph {
                style: Default::default(),
                content: vec![InlineContent::Link {
                    text: "홈페이지".to_string(),
                    url: "https://example.com".to_string(),
                }],
            }],
            rowspan: 1,
            colspan: 1,
            ..Default::default()
        };

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render_cell_content(&cell);

        assert!(
            result.contains("[홈페이지](https://example.com)"),
            "Link in cell should render as markdown link, got: {}",
            result
        );
    }

    #[test]
    fn test_empty_paragraph_included_when_option_set() {
        // When include_empty_paragraphs is true, empty paragraphs should render
        let mut doc = Document::new();
        let mut section = Section::new(0);
        section.push_paragraph(Paragraph::text("첫 단락"));
        section.push_paragraph(Paragraph::new()); // empty
        section.push_paragraph(Paragraph::text("셋째 단락"));
        doc.sections.push(section);

        let opts = RenderOptions {
            include_empty_paragraphs: true,
            ..Default::default()
        };
        let renderer = MarkdownRenderer::new(opts);
        let result = renderer.render(&doc).unwrap();

        // Should contain both paragraphs (empty paragraph renders as blank line)
        assert!(result.contains("첫 단락"));
        assert!(result.contains("셋째 단락"));
    }

    #[test]
    fn test_empty_paragraph_filtered_by_default() {
        // Default: empty paragraphs are filtered out
        let mut doc = Document::new();
        let mut section = Section::new(0);
        section.push_paragraph(Paragraph::text("유일한 단락"));
        section.push_paragraph(Paragraph::new()); // empty — should be filtered
        doc.sections.push(section);

        let renderer = MarkdownRenderer::new(RenderOptions::default());
        let result = renderer.render(&doc).unwrap();

        assert!(result.contains("유일한 단락"));
        // Result should not have excessive blank lines from empty paragraph
        let trimmed = result.trim();
        assert!(
            !trimmed.ends_with("\n\n\n"),
            "Empty paragraph should not create extra blank lines"
        );
    }
}
