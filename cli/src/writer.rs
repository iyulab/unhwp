//! Multi-format fan-out writer for the convert command.
//!
//! Writes Markdown, plain text, and/or JSON outputs in a single streaming
//! pass over the parsed document. All formats are written section-by-section
//! in `write_section`; no full `Document` is ever held in memory.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use unhwp::model::{Block, Metadata, Section, StyleRegistry};
use unhwp::render::{render_frontmatter, MarkdownRenderer, RenderOptions};

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

/// Fan-out writer that streams text, JSON, and Markdown per section.
///
/// Markdown is rendered section-by-section using
/// [`MarkdownRenderer::render_section_standalone`], which does not require
/// a full `Document` — only the section and the style registry are needed.
/// The `HeadingAnalyzer` (statistical font-size inference) is not used on
/// this path; heading levels come from `ParagraphStyle.heading_level`
/// embedded during parsing.
pub struct MultiFormatWriter {
    /// Buffered text writer — streamed per section
    txt: Option<BufWriter<File>>,
    txt_path: Option<PathBuf>,

    /// JSON writer — streamed per section with envelope
    json: Option<BufWriter<File>>,
    json_path: Option<PathBuf>,
    json_first_section: bool,

    /// Markdown writer — streamed per section
    md: Option<BufWriter<File>>,
    md_path: Option<PathBuf>,

    /// Markdown renderer (holds render_opts for per-section rendering)
    md_renderer: Option<MarkdownRenderer>,

    /// Accumulated word count across all sections
    word_count: usize,
}

impl MultiFormatWriter {
    /// Create a new writer for the given output directory and format selection.
    ///
    /// `formats`: which output formats to produce.
    /// `render_opts`: options for Markdown rendering (including cleanup).
    /// `styles`: the [`StyleRegistry`] from `ParseEvent::DocumentStart` — used
    ///   for API consistency; `render_section_standalone` uses embedded heading
    ///   levels from the parsed data.
    pub fn new(
        out_dir: &Path,
        formats: &[OutputFormat],
        render_opts: RenderOptions,
        styles: &StyleRegistry,
    ) -> io::Result<Self> {
        let want_md = formats.contains(&OutputFormat::Markdown);
        let want_txt = formats.contains(&OutputFormat::Text);
        let want_json = formats.contains(&OutputFormat::Json);

        // Suppress unused-variable warning; styles accepted for API symmetry
        let _ = styles;

        // Markdown — open writer now; render section-by-section in write_section
        let (md, md_path, md_renderer) = if want_md {
            let p = out_dir.join("extract.md");
            let f = File::create(&p)?;
            let renderer = MarkdownRenderer::new(render_opts.clone());
            (Some(BufWriter::new(f)), Some(p), Some(renderer))
        } else {
            (None, None, None)
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
            md,
            md_path,
            md_renderer,
            word_count: 0,
        })
    }

    /// Write document-level metadata (called once before any sections).
    ///
    /// For JSON: emits the opening envelope `{"metadata":...,"styles":...,"sections":[`.
    /// For Markdown: emits YAML frontmatter if `include_frontmatter` is enabled.
    pub fn write_document_start(
        &mut self,
        metadata: &Metadata,
        styles: &StyleRegistry,
    ) -> io::Result<()> {
        // JSON: emit opening {"metadata":...,"styles":...,"sections":[
        if let Some(ref mut json) = self.json {
            let meta_json = serde_json::to_string(metadata)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let styles_json = serde_json::to_string(styles)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            write!(
                json,
                "{{\"metadata\":{},\"styles\":{},\"sections\":[",
                meta_json, styles_json
            )?;
        }

        // Markdown: write YAML frontmatter if requested
        if let (Some(ref mut md), Some(ref renderer)) = (&mut self.md, &self.md_renderer) {
            if renderer.options().include_frontmatter {
                let frontmatter = render_frontmatter(metadata);
                if !frontmatter.is_empty() {
                    md.write_all(frontmatter.as_bytes())?;
                }
            }
        }

        Ok(())
    }

    /// Process one section: stream to TXT, JSON, and MD.
    ///
    /// `styles` is passed to `render_section_standalone` for API correctness;
    /// the function primarily uses embedded heading levels from `section`.
    pub fn write_section(&mut self, section: &Section, styles: &StyleRegistry) -> io::Result<()> {
        // TXT: emit plain text lines for this section and accumulate word count
        if let Some(ref mut txt) = self.txt {
            for block in &section.content {
                match block {
                    Block::Paragraph(p) => {
                        let line = p.plain_text();
                        self.word_count += line.split_whitespace().count();
                        writeln!(txt, "{}", line)?;
                    }
                    Block::Table(t) => {
                        for row in &t.rows {
                            for cell in &row.cells {
                                let text = cell.plain_text();
                                if !text.is_empty() {
                                    self.word_count += text.split_whitespace().count();
                                    writeln!(txt, "{}", text)?;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // No TXT output, but still accumulate word count
            for block in &section.content {
                match block {
                    Block::Paragraph(p) => {
                        self.word_count += p.plain_text().split_whitespace().count();
                    }
                    Block::Table(t) => {
                        for row in &t.rows {
                            for cell in &row.cells {
                                let text = cell.plain_text();
                                if !text.is_empty() {
                                    self.word_count += text.split_whitespace().count();
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

        // Markdown: render section standalone and write
        if let (Some(ref mut md), Some(ref renderer)) = (&mut self.md, &self.md_renderer) {
            let rendered = MarkdownRenderer::render_section_standalone(
                section,
                styles,
                renderer.options(),
            )
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
            md.write_all(rendered.as_bytes())?;
        }

        Ok(())
    }

    /// Finalize all writers.
    ///
    /// - Flushes the Markdown writer (already written section-by-section).
    /// - Flushes the TXT writer.
    /// - Closes the JSON array+object envelope.
    ///
    /// Returns a summary of which paths were written.
    pub fn finish(mut self) -> io::Result<WriteSummary> {
        let mut summary = WriteSummary::default();

        // --- Markdown: flush ---
        if let (Some(mut md), Some(md_path)) = (self.md.take(), self.md_path.take()) {
            md.flush()?;
            summary.md_path = Some(md_path);
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

        summary.word_count = self.word_count;

        Ok(summary)
    }
}

/// Summary of files written by the writer.
#[derive(Default)]
pub struct WriteSummary {
    pub md_path: Option<PathBuf>,
    pub txt_path: Option<PathBuf>,
    pub json_path: Option<PathBuf>,
    /// Accumulated word count across all sections
    pub word_count: usize,
}
