//! Markdown rendering for documents.

mod heading_analyzer;
mod markdown;
mod options;

pub use heading_analyzer::{
    is_korean_chapter_pattern, looks_like_korean_heading, next_korean_chapter, HeadingAnalyzer,
    HeadingConfig, HeadingDecision, KoreanChapterInfo, KoreanChapterType,
};
pub use markdown::MarkdownRenderer;
pub use options::{RenderOptions, SectionMarkerStyle, TableFallback};

use crate::error::Result;
use crate::model::{Document, Metadata};
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

/// Renders YAML frontmatter from document metadata.
///
/// Returns an empty string if there is no meaningful metadata to include.
/// This is the streaming-compatible path — it takes `&Metadata` instead of
/// `&Document`, so it can be called from `write_document_start` before any
/// sections are processed.
pub fn render_frontmatter(metadata: &Metadata) -> String {
    let mut content = String::new();

    if let Some(ref title) = metadata.title {
        content.push_str(&format!("title: \"{}\"\n", escape_yaml(title)));
    }
    if let Some(ref author) = metadata.author {
        content.push_str(&format!("author: \"{}\"\n", escape_yaml(author)));
    }
    if let Some(ref subject) = metadata.subject {
        content.push_str(&format!("description: \"{}\"\n", escape_yaml(subject)));
    }
    if let Some(ref created) = metadata.created {
        content.push_str(&format!("date: \"{}\"\n", created));
    }
    if let Some(ref modified) = metadata.modified {
        content.push_str(&format!("lastmod: \"{}\"\n", modified));
    }
    if !metadata.keywords.is_empty() {
        content.push_str("tags:\n");
        for keyword in &metadata.keywords {
            content.push_str(&format!("  - \"{}\"\n", escape_yaml(keyword)));
        }
    }
    if let Some(ref app) = metadata.creator_app {
        content.push_str(&format!("generator: \"{}\"\n", escape_yaml(app)));
    }

    if content.is_empty() {
        return String::new();
    }

    format!("---\n{}---\n\n", content)
}

/// Escapes special characters for YAML strings.
fn escape_yaml(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
