//! Paragraph and text run definitions.

use super::{ParagraphStyle, TextStyle};

/// A text run with uniform formatting.
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, Default)]
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
}

/// Reference to an embedded image.
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
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
