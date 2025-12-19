//! Document structure and metadata.

use super::{Paragraph, StyleRegistry, Table};
use serde::Serialize;
use std::collections::HashMap;

/// A complete document parsed from HWP/HWPX.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Document {
    /// Document metadata
    pub metadata: Metadata,
    /// Document sections
    pub sections: Vec<Section>,
    /// Style registry
    pub styles: StyleRegistry,
    /// Binary resources (images, etc.) keyed by ID
    pub resources: HashMap<String, Resource>,
}

impl Document {
    /// Creates a new empty document.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the total number of paragraphs in the document.
    pub fn paragraph_count(&self) -> usize {
        self.sections
            .iter()
            .map(|s| s.content.iter().filter(|b| matches!(b, Block::Paragraph(_))).count())
            .sum()
    }

    /// Returns an iterator over all paragraphs in the document.
    pub fn paragraphs(&self) -> impl Iterator<Item = &Paragraph> {
        self.sections.iter().flat_map(|s| {
            s.content.iter().filter_map(|b| match b {
                Block::Paragraph(p) => Some(p),
                _ => None,
            })
        })
    }

    /// Returns the plain text content of the entire document.
    pub fn plain_text(&self) -> String {
        let mut result = Vec::new();
        for section in &self.sections {
            for block in &section.content {
                match block {
                    Block::Paragraph(p) => result.push(p.plain_text()),
                    Block::Table(t) => {
                        for row in &t.rows {
                            for cell in &row.cells {
                                result.push(cell.plain_text());
                            }
                        }
                    }
                }
            }
        }
        result.join("\n")
    }

    /// Returns structured content as JSON with full metadata.
    ///
    /// This provides access to the full document structure including:
    /// - Document metadata (title, author, dates)
    /// - Paragraph styles (heading level, alignment, list type)
    /// - Text formatting (bold, italic, underline, font, color, etc.)
    /// - Table structure (rows, cells, colspan, rowspan)
    /// - Equations, images, and links
    ///
    /// The output is valid JSON that can be parsed by any JSON library.
    pub fn raw_content(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Document metadata.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Metadata {
    /// Document title
    pub title: Option<String>,
    /// Document author
    pub author: Option<String>,
    /// Document subject
    pub subject: Option<String>,
    /// Keywords
    pub keywords: Vec<String>,
    /// Creation date (ISO 8601 format)
    pub created: Option<String>,
    /// Last modified date (ISO 8601 format)
    pub modified: Option<String>,
    /// Application that created the document
    pub creator_app: Option<String>,
    /// HWP/HWPX version
    pub format_version: Option<String>,
}

/// A section of the document.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Section {
    /// Section index (0-based)
    pub index: usize,
    /// Content blocks in this section
    pub content: Vec<Block>,
    /// Header content (optional)
    pub header: Option<Vec<Paragraph>>,
    /// Footer content (optional)
    pub footer: Option<Vec<Paragraph>>,
}

impl Section {
    /// Creates a new empty section.
    pub fn new(index: usize) -> Self {
        Self {
            index,
            content: Vec::new(),
            header: None,
            footer: None,
        }
    }

    /// Adds a paragraph to this section.
    pub fn push_paragraph(&mut self, paragraph: Paragraph) {
        self.content.push(Block::Paragraph(paragraph));
    }

    /// Adds a table to this section.
    pub fn push_table(&mut self, table: Table) {
        self.content.push(Block::Table(table));
    }
}

/// A block-level content element.
#[derive(Debug, Clone, Serialize)]
pub enum Block {
    /// A paragraph
    Paragraph(Paragraph),
    /// A table
    Table(Table),
}

/// A binary resource (image, OLE object, etc.).
#[derive(Debug, Clone, Serialize)]
pub struct Resource {
    /// Resource type
    pub resource_type: ResourceType,
    /// Original filename (if known)
    pub filename: Option<String>,
    /// MIME type (if known)
    pub mime_type: Option<String>,
    /// Binary data
    pub data: Vec<u8>,
}

impl Resource {
    /// Creates a new resource.
    pub fn new(resource_type: ResourceType, data: Vec<u8>) -> Self {
        Self {
            resource_type,
            filename: None,
            mime_type: None,
            data,
        }
    }

    /// Creates an image resource.
    pub fn image(data: Vec<u8>, mime_type: impl Into<String>) -> Self {
        Self {
            resource_type: ResourceType::Image,
            filename: None,
            mime_type: Some(mime_type.into()),
            data,
        }
    }

    /// Returns the file extension based on MIME type.
    pub fn extension(&self) -> &str {
        match self.mime_type.as_deref() {
            Some("image/png") => "png",
            Some("image/jpeg") | Some("image/jpg") => "jpg",
            Some("image/gif") => "gif",
            Some("image/bmp") => "bmp",
            Some("image/webp") => "webp",
            Some("image/svg+xml") => "svg",
            _ => match self.resource_type {
                ResourceType::Image => "bin",
                ResourceType::OleObject => "ole",
                ResourceType::Other => "bin",
            },
        }
    }
}

/// Type of binary resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ResourceType {
    /// Image file (PNG, JPEG, etc.)
    Image,
    /// Embedded OLE object
    OleObject,
    /// Other binary data
    Other,
}
