//! ZIP container wrapper for HWPX documents.

use crate::error::{Error, Result};
use std::io::{Read, Seek, Cursor};
use std::path::Path;
use zip::ZipArchive;

/// HWPX container paths.
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
    pub fn read_file(&mut self, path: &str) -> Result<String> {
        let mut file = self.archive
            .by_name(path)
            .map_err(|_| Error::MissingComponent(path.to_string()))?;

        let mut content = String::new();
        file.read_to_string(&mut content)?;
        Ok(content)
    }

    /// Reads a binary file from the archive.
    pub fn read_binary(&mut self, path: &str) -> Result<Vec<u8>> {
        let mut file = self.archive
            .by_name(path)
            .map_err(|_| Error::MissingComponent(path.to_string()))?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Ok(data)
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
                    let name = file.name().to_string();
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
                let name = file.name().to_string();
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
}

/// Parses section order from content.hpf manifest.
fn parse_section_order(hpf_content: &str) -> Vec<String> {
    let mut sections = Vec::new();

    // Look for spine items or itemref elements
    // Format varies, but typically:
    // <hpf:itemref idref="section0" />
    // or
    // <spine><itemref idref="section0"/></spine>

    // Simple extraction of section references
    for line in hpf_content.lines() {
        if let Some(start) = line.find("idref=\"section") {
            let rest = &line[start + 7..]; // Skip 'idref="'
            if let Some(end) = rest.find('"') {
                let section_id = &rest[..end];
                let section_path = format!("Contents/{}.xml", section_id);
                sections.push(section_path);
            }
        }
    }

    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_section_order() {
        let hpf = r#"
        <package>
            <spine>
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
}
