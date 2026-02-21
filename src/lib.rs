//! # unhwp
//!
//! A high-performance Rust library for extracting HWP/HWPX Korean word processor
//! documents into structured Markdown with assets.
//!
//! ## Supported Formats
//!
//! - **HWP 5.0+**: Binary format using OLE containers (most common)
//! - **HWPX**: XML-based format using ZIP containers (modern standard)
//! - **HWP 3.x**: Legacy binary format (with `hwp3` feature)
//!
//! ## Quick Start
//!
//! ```no_run
//! use unhwp::{parse_file, RenderOptions};
//!
//! fn main() -> unhwp::Result<()> {
//!     // Parse a document
//!     let document = parse_file("document.hwp")?;
//!
//!     // Render to Markdown
//!     let options = RenderOptions::default();
//!     let markdown = unhwp::render::render_markdown(&document, &options)?;
//!
//!     println!("{}", markdown);
//!     Ok(())
//! }
//! ```
//!
//! ## Features
//!
//! - `hwp5` (default): HWP 5.0 binary format support
//! - `hwpx` (default): HWPX XML format support
//! - `hwp3`: Legacy HWP 3.x format support
//! - `async`: Async I/O support with Tokio

pub mod cleanup;
pub mod detect;
pub mod equation;
pub mod error;
pub mod model;
pub mod parse_options;
pub mod render;

#[cfg(feature = "ffi")]
pub mod ffi;

#[cfg(feature = "hwp5")]
pub mod hwp5;

#[cfg(feature = "hwpx")]
pub mod hwpx;

#[cfg(feature = "hwp3")]
pub mod hwp3;

#[cfg(feature = "async")]
pub mod async_api;

// Re-exports
pub use cleanup::{cleanup, CleanupOptions};
pub use detect::{detect_format, detect_format_from_bytes, detect_format_from_path, FormatType};
pub use error::{Error, Result};
pub use model::Document;
pub use parse_options::{ErrorMode, ExtractMode, ParseOptions};
pub use render::{RenderOptions, TableFallback};

use std::io::{Read, Seek};
use std::path::Path;

/// Parses a document from a file path.
///
/// Automatically detects the format (HWP 5.0 or HWPX) and uses the appropriate parser.
///
/// # Example
///
/// ```no_run
/// use unhwp::parse_file;
///
/// let document = parse_file("example.hwp")?;
/// println!("Paragraphs: {}", document.paragraph_count());
/// # Ok::<(), unhwp::Error>(())
/// ```
pub fn parse_file(path: impl AsRef<Path>) -> Result<Document> {
    let path = path.as_ref();
    let format = detect_format_from_path(path)?;

    match format {
        #[cfg(feature = "hwp5")]
        FormatType::Hwp5 => {
            let mut parser = hwp5::Hwp5Parser::open(path)?;
            parser.parse()
        }
        #[cfg(feature = "hwpx")]
        FormatType::Hwpx => {
            let mut parser = hwpx::HwpxParser::open(path)?;
            parser.parse()
        }
        #[cfg(feature = "hwp3")]
        FormatType::Hwp3 => {
            let mut parser = hwp3::Hwp3Parser::open(path)?;
            parser.parse()
        }
        #[cfg(not(feature = "hwp3"))]
        FormatType::Hwp3 => Err(Error::UnsupportedFormat(
            "HWP 3.x support requires 'hwp3' feature".into(),
        )),
        #[allow(unreachable_patterns)]
        _ => Err(Error::UnsupportedFormat(format.to_string())),
    }
}

/// Parses a document from a reader.
///
/// Automatically detects the format and uses the appropriate parser.
pub fn parse_reader<R: Read + Seek>(reader: R) -> Result<Document> {
    let mut buf_reader = std::io::BufReader::new(reader);
    let format = detect::detect_format(&mut buf_reader)?;

    match format {
        #[cfg(feature = "hwp5")]
        FormatType::Hwp5 => {
            let mut parser = hwp5::Hwp5Parser::from_reader(buf_reader)?;
            parser.parse()
        }
        #[cfg(feature = "hwpx")]
        FormatType::Hwpx => {
            let mut parser = hwpx::HwpxParser::from_reader(buf_reader)?;
            parser.parse()
        }
        #[cfg(feature = "hwp3")]
        FormatType::Hwp3 => {
            let mut parser = hwp3::Hwp3Parser::from_reader(buf_reader)?;
            parser.parse()
        }
        #[cfg(not(feature = "hwp3"))]
        FormatType::Hwp3 => Err(Error::UnsupportedFormat(
            "HWP 3.x support requires 'hwp3' feature".into(),
        )),
        #[allow(unreachable_patterns)]
        _ => Err(Error::UnsupportedFormat(format.to_string())),
    }
}

