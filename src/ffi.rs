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
//!     unhwp_free_document(doc);
//!     return 0;
//! }
//! ```

use std::ffi::{c_char, c_int};
use std::ptr;

use uncore::ffi::{self, invalid_argument, FfiError, LastErrorSlot};

use crate::error::ErrorKind;
use crate::model::Document;
use crate::render::RenderOptions;

// Thread-local storage for the last error message and its classification. Declared
// here rather than in `uncore` — see that crate's `ffi` module docs for why the slot
// must live in the consuming crate.
thread_local! {
    static LAST_ERROR: LastErrorSlot = const { LastErrorSlot::new() };
}

uncore::export_last_error_abi!(LAST_ERROR, unhwp_last_error, unhwp_last_error_kind);

/// `unhwp_last_error_kind` value when no error is recorded on this thread.
pub const UNHWP_ERROR_NONE: c_int = uncore::kind::NONE;
/// An argument was null or not valid UTF-8.
pub const UNHWP_ERROR_INVALID_ARGUMENT: c_int = uncore::kind::INVALID_ARGUMENT;
/// A panic was caught at the FFI boundary.
pub const UNHWP_ERROR_PANIC: c_int = uncore::kind::PANIC;
/// The produced output contains an interior NUL byte and cannot cross the C ABI.
pub const UNHWP_ERROR_INVALID_OUTPUT: c_int = uncore::kind::INVALID_OUTPUT;

/// Classify a core error and render its message, for return from a closure.
fn ffi_err(e: crate::Error) -> FfiError {
    (e.kind() as c_int, e.to_string())
}

/// Classify a JSON serialization failure — producing output is rendering.
fn json_err(e: serde_json::Error) -> FfiError {
    (ErrorKind::Render as c_int, e.to_string())
}

uncore::export_handle! {
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
pub const UNHWP_FLAG_ESCAPE_SPECIAL: u32 = 2;
pub const UNHWP_FLAG_PARAGRAPH_SPACING: u32 = 4;

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
        let path_str = uncore::with_c_str!(path)?;

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

uncore::export_string_getter!(
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
        if flags & UNHWP_FLAG_ESCAPE_SPECIAL != 0 {
            options.escape_special_chars = true;
        }
        if flags & UNHWP_FLAG_PARAGRAPH_SPACING != 0 {
            options.preserve_line_breaks = true;
        }

        crate::render::render_markdown(document, &options).map_err(ffi_err)
    }
);

uncore::export_string_getter!(
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

uncore::export_string_getter!(
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

uncore::export_string_getter!(
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

uncore::export_count_getter!(
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

uncore::export_count_getter!(
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

uncore::export_optional_string_getter!(
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

uncore::export_optional_string_getter!(
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

uncore::export_string_getter!(
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

uncore::export_string_getter!(
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
        let id_str = uncore::with_c_str!(resource_id)?;

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

uncore::export_bytes_getter!(
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
        let id_str = uncore::ffi::c_str_utf8(resource_id)?;

        let document = &(*doc).inner;

        match document.resources.get(id_str) {
            Some(resource) => Ok(resource.data.clone()),
            None => Err(ffi_err(crate::Error::ResourceNotFound(id_str.to_string()))),
        }
    }
);

uncore::export_free_string!(
    /// Free a string allocated by this library.
    ///
    /// # Safety
    ///
    /// - `s` must be a pointer returned by an unhwp function, or null.
    /// - After calling this function, the pointer is invalid and must not be used.
    unhwp_free_string
);

uncore::export_free_bytes!(
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
    use std::path::Path;

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

    #[test]
    fn test_parse_and_convert() {
        let path = "test-files/sample.hwp";
        if !Path::new(path).exists() {
            return;
        }

        let path_cstr = CString::new(path).unwrap();
        let doc = unsafe { unhwp_parse_file(path_cstr.as_ptr()) };
        assert!(!doc.is_null());

        // Test markdown conversion
        let md = unsafe { unhwp_to_markdown(doc, 0) };
        assert!(!md.is_null());
        unsafe { unhwp_free_string(md) };

        // Test text conversion
        let text = unsafe { unhwp_to_text(doc) };
        assert!(!text.is_null());
        unsafe { unhwp_free_string(text) };

        // Test JSON conversion
        let json = unsafe { unhwp_to_json(doc, UNHWP_JSON_PRETTY) };
        assert!(!json.is_null());
        unsafe { unhwp_free_string(json) };

        // Test section count
        let count = unsafe { unhwp_section_count(doc) };
        assert!(count >= 0);

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
