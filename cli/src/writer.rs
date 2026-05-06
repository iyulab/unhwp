//! Multi-format fan-out writer for the convert command.
//!
//! Buffers document sections and writes Markdown, plain text, and/or JSON
//! outputs in a single pass over the parsed document.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use unhwp::model::{Document, Section};
use unhwp::{render, RenderOptions};

/// Output formats supported by the convert command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Markdown,
    Text,
    Json,
}

impl OutputFormat {
    /// Parse a format string (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "md" | "markdown" => Some(Self::Markdown),
            "txt" | "text" => Some(Self::Text),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

/// Fan-out writer that streams text and JSON per section, then renders
/// Markdown from the original full `Document` in `finish()` to preserve
/// cross-section heading analysis and correct resource/style resolution.
pub struct MultiFormatWriter {
    /// Buffered text writer — streamed per section
    txt: Option<BufWriter<File>>,
    txt_path: Option<PathBuf>,

    /// JSON writer — streamed per section with envelope
    json: Option<BufWriter<File>>,
    json_path: Option<PathBuf>,
    json_first_section: bool,

    /// Render options for Markdown
    render_opts: RenderOptions,
    /// Path for the Markdown file (written in finish)
    md_path: Option<PathBuf>,

    /// Whether MD format is requested
    want_md: bool,

    /// Directory for images (None = skip image extraction)
    images_dir: Option<PathBuf>,
    /// How many images were extracted
    image_count: u32,
}

impl MultiFormatWriter {
    /// Create a new writer for the given output directory and format selection.
    ///
    /// `formats`: which output formats to produce.
    /// `render_opts`: options for Markdown rendering (including cleanup).
    /// `images_dir`: if `Some`, extract images there; `None` skips extraction.
    pub fn new(
        out_dir: &Path,
        formats: &[OutputFormat],
        render_opts: RenderOptions,
        images_dir: Option<PathBuf>,
    ) -> io::Result<Self> {
        let want_md = formats.contains(&OutputFormat::Markdown);
        let want_txt = formats.contains(&OutputFormat::Text);
        let want_json = formats.contains(&OutputFormat::Json);

        // Markdown — deferred to finish()
        let md_path = if want_md {
            Some(out_dir.join("extract.md"))
        } else {
            None
        };

        // Text
        let (txt, txt_path) = if want_txt {
            let p = out_dir.join("extract.txt");
            let f = File::create(&p)?;
            (Some(BufWriter::new(f)), Some(p))
        } else {
            (None, None)
        };

        // JSON — preamble is written in write_document_start
        let (json, json_path) = if want_json {
            let p = out_dir.join("content.json");
            let f = File::create(&p)?;
            (Some(BufWriter::new(f)), Some(p))
        } else {
            (None, None)
        };

        Ok(Self {
            txt,
            txt_path,
            json,
            json_path,
            json_first_section: true,
            render_opts,
            md_path,
            want_md,
            images_dir,
            image_count: 0,
        })
    }

    /// Write document-level metadata (called once before any sections).
    ///
    /// For JSON: emits the opening envelope `{"metadata":...,"styles":...,"sections":[`.
    pub fn write_document_start(&mut self, doc: &Document) -> io::Result<()> {
        // JSON: emit opening {"metadata":...,"styles":...,"sections":[
        if let Some(ref mut json) = self.json {
            let meta_json = serde_json::to_string(&doc.metadata)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let styles_json = serde_json::to_string(&doc.styles)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            write!(
                json,
                "{{\"metadata\":{},\"styles\":{},\"sections\":[",
                meta_json, styles_json
            )?;
        }
        Ok(())
    }

    /// Process one section: stream to TXT and JSON.
    ///
    /// MD is rendered from the full document in `finish()`.
    pub fn write_section(&mut self, section: &Section) -> io::Result<()> {
        // TXT: emit plain text lines for this section
        if let Some(ref mut txt) = self.txt {
            use unhwp::model::Block;
            for block in &section.content {
                match block {
                    Block::Paragraph(p) => {
                        let line = p.plain_text();
                        writeln!(txt, "{}", line)?;
                    }
                    Block::Table(t) => {
                        for row in &t.rows {
                            for cell in &row.cells {
                                let text = cell.plain_text();
                                if !text.is_empty() {
                                    writeln!(txt, "{}", text)?;
                                }
                            }
                        }
                    }
                }
            }
        }

        // JSON: emit this section (comma-separated after first)
        if let Some(ref mut json) = self.json {
            if !self.json_first_section {
                write!(json, ",")?;
            }
            let section_json = serde_json::to_string(section)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            write!(json, "{}", section_json)?;
            self.json_first_section = false;
        }

        Ok(())
    }

    /// Extract images from document resources to the images directory.
    ///
    /// Call this before `finish()` while `doc.resources` is still available.
    pub fn extract_images(
        &mut self,
        resources: &std::collections::HashMap<String, unhwp::model::Resource>,
    ) -> io::Result<()> {
        if let Some(ref images_dir) = self.images_dir {
            std::fs::create_dir_all(images_dir)?;
            for (name, resource) in resources {
                std::fs::write(images_dir.join(name), &resource.data)?;
                self.image_count += 1;
            }
        }
        Ok(())
    }

    /// Finalize all writers.
    ///
    /// - Renders Markdown from the original `doc` (preserves styles, resources,
    ///   and cross-section heading analysis).
    /// - Applies the cleanup pipeline to Markdown output if configured.
    /// - Closes the JSON envelope.
    /// - Flushes the TXT writer.
    ///
    /// Returns a summary of which paths were written.
    pub fn finish(mut self, doc: &Document) -> io::Result<WriteSummary> {
        let mut summary = WriteSummary::default();

        // --- Markdown: render the full document in one pass ---
        if self.want_md {
            if let Some(md_path) = &self.md_path {
                let raw_md = render::render_markdown(doc, &self.render_opts)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

                std::fs::write(md_path, &raw_md)?;
                summary.md_path = Some(md_path.clone());
            }
        }

        // --- TXT: flush ---
        if let (Some(mut txt), Some(txt_path)) = (self.txt.take(), self.txt_path.take()) {
            txt.flush()?;
            summary.txt_path = Some(txt_path);
        }

        // --- JSON: close array+object ---
        if let (Some(mut json), Some(json_path)) = (self.json.take(), self.json_path.take()) {
            write!(json, "]}}")?;
            json.flush()?;
            summary.json_path = Some(json_path);
        }

        // --- Images: propagate count ---
        summary.image_count = self.image_count;

        Ok(summary)
    }
}

/// Summary of files written by the writer.
#[derive(Default)]
pub struct WriteSummary {
    pub md_path: Option<PathBuf>,
    pub txt_path: Option<PathBuf>,
    pub json_path: Option<PathBuf>,
    /// Number of images extracted
    pub image_count: u32,
}