/// Parses a document from bytes.
///
/// Automatically detects the format and uses the appropriate parser.
pub fn parse_bytes(data: &[u8]) -> Result<Document> {
    let cursor = std::io::Cursor::new(data);
    parse_reader(cursor)
}

/// Extracts plain text from a document file.
///
/// This is a convenience function for when you only need the text content
/// without formatting or structure.
///
/// # Example
///
/// ```no_run
/// use unhwp::extract_text;
///
/// let text = extract_text("document.hwp")?;
/// println!("{}", text);
/// # Ok::<(), unhwp::Error>(())
/// ```
pub fn extract_text(path: impl AsRef<Path>) -> Result<String> {
    let document = parse_file(path)?;
    Ok(document.plain_text())
}

/// Converts a document to Markdown with default options.
///
/// # Example
///
/// ```no_run
/// use unhwp::to_markdown;
///
/// let markdown = to_markdown("document.hwp")?;
/// std::fs::write("output.md", markdown)?;
/// # Ok::<(), unhwp::Error>(())
/// ```
pub fn to_markdown(path: impl AsRef<Path>) -> Result<String> {
    let document = parse_file(path)?;
    render::render_markdown(&document, &RenderOptions::default())
}

/// Converts a document to Markdown with custom options.
///
/// # Example
///
/// ```no_run
/// use unhwp::{to_markdown_with_options, RenderOptions, TableFallback};
///
/// let options = RenderOptions::default()
///     .with_image_dir("./images")
///     .with_table_fallback(TableFallback::Html)
///     .with_frontmatter();
///
/// let markdown = to_markdown_with_options("document.hwp", &options)?;
/// std::fs::write("output.md", markdown)?;
/// # Ok::<(), unhwp::Error>(())
/// ```
pub fn to_markdown_with_options(path: impl AsRef<Path>, options: &RenderOptions) -> Result<String> {
    let document = parse_file(path)?;
    render::render_markdown(&document, options)
}

/// Builder for parsing and rendering documents.
///
/// Provides a fluent API for configuring document processing.
///
/// # Example
///
/// ```no_run
/// use unhwp::Unhwp;
///
/// let markdown = Unhwp::new()
///     .with_images(true)
///     .with_image_dir("./assets")
///     .parse("document.hwp")?
///     .to_markdown()?;
/// # Ok::<(), unhwp::Error>(())
/// ```
pub struct Unhwp {
    render_options: RenderOptions,
    parse_options: ParseOptions,
    extract_images: bool,
}

impl Default for Unhwp {
    fn default() -> Self {
        Self::new()
    }
}

impl Unhwp {
    /// Creates a new Unhwp builder with default settings.
    pub fn new() -> Self {
        Self {
            render_options: RenderOptions::default(),
            parse_options: ParseOptions::default(),
            extract_images: false,
        }
    }

    /// Enables image extraction.
    pub fn with_images(mut self, extract: bool) -> Self {
        self.extract_images = extract;
        self
    }

    /// Sets the directory for extracted images.
    pub fn with_image_dir(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.render_options.image_dir = Some(dir.into());
        self
    }

    /// Sets the table fallback mode.
    pub fn with_table_fallback(mut self, fallback: TableFallback) -> Self {
        self.render_options.table_fallback = fallback;
        self
    }

    /// Enables YAML frontmatter in output.
    pub fn with_frontmatter(mut self) -> Self {
        self.render_options.include_frontmatter = true;
        self
    }

    /// Sets lenient error handling (skip invalid sections).
    pub fn lenient(mut self) -> Self {
        self.parse_options = self.parse_options.lenient();
        self
    }

    /// Extracts only text content (faster, smaller output).
    pub fn text_only(mut self) -> Self {
        self.parse_options = self.parse_options.text_only();
        self
    }

    /// Sets memory limit in megabytes.
    pub fn with_memory_limit_mb(mut self, mb: usize) -> Self {
        self.parse_options = self.parse_options.with_memory_limit_mb(mb);
        self
    }

