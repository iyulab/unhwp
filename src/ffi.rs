//! C-ABI Foreign Function Interface for unhwp.
//!
//! This module provides C-compatible bindings for using unhwp from other languages
//! such as C, C++, C#, Python, and any language with C FFI support.
//!
//! # Memory Management
//!
//! All strings returned by this library must be freed using `unhwp_free_string`.
//! All document handles must be freed using `unhwp_free_document`.
//!
//! # Error Handling
//!
//! Functions that can fail return a null pointer on error. Use `unhwp_last_error`
//! to retrieve the error message.
//!
//! # Example (C)
//!
//! ```c
//! #include <stdio.h>
//! #include "unhwp.h"
//!
//! int main() {
//!     UnhwpDocument* doc = unhwp_parse_file("document.hwp");
//!     if (!doc) {
//!         const char* error = unhwp_last_error();
//!         fprintf(stderr, "Error: %s\n", error);
//!         return 1;
//!     }
//!
//!     char* markdown = unhwp_to_markdown(doc, 0);
//!     if (markdown) {
//!         printf("%s\n", markdown);
//!         unhwp_free_string(markdown);
//!     }
//!
//!     // Every rendering setting, as JSON; absent fields keep their defaults.
//!     char* html_tables = unhwp_to_markdown_with_options(
//!         doc, "{\"table_fallback\": \"html\", \"image_path_prefix\": \"img/\"}");
//!     if (html_tables) {
//!         unhwp_free_string(html_tables);
//!     }
//!
//!     unhwp_free_document(doc);
//!     return 0;
//! }
//! ```

use std::ffi::{c_char, c_int};
use std::ptr;

use unparser_shared::ffi::{self, invalid_argument, FfiError, LastErrorSlot};

use crate::cleanup::CleanupOptions;
use crate::error::ErrorKind;
use crate::model::Document;
use crate::render::{RenderOptions, SectionMarkerStyle, TableFallback};

// Thread-local storage for the last error message and its classification. Declared
// here rather than in `unparser-shared` — see that crate's `ffi` module docs for why the slot
// must live in the consuming crate.
thread_local! {
    static LAST_ERROR: LastErrorSlot = const { LastErrorSlot::new() };
}

unparser_shared::export_last_error_abi!(LAST_ERROR, unhwp_last_error, unhwp_last_error_kind);

/// `unhwp_last_error_kind` value when no error is recorded on this thread.
pub const UNHWP_ERROR_NONE: c_int = unparser_shared::kind::NONE;
/// An argument was null or not valid UTF-8.
pub const UNHWP_ERROR_INVALID_ARGUMENT: c_int = unparser_shared::kind::INVALID_ARGUMENT;
/// A panic was caught at the FFI boundary.
pub const UNHWP_ERROR_PANIC: c_int = unparser_shared::kind::PANIC;
/// The produced output contains an interior NUL byte and cannot cross the C ABI.
pub const UNHWP_ERROR_INVALID_OUTPUT: c_int = unparser_shared::kind::INVALID_OUTPUT;

/// Classify a core error and render its message, for return from a closure.
fn ffi_err(e: crate::Error) -> FfiError {
    (e.kind() as c_int, e.to_string())
}

/// Classify a JSON serialization failure — producing output is rendering.
fn json_err(e: serde_json::Error) -> FfiError {
    (ErrorKind::Render as c_int, e.to_string())
}

unparser_shared::export_handle! {
    /// Opaque handle to a parsed document.
    handle UnhwpDocument { inner: Document },

    /// Free a document handle.
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid pointer returned by `unhwp_parse_file` or `unhwp_parse_bytes`.
    /// - After calling this function, the handle is invalid and must not be used.
    free unhwp_free_document,
}

/// Flags for markdown rendering.
pub const UNHWP_FLAG_FRONTMATTER: u32 = 1;
/// Accepted and without effect: escaping special Markdown characters is the default, as it
/// is for the Rust API. Turn it off with `UNHWP_FLAG_NO_ESCAPE`. The bit is not reused.
pub const UNHWP_FLAG_ESCAPE_SPECIAL: u32 = 2;
/// Accepted and without effect: line breaks inside paragraphs are kept by default. Turn
/// that off with `unhwp_to_markdown_with_options` (`"preserve_line_breaks": false`). The
/// bit is not reused.
pub const UNHWP_FLAG_PARAGRAPH_SPACING: u32 = 4;
pub const UNHWP_FLAG_REFINE: u32 = 8;
/// Write text without escaping special Markdown characters. No flags means the library's
/// defaults, and escaping is one of them.
pub const UNHWP_FLAG_NO_ESCAPE: u32 = 16;

