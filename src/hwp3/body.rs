//! HWP 3.x body/content parsing.
//!
//! Parses the document body which contains text and control codes.

use super::{decode_euckr, Hwp3Header};
use crate::error::{Error, Result};
use crate::model::{Block, Document, InlineContent, Paragraph, Section, TextRun, TextStyle};
use flate2::read::ZlibDecoder;
use std::io::{Read, Seek, SeekFrom};

/// Check if byte is a valid CP949/EUC-KR lead byte (first byte of 2-byte sequence).
/// CP949 lead byte range: 0x81-0xFE
#[inline]
fn is_cp949_lead_byte(byte: u8) -> bool {
    (0x81..=0xFE).contains(&byte)
}

/// Check if byte is a valid CP949/EUC-KR trail byte (second byte of 2-byte sequence).
/// CP949 trail byte ranges: 0x41-0x5A (A-Z), 0x61-0x7A (a-z), 0x81-0xFE
/// Note: 0x5B-0x60 and 0x7B-0x80 are not valid trail bytes in standard CP949.
#[inline]
fn is_cp949_trail_byte(byte: u8) -> bool {
    (0x41..=0x5A).contains(&byte)      // Uppercase ASCII range
        || (0x61..=0x7A).contains(&byte)  // Lowercase ASCII range
        || (0x81..=0xFE).contains(&byte) // High byte range
}

/// HWP 3.x control codes.
mod control {
    /// End of paragraph
    pub const PARA_END: u8 = 0x0D;
    /// Line break
    pub const LINE_BREAK: u8 = 0x0A;
    /// Hard space (non-breaking)
    pub const HARD_SPACE: u8 = 0xA0;
    /// Tab character
    pub const TAB: u8 = 0x09;
    /// Start of control sequence
    pub const CTRL_START: u8 = 0x1B;
    /// Bold on/off
    pub const BOLD: u8 = 0x01;
    /// Italic on/off
    pub const ITALIC: u8 = 0x02;
    /// Underline on/off
    pub const UNDERLINE: u8 = 0x03;
}

/// Body content parser.
pub struct BodyParser {
    compressed: bool,
    body_offset: u32,
    body_size: u32,
}

impl BodyParser {
    /// Creates a new body parser from header info.
    pub fn new(header: &Hwp3Header) -> Self {
        Self {
            compressed: header.compressed,
            body_offset: header.body_offset,
            body_size: header.body_size,
        }
    }

    /// Parses the document body.
    pub fn parse<R: Read + Seek>(&self, reader: &mut R, document: &mut Document) -> Result<()> {
        // Seek to body position
        if self.body_offset == 0 {
            // If no explicit offset, body starts after header
            reader.seek(SeekFrom::Start(128))?;
        } else {
            reader.seek(SeekFrom::Start(self.body_offset as u64))?;
        }

        // Read body data
        let body_data = if self.body_size > 0 {
            let mut data = vec![0u8; self.body_size as usize];
            reader.read_exact(&mut data)?;
            data
        } else {
            // Read remaining file
            let mut data = Vec::new();
            reader.read_to_end(&mut data)?;
            data
        };

        // Decompress if needed
        let content = if self.compressed && !body_data.is_empty() {
            decompress_body(&body_data)?
        } else {
            body_data
        };

        // Parse content into paragraphs
        let section = self.parse_content(&content)?;
        document.sections.push(section);

        Ok(())
    }