    /// Disables parallel processing.
    pub fn sequential(mut self) -> Self {
        self.parse_options = self.parse_options.sequential();
        self
    }

    /// Parses a document from a file path.
    pub fn parse(self, path: impl AsRef<Path>) -> Result<ParsedDocument> {
        let document = parse_file(path)?;
        Ok(ParsedDocument {
            document,
            render_options: self.render_options,
            extract_images: self.extract_images,
        })
    }
}

/// A parsed document ready for rendering.
pub struct ParsedDocument {
    document: Document,
    render_options: RenderOptions,
    extract_images: bool,
}

impl ParsedDocument {
    /// Returns a reference to the underlying document.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Renders the document to Markdown.
    pub fn to_markdown(&self) -> Result<String> {
        // Extract images if requested
        if self.extract_images {
            if let Some(ref image_dir) = self.render_options.image_dir {
                std::fs::create_dir_all(image_dir)?;

                for (name, resource) in &self.document.resources {
                    let path = image_dir.join(name);
                    std::fs::write(path, &resource.data)?;
                }
            }
        }

        render::render_markdown(&self.document, &self.render_options)
    }

    /// Returns the plain text content.
    pub fn to_text(&self) -> String {
        self.document.plain_text()
    }

    /// Returns the number of sections in the document.
    pub fn section_count(&self) -> usize {
        self.document.sections.len()
    }

    /// Returns the number of paragraphs in the document.
    pub fn paragraph_count(&self) -> usize {
        self.document.paragraph_count()
    }

    /// Consumes self and returns the underlying document.
    pub fn into_document(self) -> Document {
        self.document
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_detection_hwp5() {
        let data = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0x00, 0x00];
        let format = detect_format_from_bytes(&data).unwrap();
        assert_eq!(format, FormatType::Hwp5);
    }

    #[test]
    fn test_format_detection_hwpx() {
        let data = [0x50, 0x4B, 0x03, 0x04, 0x00, 0x00, 0x00, 0x00];
        let format = detect_format_from_bytes(&data).unwrap();
        assert_eq!(format, FormatType::Hwpx);
    }

    #[test]
    fn test_render_options_builder() {
        let options = RenderOptions::default()
            .with_image_dir("./images")
            .with_table_fallback(TableFallback::Html)
            .with_frontmatter();

        assert!(options.image_dir.is_some());
        assert_eq!(options.table_fallback, TableFallback::Html);
        assert!(options.include_frontmatter);
    }

    // ==================== Edge Case Tests ====================

    #[test]
    fn test_format_detection_empty_data() {
        // Empty data should return InvalidData error (data too small)
        let data: [u8; 0] = [];
        let result = detect_format_from_bytes(&data);
        assert!(result.is_err());
        match result {
            Err(Error::InvalidData(_)) => {} // Expected: data too small
            _ => panic!("Expected InvalidData error for empty data"),
        }
    }

    #[test]
    fn test_format_detection_too_short() {
        // Data shorter than magic bytes should return InvalidData error
        let data = [0xD0, 0xCF]; // Incomplete OLE magic (less than 8 bytes)
        let result = detect_format_from_bytes(&data);
        assert!(result.is_err());
        match result {
            Err(Error::InvalidData(_)) => {} // Expected: data too small
            _ => panic!("Expected InvalidData error for data too short"),
        }
    }

