//! Rendering options for Markdown output.

use super::heading_analyzer::HeadingConfig;
use crate::cleanup::CleanupOptions;
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
    /// Default: 4 (HWP documents often misuse deep heading levels for visual styling)
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

    /// Cleanup options for purifying output for LLM training.
    /// If None, no cleanup is performed.
    pub cleanup: Option<CleanupOptions>,

    /// Heading analysis configuration.
    /// When set, enables sophisticated heading detection with sequence analysis.
    /// If None, uses legacy inline heading detection.
    pub heading_config: Option<HeadingConfig>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            image_dir: None,
            image_path_prefix: "assets/".to_string(),
            table_fallback: TableFallback::default(),
            max_heading_level: 4,
            include_frontmatter: false,
            preserve_line_breaks: true,
            include_empty_paragraphs: false,
            list_marker: '-',
            use_atx_headers: true,
            paragraph_spacing: true,
            escape_special_chars: false,
            cleanup: None,
            // Enable statistical heading analysis by default (font-size based)
            heading_config: Some(super::heading_analyzer::HeadingConfig::default()),
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

    /// Enables cleanup with default options.
    ///
    /// This applies a 4-stage cleanup pipeline to purify markdown output:
    /// 1. String normalization (Unicode, bullets, control chars)
    /// 2. Line-based cleaning (page numbers, headers, TOC)
    /// 3. Structural filtering (empty tags)
    /// 4. Final normalization (newlines, whitespace)
    pub fn with_cleanup(mut self) -> Self {
        self.cleanup = Some(CleanupOptions::default());
        self
    }

    /// Enables cleanup with custom options.
    pub fn with_cleanup_options(mut self, options: CleanupOptions) -> Self {
        self.cleanup = Some(options);
        self
    }

    /// Enables minimal cleanup (only essential normalization).
    pub fn with_minimal_cleanup(mut self) -> Self {
        self.cleanup = Some(CleanupOptions::minimal());
        self
    }

    /// Enables aggressive cleanup (maximum purification).
    pub fn with_aggressive_cleanup(mut self) -> Self {
        self.cleanup = Some(CleanupOptions::aggressive());
        self
    }

    /// Sets the maximum heading level (1-6).
    /// Headings beyond this level will be capped to this level.
    pub fn with_max_heading_level(mut self, level: u8) -> Self {
        self.max_heading_level = level.clamp(1, 6);
        self
    }

    /// Enables sophisticated heading analysis with default config.
    ///
    /// This activates the two-pass heading analyzer that can:
    /// - Detect and demote consecutive numbered sequences to lists
    /// - Better discriminate between standalone headings and list items
    /// - Support Korean sequence patterns (가나다...)
    pub fn with_heading_analysis(mut self) -> Self {
        self.heading_config = Some(HeadingConfig::default());
        self
    }

    /// Enables sophisticated heading analysis with custom config.
    pub fn with_heading_config(mut self, config: HeadingConfig) -> Self {
        self.heading_config = Some(config);
        self
    }
}

/// Fallback modes for tables with merged cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableFallback {
    /// Render as HTML table with rowspan/colspan.
    Html,
    /// Render as simplified Markdown table (ignore merges).
    /// This is the default as it produces cleaner output for LLM training.
    #[default]
    SimplifiedMarkdown,
    /// Skip tables with merged cells entirely.
    Skip,
}