/// Deserializable mirror of [`RenderOptions`] for `unhwp_to_markdown_with_options`.
///
/// The flag bitmask of `unhwp_to_markdown` reaches three of `RenderOptions`' settings; this
/// reaches every one a C-ABI caller can meaningfully set. Every field is optional and an
/// absent one keeps `RenderOptions::default()`'s value; a field this type does not know is
/// an error rather than something silently ignored.
///
/// # Schema
///
/// ```json
/// {
///   "image_path_prefix": string,
///   "table_fallback": "simplified_markdown" | "html" | "skip",
///   "max_heading_level": number,
///   "include_frontmatter": bool,
///   "preserve_line_breaks": bool,
///   "include_empty_paragraphs": bool,
///   "list_marker": string,
///   "paragraph_spacing": bool,
///   "escape_special_chars": bool,
///   "section_markers": "none" | "comment",
///   "cleanup_preset": "minimal" | "standard" | "aggressive" | null,
///   "refine": bool
/// }
/// ```
///
/// `max_heading_level` is 1-6 and `list_marker` one character; anything else is an error.
/// `cleanup_preset` absent or `null` means no cleanup.
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
struct FfiRenderOptions {
    image_path_prefix: Option<String>,
    table_fallback: Option<FfiTableFallback>,
    max_heading_level: Option<u8>,
    include_frontmatter: Option<bool>,
    preserve_line_breaks: Option<bool>,
    include_empty_paragraphs: Option<bool>,
    list_marker: Option<String>,
    paragraph_spacing: Option<bool>,
    escape_special_chars: Option<bool>,
    section_markers: Option<FfiSectionMarkerStyle>,
    cleanup_preset: Option<FfiCleanupPreset>,
    refine: Option<bool>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum FfiTableFallback {
    SimplifiedMarkdown,
    Html,
    Skip,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum FfiSectionMarkerStyle {
    None,
    Comment,
}

/// The cleanup pipeline's three configurations: [`CleanupOptions::minimal`],
/// [`CleanupOptions::default`] and [`CleanupOptions::aggressive`].
#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum FfiCleanupPreset {
    Minimal,
    Standard,
    Aggressive,
}

impl TryFrom<FfiRenderOptions> for RenderOptions {
    type Error = String;

