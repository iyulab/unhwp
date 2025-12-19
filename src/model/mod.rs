//! Document model (Intermediate Representation).
//!
//! This module defines the unified document model that serves as the
//! intermediate representation between format-specific parsers and renderers.

mod document;
mod paragraph;
mod style;
mod table;

pub use document::*;
pub use paragraph::*;
pub use style::*;
pub use table::*;
