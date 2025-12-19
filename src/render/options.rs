//! Rendering options for Markdown output.

use std::path::PathBuf;

/// Options for Markdown rendering.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Directory to save extracted images.
    /// If None, images are embedded as base64 data URIs.
    pub image_dir: Option<PathBuf>,

    /// Prefix for image paths in Markdown.
    /// Default: "assets/"
    pub image_path_prefix: String,

    /// How to handle tables with merged cells.
    pub table_fallback: TableFallback,

    /// Maximum heading level to use (1-6).
    /// Headings beyond this level will use this level.
    pub max_heading_level: u8,

    /// Whether to include metadata as YAML frontmatter.
    pub include_frontmatter: bool,

    /// Whether to preserve line breaks within paragraphs.
    pub preserve_line_breaks: bool,

    /// Whether to include empty paragraphs.
    pub include_empty_paragraphs: bool,

    /// Character to use for unordered lists.
    /// Default: '-'
    pub list_marker: char,

    /// Whether to use ATX-style headers (#) or Setext-style (underline).
    pub use_atx_headers: bool,

    /// Whether to add blank lines between paragraphs.
    pub paragraph_spacing: bool,

    /// Whether to escape special Markdown characters in text.
    pub escape_special_chars: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            image_dir: None,
            image_path_prefix: "assets/".to_string(),
            table_fallback: TableFallback::Html,
            max_heading_level: 6,
            include_frontmatter: false,
            preserve_line_breaks: true,
            include_empty_paragraphs: false,
            list_marker: '-',
            use_atx_headers: true,
            paragraph_spacing: true,
            escape_special_chars: false,
        }
    }
}

impl RenderOptions {
    /// Creates new options with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the image output directory.
    pub fn with_image_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.image_dir = Some(dir.into());
        self
    }

    /// Sets the image path prefix for Markdown output.
    pub fn with_image_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.image_path_prefix = prefix.into();
        self
    }

    /// Sets the table fallback mode.
    pub fn with_table_fallback(mut self, fallback: TableFallback) -> Self {
        self.table_fallback = fallback;
        self
    }

    /// Enables YAML frontmatter output.
    pub fn with_frontmatter(mut self) -> Self {
        self.include_frontmatter = true;
        self
    }

    /// Disables paragraph spacing.
    pub fn without_paragraph_spacing(mut self) -> Self {
        self.paragraph_spacing = false;
        self
    }
}

/// Fallback modes for tables with merged cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableFallback {
    /// Render as HTML table with rowspan/colspan.
    Html,
    /// Render as simplified Markdown table (ignore merges).
    SimplifiedMarkdown,
    /// Skip tables with merged cells entirely.
    Skip,
}

impl Default for TableFallback {
    fn default() -> Self {
        Self::Html
    }
}