    fn try_from(ffi: FfiRenderOptions) -> Result<Self, Self::Error> {
        let mut options = RenderOptions::default();
        if let Some(prefix) = ffi.image_path_prefix {
            options.image_path_prefix = prefix;
        }
        if let Some(fallback) = ffi.table_fallback {
            options.table_fallback = match fallback {
                FfiTableFallback::SimplifiedMarkdown => TableFallback::SimplifiedMarkdown,
                FfiTableFallback::Html => TableFallback::Html,
                FfiTableFallback::Skip => TableFallback::Skip,
            };
        }
        if let Some(level) = ffi.max_heading_level {
            if !(1..=6).contains(&level) {
                return Err(format!("max_heading_level must be 1-6, got {level}"));
            }
            options.max_heading_level = level;
        }
        if let Some(v) = ffi.include_frontmatter {
            options.include_frontmatter = v;
        }
        if let Some(v) = ffi.preserve_line_breaks {
            options.preserve_line_breaks = v;
        }
        if let Some(v) = ffi.include_empty_paragraphs {
            options.include_empty_paragraphs = v;
        }
        if let Some(marker) = ffi.list_marker {
            let mut chars = marker.chars();
            options.list_marker = match (chars.next(), chars.next()) {
                (Some(c), None) => c,
                _ => return Err(format!("list_marker must be one character, got {marker:?}")),
            };
        }
        if let Some(v) = ffi.paragraph_spacing {
            options.paragraph_spacing = v;
        }
        if let Some(v) = ffi.escape_special_chars {
            options.escape_special_chars = v;
        }
        if let Some(style) = ffi.section_markers {
            options.section_markers = match style {
                FfiSectionMarkerStyle::None => SectionMarkerStyle::None,
                FfiSectionMarkerStyle::Comment => SectionMarkerStyle::Comment,
            };
        }
        if let Some(preset) = ffi.cleanup_preset {
            options.cleanup = Some(match preset {
                FfiCleanupPreset::Minimal => CleanupOptions::minimal(),
                FfiCleanupPreset::Standard => CleanupOptions::default(),
                FfiCleanupPreset::Aggressive => CleanupOptions::aggressive(),
            });
        }
        if ffi.refine == Some(true) {
            options = options.with_refine();
        }
        Ok(options)
    }
}

/// Resolve the `options_json` argument of `unhwp_to_markdown_with_options`.
/// A null pointer means "use defaults" — it is not an error.
///
/// # Safety
/// `ptr` must be null or a valid null-terminated UTF-8 string.
unsafe fn render_options_from_json(ptr: *const c_char) -> Result<RenderOptions, FfiError> {
    if ptr.is_null() {
        return Ok(RenderOptions::default());
    }
    let json = unparser_shared::with_c_str!(ptr)?;
    let ffi = serde_json::from_str::<FfiRenderOptions>(json)
        .map_err(|e| invalid_argument(format!("invalid options_json: {e}")))?;
    RenderOptions::try_from(ffi).map_err(|e| invalid_argument(format!("invalid options_json: {e}")))
}

/// JSON format options.
pub const UNHWP_JSON_PRETTY: c_int = 0;
pub const UNHWP_JSON_COMPACT: c_int = 1;

/// Get the version of the library.
///
/// # Safety
///
/// Returns a static string that must not be freed.
#[no_mangle]
pub extern "C" fn unhwp_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

/// Parse a document from a file path.
///
/// # Safety
///
/// - `path` must be a valid null-terminated UTF-8 string.
/// - Returns null on error. Use `unhwp_last_error` to get the error message.
/// - The returned handle must be freed with `unhwp_free_document`.
#[no_mangle]
pub unsafe extern "C" fn unhwp_parse_file(path: *const c_char) -> *mut UnhwpDocument {
    LAST_ERROR.with(|slot| slot.clear());

    let result: Result<*mut UnhwpDocument, FfiError> = ffi::catch(|| {
        let path_str = unparser_shared::with_c_str!(path)?;

        crate::parse_file(path_str)
            .map(|doc| Box::into_raw(Box::new(UnhwpDocument { inner: doc })))
            .map_err(ffi_err)
    });

    match result {
        Ok(doc) => doc,
        Err(error) => {
            LAST_ERROR.with(|slot| slot.set_error(&error));
            ptr::null_mut()
        }
    }
}

/// Parse a document from a byte buffer.
///
/// # Safety
///
/// - `data` must be a valid pointer to a byte buffer of at least `len` bytes.
/// - Returns null on error. Use `unhwp_last_error` to get the error message.
/// - The returned handle must be freed with `unhwp_free_document`.
#[no_mangle]
pub unsafe extern "C" fn unhwp_parse_bytes(data: *const u8, len: usize) -> *mut UnhwpDocument {
    LAST_ERROR.with(|slot| slot.clear());

    if data.is_null() {
        LAST_ERROR.with(|slot| slot.set_error(&invalid_argument("data is null")));
        return ptr::null_mut();
    }

    let result: Result<*mut UnhwpDocument, FfiError> = ffi::catch(|| {
        let bytes = std::slice::from_raw_parts(data, len);

        crate::parse_bytes(bytes)
            .map(|doc| Box::into_raw(Box::new(UnhwpDocument { inner: doc })))
            .map_err(ffi_err)
    });

    match result {
        Ok(doc) => doc,
        Err(error) => {
            LAST_ERROR.with(|slot| slot.set_error(&error));
            ptr::null_mut()
        }
    }
}

unparser_shared::export_string_getter!(
    /// Convert a document to Markdown.
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid document handle.
    /// - `flags` is a bitwise OR of `UNHWP_FLAG_*` constants.
    /// - Returns null on error. Use `unhwp_last_error` to get the error message.
    /// - The returned string must be freed with `unhwp_free_string`.
    LAST_ERROR,
    unhwp_to_markdown(doc: UnhwpDocument, flags: u32),
    {
        let document = &(*doc).inner;

        let mut options = RenderOptions::default();

        if flags & UNHWP_FLAG_FRONTMATTER != 0 {
            options = options.with_frontmatter();
        }
        if flags & UNHWP_FLAG_NO_ESCAPE != 0 {
            options.escape_special_chars = false;
        }
        if flags & UNHWP_FLAG_PARAGRAPH_SPACING != 0 {
            options.preserve_line_breaks = true;
        }
        if flags & UNHWP_FLAG_REFINE != 0 {
            options = options.with_refine();
        }

        crate::render::render_markdown(document, &options).map_err(ffi_err)
    }
);

unparser_shared::export_string_getter!(
    /// Convert a document to Markdown, with options.
    ///
    /// The counterpart to `unhwp_to_markdown`'s flag bitmask, which reaches only three
    /// settings. `unhwp_to_markdown` keeps working unchanged; this is the surface for
    /// everything the bitmask cannot express.
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid document handle.
    /// - `options_json` may be null (the default options) or a valid null-terminated UTF-8
    ///   JSON object matching [`FfiRenderOptions`]'s schema (see that type's docs).
    /// - Returns null on error, malformed `options_json` included
    ///   (`UNHWP_ERROR_INVALID_ARGUMENT`). Use `unhwp_last_error` to get the error message.
    /// - The returned string must be freed with `unhwp_free_string`.
    LAST_ERROR,
    unhwp_to_markdown_with_options(doc: UnhwpDocument, options_json: *const c_char),
    {
        let document = &(*doc).inner;
        let options = render_options_from_json(options_json)?;
        crate::render::render_markdown(document, &options).map_err(ffi_err)
    }
);

unparser_shared::export_string_getter!(
    /// Convert a document to plain text.
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid document handle.
    /// - Returns null on error. Use `unhwp_last_error` to get the error message.
    /// - The returned string must be freed with `unhwp_free_string`.
    LAST_ERROR,
    unhwp_to_text(doc: UnhwpDocument),
    {
        let document = &(*doc).inner;
        Ok(document.plain_text())
    }
);

unparser_shared::export_string_getter!(
    /// Convert a document to JSON.
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid document handle.
    /// - `format` is one of `UNHWP_JSON_PRETTY` or `UNHWP_JSON_COMPACT`.
    /// - Returns null on error. Use `unhwp_last_error` to get the error message.
    /// - The returned string must be freed with `unhwp_free_string`.
    LAST_ERROR,
    unhwp_to_json(doc: UnhwpDocument, format: c_int),
    {
        let document = &(*doc).inner;
        if format == UNHWP_JSON_COMPACT {
            serde_json::to_string(document).map_err(json_err)
        } else {
            serde_json::to_string_pretty(document).map_err(json_err)
        }
    }
);

unparser_shared::export_string_getter!(
    /// Get the plain text content of a document.
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid document handle.
    /// - Returns null on error.
    /// - The returned string must be freed with `unhwp_free_string`.
    LAST_ERROR,
    unhwp_plain_text(doc: UnhwpDocument),
    {
        let document = &(*doc).inner;
        Ok(document.plain_text())
    }
);

unparser_shared::export_count_getter!(
    /// Get the number of sections in a document.
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid document handle.
    /// - Returns -1 on error.
    LAST_ERROR,
    unhwp_section_count(doc: UnhwpDocument),
    {
        let document = &(*doc).inner;
        Ok(document.sections.len() as c_int)
    }
);

unparser_shared::export_count_getter!(
    /// Get the number of resources in a document.
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid document handle.
    /// - Returns -1 on error.
    LAST_ERROR,
    unhwp_resource_count(doc: UnhwpDocument),
    {
        let document = &(*doc).inner;
        Ok(document.resources.len() as c_int)
    }
);

unparser_shared::export_optional_string_getter!(
    /// Get the document title.
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid document handle.
    /// - Returns null if no title is set — with `unhwp_last_error_kind` left at
    ///   `UNHWP_ERROR_NONE`, since an absent title is not a failure. A null return paired
    ///   with a non-zero kind means the title could not be produced (for instance
    ///   `UNHWP_ERROR_INVALID_OUTPUT` when it holds an interior NUL byte).
    /// - The returned string must be freed with `unhwp_free_string`.
    LAST_ERROR,
    unhwp_get_title(doc: UnhwpDocument),
    {
        let document = &(*doc).inner;
        Ok(document.metadata.title.clone())
    }
);

unparser_shared::export_optional_string_getter!(
    /// Get the document author.
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid document handle.
    /// - Returns null if no author is set — with `unhwp_last_error_kind` left at
    ///   `UNHWP_ERROR_NONE`, since an absent author is not a failure. A null return paired
    ///   with a non-zero kind means the author could not be produced (for instance
    ///   `UNHWP_ERROR_INVALID_OUTPUT` when it holds an interior NUL byte).
    /// - The returned string must be freed with `unhwp_free_string`.
    LAST_ERROR,
    unhwp_get_author(doc: UnhwpDocument),
    {
        let document = &(*doc).inner;
        Ok(document.metadata.author.clone())
    }
);

unparser_shared::export_string_getter!(
    /// Get all resource IDs as a JSON array.
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid document handle.
    /// - Returns null on error. Use `unhwp_last_error` to get the error message.
    /// - The returned string must be freed with `unhwp_free_string`.
    ///
    /// # Returns
    ///
    /// A JSON array of resource IDs, e.g., `["image1.png", "image2.jpg"]`
    LAST_ERROR,
    unhwp_get_resource_ids(doc: UnhwpDocument),
    {
        let document = &(*doc).inner;
        let ids: Vec<&String> = document.resources.keys().collect();
        serde_json::to_string(&ids).map_err(json_err)
    }
);

unparser_shared::export_string_getter!(
    /// Get resource metadata as JSON (without binary data).
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid document handle.
    /// - `resource_id` must be a valid null-terminated UTF-8 string.
    /// - Returns null if resource not found or on error.
    /// - The returned string must be freed with `unhwp_free_string`.
    ///
    /// # Returns
    ///
    /// JSON object with resource metadata:
    /// `{"id":"image1.png","type":"Image","filename":"image1.png","mime_type":"image/png","size":1024}`
    LAST_ERROR,
    unhwp_get_resource_info(doc: UnhwpDocument, resource_id: *const c_char),
    {
        let id_str = unparser_shared::with_c_str!(resource_id)?;

        let document = &(*doc).inner;

        match document.resources.get(id_str) {
            Some(resource) => {
                let info = serde_json::json!({
                    "id": id_str,
                    "type": resource.resource_type,
                    "filename": resource.filename,
                    "mime_type": resource.mime_type,
                    "size": resource.size,
                });
                serde_json::to_string(&info).map_err(json_err)
            }
            None => Err(ffi_err(crate::Error::ResourceNotFound(id_str.to_string()))),
        }
    }
);

unparser_shared::export_bytes_getter!(
    /// Get resource binary data.
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid document handle.
    /// - `resource_id` must be a valid null-terminated UTF-8 string.
    /// - `out_len` must be a valid pointer to receive the data length.
    /// - Returns null if resource not found or on error.
    /// - The returned pointer must be freed with `unhwp_free_bytes`.
    LAST_ERROR,
    unhwp_get_resource_data(doc: UnhwpDocument, resource_id, out out_len),
    {
        let id_str = unparser_shared::ffi::c_str_utf8(resource_id)?;

        let document = &(*doc).inner;

        match document.resources.get(id_str) {
            Some(resource) => Ok(resource.data.clone()),
            None => Err(ffi_err(crate::Error::ResourceNotFound(id_str.to_string()))),
        }
    }
);

unparser_shared::export_string_getter!(
    /// Get every table of the document as delimited text, in reading order, as JSON:
    /// `[{"section","index","text"}]` — the section's number (from 1), the table's place
    /// among that section's tables (from 1), and the table as CSV (RFC 4180), or
    /// tab-separated when `tsv` is non-zero. A merged cell's text is in its top-left position
    /// and the positions it covers are empty; records end with CRLF. `[]` when the document
    /// has no tables.
    ///
    /// # Safety
    ///
    /// - `doc` must be a valid document handle.
    /// - Returns null on error. Use `unhwp_last_error` to get the error message.
    /// - The returned string must be freed with `unhwp_free_string`.
    LAST_ERROR,
    unhwp_tables(doc: UnhwpDocument, tsv: c_int),
    {
        let document = &(*doc).inner;
        let delimiter = if tsv != 0 { '\t' } else { ',' };
        let tables: Vec<serde_json::Value> = document
            .tables()
            .map(|(section, index, table)| {
                serde_json::json!({
                    "section": section,
                    "index": index,
                    "text": table.to_delimited(delimiter),
                })
            })
            .collect();
        serde_json::to_string(&tables).map_err(json_err)
    }
);

unparser_shared::export_free_string!(
    /// Free a string allocated by this library.
    ///
    /// # Safety
    ///
    /// - `s` must be a pointer returned by an unhwp function, or null.
    /// - After calling this function, the pointer is invalid and must not be used.
    unhwp_free_string
);

unparser_shared::export_free_bytes!(
    /// Free binary data allocated by `unhwp_get_resource_data`.
    ///
    /// # Safety
    ///
    /// - `data` must be a pointer returned by `unhwp_get_resource_data`, or null.
    /// - `len` must be the length returned by `unhwp_get_resource_data`.
    /// - After calling this function, the pointer is invalid and must not be used.
    unhwp_free_bytes
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, CString};

