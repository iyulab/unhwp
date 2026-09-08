//! HWPX (OWPML) XML format parser.
//!
//! HWPX files are ZIP archives containing XML documents following the
//! KS X 6101 OWPML standard.

mod container;
mod header;
mod section;
mod styles;
mod xml;

pub use container::HwpxContainer;

use crate::error::{Error, Result};
use crate::model::Document;
use crate::streaming::{ParseEvent, SectionStreamOptions};
use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};

use self::xml::{decode_text, resolve_general_ref};
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
use std::io::{Read, Seek};
use std::ops::ControlFlow;
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
        self.parse_with_options(&crate::ParseOptions::default())
    }

    /// Parses the document with the given options.
    pub fn parse_with_options(&mut self, opts: &crate::ParseOptions) -> Result<Document> {
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
        self.parse_sections(&mut document, opts)?;

        // Extract resources (skip if not requested)
        if opts.extract_resources {
            self.extract_resources(&mut document)?;
        }

        Ok(document)
    }

    /// Parses the document in streaming mode, emitting events for each section.
    ///
    /// This is the bounded-memory alternative to [`parse`](Self::parse). Each
    /// section is parsed and emitted individually; its memory is freed before
    /// the next section is loaded. The `rayon` parallel path used by `parse()`
    /// is not used here — sections are always processed sequentially.
    ///
    /// See [`crate::streaming::parse_file_streaming`] for the public API.
    pub fn for_each_section<F>(&mut self, opts: SectionStreamOptions, mut f: F) -> Result<()>
    where
        F: FnMut(ParseEvent<'_>) -> ControlFlow<()>,
    {
        // Phase 1: parse prerequisites using a temporary Document.
        // We move metadata and styles out so they become owned stack locals,
        // giving us the stack-frame lifetime that satisfies ParseEvent<'doc>.
        let (metadata, styles) = {
            let mut tmp = Document::new();
            tmp.metadata.format_version = Some("HWPX".to_string());
            self.parse_metadata(&mut tmp)?;
            self.parse_header_options(&mut tmp)?;
            self.parse_styles(&mut tmp)?;
            (tmp.metadata, tmp.styles)
        };

        let section_files = self.container.list_sections()?;
        let section_count = section_files.len();
        let image_map = self.container.build_image_map();

        if f(ParseEvent::DocumentStart {
            metadata: &metadata,
            styles: &styles,
            section_count,
            image_map,
        }) == ControlFlow::Break(())
        {
            return Ok(());
        }

        // Phase 2: parse and emit sections one at a time (no rayon).
        for (index, path) in section_files.iter().enumerate() {
            match self.container.read_file(path) {
                Err(e) if opts.error_mode == crate::parse_options::ErrorMode::Lenient => {
                    if f(ParseEvent::SectionFailed { index, error: e }) == ControlFlow::Break(()) {
                        return Ok(());
                    }
                }
                Err(e) => return Err(e),
                Ok(xml) => match section::parse_section(&xml, index, &styles) {
                    Err(e) if opts.error_mode == crate::parse_options::ErrorMode::Lenient => {
                        if f(ParseEvent::SectionFailed { index, error: e })
                            == ControlFlow::Break(())
                        {
                            return Ok(());
                        }
                    }
                    Err(e) => return Err(e),
                    Ok(sec) => {
                        if f(ParseEvent::SectionParsed(&sec)) == ControlFlow::Break(()) {
                            return Ok(());
                        }
                        // `sec` is dropped here — memory reclaimed before
                        // the next section is loaded.
                    }
                },
            }
        }

        // Phase 3: emit DocumentEnd before resources.
        if f(ParseEvent::DocumentEnd) == ControlFlow::Break(()) {
            return Ok(());
        }

        // Phase 4: resource extraction (after DocumentEnd).
        if opts.extract_resources {
            if let Ok(resources) = self.container.list_bindata() {
                for resource_path in resources {
                    if let Ok(data) = self.container.read_binary(&resource_path) {
                        let name = resource_path
                            .rsplit('/')
                            .next()
                            .unwrap_or(&resource_path)
                            .to_string();
                        if f(ParseEvent::ResourceExtracted { name, data }) == ControlFlow::Break(())
                        {
                            return Ok(());
                        }
                    }
                }
            }
        }

        Ok(())
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
    ///
    /// `header.xml` is optional, so its absence is not an error — but only its absence.
    /// Matching on [`Error::MissingComponent`] rather than discarding every error keeps a
    /// present-but-unreadable header from being indistinguishable from a missing one.
    fn parse_styles(&mut self, document: &mut Document) -> Result<()> {
        match self.container.read_file("Contents/header.xml") {
            Ok(styles_xml) => styles::parse_styles(&styles_xml, &mut document.styles),
            Err(Error::MissingComponent(_)) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Parses header options from header.xml.
    ///
    /// Optional in the same way, and narrowed for the same reason, as [`Self::parse_styles`].
    fn parse_header_options(&mut self, document: &mut Document) -> Result<()> {
        match self.container.read_file("Contents/header.xml") {
            Ok(header_xml) => {
                document.metadata.is_distribution = header::parse_header(&header_xml)?;
                Ok(())
            }
            Err(Error::MissingComponent(_)) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Parses all sections.
    ///
    /// Uses parallel processing when there are multiple sections.
    ///
    /// Honours [`ErrorMode`]. Under the default [`Strict`] the first unreadable or
    /// unparsable section fails the document; under [`Lenient`] such a section is skipped
    /// and the rest are returned. This path used to skip unconditionally, which made a
    /// damaged section indistinguishable from one that was never there — the caller got a
    /// successful `Document` with content silently missing, while the streaming path
    /// ([`Self::for_each_section`]) honoured the same option correctly.
    ///
    /// [`ErrorMode`]: crate::parse_options::ErrorMode
    /// [`Strict`]: crate::parse_options::ErrorMode::Strict
    /// [`Lenient`]: crate::parse_options::ErrorMode::Lenient
    fn parse_sections(
        &mut self,
        document: &mut Document,
        opts: &crate::ParseOptions,
    ) -> Result<()> {
        use crate::parse_options::ErrorMode;

        let lenient = opts.error_mode == ErrorMode::Lenient;
        let section_files = self.container.list_sections()?;

        // Read all section XML content first (requires mutable borrow)
        let mut section_data: Vec<(usize, String)> = Vec::with_capacity(section_files.len());
        for (index, path) in section_files.iter().enumerate() {
            match self.container.read_file(path) {
                Ok(xml) => section_data.push((index, xml)),
                Err(_) if lenient => {}
                Err(e) => return Err(e),
            }
        }

        // Clone styles for parallel access
        let styles = document.styles.clone();

        let parse_one =
            |(index, xml): &(usize, String)| section::parse_section(xml, *index, &styles);

        // Use parallel processing only when there are enough sections to benefit
        // Threshold of 3 sections avoids parallel overhead for small documents
        #[cfg(not(target_arch = "wasm32"))]
        const PARALLEL_THRESHOLD: usize = 3;

        #[cfg(not(target_arch = "wasm32"))]
        let mut sections: Vec<_> = if section_data.len() >= PARALLEL_THRESHOLD {
            if lenient {
                section_data
                    .par_iter()
                    .filter_map(|d| parse_one(d).ok())
                    .collect()
            } else {
                section_data
                    .par_iter()
                    .map(parse_one)
                    .collect::<Result<Vec<_>>>()?
            }
        } else if lenient {
            section_data
                .iter()
                .filter_map(|d| parse_one(d).ok())
                .collect()
        } else {
            section_data
                .iter()
                .map(parse_one)
                .collect::<Result<Vec<_>>>()?
        };

        #[cfg(target_arch = "wasm32")]
        let mut sections: Vec<_> = if lenient {
            section_data
                .iter()
                .filter_map(|d| parse_one(d).ok())
                .collect()
        } else {
            section_data
                .iter()
                .map(parse_one)
                .collect::<Result<Vec<_>>>()?
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
    // Text is accumulated per element and trimmed once at dispatch (see the
    // `End` arm below). Per-event trimming must stay off: quick-xml 0.40+ splits
    // entity references into their own events, so a value like "Q&amp;A B" is
    // delivered as several fragments — trimming each would drop the internal
    // space adjacent to an entity boundary.
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut current_element: Option<String> = None;
    let mut current_meta_name: Option<String> = None;
    // Text and entity-reference events are accumulated here across the open
    // element and dispatched on its `End`, so entity references (e.g. a title
    // containing `&amp;`) split into separate events by quick-xml 0.40+ are not
    // lost or truncated.
    let mut current_text = String::new();

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
                            if attr.key.local_name().as_ref() == "name" {
                                if let Ok(value) = attr.normalized_value(XmlVersion::Implicit1_0) {
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
                current_text.push_str(&decode_text(&e));
            }
            // Entity references (`&amp;` etc.) are separate events in quick-xml
            // 0.40+; fold them into the buffer so metadata values survive.
            Ok(Event::GeneralRef(r)) => {
                current_text.push_str(&resolve_general_ref(&r));
            }
            Ok(Event::End(_)) => {
                let text = current_text.trim();
                // Element-based (`<title>…</title>`) and meta-name-based
                // (`<meta name="title">…</meta>`) metadata share one field
                // mapping; element context takes precedence when both are set.
                if let Some(key) = current_element.as_deref().or(current_meta_name.as_deref()) {
                    if !text.is_empty() {
                        match key {
                            "title" => result.title = Some(text.to_string()),
                            "creator" => result.creator = Some(text.to_string()),
                            "description" => result.description = Some(text.to_string()),
                            "date" => result.date = Some(text.to_string()),
                            "modified" => result.modified = Some(text.to_string()),
                            "generator" => result.generator = Some(text.to_string()),
                            "subject" | "keywords" => {
                                // Split by common delimiters
                                for kw in text.split([',', ';', '|']) {
                                    let kw = kw.trim();
                                    if !kw.is_empty() && !result.keywords.contains(&kw.to_string())
                                    {
                                        result.keywords.push(kw.to_string());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                current_element = None;
                current_meta_name = None;
                current_text.clear();
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
    local.as_ref().to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_entity_references_resolved() {
        // A title such as "Q&A" is delivered by quick-xml 0.40+ as
        // Text("Q") / GeneralRef("amp") / Text("A"). The metadata loop must
        // accumulate across the element and dispatch on `End`; the previous
        // dispatch-per-Text-event logic would keep only the last fragment ("A").
        let xml =
            r#"<metadata><title>Q&amp;A &#48;</title><creator>a &lt;b&gt; c</creator></metadata>"#;
        let meta = parse_metadata_xml(xml);
        assert_eq!(meta.title.as_deref(), Some("Q&A 0"));
        assert_eq!(meta.creator.as_deref(), Some("a <b> c"));
    }
}
