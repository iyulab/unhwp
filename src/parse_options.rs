//! Parsing options for document extraction.

/// Options for controlling document parsing behavior.
#[derive(Debug, Clone)]
pub struct ParseOptions {
    /// How to handle parsing errors.
    ///
    /// Honoured by the HWP 5.0 and HWPX paths, whose unit of failure is a section. HWP 3.0
    /// has a single body with no skippable unit, so it parses strictly whatever this says --
    /// it fails rather than dropping content silently, which is the direction that cannot
    /// lose anything without saying so.
    pub error_mode: ErrorMode,

    /// What content to extract.
    ///
    /// **Not honoured yet.** Parsing produces the whole document whatever this says; the
    /// only effect of [`Self::text_only`] and [`Self::structure_only`] is the
    /// [`Self::extract_resources`] they also turn off. What a structure-only `Document`
    /// should contain -- paragraphs with empty text, or no paragraphs -- is a decision about
    /// this crate's output that has not been made, and the sibling parser does not answer it
    /// either: `unpdf` gates its text pipeline on `StructureOnly` but treats `TextOnly`
    /// exactly like `Full`.
    pub extract_mode: ExtractMode,

    /// Memory limit in bytes (0 = unlimited).
    ///
    /// **Not enforced.** Nothing reads this field; a document larger than the limit is
    /// parsed like any other. Enforcing it means choosing what to measure (the input, the
    /// peak, the total allocated) and what to raise on the way past it, and neither sibling
    /// parser has such an option to follow.
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
    ///
    /// Today this only turns off [`Self::extract_resources`] -- see [`Self::extract_mode`].
    pub fn text_only(mut self) -> Self {
        self.extract_mode = ExtractMode::TextOnly;
        self.extract_resources = false;
        self
    }

    /// Extracts only document structure (no text content).
    ///
    /// Today this only turns off [`Self::extract_resources`] -- text is still extracted.
    /// See [`Self::extract_mode`].
    pub fn structure_only(mut self) -> Self {
        self.extract_mode = ExtractMode::StructureOnly;
        self.extract_resources = false;
        self
    }

    /// Sets memory limit in megabytes.
    ///
    /// Recorded but not enforced -- see [`Self::memory_limit`].
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
    ///
    /// Applies to HWPX section parsing, the only path that parallelises. HWP 5.0 and HWP 3.0
    /// are single-threaded regardless.
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