    #[test]
    fn test_version() {
        let version = unhwp_version();
        assert!(!version.is_null());
        let version_str = unsafe { CStr::from_ptr(version) }.to_str().unwrap();
        assert!(!version_str.is_empty());
    }

    #[test]
    fn test_parse_null_path() {
        let doc = unsafe { unhwp_parse_file(ptr::null()) };
        assert!(doc.is_null());

        let error = unhwp_last_error();
        assert!(!error.is_null());
        assert_eq!(unhwp_last_error_kind(), UNHWP_ERROR_INVALID_ARGUMENT);
    }

    #[test]
    fn test_parse_invalid_path() {
        let path = CString::new("nonexistent.hwp").unwrap();
        let doc = unsafe { unhwp_parse_file(path.as_ptr()) };
        assert!(doc.is_null());

        let error = unhwp_last_error();
        assert!(!error.is_null());
        assert_eq!(
            unhwp_last_error_kind(),
            crate::ErrorKind::Io as c_int,
            "a missing file is classified as an I/O failure"
        );
    }

    #[test]
    fn test_parse_bytes_null_data_sets_invalid_argument() {
        let doc = unsafe { unhwp_parse_bytes(ptr::null(), 0) };
        assert!(doc.is_null());
        assert_eq!(unhwp_last_error_kind(), UNHWP_ERROR_INVALID_ARGUMENT);
    }

