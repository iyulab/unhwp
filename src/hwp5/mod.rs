//! HWP 5.0 binary format parser.
//!
//! This module handles parsing of HWP 5.0 documents stored in OLE containers.

mod container;
mod header;
mod record;
mod docinfo;
mod bodytext;
mod control;

pub use container::Hwp5Container;
pub use header::FileHeader;
pub use record::{Record, RecordHeader, RecordIterator, TagId};

use crate::error::Result;
use crate::model::Document;
use rayon::prelude::*;
use std::io::{Read, Seek};
use std::path::Path;

/// HWP 5.0 document parser.
pub struct Hwp5Parser {
    container: Hwp5Container,
    header: FileHeader,
}

impl Hwp5Parser {
    /// Opens an HWP 5.0 document from a file path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let container = Hwp5Container::open(path)?;
        let header = container.read_file_header()?;
        Ok(Self { container, header })
    }

    /// Opens an HWP 5.0 document from a reader.
    pub fn from_reader<R: Read + Seek>(reader: R) -> Result<Self> {
        let container = Hwp5Container::from_reader(reader)?;
        let header = container.read_file_header()?;
        Ok(Self { container, header })
    }

    /// Returns the file header.
    pub fn header(&self) -> &FileHeader {
        &self.header
    }

    /// Returns true if the document is compressed.
    pub fn is_compressed(&self) -> bool {
        self.header.is_compressed()
    }

    /// Returns true if the document is encrypted.
    pub fn is_encrypted(&self) -> bool {
        self.header.is_encrypted()
    }

    /// Parses the document into the unified document model.
    pub fn parse(&mut self) -> Result<Document> {
        if self.is_encrypted() {
            return Err(crate::error::Error::Encrypted);
        }

        let mut document = Document::new();

        // Set format version
        document.metadata.format_version = Some(self.header.version_string());

        // Parse DocInfo for styles
        self.parse_docinfo(&mut document)?;

        // Parse BodyText sections
        self.parse_bodytext(&mut document)?;

        // Extract BinData resources
        self.extract_bindata(&mut document)?;

        Ok(document)
    }

    /// Parses DocInfo stream for style definitions.
    fn parse_docinfo(&self, document: &mut Document) -> Result<()> {
        let data = self.container.read_stream_decompressed(
            "DocInfo",
            self.is_compressed(),
        )?;

        docinfo::parse_docinfo(&data, &mut document.styles)?;
        Ok(())
    }

    /// Parses BodyText sections.
    ///
    /// Uses parallel processing when there are multiple sections.
    fn parse_bodytext(&self, document: &mut Document) -> Result<()> {
        let section_names = self.container.list_bodytext_sections()?;
        let is_compressed = self.is_compressed();

        // Read all section data first
        let section_data: Vec<(usize, Vec<u8>)> = section_names
            .iter()
            .enumerate()
            .filter_map(|(index, name)| {
                self.container
                    .read_stream_decompressed(name, is_compressed)
                    .ok()
                    .map(|data| (index, data))
            })
            .collect();

        // Clone styles for parallel access
        let styles = document.styles.clone();

        // Parse sections in parallel
        let mut sections: Vec<_> = section_data
            .par_iter()
            .filter_map(|(index, data)| {
                bodytext::parse_section(data, *index, &styles).ok()
            })
            .collect();

        // Sort by index to maintain order
        sections.sort_by_key(|s| s.index);

        document.sections = sections;
        Ok(())
    }

    /// Extracts binary resources from BinData storage.
    fn extract_bindata(&self, document: &mut Document) -> Result<()> {
        let resources = self.container.list_bindata()?;

        for name in resources {
            if let Ok(data) = self.container.read_bindata(&name, self.is_compressed()) {
                let resource = crate::model::Resource::new(
                    crate::model::ResourceType::Image,
                    data,
                );
                document.resources.insert(name, resource);
            }
        }

        Ok(())
    }
}
