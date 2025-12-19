//! Parsing options for document extraction.

/// Options for controlling document parsing behavior.
#[derive(Debug, Clone)]
pub struct ParseOptions {
    /// How to handle parsing errors.
    pub error_mode: ErrorMode,

    /// What content to extract.
    pub extract_mode: ExtractMode,

    /// Memory limit in bytes (0 = unlimited).
    pub memory_limit: usize,

    /// Whether to extract binary resources (images, etc.).
    pub extract_resources: bool,

    /// Whether to enable parallel section processing.
    pub parallel: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            error_mode: ErrorMode::Strict,
            extract_mode: ExtractMode::Full,
            memory_limit: 0,
            extract_resources: true,
            parallel: true,
        }
    }
}

impl ParseOptions {
    /// Creates new options with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets lenient error handling (skip invalid sections).
    pub fn lenient(mut self) -> Self {
        self.error_mode = ErrorMode::Lenient;
        self
    }

    /// Sets strict error handling (fail on any error).
    pub fn strict(mut self) -> Self {
        self.error_mode = ErrorMode::Strict;
        self
    }

    /// Extracts only text content (no images, equations).
    pub fn text_only(mut self) -> Self {
        self.extract_mode = ExtractMode::TextOnly;
        self.extract_resources = false;
        self
    }

    /// Extracts only document structure (no text content).
    pub fn structure_only(mut self) -> Self {
        self.extract_mode = ExtractMode::StructureOnly;
        self.extract_resources = false;
        self
    }

    /// Sets memory limit in megabytes.
    pub fn with_memory_limit_mb(mut self, mb: usize) -> Self {
        self.memory_limit = mb * 1024 * 1024;
        self
    }

    /// Disables binary resource extraction.
    pub fn without_resources(mut self) -> Self {
        self.extract_resources = false;
        self
    }

    /// Disables parallel processing.
    pub fn sequential(mut self) -> Self {
        self.parallel = false;
        self
    }

    /// Returns true if errors should be ignored where possible.
    pub fn is_lenient(&self) -> bool {
        matches!(self.error_mode, ErrorMode::Lenient)
    }
}

/// How to handle parsing errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErrorMode {
    /// Fail immediately on any error.
    #[default]
    Strict,
    /// Skip problematic sections and continue parsing.
    Lenient,
}

/// What content to extract from the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtractMode {
    /// Extract all content (text, styles, structure, resources).
    #[default]
    Full,
    /// Extract only text content.
    TextOnly,
    /// Extract only document structure (headings, paragraphs, tables).
    StructureOnly,
}
