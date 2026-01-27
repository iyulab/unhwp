//! Paragraph and text run definitions.

use super::{ParagraphStyle, TextStyle};
use serde::Serialize;

/// A text run with uniform formatting.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TextRun {
    /// The text content
    pub text: String,
    /// Text style applied to this run
    pub style: TextStyle,
}

impl TextRun {
    /// Creates a new text run with default style.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: TextStyle::default(),
        }
    }

    /// Creates a new text run with the specified style.
    pub fn with_style(text: impl Into<String>, style: TextStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    /// Returns true if this run is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// Content that can appear within a paragraph.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum InlineContent {
    /// Plain or formatted text
    Text(TextRun),
    /// Line break within paragraph
    LineBreak,
    /// Inline image reference
    Image(ImageRef),
    /// Inline equation
    Equation(Equation),
    /// Footnote reference
    Footnote(String),
    /// Hyperlink
    Link { text: String, url: String },
}

/// A paragraph containing inline content.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Paragraph {
    /// Paragraph style
    pub style: ParagraphStyle,
    /// Content elements within this paragraph
    pub content: Vec<InlineContent>,
}

impl Paragraph {
    /// Creates a new empty paragraph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a paragraph with the specified style.
    pub fn with_style(style: ParagraphStyle) -> Self {
        Self {
            style,
            content: Vec::new(),
        }
    }

    /// Creates a simple paragraph with plain text.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            style: ParagraphStyle::default(),
            content: vec![InlineContent::Text(TextRun::new(text))],
        }
    }

    /// Adds a text run to this paragraph.
    pub fn push_text(&mut self, run: TextRun) {
        self.content.push(InlineContent::Text(run));
    }

    /// Adds a line break to this paragraph.
    pub fn push_line_break(&mut self) {
        self.content.push(InlineContent::LineBreak);
    }

    /// Returns the plain text content of this paragraph.
    pub fn plain_text(&self) -> String {
        let mut result = String::new();
        for item in &self.content {
            match item {
                InlineContent::Text(run) => result.push_str(&run.text),
                InlineContent::LineBreak => result.push('\n'),
                InlineContent::Link { text, .. } => result.push_str(text),
                _ => {}
            }
        }
        result
    }

    /// Returns true if this paragraph is empty.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
            || self.content.iter().all(|c| match c {
                InlineContent::Text(run) => run.is_empty(),
                _ => false,
            })
    }

    /// Returns true if this paragraph has meaningful text content.
    ///
    /// Returns false if the paragraph is empty or contains only:
    /// - Images
    /// - Line breaks
    /// - Empty text runs
    /// - Equations without text
    ///
    /// This is useful for determining whether heading markers should be applied.
    pub fn has_text_content(&self) -> bool {
        self.content.iter().any(|c| match c {
            InlineContent::Text(run) => !run.text.trim().is_empty(),
            InlineContent::Link { text, .. } => !text.trim().is_empty(),
            InlineContent::Footnote(text) => !text.trim().is_empty(),
            _ => false,
        })
    }

    /// Returns true if this paragraph contains only images (and possibly line breaks).
    ///
    /// This is used to determine if a paragraph should not have heading markers.
    pub fn is_image_only(&self) -> bool {
        if self.content.is_empty() {
            return false;
        }

        let has_images = self
            .content
            .iter()
            .any(|c| matches!(c, InlineContent::Image(_)));
        let has_non_empty_text = self.content.iter().any(|c| match c {
            InlineContent::Text(run) => !run.text.trim().is_empty(),
            InlineContent::Link { text, .. } => !text.trim().is_empty(),
            InlineContent::Footnote(text) => !text.trim().is_empty(),
            InlineContent::Equation(_) => true,
            _ => false,
        });

        has_images && !has_non_empty_text
    }

    /// Returns the dominant (most common) font size in this paragraph.
    ///
    /// Calculates weighted by text length - longer runs contribute more.
    /// Returns None if no text runs have font size information.
    pub fn dominant_font_size(&self) -> Option<f32> {
        use std::collections::HashMap;

        let mut size_weights: HashMap<u32, usize> = HashMap::new();

        for item in &self.content {
            if let InlineContent::Text(run) = item {
                if let Some(size) = run.style.font_size {
                    // Convert to integer key (tenths of a point for precision)
                    let key = (size * 10.0) as u32;
                    let text_len = run.text.chars().count();
                    *size_weights.entry(key).or_insert(0) += text_len;
                }
            }
        }

        size_weights
            .into_iter()
            .max_by_key(|(_, weight)| *weight)
            .map(|(key, _)| key as f32 / 10.0)
    }

    /// Returns true if all non-empty text runs in this paragraph are bold.
    pub fn is_all_bold(&self) -> bool {
        let text_runs: Vec<_> = self
            .content
            .iter()
            .filter_map(|c| {
                if let InlineContent::Text(run) = c {
                    if !run.text.trim().is_empty() {
                        return Some(run);
                    }
                }
                None
            })
            .collect();

        // Must have at least one text run
        !text_runs.is_empty() && text_runs.iter().all(|r| r.style.bold)
    }
}

