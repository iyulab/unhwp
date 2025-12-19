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

pub mod detect;
pub mod error;
pub mod model;
pub mod parse_options;
pub mod render;

#[cfg(feature = "hwp5")]
pub mod hwp5;

#[cfg(feature = "hwpx")]
pub mod hwpx;

#[cfg(feature = "hwp3")]
pub mod hwp3;

#[cfg(feature = "async")]
pub mod async_api;

// Re-exports
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
pub fn to_markdown_with_options(
    path: impl AsRef<Path>,
    options: &RenderOptions,
) -> Result<String> {
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
}
