//! Style definitions for text and paragraphs.

use serde::Serialize;
use std::collections::HashMap;

/// Text formatting style.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TextStyle {
    /// Bold text
    pub bold: bool,
    /// Italic text
    pub italic: bool,
    /// Underline
    pub underline: bool,
    /// Strikethrough
    pub strikethrough: bool,
    /// Superscript
    pub superscript: bool,
    /// Subscript
    pub subscript: bool,
    /// Font name
    pub font_name: Option<String>,
    /// Font size in points
    pub font_size: Option<f32>,
    /// Text color (RGB hex)
    pub color: Option<String>,
    /// Background/highlight color (RGB hex)
    pub background_color: Option<String>,
}

impl TextStyle {
    /// Creates a new empty text style.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a bold style.
    pub fn bold() -> Self {
        Self {
            bold: true,
            ..Default::default()
        }
    }

    /// Creates an italic style.
    pub fn italic() -> Self {
        Self {
            italic: true,
            ..Default::default()
        }
    }

    /// Returns true if this style has any formatting.
    pub fn has_formatting(&self) -> bool {
        self.bold
            || self.italic
            || self.underline
            || self.strikethrough
            || self.superscript
            || self.subscript
    }
}

/// Paragraph-level style.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ParagraphStyle {
    /// Heading level (0 = normal paragraph, 1-6 = heading levels)
    pub heading_level: u8,
    /// Text alignment
    pub alignment: Alignment,
    /// List style
    pub list_style: Option<ListStyle>,
    /// Indentation level (for nested lists)
    pub indent_level: u8,
    /// Line spacing multiplier (1.0 = single, 1.5, 2.0, etc.)
    pub line_spacing: Option<f32>,
    /// Space before paragraph in points
    pub space_before: Option<f32>,
    /// Space after paragraph in points
    pub space_after: Option<f32>,
}

impl ParagraphStyle {
    /// Creates a new default paragraph style.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a heading style with the specified level.
    pub fn heading(level: u8) -> Self {
        Self {
            heading_level: level.min(6),
            ..Default::default()
        }
    }

    /// Returns true if this is a heading.
    pub fn is_heading(&self) -> bool {
        self.heading_level > 0
    }

    /// Returns true if this is a list item.
    pub fn is_list_item(&self) -> bool {
        self.list_style.is_some()
    }
}

/// Text alignment options.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// List style types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ListStyle {
    /// Ordered list (1, 2, 3, ...)
    Ordered,
    /// Unordered list with bullet
    Unordered,
    /// Custom bullet character
    CustomBullet(char),
}

/// Style registry for resolving style references.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StyleRegistry {
    /// Character (text) styles by ID
    pub char_styles: HashMap<u32, TextStyle>,
    /// Paragraph styles by ID
    pub para_styles: HashMap<u32, ParagraphStyle>,
    /// Named styles
    pub named_styles: HashMap<String, u32>,
}

impl StyleRegistry {
    /// Creates a new empty style registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a character style.
    pub fn register_char_style(&mut self, id: u32, style: TextStyle) {
        self.char_styles.insert(id, style);
    }

    /// Registers a paragraph style.
    pub fn register_para_style(&mut self, id: u32, style: ParagraphStyle) {
        self.para_styles.insert(id, style);
    }

    /// Gets a character style by ID.
    pub fn get_char_style(&self, id: u32) -> Option<&TextStyle> {
        self.char_styles.get(&id)
    }

    /// Gets a paragraph style by ID.
    pub fn get_para_style(&self, id: u32) -> Option<&ParagraphStyle> {
        self.para_styles.get(&id)
    }
}
