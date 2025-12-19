//! HWPX (OWPML) XML format parser.
//!
//! HWPX files are ZIP archives containing XML documents following the
//! KS X 6101 OWPML standard.

mod container;
mod section;
mod styles;

pub use container::HwpxContainer;

use crate::error::{Error, Result};
use crate::model::Document;
use std::io::{Read, Seek};
use std::path::Path;

/// HWPX XML namespaces.
pub mod ns {
    /// Hancom Paragraph namespace
    pub const HP: &str = "http://www.hancom.co.kr/hwpml/2011/paragraph";
    /// Hancom Core namespace
    pub const HC: &str = "http://www.hancom.co.kr/hwpml/2011/core";
    /// Hancom Head namespace
    pub const HH: &str = "http://www.hancom.co.kr/hwpml/2011/head";
    /// Hancom Master namespace
    pub const HM: &str = "http://www.hancom.co.kr/hwpml/2011/master";
}

/// HWPX document parser.
pub struct HwpxParser {
    container: HwpxContainer,
}

impl HwpxParser {
    /// Opens an HWPX document from a file path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let container = HwpxContainer::open(path)?;
        Ok(Self { container })
    }

    /// Opens an HWPX document from a reader.
    pub fn from_reader<R: Read + Seek>(reader: R) -> Result<Self> {
        let container = HwpxContainer::from_reader(reader)?;
        Ok(Self { container })
    }

    /// Parses the document into the unified document model.
    pub fn parse(&mut self) -> Result<Document> {
        let mut document = Document::new();

        // Set format info
        document.metadata.format_version = Some("HWPX".to_string());

        // Parse metadata from content.hpf
        self.parse_metadata(&mut document)?;

        // Parse styles
        self.parse_styles(&mut document)?;

        // Parse sections
        self.parse_sections(&mut document)?;

        // Extract resources
        self.extract_resources(&mut document)?;

        Ok(document)
    }

    /// Parses document metadata from content.hpf.
    fn parse_metadata(&mut self, document: &mut Document) -> Result<()> {
        let content_hpf = self.container.read_content_hpf()?;

        // Parse basic metadata from content.hpf
        // Title, author, etc. are in the opf:metadata element

        if let Some(title) = extract_metadata_field(&content_hpf, "title") {
            document.metadata.title = Some(title);
        }
        if let Some(author) = extract_metadata_field(&content_hpf, "creator") {
            document.metadata.author = Some(author);
        }

        Ok(())
    }

    /// Parses styles from header.xml or section header.
    fn parse_styles(&mut self, document: &mut Document) -> Result<()> {
        if let Ok(styles_xml) = self.container.read_file("Contents/header.xml") {
            styles::parse_styles(&styles_xml, &mut document.styles)?;
        }
        Ok(())
    }

    /// Parses all sections.
    fn parse_sections(&mut self, document: &mut Document) -> Result<()> {
        let section_files = self.container.list_sections()?;

        for (index, section_path) in section_files.iter().enumerate() {
            let section_xml = self.container.read_file(section_path)?;
            let section = section::parse_section(&section_xml, index, &document.styles)?;
            document.sections.push(section);
        }

        Ok(())
    }

    /// Extracts binary resources from BinData folder.
    fn extract_resources(&mut self, document: &mut Document) -> Result<()> {
        let resources = self.container.list_bindata()?;

        for resource_path in resources {
            if let Ok(data) = self.container.read_binary(&resource_path) {
                let filename = resource_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&resource_path)
                    .to_string();

                let mime_type = guess_mime_type(&filename);

                let resource = crate::model::Resource {
                    resource_type: crate::model::ResourceType::Image,
                    filename: Some(filename.clone()),
                    mime_type,
                    data,
                };

                document.resources.insert(filename, resource);
            }
        }

        Ok(())
    }
}

/// Extracts a metadata field from content.hpf XML.
fn extract_metadata_field(xml: &str, field: &str) -> Option<String> {
    // Simple regex-like extraction for dc:title, dc:creator, etc.
    let start_tag = format!("<dc:{}>", field);
    let end_tag = format!("</dc:{}>", field);

    if let Some(start) = xml.find(&start_tag) {
        let content_start = start + start_tag.len();
        if let Some(end) = xml[content_start..].find(&end_tag) {
            return Some(xml[content_start..content_start + end].to_string());
        }
    }

    None
}

/// Guesses MIME type from filename extension.
fn guess_mime_type(filename: &str) -> Option<String> {
    let ext = filename.rsplit('.').next()?.to_lowercase();
    match ext.as_str() {
        "png" => Some("image/png".to_string()),
        "jpg" | "jpeg" => Some("image/jpeg".to_string()),
        "gif" => Some("image/gif".to_string()),
        "bmp" => Some("image/bmp".to_string()),
        "webp" => Some("image/webp".to_string()),
        "svg" => Some("image/svg+xml".to_string()),
        "wmf" => Some("image/x-wmf".to_string()),
        "emf" => Some("image/x-emf".to_string()),
        _ => None,
    }
}
