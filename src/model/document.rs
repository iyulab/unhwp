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
    /// Indices of sections that could not be read or parsed and were left out.
    ///
    /// Always empty under the default [`ErrorMode::Strict`], where an unreadable section
    /// fails the whole document. Under [`ErrorMode::Lenient`] the section is skipped and its
    /// index is recorded here, so a caller can tell a document that really has three sections
    /// from one that had five and lost two. Without it `sections.len()` is the same number
    /// either way, and the loss is invisible -- the streaming API has always reported it as
    /// [`ParseEvent::SectionFailed`], and this is the batch API saying the same thing.
    ///
    /// [`ErrorMode::Strict`]: crate::ErrorMode::Strict
    /// [`ErrorMode::Lenient`]: crate::ErrorMode::Lenient
    /// [`ParseEvent::SectionFailed`]: crate::ParseEvent::SectionFailed
    // Serialised even when empty. A consumer reading JSON has to be able to tell "nothing was
    // lost" from "this build does not report losses" -- and an absent key says both.
    #[serde(default)]
    pub skipped_sections: Vec<usize>,
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
            .map(|s| {
                s.content
                    .iter()
                    .filter(|b| matches!(b, Block::Paragraph(_)))
                    .count()
            })
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

    /// Every table of the document in reading order, with where it is: the section's number
    /// (from 1) and the table's place among that section's tables (from 1) — the names
    /// `unhwp tables` writes them under (`s<section>-t<n>`).
    pub fn tables(&self) -> impl Iterator<Item = (usize, usize, &Table)> {
        self.sections.iter().enumerate().flat_map(|(s, section)| {
            section
                .content
                .iter()
                .filter_map(|block| match block {
                    Block::Table(table) => Some(table),
                    _ => None,
                })
                .enumerate()
                .map(move |(t, table)| (s + 1, t + 1, table))
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
    /// Distribution document flag
    #[serde(default)]
    pub is_distribution: bool,
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
    /// Binary data (excluded from JSON serialization - extracted to separate files)
    #[serde(skip)]
    pub data: Vec<u8>,
    /// Size of binary data in bytes
    pub size: usize,
}

impl Resource {
    /// Creates a new resource.
    pub fn new(resource_type: ResourceType, data: Vec<u8>) -> Self {
        let size = data.len();
        Self {
            resource_type,
            filename: None,
            mime_type: None,
            data,
            size,
        }
    }

    /// Creates an image resource.
    pub fn image(data: Vec<u8>, mime_type: impl Into<String>) -> Self {
        let size = data.len();
        Self {
            resource_type: ResourceType::Image,
            filename: None,
            mime_type: Some(mime_type.into()),
            data,
            size,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{TableCell, TableRow};

    /// A one-cell table holding `text`.
    fn table(text: &str) -> Table {
        let mut row = TableRow::new();
        row.cells.push(TableCell::text(text));
        let mut table = Table::new();
        table.rows.push(row);
        table
    }

    #[test]
    fn tables_are_numbered_per_section_in_reading_order() {
        let mut doc = Document::new();
        let mut first = Section::new(0);
        first.push_table(table("a"));
        first.push_paragraph(Paragraph::text("between"));
        first.push_table(table("b"));
        doc.sections.push(first);
        doc.sections.push(Section::new(1));
        let mut third = Section::new(2);
        third.push_table(table("c"));
        doc.sections.push(third);

        let found: Vec<(usize, usize, String)> = doc
            .tables()
            .map(|(s, t, table)| (s, t, table.rows[0].cells[0].plain_text()))
            .collect();
        let expected =
            [(1, 1, "a"), (1, 2, "b"), (3, 1, "c")].map(|(s, t, text)| (s, t, text.to_string()));
        assert_eq!(found, expected);
        assert_eq!(Document::new().tables().count(), 0);
    }
}
