//! Streaming document parsing API.
//!
//! This module provides [`parse_file_streaming`], a public API for processing
//! large HWP/HWPX documents with bounded memory. Instead of materializing the
//! entire [`Document`](crate::model::Document) in memory, it emits events for
//! each section as it is parsed, allowing the caller to process and discard
//! each section before the next one is loaded.
//!
//! ## Event order
//!
//! Events are always emitted in the following order:
//!
//! ```text
//! DocumentStart → (SectionParsed | SectionFailed)* → DocumentEnd → ResourceExtracted*
//! ```
//!
//! `ResourceExtracted` events are emitted after `DocumentEnd` so that section
//! memory is fully freed before any large binary data arrives.
//!
//! ## Early termination
//!
//! Return [`ControlFlow::Break(())`](std::ops::ControlFlow::Break) from the
//! callback to stop parsing early. No `DocumentEnd` event is emitted on early
//! break. Any output files written up to that point are **not** cleaned up.

use crate::detect::{detect_format_from_path, FormatType};
use crate::error::Result;
use crate::model::{Metadata, Section, StyleRegistry};
use crate::parse_options::{ErrorMode, ExtractMode, ParseOptions};
use crate::Error;
use std::ops::ControlFlow;
use std::path::Path;

/// An event emitted during streaming document parsing.
///
/// Events are always ordered:
/// ```text
/// DocumentStart → (SectionParsed | SectionFailed)* → DocumentEnd → ResourceExtracted*
/// ```
pub enum ParseEvent<'doc> {
    /// Emitted once before any section events.
    ///
    /// `metadata` and `styles` are valid for the lifetime of the entire stream
    /// (i.e., until `DocumentEnd` or early termination).
    DocumentStart {
        /// Document metadata (title, author, dates, etc.)
        metadata: &'doc Metadata,
        /// Style registry used for rendering
        styles: &'doc StyleRegistry,
        /// Number of sections detected (may be 0 if the manifest is unavailable).
        section_count: usize,
    },

    /// A section was successfully parsed.
    ///
    /// The section is dropped at the end of the callback invocation — its
    /// memory is freed before the next event is emitted.
    SectionParsed(&'doc Section),

    /// A section failed to parse.
    ///
    /// Only emitted when [`SectionStreamOptions::error_mode`] is
    /// [`ErrorMode::Lenient`]. In strict mode, the stream terminates with an
    /// `Err` instead.
    SectionFailed {
        /// Zero-based section index
        index: usize,
        /// The parse error
        error: Error,
    },

    /// Emitted once after all section events and before any `ResourceExtracted`
    /// events. Signals that all text content has been processed.
    DocumentEnd,

    /// A binary resource (image, OLE object) extracted from the document.
    ///
    /// Emitted after `DocumentEnd` when
    /// [`SectionStreamOptions::extract_resources`] is `true`. The `data` is
    /// owned and freed when the callback returns.
    ResourceExtracted {
        /// Resource name/filename (e.g., `"BIN0001.png"`)
        name: String,
        /// Raw binary data
        data: Vec<u8>,
    },
}

/// Options for streaming document parsing.
#[derive(Debug, Clone)]
pub struct SectionStreamOptions {
    /// How to handle per-section parse errors.
    pub error_mode: ErrorMode,

    /// What content to extract from each section.
    pub extract_mode: ExtractMode,

    /// Whether to read binary resources (images) from BinData.
    ///
    /// When `true`, each resource is emitted as a [`ParseEvent::ResourceExtracted`]
    /// event after [`ParseEvent::DocumentEnd`].
    pub extract_resources: bool,
}

impl Default for SectionStreamOptions {
    fn default() -> Self {
        Self {
            error_mode: ErrorMode::Strict,
            extract_mode: ExtractMode::Full,
            extract_resources: true,
        }
    }
}

impl From<&ParseOptions> for SectionStreamOptions {
    fn from(opts: &ParseOptions) -> Self {
        Self {
            error_mode: opts.error_mode,
            extract_mode: opts.extract_mode,
            extract_resources: opts.extract_resources,
        }
    }
}

/// Parses a document from a file, emitting events for each section.
///
/// `f` is called once per event in strict order. Return
/// [`ControlFlow::Break(())`](std::ops::ControlFlow::Break) to stop parsing
/// early (no `DocumentEnd` is emitted on early break).
/// Return [`ControlFlow::Continue(())`](std::ops::ControlFlow::Continue) to
/// continue.
///
/// In strict mode ([`ErrorMode::Strict`]), any section parse error terminates
/// the stream and returns `Err`. In lenient mode, [`ParseEvent::SectionFailed`]
/// is emitted and parsing continues.
///
/// # Example
///
/// ```no_run
/// use std::ops::ControlFlow;
/// use unhwp::{parse_file_streaming, ParseEvent, SectionStreamOptions};
///
/// parse_file_streaming("doc.hwp", SectionStreamOptions::default(), |event| {
///     match event {
///         ParseEvent::DocumentStart { metadata, .. } => {
///             println!("Title: {:?}", metadata.title);
///         }
///         ParseEvent::SectionParsed(section) => {
///             println!("Section {}: {} blocks", section.index, section.content.len());
///         }
///         ParseEvent::DocumentEnd => {}
///         ParseEvent::SectionFailed { index, error } => {
///             eprintln!("Section {} failed: {}", index, error);
///         }
///         ParseEvent::ResourceExtracted { name, .. } => {
///             println!("Resource: {}", name);
///         }
///     }
///     ControlFlow::Continue(())
/// })?;
/// # Ok::<(), unhwp::Error>(())
/// ```
pub fn parse_file_streaming<F>(
    path: impl AsRef<Path>,
    opts: SectionStreamOptions,
    f: F,
) -> Result<()>
where
    F: FnMut(ParseEvent<'_>) -> ControlFlow<()>,
{
    let path = path.as_ref();
    let format = detect_format_from_path(path)?;

    match format {
        #[cfg(feature = "hwp5")]
        FormatType::Hwp5 => {
            let mut parser = crate::hwp5::Hwp5Parser::open(path)?;
            parser.for_each_section(opts, f)
        }
        #[cfg(feature = "hwpx")]
        FormatType::Hwpx => {
            let mut parser = crate::hwpx::HwpxParser::open(path)?;
            parser.for_each_section(opts, f)
        }
        #[cfg(feature = "hwp3")]
        FormatType::Hwp3 => Err(Error::UnsupportedFormat(
            "streaming not yet supported for HWP 3.x".into(),
        )),
        #[cfg(not(feature = "hwp3"))]
        FormatType::Hwp3 => Err(Error::UnsupportedFormat(
            "HWP 3.x support requires 'hwp3' feature".into(),
        )),
        #[allow(unreachable_patterns)]
        _ => Err(Error::UnsupportedFormat(format.to_string())),
    }
}