    #[test]
    fn test_format_detection_unknown_magic() {
        // Random bytes that don't match any known format
        let data = [0xFF, 0xFE, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        let result = detect_format_from_bytes(&data);
        assert!(result.is_err());
        match result {
            Err(Error::UnknownFormat) => {}
            _ => panic!("Expected UnknownFormat error for unknown magic bytes"),
        }
    }

    #[test]
    fn test_format_detection_hwp3_signature() {
        // HWP 3.x signature: "HWP Document File V"
        let data = b"HWP Document File V3.00 \x1A\x01\x02\x03\x04";
        let format = detect_format_from_bytes(data).unwrap();
        assert_eq!(format, FormatType::Hwp3);
    }

    #[test]
    fn test_render_options_empty_image_dir() {
        // Empty string for image_dir should still work
        use std::path::PathBuf;
        let options = RenderOptions::default().with_image_dir("");
        assert_eq!(options.image_dir, Some(PathBuf::from("")));
    }

    #[test]
    fn test_render_options_special_chars_in_path() {
        // Paths with special characters (Korean, spaces, etc.)
        use std::path::PathBuf;
        let options = RenderOptions::default().with_image_dir("./이미지 폴더/test images");
        assert_eq!(
            options.image_dir,
            Some(PathBuf::from("./이미지 폴더/test images"))
        );
    }

    #[test]
    fn test_document_model_empty() {
        // Empty document should have valid defaults
        let doc = model::Document::new();
        assert!(doc.sections.is_empty());
        assert!(doc.resources.is_empty());
        assert!(doc.metadata.title.is_none());
        assert!(!doc.metadata.is_distribution);
    }

    #[test]
    fn test_table_fallback_variants() {
        // All TableFallback variants should be usable
        let options_html = RenderOptions::default().with_table_fallback(TableFallback::Html);
        let options_simplified =
            RenderOptions::default().with_table_fallback(TableFallback::SimplifiedMarkdown);
        let options_skip = RenderOptions::default().with_table_fallback(TableFallback::Skip);

        assert_eq!(options_html.table_fallback, TableFallback::Html);
        assert_eq!(
            options_simplified.table_fallback,
            TableFallback::SimplifiedMarkdown
        );
        assert_eq!(options_skip.table_fallback, TableFallback::Skip);
    }

    #[test]
    fn test_paragraph_style_heading_level() {
        // Test ParagraphStyle heading level clamping
        use model::ParagraphStyle;

        // Valid heading levels
        let h1 = ParagraphStyle::heading(1);
        assert_eq!(h1.heading_level, 1);
        assert!(h1.is_heading());

        let h6 = ParagraphStyle::heading(6);
        assert_eq!(h6.heading_level, 6);

        // Out of bounds should clamp to 6
        let h_overflow = ParagraphStyle::heading(10);
        assert_eq!(h_overflow.heading_level, 6);

        // Zero is not a heading
        let normal = ParagraphStyle::heading(0);
        assert_eq!(normal.heading_level, 0);
        assert!(!normal.is_heading());
    }

    #[test]
    fn test_resource_type_variants() {
        // All ResourceType variants should work
        use model::{Resource, ResourceType};

        let image_resource = Resource {
            resource_type: ResourceType::Image,
            filename: Some("test.png".to_string()),
            mime_type: Some("image/png".to_string()),
            data: vec![0x89, 0x50, 0x4E, 0x47], // PNG magic
            size: 4,
        };

        let ole_resource = Resource {
            resource_type: ResourceType::OleObject,
            filename: Some("object.ole".to_string()),
            mime_type: None,
            data: vec![0xD0, 0xCF, 0x11, 0xE0],
            size: 4,
        };

        let other_resource = Resource::new(ResourceType::Other, vec![0x00, 0x01]);

        assert!(matches!(image_resource.resource_type, ResourceType::Image));
        assert!(matches!(
            ole_resource.resource_type,
            ResourceType::OleObject
        ));
        assert!(matches!(other_resource.resource_type, ResourceType::Other));
    }

    #[test]
    fn test_style_registry_empty() {
        // Empty style registry should handle lookups gracefully
        let registry = model::StyleRegistry::new();

        // Looking up non-existent styles should return None
        assert!(registry.get_char_style(0).is_none());
        assert!(registry.get_para_style(0).is_none());
        assert!(registry.get_bindata_filename(0).is_none());
        assert!(registry.get_named_style(0).is_none());
        assert!(registry.get_named_style_by_name("NonExistent").is_none());
    }

    #[test]
    fn test_paragraph_default() {
        // Default paragraph should have sensible defaults
        let para = model::Paragraph::default();
        assert!(para.content.is_empty());
        assert!(!para.style.is_heading());
        assert!(para.is_empty());
    }

    #[test]
    fn test_text_run_with_unicode() {
        // Text runs should handle Unicode correctly
        let run = model::TextRun::new("한글 테스트 🎉 Unicode");

        assert!(run.text.contains("한글"));
        assert!(run.text.contains("🎉"));
        assert!(!run.is_empty());
    }

    #[test]
    fn test_text_run_with_style() {
        // TextRun with custom style
        use model::{TextRun, TextStyle};

        let style = TextStyle {
            bold: true,
            italic: true,
            font_size: Some(14.0),
            ..Default::default()
        };
        let run = TextRun::with_style("Formatted text", style);

        assert_eq!(run.text, "Formatted text");
        assert!(run.style.bold);
        assert!(run.style.italic);
        assert_eq!(run.style.font_size, Some(14.0));
    }

    #[test]
    fn test_section_new() {
        // Section should be created with correct index
        let section = model::Section::new(42);

        assert_eq!(section.index, 42);
        assert!(section.content.is_empty());
        assert!(section.header.is_none());
        assert!(section.footer.is_none());
    }

    #[test]
    fn test_resource_extension() {
        // Resource should return correct extension based on MIME type
        use model::{Resource, ResourceType};

        let png = Resource::image(vec![], "image/png");
        assert_eq!(png.extension(), "png");

        let jpg = Resource::image(vec![], "image/jpeg");
        assert_eq!(jpg.extension(), "jpg");

        let unknown = Resource::new(ResourceType::Image, vec![]);
        assert_eq!(unknown.extension(), "bin"); // No MIME type

        let ole = Resource::new(ResourceType::OleObject, vec![]);
        assert_eq!(ole.extension(), "ole");
    }

    #[test]
    fn test_text_style_has_formatting() {
        // TextStyle should detect formatting presence
        use model::TextStyle;

        let plain = TextStyle::default();
        assert!(!plain.has_formatting());

        let bold = TextStyle::bold();
        assert!(bold.has_formatting());

        let italic = TextStyle::italic();
        assert!(italic.has_formatting());

        let complex = TextStyle {
            underline: true,
            strikethrough: true,
            ..Default::default()
        };
        assert!(complex.has_formatting());
    }

    #[test]
    fn test_cleanup_options_presets() {
        // CleanupOptions presets should work
        use crate::cleanup::CleanupOptions;

        let default = CleanupOptions::default();
        assert!(default.normalize_strings);
        assert!(default.clean_lines);
        assert!(default.filter_structure);

        let minimal = CleanupOptions::minimal();
        assert!(minimal.normalize_strings);
        assert!(!minimal.clean_lines); // Minimal disables line cleaning
        assert!(!minimal.filter_structure); // Minimal disables structure filtering

        let aggressive = CleanupOptions::aggressive();
        assert!(aggressive.normalize_strings);
        assert!(aggressive.clean_lines);
        assert!(aggressive.filter_structure);
        // Aggressive has lower threshold for more aggressive header/footer detection
        assert!(aggressive.header_footer_threshold < default.header_footer_threshold);
    }

    #[test]
    fn test_render_options_cleanup_chain() {
        // RenderOptions cleanup builder methods
        let with_cleanup = RenderOptions::default().with_cleanup();
        assert!(with_cleanup.cleanup.is_some());

        let with_minimal = RenderOptions::default().with_minimal_cleanup();
        assert!(with_minimal.cleanup.is_some());

        let with_aggressive = RenderOptions::default().with_aggressive_cleanup();
        assert!(with_aggressive.cleanup.is_some());
    }

    #[test]
    fn test_render_options_max_heading_level() {
        // Max heading level should be clamped to 1-6
        let level_0 = RenderOptions::default().with_max_heading_level(0);
        assert_eq!(level_0.max_heading_level, 1); // Clamped to minimum

        let level_10 = RenderOptions::default().with_max_heading_level(10);
        assert_eq!(level_10.max_heading_level, 6); // Clamped to maximum

        let level_4 = RenderOptions::default().with_max_heading_level(4);
        assert_eq!(level_4.max_heading_level, 4); // Within range
    }

    #[test]
    fn test_paragraph_plain_text_extraction() {
        // Paragraph should extract plain text from mixed content
        use model::{InlineContent, Paragraph, TextRun};

        let mut para = Paragraph::new();
        para.content
            .push(InlineContent::Text(TextRun::new("Hello ")));
        para.content.push(InlineContent::LineBreak);
        para.content
            .push(InlineContent::Text(TextRun::new("World")));
        para.content.push(InlineContent::Link {
            text: " Link".to_string(),
            url: "https://example.com".to_string(),
        });

        let text = para.plain_text();
        assert!(text.contains("Hello"));
        assert!(text.contains("\n")); // Line break
        assert!(text.contains("World"));
        assert!(text.contains("Link"));
    }
}