/// Reference to an embedded image.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImageRef {
    /// Resource identifier (key in Document.resources)
    pub id: String,
    /// Alternative text for accessibility
    pub alt_text: Option<String>,
    /// Width in pixels (optional)
    pub width: Option<u32>,
    /// Height in pixels (optional)
    pub height: Option<u32>,
}

impl ImageRef {
    /// Creates a new image reference.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            alt_text: None,
            width: None,
            height: None,
        }
    }
}

/// An equation/formula.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Equation {
    /// Original equation script (EQEdit format for HWP)
    pub script: String,
    /// LaTeX representation (if converted)
    pub latex: Option<String>,
}

impl Equation {
    /// Creates a new equation from script.
    pub fn new(script: impl Into<String>) -> Self {
        Self {
            script: script.into(),
            latex: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dominant_font_size() {
        let mut para = Paragraph::new();

        // Add runs with different sizes
        para.content.push(InlineContent::Text(TextRun::with_style(
            "Short",
            TextStyle {
                font_size: Some(14.0),
                ..Default::default()
            },
        )));
        para.content.push(InlineContent::Text(TextRun::with_style(
            "This is a much longer text that should dominate",
            TextStyle {
                font_size: Some(12.0),
                ..Default::default()
            },
        )));

        // 12.0 should win because of longer text
        assert_eq!(para.dominant_font_size(), Some(12.0));
    }

    #[test]
    fn test_dominant_font_size_empty() {
        let para = Paragraph::new();
        assert_eq!(para.dominant_font_size(), None);
    }

    #[test]
    fn test_dominant_font_size_no_size_info() {
        let mut para = Paragraph::new();
        para.content
            .push(InlineContent::Text(TextRun::new("No size info")));
        assert_eq!(para.dominant_font_size(), None);
    }

    #[test]
    fn test_is_all_bold() {
        let mut para = Paragraph::new();
        para.content.push(InlineContent::Text(TextRun::with_style(
            "Bold text",
            TextStyle {
                bold: true,
                ..Default::default()
            },
        )));
        para.content.push(InlineContent::Text(TextRun::with_style(
            "Also bold",
            TextStyle {
                bold: true,
                ..Default::default()
            },
        )));

        assert!(para.is_all_bold());
    }

    #[test]
    fn test_is_all_bold_mixed() {
        let mut para = Paragraph::new();
        para.content.push(InlineContent::Text(TextRun::with_style(
            "Bold text",
            TextStyle {
                bold: true,
                ..Default::default()
            },
        )));
        para.content
            .push(InlineContent::Text(TextRun::new("Not bold")));

        assert!(!para.is_all_bold());
    }

    #[test]
    fn test_is_all_bold_empty() {
        let para = Paragraph::new();
        assert!(!para.is_all_bold());
    }
}