    /// Takes an owned copy of a returned string and frees the original.
    fn take_string(ptr: *mut c_char) -> String {
        assert!(!ptr.is_null());
        let owned = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_owned();
        unsafe { unhwp_free_string(ptr) };
        owned
    }

    /// No flags means the library's defaults, and escaping is one of them; only
    /// `UNHWP_FLAG_NO_ESCAPE` turns it off. The old escape bit is accepted and changes nothing.
    #[test]
    fn escaping_is_the_default_and_no_escape_turns_it_off() {
        let mut document = Document::new();
        let mut section = crate::model::Section::new(0);
        section.push_paragraph(crate::model::Paragraph::text("see [x](y) and a*b*c"));
        document.sections.push(section);
        let doc = Box::into_raw(Box::new(UnhwpDocument { inner: document }));
        let render = |flags| take_string(unsafe { unhwp_to_markdown(doc, flags) });

        let default = render(0);
        assert!(default.contains(r"see \[x\](y) and a\*b\*c"), "{default}");
        assert_eq!(render(UNHWP_FLAG_ESCAPE_SPECIAL), default);
        let plain = render(UNHWP_FLAG_NO_ESCAPE);
        assert!(plain.contains("see [x](y) and a*b*c"), "{plain}");
        assert_eq!(
            UNHWP_FLAG_NO_ESCAPE, 16,
            "flag values are part of the C ABI"
        );

        unsafe { unhwp_free_document(doc) };
    }

