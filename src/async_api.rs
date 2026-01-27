//! Async API for non-blocking document processing.
//!
//! Enable the `async` feature to use these APIs:
//!
//! ```toml
//! [dependencies]
//! unhwp = { version = "0.1", features = ["async"] }
//! ```

use crate::error::Result;
use crate::model::Document;
use crate::render::RenderOptions;
use crate::{FormatType, ParseOptions};
use std::path::Path;
use tokio::fs;
use tokio::io::{AsyncRead, AsyncReadExt};

/// Asynchronously parses a document from a file path.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> unhwp::Result<()> {
/// let document = unhwp::async_api::parse_file("document.hwp").await?;
/// println!("Paragraphs: {}", document.paragraph_count());
/// # Ok(())
/// # }
/// ```
pub async fn parse_file(path: impl AsRef<Path>) -> Result<Document> {
    let data = fs::read(path).await?;
    parse_bytes(&data).await
}

/// Asynchronously parses a document from bytes.
pub async fn parse_bytes(data: &[u8]) -> Result<Document> {
    // Async wrapper around sync parsing
    // The actual parsing is CPU-bound, so we spawn it in a blocking task
    let data = data.to_vec();
    tokio::task::spawn_blocking(move || crate::parse_bytes(&data))
        .await
        .map_err(|e| crate::error::Error::Io(std::io::Error::other(e.to_string())))?
}

/// Asynchronously parses a document from an async reader.
pub async fn parse_reader<R: AsyncRead + Unpin>(mut reader: R) -> Result<Document> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data).await?;
    parse_bytes(&data).await
}

/// Asynchronously extracts plain text from a document.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> unhwp::Result<()> {
/// let text = unhwp::async_api::extract_text("document.hwp").await?;
/// println!("{}", text);
/// # Ok(())
/// # }
/// ```
pub async fn extract_text(path: impl AsRef<Path>) -> Result<String> {
    let document = parse_file(path).await?;
    Ok(document.plain_text())
}

/// Asynchronously converts a document to Markdown.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> unhwp::Result<()> {
/// let markdown = unhwp::async_api::to_markdown("document.hwp").await?;
/// tokio::fs::write("output.md", markdown).await?;
/// # Ok(())
/// # }
/// ```
pub async fn to_markdown(path: impl AsRef<Path>) -> Result<String> {
    let document = parse_file(path).await?;
    crate::render::render_markdown(&document, &RenderOptions::default())
}

/// Asynchronously converts a document to Markdown with custom options.
pub async fn to_markdown_with_options(
    path: impl AsRef<Path>,
    options: &RenderOptions,
) -> Result<String> {
    let document = parse_file(path).await?;
    let options = options.clone();
    tokio::task::spawn_blocking(move || crate::render::render_markdown(&document, &options))
        .await
        .map_err(|e| crate::error::Error::Io(std::io::Error::other(e.to_string())))?
}

/// Asynchronously detects the format of a file.
pub async fn detect_format(path: impl AsRef<Path>) -> Result<FormatType> {
    let data = fs::read(path).await?;
    crate::detect_format_from_bytes(&data)
}

/// Async builder for document processing.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> unhwp::Result<()> {
/// use unhwp::async_api::AsyncUnhwp;
///
/// let markdown = AsyncUnhwp::new()
///     .with_frontmatter()
///     .parse("document.hwp")
///     .await?
///     .to_markdown()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct AsyncUnhwp {
    render_options: RenderOptions,
    parse_options: ParseOptions,
    extract_images: bool,
}

impl Default for AsyncUnhwp {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncUnhwp {
    /// Creates a new async builder with default settings.
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

    /// Sets the image output directory.
    pub fn with_image_dir(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.render_options.image_dir = Some(dir.into());
        self
    }

    /// Enables YAML frontmatter.
    pub fn with_frontmatter(mut self) -> Self {
        self.render_options.include_frontmatter = true;
        self
    }

    /// Sets lenient error handling.
    pub fn lenient(mut self) -> Self {
        self.parse_options = self.parse_options.lenient();
        self
    }

    /// Parses a document asynchronously.
    pub async fn parse(self, path: impl AsRef<Path>) -> Result<AsyncParsedDocument> {
        let document = parse_file(path).await?;
        Ok(AsyncParsedDocument {
            document,
            render_options: self.render_options,
            extract_images: self.extract_images,
        })
    }
}

/// An asynchronously parsed document.
pub struct AsyncParsedDocument {
    document: Document,
    render_options: RenderOptions,
    extract_images: bool,
}

impl AsyncParsedDocument {
    /// Returns a reference to the document.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Renders to Markdown asynchronously.
    pub async fn to_markdown(&self) -> Result<String> {
        // Extract images if requested
        if self.extract_images {
            if let Some(ref image_dir) = self.render_options.image_dir {
                fs::create_dir_all(image_dir).await?;

                for (name, resource) in &self.document.resources {
                    let path = image_dir.join(name);
                    fs::write(path, &resource.data).await?;
                }
            }
        }

        let document = self.document.clone();
        let options = self.render_options.clone();

        tokio::task::spawn_blocking(move || crate::render::render_markdown(&document, &options))
            .await
            .map_err(|e| crate::error::Error::Io(std::io::Error::other(e.to_string())))?
    }

    /// Returns the plain text content.
    pub fn to_text(&self) -> String {
        self.document.plain_text()
    }

    /// Consumes self and returns the document.
    pub fn into_document(self) -> Document {
        self.document
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_format_bytes() {
        // Need at least 8 bytes for format detection
        let hwpx_magic = [0x50, 0x4B, 0x03, 0x04, 0x00, 0x00, 0x00, 0x00];
        let format = crate::detect_format_from_bytes(&hwpx_magic).unwrap();
        assert_eq!(format, FormatType::Hwpx);
    }
}