    /// Parses raw content bytes into a section.
    fn parse_content(&self, data: &[u8]) -> Result<Section> {
        let mut section = Section::new(0);
        let mut current_para = Paragraph::default();
        let mut current_text = Vec::new();
        let mut current_style = TextStyle::default();

        let mut i = 0;
        while i < data.len() {
            let byte = data[i];

            match byte {
                // Paragraph end
                control::PARA_END => {
                    // Flush current text
                    flush_text(&mut current_text, &current_style, &mut current_para);

                    // Add paragraph if not empty
                    if !current_para.content.is_empty() {
                        section.content.push(Block::Paragraph(current_para));
                        current_para = Paragraph::default();
                    }
                    i += 1;
                }

                // Line break
                control::LINE_BREAK => {
                    // Flush current text
                    flush_text(&mut current_text, &current_style, &mut current_para);
                    current_para.content.push(InlineContent::LineBreak);
                    i += 1;
                }

                // Tab
                control::TAB => {
                    current_text.push(b'\t');
                    i += 1;
                }

                // Hard space
                control::HARD_SPACE => {
                    current_text.push(b' ');
                    i += 1;
                }

                // Control sequence
                control::CTRL_START => {
                    // Flush current text before style change
                    flush_text(&mut current_text, &current_style, &mut current_para);

                    // Parse control code
                    if i + 1 < data.len() {
                        let ctrl_code = data[i + 1];
                        match ctrl_code {
                            control::BOLD => {
                                current_style.bold = !current_style.bold;
                            }
                            control::ITALIC => {
                                current_style.italic = !current_style.italic;
                            }
                            control::UNDERLINE => {
                                current_style.underline = !current_style.underline;
                            }
                            _ => {
                                // Unknown control, skip
                            }
                        }
                        i += 2;
                    } else {
                        i += 1;
                    }
                }

                // Regular character or Korean character (CP949/EUC-KR encoding)
                _ => {
                    // Check if this is a 2-byte Korean character (CP949/EUC-KR)
                    // Lead byte: 0x81-0xFE, Trail byte: 0x41-0x5A, 0x61-0x7A, 0x81-0xFE
                    if is_cp949_lead_byte(byte) {
                        if i + 1 < data.len() {
                            let second = data[i + 1];
                            if is_cp949_trail_byte(second) {
                                // Valid 2-byte character
                                current_text.push(byte);
                                current_text.push(second);
                                i += 2;
                                continue;
                            }
                        }
                        // Incomplete or invalid multi-byte sequence at end of data
                        // Skip the orphan lead byte to prevent mojibake
                        i += 1;
                        continue;
                    }

                    // Single byte character (ASCII printable range)
                    if byte >= 0x20 && byte != 0x7F {
                        current_text.push(byte);
                    }
                    i += 1;
                }
            }
        }

        // Flush remaining text
        flush_text(&mut current_text, &current_style, &mut current_para);

        // Add final paragraph
        if !current_para.content.is_empty() {
            section.content.push(Block::Paragraph(current_para));
        }

        Ok(section)
    }
}

/// Flushes accumulated text bytes to paragraph content.
fn flush_text(text_bytes: &mut Vec<u8>, style: &TextStyle, para: &mut Paragraph) {
    if !text_bytes.is_empty() {
        let text = decode_euckr(text_bytes);
        if !text.is_empty() {
            let run = TextRun::with_style(text, style.clone());
            para.content.push(InlineContent::Text(run));
        }
        text_bytes.clear();
    }
}

/// Decompresses zlib-compressed body data.
fn decompress_body(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| Error::Decompression(e.to_string()))?;
    Ok(decompressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_body_parser_simple() {
        let header = Hwp3Header::default();
        let parser = BodyParser::new(&header);

        // Simple text: "테스트" in EUC-KR + paragraph end
        // 테: 0xC5, 0xD7
        // 스: 0xBD, 0xBA
        // 트: 0xC6, 0xAE
        let data = [0xC5, 0xD7, 0xBD, 0xBA, 0xC6, 0xAE, 0x0D];
        let section = parser.parse_content(&data).unwrap();

        assert_eq!(section.content.len(), 1);
        let Block::Paragraph(p) = &section.content[0] else {
            unreachable!("Expected Paragraph block, got {:?}", section.content[0]);
        };
        assert_eq!(p.content.len(), 1);
        let InlineContent::Text(run) = &p.content[0] else {
            unreachable!("Expected Text inline, got {:?}", p.content[0]);
        };
        assert_eq!(run.text, "테스트");
    }

    #[test]
    fn test_body_parser_ascii() {
        let header = Hwp3Header::default();
        let parser = BodyParser::new(&header);

        // Simple ASCII text
        let data = b"Hello World\x0D";
        let section = parser.parse_content(data).unwrap();

        assert_eq!(section.content.len(), 1);
        if let Block::Paragraph(p) = &section.content[0] {
            if let InlineContent::Text(run) = &p.content[0] {
                assert_eq!(run.text, "Hello World");
            }
        }
    }
}
