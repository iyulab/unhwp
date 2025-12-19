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
    fn parse_bodytext(&self, document: &mut Document) -> Result<()> {
        let section_names = self.container.list_bodytext_sections()?;

        for (index, name) in section_names.iter().enumerate() {
            let data = self.container.read_stream_decompressed(
                name,
                self.is_compressed(),
            )?;

            let section = bodytext::parse_section(&data, index, &document.styles)?;
            document.sections.push(section);
        }

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
