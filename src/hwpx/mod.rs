//! HWPX (OWPML) XML format parser.
//!
//! HWPX files are ZIP archives containing XML documents following the
//! KS X 6101 OWPML standard.

mod container;
mod header;
mod section;
mod styles;

pub use container::HwpxContainer;

use crate::error::Result;
use crate::model::Document;
use quick_xml::events::Event;
use quick_xml::Reader;
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

        // Parse header options (distribution flag)
        self.parse_header_options(&mut document)?;

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

    /// Parses header options from header.xml.
    fn parse_header_options(&mut self, document: &mut Document) -> Result<()> {
        if let Ok(header_xml) = self.container.read_file("Contents/header.xml") {
            let is_distribution = header::parse_header(&header_xml)?;
            document.metadata.is_distribution = is_distribution;
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

        // Use parallel processing only when there are enough sections to benefit
        // Threshold of 3 sections avoids parallel overhead for small documents
        const PARALLEL_THRESHOLD: usize = 3;

        let mut sections: Vec<_> = if section_data.len() >= PARALLEL_THRESHOLD {
            // Parse sections in parallel
            section_data
                .par_iter()
                .filter_map(|(index, xml)| section::parse_section(xml, *index, &styles).ok())
                .collect()
        } else {
            // Parse sections sequentially for small documents
            section_data
                .iter()
                .filter_map(|(index, xml)| section::parse_section(xml, *index, &styles).ok())
                .collect()
        };

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

/// Metadata extraction result from content.hpf.
#[derive(Default)]
struct MetadataResult {
    title: Option<String>,
    creator: Option<String>,
    description: Option<String>,
    date: Option<String>,
    modified: Option<String>,
    generator: Option<String>,
    keywords: Vec<String>,
}

/// Extracts all metadata fields from content.hpf XML using proper XML parsing.
/// This replaces the naive string-based extraction with quick-xml parsing.
fn parse_metadata_xml(xml: &str) -> MetadataResult {
    let mut result = MetadataResult::default();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut current_element: Option<String> = None;
    let mut current_meta_name: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = get_local_name(&e);
                match name.as_str() {
                    "title" | "creator" | "description" | "date" | "modified" | "generator"
                    | "subject" | "keywords" => {
                        current_element = Some(name);
                    }
                    "meta" => {
                        // Check for name attribute
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"name" {
                                if let Ok(value) = attr.unescape_value() {
                                    current_meta_name = Some(value.to_string());
                                }
                            }
                        }
                    }
                    _ => {
                        current_element = None;
                    }
                }
            }
            Ok(Event::Text(e)) => {
                if let Ok(text) = e.unescape() {
                    let text = text.trim().to_string();
                    if !text.is_empty() {
                        // Handle element-based metadata
                        if let Some(ref elem) = current_element {
                            match elem.as_str() {
                                "title" => result.title = Some(text.clone()),
                                "creator" => result.creator = Some(text.clone()),
                                "description" => result.description = Some(text.clone()),
                                "date" => result.date = Some(text.clone()),
                                "modified" => result.modified = Some(text.clone()),
                                "generator" => result.generator = Some(text.clone()),
                                "subject" | "keywords" => {
                                    // Split by common delimiters
                                    for kw in text.split([',', ';', '|']) {
                                        let kw = kw.trim();
                                        if !kw.is_empty()
                                            && !result.keywords.contains(&kw.to_string())
                                        {
                                            result.keywords.push(kw.to_string());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        // Handle meta name-based metadata
                        else if let Some(ref meta_name) = current_meta_name {
                            match meta_name.as_str() {
                                "title" => result.title = Some(text.clone()),
                                "creator" => result.creator = Some(text.clone()),
                                "description" => result.description = Some(text.clone()),
                                "date" => result.date = Some(text.clone()),
                                "modified" => result.modified = Some(text.clone()),
                                "generator" => result.generator = Some(text.clone()),
                                "subject" | "keywords" => {
                                    for kw in text.split([',', ';', '|']) {
                                        let kw = kw.trim();
                                        if !kw.is_empty()
                                            && !result.keywords.contains(&kw.to_string())
                                        {
                                            result.keywords.push(kw.to_string());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Ok(Event::End(_)) => {
                current_element = None;
                current_meta_name = None;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    result
}

/// Extracts a single metadata field from content.hpf XML.
fn extract_metadata_field(xml: &str, field: &str) -> Option<String> {
    let metadata = parse_metadata_xml(xml);
    match field {
        "title" => metadata.title,
        "creator" => metadata.creator,
        "description" => metadata.description,
        "date" => metadata.date,
        "modified" => metadata.modified,
        "generator" => metadata.generator,
        "subject" | "keywords" => {
            if metadata.keywords.is_empty() {
                None
            } else {
                Some(metadata.keywords.join(", "))
            }
        }
        _ => None,
    }
}

/// Extracts keywords from content.hpf XML.
fn extract_keywords(xml: &str) -> Vec<String> {
    parse_metadata_xml(xml).keywords
}

/// Gets the local name from an XML element (strips namespace prefix).
fn get_local_name(e: &quick_xml::events::BytesStart) -> String {
    let name = e.name();
    let local = name.local_name();
    String::from_utf8_lossy(local.as_ref()).to_string()
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
