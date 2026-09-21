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

    /// Whether to extract the text content of each section.
    ///
    /// `true` by default. Setting it to `false` is "structure only": every section is still
    /// produced, carrying its index, and none of its content blocks are built -- the same
    /// shape the sibling PDF parser gives a page. It is an axis of its own, orthogonal to
    /// [`Self::extract_resources`], so asking for structure decides nothing about images.
    ///
    /// The section is still *read* in this mode, so a section that cannot be read is still
    /// reported; what is skipped is parsing it, so a body that could not be parsed no longer
    /// fails the document. HWP 3.0 differs: its single body is read by the same call that
    /// parses it, so structure-only parsing of that format does not touch the body at all.
    pub extract_text: bool,

    /// Whether to extract binary resources (images, etc.).
    pub extract_resources: bool,

    /// Whether to enable parallel section processing.
    pub parallel: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            error_mode: ErrorMode::Strict,
            extract_text: true,
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

    /// Extracts only document structure: no text content and no binary resources.
    ///
    /// A preset over the two axes -- see [`Self::extract_text`] and
    /// [`Self::extract_resources`], which can be set independently.
    pub fn structure_only(mut self) -> Self {
        self.extract_text = false;
        self.extract_resources = false;
        self
    }

    /// Extracts the text content of each section, or leaves it out ("structure only").
    pub fn with_text(mut self, extract: bool) -> Self {
        self.extract_text = extract;
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
