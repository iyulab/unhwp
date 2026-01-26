//! HWPX (OWPML) XML format parser.
//!
//! HWPX files are ZIP archives containing XML documents following the
//! KS X 6101 OWPML standard.

mod container;
mod section;
mod styles;

pub use container::HwpxContainer;

use crate::error::Result;
use crate::model::Document;
use rayon::prelude::*;
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
        if let Some(subject) = extract_metadata_field(&content_hpf, "description") {
            document.metadata.subject = Some(subject);
        }
        if let Some(date) = extract_metadata_field(&content_hpf, "date") {
            document.metadata.created = Some(date);
        }
        if let Some(modified) = extract_metadata_field(&content_hpf, "modified") {
            document.metadata.modified = Some(modified);
        }

        // Extract keywords
        let keywords = extract_keywords(&content_hpf);
        if !keywords.is_empty() {
            document.metadata.keywords = keywords;
        }

        // Try to get application info
        if let Some(generator) = extract_metadata_field(&content_hpf, "generator") {
            document.metadata.creator_app = Some(generator);
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
    ///
    /// Uses parallel processing when there are multiple sections.
    fn parse_sections(&mut self, document: &mut Document) -> Result<()> {
        let section_files = self.container.list_sections()?;

        // Read all section XML content first (requires mutable borrow)
        let section_data: Vec<(usize, String)> = section_files
            .iter()
            .enumerate()
            .filter_map(|(index, path)| self.container.read_file(path).ok().map(|xml| (index, xml)))
            .collect();

        // Clone styles for parallel access
        let styles = document.styles.clone();

        // Parse sections in parallel
        let mut sections: Vec<_> = section_data
            .par_iter()
            .filter_map(|(index, xml)| section::parse_section(xml, *index, &styles).ok())
            .collect();

        // Sort by index to maintain order
        sections.sort_by_key(|s| s.index);

        document.sections = sections;
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

                let size = data.len();
                let resource = crate::model::Resource {
                    resource_type: crate::model::ResourceType::Image,
                    filename: Some(filename.clone()),
                    mime_type,
                    data,
                    size,
                };

                document.resources.insert(filename, resource);
            }
        }

        Ok(())
    }
}

/// Extracts a metadata field from content.hpf XML.
fn extract_metadata_field(xml: &str, field: &str) -> Option<String> {
    // Try dc: namespace first (Dublin Core)
    let start_tag = format!("<dc:{}>", field);
    let end_tag = format!("</dc:{}>", field);

    if let Some(start) = xml.find(&start_tag) {
        let content_start = start + start_tag.len();
        if let Some(end) = xml[content_start..].find(&end_tag) {
            let value = xml[content_start..content_start + end].trim().to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }

    // Try opf: namespace
    let start_tag = format!("<opf:{}>", field);
    let end_tag = format!("</opf:{}>", field);

    if let Some(start) = xml.find(&start_tag) {
        let content_start = start + start_tag.len();
        if let Some(end) = xml[content_start..].find(&end_tag) {
            let value = xml[content_start..content_start + end].trim().to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }

    // Try meta with name attribute
    // Format: <opf:meta name="creator" content="text">actual value</opf:meta>
    // The actual value is in the element text, not the content attribute
    let name_attr = format!("name=\"{}\"", field);
    if let Some(start) = xml.find(&name_attr) {
        let rest = &xml[start..];
        // Find the closing > of the opening tag
        if let Some(tag_end) = rest.find('>') {
            // Check if it's a self-closing tag
            if rest[..tag_end].ends_with('/') {
                // Self-closing, no text content
            } else {
                // Look for text content between > and </
                let after_tag = &rest[tag_end + 1..];
                if let Some(close_start) = after_tag.find("</") {
                    let text_value = after_tag[..close_start].trim().to_string();
                    if !text_value.is_empty() {
                        return Some(text_value);
                    }
                }
            }
        }
    }

    None
}

/// Extracts keywords from content.hpf XML (may be comma-separated).
fn extract_keywords(xml: &str) -> Vec<String> {
    let mut keywords = Vec::new();

    // Try dc:subject first
    if let Some(subject) = extract_metadata_field(xml, "subject") {
        // Split by common delimiters
        for kw in subject.split(&[',', ';', '|'][..]) {
            let kw = kw.trim();
            if !kw.is_empty() {
                keywords.push(kw.to_string());
            }
        }
    }

    // Also try meta keywords
    if let Some(kw_str) = extract_metadata_field(xml, "keywords") {
        for kw in kw_str.split(&[',', ';', '|'][..]) {
            let kw = kw.trim();
            if !kw.is_empty() && !keywords.contains(&kw.to_string()) {
                keywords.push(kw.to_string());
            }
        }
    }

    keywords
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