    #[test]
    fn test_parse_and_convert() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/two_sections.hwpx"
        );
        let path_cstr = CString::new(path).unwrap();
        let doc = unsafe { unhwp_parse_file(path_cstr.as_ptr()) };
        assert!(!doc.is_null(), "the committed fixture must parse");

        let md = take_string(unsafe { unhwp_to_markdown(doc, 0) });
        assert!(md.contains("Section zero content"), "markdown: {md}");
        assert!(md.contains("Section one content"), "markdown: {md}");

        let text = take_string(unsafe { unhwp_to_text(doc) });
        assert!(text.contains("Section zero content"), "text: {text}");
        assert!(text.contains("Section one content"), "text: {text}");

        let json = take_string(unsafe { unhwp_to_json(doc, UNHWP_JSON_PRETTY) });
        assert!(json.contains("Section one content"), "json: {json}");

        assert_eq!(unsafe { unhwp_section_count(doc) }, 2);

        // A successful call resets the kind, or a caller polling
        // `unhwp_last_error_kind` after success would see a stale failure.
        assert_eq!(unhwp_last_error_kind(), UNHWP_ERROR_NONE);

        // An unknown resource id is classified, not just reported by message.
        let missing_id = CString::new("does-not-exist").unwrap();
        let info = unsafe { unhwp_get_resource_info(doc, missing_id.as_ptr()) };
        assert!(info.is_null());
        assert_eq!(
            unhwp_last_error_kind(),
            crate::ErrorKind::ResourceNotFound as c_int
        );

        // Free document
        unsafe { unhwp_free_document(doc) };
    }

    /// A table of `rows`, one text cell per field.
    fn table_of(rows: &[&[&str]]) -> crate::model::Table {
        let mut table = crate::model::Table::new();
        for fields in rows {
            let mut row = crate::model::TableRow::new();
            row.cells
                .extend(fields.iter().map(|f| crate::model::TableCell::text(*f)));
            table.rows.push(row);
        }
        table
    }

    #[test]
    fn tables_come_as_csv_with_their_place() {
        // Name | Age over Alice | 30 in the first section; «Bob, Jr.» alone in the second,
        // after a paragraph.
        let mut document = Document::new();
        let mut first = crate::model::Section::new(0);
        first.push_table(table_of(&[&["Name", "Age"], &["Alice", "30"]]));
        document.sections.push(first);
        let mut second = crate::model::Section::new(1);
        second.push_paragraph(crate::model::Paragraph::text("before"));
        second.push_table(table_of(&[&["Bob, Jr."]]));
        document.sections.push(second);
        let doc = Box::into_raw(Box::new(UnhwpDocument { inner: document }));

        let json = take_string(unsafe { unhwp_tables(doc, 0) });
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            v,
            serde_json::json!([
                {"section": 1, "index": 1, "text": "Name,Age\r\nAlice,30\r\n"},
                {"section": 2, "index": 1, "text": "\"Bob, Jr.\"\r\n"},
            ]),
            "{json}"
        );
        let tsv: serde_json::Value =
            serde_json::from_str(&take_string(unsafe { unhwp_tables(doc, 1) })).unwrap();
        assert_eq!(tsv[0]["text"], "Name\tAge\r\nAlice\t30\r\n");
        assert_eq!(tsv[1]["text"], "Bob, Jr.\r\n");

        unsafe { unhwp_free_document(doc) };
    }

    #[test]
    fn a_document_without_tables_has_an_empty_list() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/two_sections.hwpx"
        );
        let path_cstr = CString::new(path).unwrap();
        let doc = unsafe { unhwp_parse_file(path_cstr.as_ptr()) };
        assert!(!doc.is_null(), "the committed fixture must parse");
        assert_eq!(take_string(unsafe { unhwp_tables(doc, 0) }), "[]");
        unsafe { unhwp_free_document(doc) };

        assert!(unsafe { unhwp_tables(ptr::null(), 0) }.is_null());
        assert_eq!(unhwp_last_error_kind(), UNHWP_ERROR_INVALID_ARGUMENT);
    }

    /// A document that exercises every setting `unhwp_to_markdown_with_options` reaches:
    /// a title, headings three levels deep, a line break, an empty paragraph, a list,
    /// Markdown syntax in text, an image, a merged table, a private-use character and a
    /// second section.
    fn every_setting_shows() -> *mut UnhwpDocument {
        use crate::model::{
            InlineContent, ListStyle, Paragraph, ParagraphStyle, Section, TableCell, TextRun,
        };

        let mut document = Document::new();
        document.metadata.title = Some("Quarterly report".into());

        let mut first = Section::new(0);
        for (level, text) in [(2, "Overview"), (3, "Regions"), (4, "North")] {
            let mut heading = Paragraph::with_style(ParagraphStyle::heading(level));
            heading
                .content
                .push(InlineContent::Text(TextRun::new(text)));
            first.push_paragraph(heading);
        }
        let mut broken = Paragraph::new();
        broken
            .content
            .push(InlineContent::Text(TextRun::new("first line")));
        broken.content.push(InlineContent::LineBreak);
        broken
            .content
            .push(InlineContent::Text(TextRun::new("second line")));
        first.push_paragraph(broken);
        first.push_paragraph(Paragraph::new());
        first.push_paragraph(Paragraph::text("see [x](y) and a*b*c\u{E000}"));
        for item in ["apples", "pears"] {
            let mut entry = Paragraph::with_style(ParagraphStyle {
                list_style: Some(ListStyle::Unordered),
                ..ParagraphStyle::default()
            });
            entry.content.push(InlineContent::Text(TextRun::new(item)));
            first.push_paragraph(entry);
        }
        let mut figure = Paragraph::new();
        figure
            .content
            .push(InlineContent::Image(crate::model::ImageRef::new(
                "BIN0001.png",
            )));
        first.push_paragraph(figure);
        let mut table = table_of(&[&["Region", "Sales"], &["North", "10"], &["South", "12"]]);
        table.rows[0].cells = vec![{
            let mut merged = TableCell::text("Region and sales");
            merged.colspan = 2;
            merged
        }];
        first.push_table(table);
        document.sections.push(first);

        let mut second = Section::new(1);
        second.push_paragraph(Paragraph::text("closing words"));
        document.sections.push(second);

        Box::into_raw(Box::new(UnhwpDocument { inner: document }))
    }

    fn render_with_options(doc: *const UnhwpDocument, json: &str) -> String {
        let json = CString::new(json).unwrap();
        take_string(unsafe { unhwp_to_markdown_with_options(doc, json.as_ptr()) })
    }

    /// Every field of the options JSON takes effect: each gives the same Markdown as the
    /// Rust options it stands for, and that Markdown differs from the defaults' — so a field
    /// that is parsed and then dropped fails here instead of rendering the defaults.
    #[test]
    fn every_option_takes_effect() {
        let doc = every_setting_shows();
        let default = render_with_options(doc, "{}");
        let base = RenderOptions::default;
        let cases: Vec<(&str, RenderOptions)> = vec![
            (
                r#"{"image_path_prefix": "img/"}"#,
                base().with_image_prefix("img/"),
            ),
            (
                r#"{"table_fallback": "html"}"#,
                base().with_table_fallback(TableFallback::Html),
            ),
            (
                r#"{"table_fallback": "skip"}"#,
                base().with_table_fallback(TableFallback::Skip),
            ),
            (
                r#"{"max_heading_level": 2}"#,
                base().with_max_heading_level(2),
            ),
            (
                r#"{"include_frontmatter": true}"#,
                base().with_frontmatter(),
            ),
            (
                r#"{"preserve_line_breaks": false}"#,
                RenderOptions {
                    preserve_line_breaks: false,
                    ..base()
                },
            ),
            (
                r#"{"include_empty_paragraphs": true}"#,
                RenderOptions {
                    include_empty_paragraphs: true,
                    ..base()
                },
            ),
            (
                r#"{"list_marker": "*"}"#,
                RenderOptions {
                    list_marker: '*',
                    ..base()
                },
            ),
            (
                r#"{"paragraph_spacing": false}"#,
                base().without_paragraph_spacing(),
            ),
            (
                r#"{"escape_special_chars": false}"#,
                RenderOptions {
                    escape_special_chars: false,
                    ..base()
                },
            ),
            (
                r#"{"section_markers": "comment"}"#,
                base().with_section_markers(SectionMarkerStyle::Comment),
            ),
            (
                r#"{"cleanup_preset": "minimal"}"#,
                base().with_minimal_cleanup(),
            ),
            (r#"{"cleanup_preset": "standard"}"#, base().with_cleanup()),
            (
                r#"{"cleanup_preset": "aggressive"}"#,
                base().with_aggressive_cleanup(),
            ),
            (
                r#"{"image_path_prefix": "img\\sub\\", "refine": true}"#,
                base().with_image_prefix("img\\sub\\").with_refine(),
            ),
        ];
        for (json, options) in cases {
            let document = unsafe { &(*doc).inner };
            let expected = crate::render::render_markdown(document, &options).unwrap();
            let actual = render_with_options(doc, json);
            assert_eq!(actual, expected, "{json}");
            assert_ne!(actual, default, "{json} left the output as the defaults'");
        }

        // refine is judged against the same prefix without it: the pass rewrites the
        // backslashes of the image path.
        let unrefined = render_with_options(doc, r#"{"image_path_prefix": "img\\sub\\"}"#);
        let refined = render_with_options(
            doc,
            r#"{"image_path_prefix": "img\\sub\\", "refine": true}"#,
        );
        assert_ne!(refined, unrefined);
        assert_eq!(render_with_options(doc, r#"{"refine": false}"#), default);
        assert_eq!(
            render_with_options(doc, r#"{"cleanup_preset": null}"#),
            default
        );

        unsafe { unhwp_free_document(doc) };
    }

    /// A null `options_json` is the defaults, and so is `{}`: both match `unhwp_to_markdown`
    /// with no flags.
    #[test]
    fn null_options_are_the_defaults() {
        let doc = every_setting_shows();
        let flags = take_string(unsafe { unhwp_to_markdown(doc, 0) });
        let null = take_string(unsafe { unhwp_to_markdown_with_options(doc, ptr::null()) });
        assert_eq!(null, flags);
        assert_eq!(render_with_options(doc, "{}"), flags);
        assert_eq!(unhwp_last_error_kind(), UNHWP_ERROR_NONE);
        unsafe { unhwp_free_document(doc) };
    }

    /// Options that cannot be honoured are refused, not ignored: malformed JSON, a field
    /// this library does not know, a value outside its range.
    #[test]
    fn options_that_cannot_be_honoured_are_invalid_arguments() {
        let doc = every_setting_shows();
        for json in [
            "not json",
            "[]",
            r#"{"image_prefix": "img/"}"#,
            r#"{"table_fallback": "ascii"}"#,
            r#"{"max_heading_level": 0}"#,
            r#"{"max_heading_level": 7}"#,
            r#"{"list_marker": "**"}"#,
            r#"{"list_marker": ""}"#,
            r#"{"cleanup_preset": "default"}"#,
            r#"{"refine": "yes"}"#,
        ] {
            let c = CString::new(json).unwrap();
            let out = unsafe { unhwp_to_markdown_with_options(doc, c.as_ptr()) };
            assert!(out.is_null(), "{json} was accepted");
            assert_eq!(
                unhwp_last_error_kind(),
                UNHWP_ERROR_INVALID_ARGUMENT,
                "{json}"
            );
            let message = unsafe { CStr::from_ptr(unhwp_last_error()) }
                .to_str()
                .unwrap();
            assert!(message.contains("options_json"), "{json}: {message}");
        }
        unsafe { unhwp_free_document(doc) };

        let c = CString::new("{}").unwrap();
        assert!(unsafe { unhwp_to_markdown_with_options(ptr::null(), c.as_ptr()) }.is_null());
        assert_eq!(unhwp_last_error_kind(), UNHWP_ERROR_INVALID_ARGUMENT);
    }

    #[test]
    fn test_null_document_operations() {
        let md = unsafe { unhwp_to_markdown(ptr::null(), 0) };
        assert!(md.is_null());

        let text = unsafe { unhwp_to_text(ptr::null()) };
        assert!(text.is_null());

        let json = unsafe { unhwp_to_json(ptr::null(), 0) };
        assert!(json.is_null());

        let count = unsafe { unhwp_section_count(ptr::null()) };
        assert_eq!(count, -1);

        let res_count = unsafe { unhwp_resource_count(ptr::null()) };
        assert_eq!(res_count, -1);
    }

    /// Serialising a rendered result is rendering, so its failure is a rendering failure
    /// rather than an unclassified one. Pinned because the value crosses the ABI and
    /// because `Other` is worth keeping to mean "this failure carries no classification".
    #[test]
    fn test_json_serialisation_failure_is_a_render_failure() {
        assert_eq!(
            ErrorKind::Render as c_int,
            13,
            "the sibling libraries use 13 for the same reason"
        );
        assert_ne!(ErrorKind::Render as c_int, ErrorKind::Other as c_int);
    }

    /// A null return does not always mean failure: an absent title is not an error.
    /// The kind channel is what lets a caller tell the two apart.
    #[test]
    fn test_absent_metadata_is_not_reported_as_a_failure() {
        let doc = Box::into_raw(Box::new(UnhwpDocument {
            inner: Document::new(),
        }));

        assert!(unsafe { unhwp_get_title(doc) }.is_null());
        assert_eq!(unhwp_last_error_kind(), UNHWP_ERROR_NONE);

        assert!(unsafe { unhwp_get_author(doc) }.is_null());
        assert_eq!(unhwp_last_error_kind(), UNHWP_ERROR_NONE);

        unsafe { unhwp_free_document(doc) };
    }

    /// The counterpart: metadata that *exists* but cannot cross the ABI must not be
    /// reported as absent. Both cases return null, so the kind is the only thing that
    /// separates "there is nothing" from "we could not give it to you".
    #[test]
    fn test_unrepresentable_metadata_is_not_reported_as_absent() {
        let mut document = Document::new();
        document.metadata.title = Some("has\0interior nul".to_string());
        document.metadata.author = Some("also\0bad".to_string());
        let doc = Box::into_raw(Box::new(UnhwpDocument { inner: document }));

        assert!(unsafe { unhwp_get_title(doc) }.is_null());
        assert_eq!(unhwp_last_error_kind(), UNHWP_ERROR_INVALID_OUTPUT);

        assert!(unsafe { unhwp_get_author(doc) }.is_null());
        assert_eq!(unhwp_last_error_kind(), UNHWP_ERROR_INVALID_OUTPUT);

        unsafe { unhwp_free_document(doc) };
    }

    /// `out_len` is written only once the call has reached the point of producing a
    /// buffer. A rejected argument leaves the caller's variable alone, so a caller that
    /// seeded it can tell "not attempted" from "attempted and produced nothing".
    #[test]
    fn test_rejected_arguments_leave_out_len_untouched() {
        let doc = Box::into_raw(Box::new(UnhwpDocument {
            inner: Document::new(),
        }));
        let id = CString::new("BIN0001.jpg").unwrap();
        const SEEDED: usize = 0xDEAD;

        let mut out_len: usize = SEEDED;
        assert!(
            unsafe { unhwp_get_resource_data(ptr::null(), id.as_ptr(), &mut out_len) }.is_null()
        );
        assert_eq!(out_len, SEEDED, "a null document must not write out_len");

        assert!(unsafe { unhwp_get_resource_data(doc, ptr::null(), &mut out_len) }.is_null());
        assert_eq!(out_len, SEEDED, "a null resource_id must not write out_len");

        // A resource that is merely absent *is* looked up, so the length is zeroed.
        assert!(unsafe { unhwp_get_resource_data(doc, id.as_ptr(), &mut out_len) }.is_null());
        assert_eq!(out_len, 0, "a lookup that failed reports zero length");
        assert_eq!(
            unhwp_last_error_kind(),
            crate::ErrorKind::ResourceNotFound as c_int
        );

        unsafe { unhwp_free_document(doc) };
    }

    #[test]
    fn test_free_null() {
        // Should not crash
        unsafe {
            unhwp_free_document(ptr::null_mut());
            unhwp_free_string(ptr::null_mut());
        }
    }
}
