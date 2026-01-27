//! Markdown rendering for documents.

mod heading_analyzer;
mod markdown;
mod options;

pub use heading_analyzer::{
    is_korean_chapter_pattern, looks_like_korean_heading, next_korean_chapter, HeadingAnalyzer,
    HeadingConfig, HeadingDecision, KoreanChapterInfo, KoreanChapterType,
};
pub use markdown::MarkdownRenderer;
pub use options::{RenderOptions, TableFallback};

use crate::error::Result;
use crate::model::Document;
use std::io::Write;
use std::path::Path;

/// Renders a document to Markdown.
pub fn render_markdown(document: &Document, options: &RenderOptions) -> Result<String> {
    let renderer = MarkdownRenderer::new(options.clone());
    renderer.render(document)
}

/// Renders a document to Markdown and writes to a file.
pub fn render_to_file(
    document: &Document,
    path: impl AsRef<Path>,
    options: &RenderOptions,
) -> Result<()> {
    let content = render_markdown(document, options)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Renders a document to Markdown and writes to a writer.
pub fn render_to_writer<W: Write>(
    document: &Document,
    writer: &mut W,
    options: &RenderOptions,
) -> Result<()> {
    let content = render_markdown(document, options)?;
    writer.write_all(content.as_bytes())?;
    Ok(())
}
