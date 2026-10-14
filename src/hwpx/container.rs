//! ZIP container wrapper for HWPX documents.

use crate::error::{Error, Result};
use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use unparser_shared::inflate::{is_inflation_limit, InflateGuard};
use zip::ZipArchive;

/// The bytes of the entry `path`, read from its decompressing reader through
/// [`InflateGuard`]: an entry that expands past what a document's data does (a decompression
/// bomb) is a damaged archive, not an allocation that ends the process.
fn read_entry(path: &str, file: impl Read, compressed_size: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    match InflateGuard::new(file, compressed_size).read_to_end(&mut bytes) {
        Ok(_) => Ok(bytes),
        Err(e) if is_inflation_limit(&e) => Err(Error::ZipArchive(format!("{path}: {e}"))),
        Err(e) => Err(e.into()),
    }
}

/// HWPX container paths.
#[allow(dead_code)]
mod paths {
    pub const MIMETYPE: &str = "mimetype";
    pub const CONTENT_HPF: &str = "Contents/content.hpf";
    pub const HEADER_XML: &str = "Contents/header.xml";
    pub const SETTINGS_XML: &str = "Contents/settings.xml";
    pub const BINDATA_DIR: &str = "BinData/";
    pub const CONTENTS_DIR: &str = "Contents/";
}

/// ZIP container wrapper for HWPX files.
pub struct HwpxContainer {
    archive: ZipArchive<Cursor<Vec<u8>>>,
}

impl HwpxContainer {
    /// Opens an HWPX container from a file path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_bytes(data)
    }

    /// Opens an HWPX container from a reader.
    pub fn from_reader<R: Read + Seek>(mut reader: R) -> Result<Self> {
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        Self::from_bytes(data)
    }

    /// Opens an HWPX container from bytes.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let cursor = Cursor::new(data);
        let archive = ZipArchive::new(cursor)?;
        Ok(Self { archive })
    }

    /// Verifies this is a valid HWPX file by checking mimetype.
    pub fn verify_mimetype(&mut self) -> Result<bool> {
        if let Ok(content) = self.read_file(paths::MIMETYPE) {
            // HWPX mimetype should be "application/hwp+zip" or similar
            Ok(content.contains("hwp") || content.contains("owpml"))
        } else {
            // Some HWPX files might not have mimetype
            Ok(true)
        }
    }

    /// Reads a file from the archive as UTF-8 string.
    ///
    /// Bytes that are not valid UTF-8 are reported as [`Error::Encoding`], not as an I/O
    /// failure. Reading straight into a `String` would classify them by where they were
    /// noticed rather than by what went wrong: the reader raises `InvalidData`, `?` maps
    /// any [`std::io::Error`] to [`Error::Io`], and a caller branching on the kind then
    /// sees a disk problem where the real cause is a malformed part. Decoding as its own
    /// step keeps the reason attached at the point that knows it — the same contract
    /// [`From<std::str::Utf8Error>`] already states for this crate.
    ///
    /// [`From<std::str::Utf8Error>`]: crate::error::Error
    pub fn read_file(&mut self, path: &str) -> Result<String> {
        let file = self
            .archive
            .by_name(path)
            .map_err(|_| Error::MissingComponent(path.to_string()))?;

        let compressed = file.compressed_size();
        let bytes = read_entry(path, file, compressed)?;
        String::from_utf8(bytes)
            .map_err(|err| Error::Encoding(format!("{path} is not valid UTF-8: {err}")))
    }

    /// Reads a binary file from the archive.
    pub fn read_binary(&mut self, path: &str) -> Result<Vec<u8>> {
        let file = self
            .archive
            .by_name(path)
            .map_err(|_| Error::MissingComponent(path.to_string()))?;

        let compressed = file.compressed_size();
        read_entry(path, file, compressed)
    }

    /// Reads the content.hpf manifest file.
    pub fn read_content_hpf(&mut self) -> Result<String> {
        self.read_file(paths::CONTENT_HPF)
    }

    /// Lists all section files in order.
    pub fn list_sections(&mut self) -> Result<Vec<String>> {
        let mut sections = Vec::new();

        // First try to get section order from content.hpf
        if let Ok(hpf) = self.read_content_hpf() {
            // Parse spine to get section order
            sections = parse_section_order(&hpf);
        }

        // If no sections found from manifest, scan for section files
        if sections.is_empty() {
            for i in 0..self.archive.len() {
                if let Ok(file) = self.archive.by_index(i) {
                    let name = file.name()?.to_string();
                    if name.starts_with("Contents/section") && name.ends_with(".xml") {
                        sections.push(name);
                    }
                }
            }
            sections.sort();
        }

        if sections.is_empty() {
            return Err(Error::MissingComponent("section files".into()));
        }

        Ok(sections)
    }

    /// Lists all files in BinData directory.
    pub fn list_bindata(&mut self) -> Result<Vec<String>> {
        let mut resources = Vec::new();

        for i in 0..self.archive.len() {
            if let Ok(file) = self.archive.by_index(i) {
                let name = file.name()?.to_string();
                if name.starts_with(paths::BINDATA_DIR) && !name.ends_with('/') {
                    resources.push(name);
                }
            }
        }

        Ok(resources)
    }

    /// Checks if a file exists in the archive.
    pub fn file_exists(&mut self, path: &str) -> bool {
        self.archive.by_name(path).is_ok()
    }

    /// Builds a `binaryItemIDRef → filename` map from the OPF manifest.
    ///
    /// Returns e.g. `"image5"` → `"image5.bmp"` for use by the streaming
    /// renderer so image paths include the correct file extension.
    pub fn build_image_map(&mut self) -> HashMap<String, String> {
        if let Ok(hpf) = self.read_content_hpf() {
            build_image_map_from_hpf(&hpf)
        } else {
            HashMap::new()
        }
    }
}

/// Parses the OPF manifest from content.hpf into an `id → href` map.
///
/// Handles compact single-line XML (as real HWPX files use). Shared by
/// `parse_section_order` and `build_image_map_from_hpf`.
fn parse_manifest_map(hpf_content: &str) -> (HashMap<String, String>, Vec<String>) {
    let mut manifest: HashMap<String, String> = HashMap::new();
    let mut spine: Vec<String> = Vec::new();
    let mut in_spine = false;
    let mut reader = Reader::from_str(hpf_content);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();
                match name {
                    "item" => {
                        let (mut id, mut href) = (None::<String>, None::<String>);
                        for attr in e.attributes().flatten() {
                            let key = attr.key.local_name();
                            let k = key.as_ref();
                            match k {
                                "id" => {
                                    id = attr
                                        .normalized_value(XmlVersion::Implicit1_0)
                                        .ok()
                                        .map(|v| v.into_owned())
                                }
                                "href" => {
                                    href = attr
                                        .normalized_value(XmlVersion::Implicit1_0)
                                        .ok()
                                        .map(|v| v.into_owned())
                                }
                                _ => {}
                            }
                        }
                        if let (Some(id), Some(href)) = (id, href) {
                            manifest.insert(id, href);
                        }
                    }
                    "spine" => in_spine = true,
                    "itemref" if in_spine => {
                        for attr in e.attributes().flatten() {
                            let key = attr.key.local_name();
                            let k = key.as_ref();
                            if k == "idref" {
                                if let Ok(v) = attr.normalized_value(XmlVersion::Implicit1_0) {
                                    spine.push(v.into_owned());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                if local.as_ref() == "spine" {
                    in_spine = false;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    (manifest, spine)
}

/// Parses section order from content.hpf manifest using proper XML parsing.
///
/// Reads the OPF manifest to build an id→href map, then follows the spine
/// order to resolve section file paths. This handles compact single-line XML
/// (which real HWPX files use) correctly — the previous string/line approach
/// only found the first section per line.
fn parse_section_order(hpf_content: &str) -> Vec<String> {
    let (manifest, spine) = parse_manifest_map(hpf_content);

    // Resolve spine idrefs → file paths via manifest, keeping only section XMLs.
    spine
        .into_iter()
        .filter_map(|idref| manifest.get(&idref).cloned())
        .filter(|href| href.ends_with(".xml") && href.to_lowercase().contains("section"))
        .collect()
}

/// Builds a `binaryItemIDRef → filename` map from content.hpf manifest.
///
/// e.g. `"image5"` → `"image5.bmp"`. Used by the streaming path to resolve
/// image IDs to filenames with extensions before sections are rendered.
fn build_image_map_from_hpf(hpf_content: &str) -> HashMap<String, String> {
    let (manifest, _) = parse_manifest_map(hpf_content);

    manifest
        .into_iter()
        .filter(|(_, href)| !href.ends_with(".xml") && !href.ends_with(".js"))
        .filter_map(|(id, href)| {
            let filename = href.rsplit('/').next()?.to_string();
            Some((id, filename))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An entry that expands past a hundredfold is a damaged archive (a decompression bomb),
    /// read as an error instead of being allocated; an ordinary entry is read whole.
    #[test]
    fn an_entry_that_expands_past_a_hundredfold_is_a_damaged_archive() {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let deflated = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("Contents/section0.xml", deflated).unwrap();
        zip.write_all(&vec![b' '; 20 << 20]).unwrap();
        zip.start_file("Contents/header.xml", deflated).unwrap();
        zip.write_all(b"<hh:head/>").unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let file = archive.by_name("Contents/section0.xml").unwrap();
        let compressed = file.compressed_size();
        let err = read_entry("Contents/section0.xml", file, compressed).unwrap_err();
        assert!(
            matches!(err, Error::ZipArchive(ref m) if m.contains("decompression bomb")),
            "{err}"
        );
        let file = archive.by_name("Contents/header.xml").unwrap();
        let compressed = file.compressed_size();
        assert_eq!(
            read_entry("Contents/header.xml", file, compressed).unwrap(),
            b"<hh:head/>"
        );
    }

    // Realistic single-line compact XML — the format real HWPX files actually use.
    // The previous string/line parser only found section0 here because find() returns
    // the first match per line and the entire file is one line.
    const SINGLE_LINE_HPF: &str = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        r#"<opf:package xmlns:opf="http://www.idpf.org/2007/opf/">"#,
        r#"<opf:manifest>"#,
        r#"<opf:item id="header" href="Contents/header.xml" media-type="application/xml"/>"#,
        r#"<opf:item id="section0" href="Contents/section0.xml" media-type="application/xml"/>"#,
        r#"<opf:item id="section1" href="Contents/section1.xml" media-type="application/xml"/>"#,
        r#"<opf:item id="headersc" href="Scripts/headerScripts.js" media-type="application/javascript"/>"#,
        r#"</opf:manifest>"#,
        r#"<opf:spine>"#,
        r#"<opf:itemref idref="header" linear="yes"/>"#,
        r#"<opf:itemref idref="section0" linear="yes"/>"#,
        r#"<opf:itemref idref="section1" linear="yes"/>"#,
        r#"<opf:itemref idref="headersc" linear="yes"/>"#,
        r#"</opf:spine>"#,
        r#"</opf:package>"#,
    );

    #[test]
    fn test_parse_section_order_single_line() {
        let sections = parse_section_order(SINGLE_LINE_HPF);
        assert_eq!(
            sections.len(),
            2,
            "must find both sections in compact single-line XML"
        );
        assert_eq!(sections[0], "Contents/section0.xml");
        assert_eq!(sections[1], "Contents/section1.xml");
    }

    #[test]
    fn test_parse_section_order_multiline() {
        let hpf = r#"
        <package>
            <manifest>
                <item id="header" href="Contents/header.xml" media-type="application/xml"/>
                <item id="section0" href="Contents/section0.xml" media-type="application/xml"/>
                <item id="section1" href="Contents/section1.xml" media-type="application/xml"/>
            </manifest>
            <spine>
                <itemref idref="header"/>
                <itemref idref="section0"/>
                <itemref idref="section1"/>
            </spine>
        </package>
        "#;

        let sections = parse_section_order(hpf);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0], "Contents/section0.xml");
        assert_eq!(sections[1], "Contents/section1.xml");
    }

    #[test]
    fn test_parse_section_order_spine_ordering() {
        // Manifest lists section1 before section0, but spine reverses order.
        // Result must follow spine order.
        let hpf = r#"
        <package>
            <manifest>
                <item id="section0" href="Contents/section0.xml" media-type="application/xml"/>
                <item id="section1" href="Contents/section1.xml" media-type="application/xml"/>
            </manifest>
            <spine>
                <itemref idref="section1"/>
                <itemref idref="section0"/>
            </spine>
        </package>
        "#;

        let sections = parse_section_order(hpf);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0], "Contents/section1.xml");
        assert_eq!(sections[1], "Contents/section0.xml");
    }

    #[test]
    fn test_parse_section_order_excludes_non_sections() {
        // header.xml and JS files in spine must not appear in result.
        let sections = parse_section_order(SINGLE_LINE_HPF);
        assert!(!sections.iter().any(|s| s.contains("header.xml")));
        assert!(!sections.iter().any(|s| s.ends_with(".js")));
    }
}
