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

use std::cell::RefCell;
use std::ffi::{c_char, c_int, CStr, CString};
use std::panic::catch_unwind;
use std::ptr;

use crate::model::Document;
use crate::render::RenderOptions;

// Thread-local storage for the last error message.
thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

/// Set the last error message.
fn set_last_error(msg: &str) {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = CString::new(msg).ok();
    });
}

/// Clear the last error message.
fn clear_last_error() {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = None;
    });
}

/// Opaque handle to a parsed document.
#[repr(C)]
pub struct UnhwpDocument {
    inner: Document,
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

/// Get the last error message.
///
/// # Safety
///
/// Returns a pointer to a thread-local error string. The pointer is valid until
/// the next call to any unhwp function on the same thread.
#[no_mangle]
pub extern "C" fn unhwp_last_error() -> *const c_char {
    LAST_ERROR.with(|e| {
        e.borrow()
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(ptr::null())
    })
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
    clear_last_error();

    if path.is_null() {
        set_last_error("path is null");
        return ptr::null_mut();
    }

    let result = catch_unwind(|| {
        let path_str = CStr::from_ptr(path).to_str().map_err(|e| e.to_string())?;

        crate::parse_file(path_str)
            .map(|doc| Box::into_raw(Box::new(UnhwpDocument { inner: doc })))
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(Ok(doc)) => doc,
        Ok(Err(e)) => {
            set_last_error(&e);
            ptr::null_mut()
        }
        Err(_) => {
            set_last_error("panic occurred during parsing");
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
    clear_last_error();

    if data.is_null() {
        set_last_error("data is null");
        return ptr::null_mut();
    }

    let result = catch_unwind(|| {
        let bytes = std::slice::from_raw_parts(data, len);

        crate::parse_bytes(bytes)
            .map(|doc| Box::into_raw(Box::new(UnhwpDocument { inner: doc })))
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(Ok(doc)) => doc,
        Ok(Err(e)) => {
            set_last_error(&e);
            ptr::null_mut()
        }
        Err(_) => {
            set_last_error("panic occurred during parsing");
            ptr::null_mut()
        }
    }
}

/// Free a document handle.
///
/// # Safety
///
/// - `doc` must be a valid pointer returned by `unhwp_parse_file` or `unhwp_parse_bytes`.
/// - After calling this function, the handle is invalid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn unhwp_free_document(doc: *mut UnhwpDocument) {
    if !doc.is_null() {
        let _ = Box::from_raw(doc);
    }
}

/// Convert a document to Markdown.
///
/// # Safety
///
/// - `doc` must be a valid document handle.
/// - `flags` is a bitwise OR of `UNHWP_FLAG_*` constants.
/// - Returns null on error. Use `unhwp_last_error` to get the error message.
/// - The returned string must be freed with `unhwp_free_string`.
#[no_mangle]
pub unsafe extern "C" fn unhwp_to_markdown(doc: *const UnhwpDocument, flags: u32) -> *mut c_char {
    clear_last_error();

    if doc.is_null() {
        set_last_error("document is null");
        return ptr::null_mut();
    }

    let result = catch_unwind(|| {
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

        crate::render::render_markdown(document, &options).map_err(|e| e.to_string())
    });

    match result {
        Ok(Ok(md)) => match CString::new(md) {
            Ok(s) => s.into_raw(),
            Err(_) => {
                set_last_error("output contains null byte");
                ptr::null_mut()
            }
        },
        Ok(Err(e)) => {
            set_last_error(&e);
            ptr::null_mut()
        }
        Err(_) => {
            set_last_error("panic occurred during rendering");
            ptr::null_mut()
        }
    }
}

/// Convert a document to plain text.
///
/// # Safety
///
/// - `doc` must be a valid document handle.
/// - Returns null on error. Use `unhwp_last_error` to get the error message.
/// - The returned string must be freed with `unhwp_free_string`.
#[no_mangle]
pub unsafe extern "C" fn unhwp_to_text(doc: *const UnhwpDocument) -> *mut c_char {
    clear_last_error();

    if doc.is_null() {
        set_last_error("document is null");
        return ptr::null_mut();
    }

    let result = catch_unwind(|| {
        let document = &(*doc).inner;
        document.plain_text()
    });

    match result {
        Ok(text) => match CString::new(text) {
            Ok(s) => s.into_raw(),
            Err(_) => {
                set_last_error("output contains null byte");
                ptr::null_mut()
            }
        },
        Err(_) => {
            set_last_error("panic occurred during rendering");
            ptr::null_mut()
        }
    }
}

/// Convert a document to JSON.
///
/// # Safety
///
/// - `doc` must be a valid document handle.
/// - `format` is one of `UNHWP_JSON_PRETTY` or `UNHWP_JSON_COMPACT`.
/// - Returns null on error. Use `unhwp_last_error` to get the error message.
/// - The returned string must be freed with `unhwp_free_string`.
#[no_mangle]
pub unsafe extern "C" fn unhwp_to_json(doc: *const UnhwpDocument, format: c_int) -> *mut c_char {
    clear_last_error();

    if doc.is_null() {
        set_last_error("document is null");
        return ptr::null_mut();
    }

    let result = catch_unwind(|| {
        let document = &(*doc).inner;
        if format == UNHWP_JSON_COMPACT {
            serde_json::to_string(document).map_err(|e| e.to_string())
        } else {
            serde_json::to_string_pretty(document).map_err(|e| e.to_string())
        }
    });

    match result {
        Ok(Ok(json)) => match CString::new(json) {
            Ok(s) => s.into_raw(),
            Err(_) => {
                set_last_error("output contains null byte");
                ptr::null_mut()
            }
        },
        Ok(Err(e)) => {
            set_last_error(&e);
            ptr::null_mut()
        }
        Err(_) => {
            set_last_error("panic occurred during rendering");
            ptr::null_mut()
        }
    }
}

/// Get the plain text content of a document.
///
/// # Safety
///
/// - `doc` must be a valid document handle.
/// - Returns null on error.
/// - The returned string must be freed with `unhwp_free_string`.
#[no_mangle]
pub unsafe extern "C" fn unhwp_plain_text(doc: *const UnhwpDocument) -> *mut c_char {
    clear_last_error();

    if doc.is_null() {
        set_last_error("document is null");
        return ptr::null_mut();
    }

    let result = catch_unwind(|| {
        let document = &(*doc).inner;
        document.plain_text()
    });

    match result {
        Ok(text) => match CString::new(text) {
            Ok(s) => s.into_raw(),
            Err(_) => {
                set_last_error("output contains null byte");
                ptr::null_mut()
            }
        },
        Err(_) => {
            set_last_error("panic occurred");
            ptr::null_mut()
        }
    }
}

/// Get the number of sections in a document.
///
/// # Safety
///
/// - `doc` must be a valid document handle.
/// - Returns -1 on error.
#[no_mangle]
pub unsafe extern "C" fn unhwp_section_count(doc: *const UnhwpDocument) -> c_int {
    if doc.is_null() {
        set_last_error("document is null");
        return -1;
    }

    match catch_unwind(|| (*doc).inner.sections.len() as c_int) {
        Ok(count) => count,
        Err(_) => {
            set_last_error("panic occurred");
            -1
        }
    }
}

/// Get the number of resources in a document.
///
/// # Safety
///
/// - `doc` must be a valid document handle.
/// - Returns -1 on error.
#[no_mangle]
pub unsafe extern "C" fn unhwp_resource_count(doc: *const UnhwpDocument) -> c_int {
    if doc.is_null() {
        set_last_error("document is null");
        return -1;
    }

    match catch_unwind(|| (*doc).inner.resources.len() as c_int) {
        Ok(count) => count,
        Err(_) => {
            set_last_error("panic occurred");
            -1
        }
    }
}

/// Get the document title.
///
/// # Safety
///
/// - `doc` must be a valid document handle.
/// - Returns null if no title is set.
/// - The returned string must be freed with `unhwp_free_string`.
#[no_mangle]
pub unsafe extern "C" fn unhwp_get_title(doc: *const UnhwpDocument) -> *mut c_char {
    clear_last_error();

    if doc.is_null() {
        set_last_error("document is null");
        return ptr::null_mut();
    }

    let result = catch_unwind(|| {
        (*doc)
            .inner
            .metadata
            .title
            .as_ref()
            .and_then(|t| CString::new(t.as_str()).ok())
    });

    match result {
        Ok(Some(s)) => s.into_raw(),
        Ok(None) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic occurred");
            ptr::null_mut()
        }
    }
}

/// Get the document author.
///
/// # Safety
///
/// - `doc` must be a valid document handle.
/// - Returns null if no author is set.
/// - The returned string must be freed with `unhwp_free_string`.
#[no_mangle]
pub unsafe extern "C" fn unhwp_get_author(doc: *const UnhwpDocument) -> *mut c_char {
    clear_last_error();

    if doc.is_null() {
        set_last_error("document is null");
        return ptr::null_mut();
    }

    let result = catch_unwind(|| {
        (*doc)
            .inner
            .metadata
            .author
            .as_ref()
            .and_then(|a| CString::new(a.as_str()).ok())
    });

    match result {
        Ok(Some(s)) => s.into_raw(),
        Ok(None) => ptr::null_mut(),
        Err(_) => {
            set_last_error("panic occurred");
            ptr::null_mut()
        }
    }
}

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
#[no_mangle]
pub unsafe extern "C" fn unhwp_get_resource_ids(doc: *const UnhwpDocument) -> *mut c_char {
    clear_last_error();

    if doc.is_null() {
        set_last_error("document is null");
        return ptr::null_mut();
    }

    let result = catch_unwind(|| {
        let document = &(*doc).inner;
        let ids: Vec<&String> = document.resources.keys().collect();
        serde_json::to_string(&ids).map_err(|e| e.to_string())
    });

    match result {
        Ok(Ok(json)) => match CString::new(json) {
            Ok(s) => s.into_raw(),
            Err(_) => {
                set_last_error("output contains null byte");
                ptr::null_mut()
            }
        },
        Ok(Err(e)) => {
            set_last_error(&e);
            ptr::null_mut()
        }
        Err(_) => {
            set_last_error("panic occurred");
            ptr::null_mut()
        }
    }
}

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
#[no_mangle]
pub unsafe extern "C" fn unhwp_get_resource_info(
    doc: *const UnhwpDocument,
    resource_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    if doc.is_null() {
        set_last_error("document is null");
        return ptr::null_mut();
    }

    if resource_id.is_null() {
        set_last_error("resource_id is null");
        return ptr::null_mut();
    }

    let result = catch_unwind(|| {
        let id_str = CStr::from_ptr(resource_id)
            .to_str()
            .map_err(|e| e.to_string())?;

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
                serde_json::to_string(&info).map_err(|e| e.to_string())
            }
            None => Err(format!("resource not found: {}", id_str)),
        }
    });

    match result {
        Ok(Ok(json)) => match CString::new(json) {
            Ok(s) => s.into_raw(),
            Err(_) => {
                set_last_error("output contains null byte");
                ptr::null_mut()
            }
        },
        Ok(Err(e)) => {
            set_last_error(&e);
            ptr::null_mut()
        }
        Err(_) => {
            set_last_error("panic occurred");
            ptr::null_mut()
        }
    }
}

/// Get resource binary data.
///
/// # Safety
///
/// - `doc` must be a valid document handle.
/// - `resource_id` must be a valid null-terminated UTF-8 string.
/// - `out_len` must be a valid pointer to receive the data length.
/// - Returns null if resource not found or on error.
/// - The returned pointer must be freed with `unhwp_free_bytes`.
#[no_mangle]
pub unsafe extern "C" fn unhwp_get_resource_data(
    doc: *const UnhwpDocument,
    resource_id: *const c_char,
    out_len: *mut usize,
) -> *mut u8 {
    clear_last_error();

    if doc.is_null() {
        set_last_error("document is null");
        return ptr::null_mut();
    }

    if resource_id.is_null() {
        set_last_error("resource_id is null");
        return ptr::null_mut();
    }

    if out_len.is_null() {
        set_last_error("out_len is null");
        return ptr::null_mut();
    }

    let result = catch_unwind(|| {
        let id_str = CStr::from_ptr(resource_id)
            .to_str()
            .map_err(|e| e.to_string())?;

        let document = &(*doc).inner;

        match document.resources.get(id_str) {
            Some(resource) => {
                let data = resource.data.clone();
                let len = data.len();
                let boxed = data.into_boxed_slice();
                let ptr = Box::into_raw(boxed) as *mut u8;
                Ok((ptr, len))
            }
            None => Err(format!("resource not found: {}", id_str)),
        }
    });

    match result {
        Ok(Ok((ptr, len))) => {
            *out_len = len;
            ptr
        }
        Ok(Err(e)) => {
            set_last_error(&e);
            *out_len = 0;
            ptr::null_mut()
        }
        Err(_) => {
            set_last_error("panic occurred");
            *out_len = 0;
            ptr::null_mut()
        }
    }
}

/// Free a string allocated by this library.
///
/// # Safety
///
/// - `s` must be a pointer returned by an unhwp function, or null.
/// - After calling this function, the pointer is invalid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn unhwp_free_string(s: *mut c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s);
    }
}

/// Free binary data allocated by `unhwp_get_resource_data`.
///
/// # Safety
///
/// - `data` must be a pointer returned by `unhwp_get_resource_data`, or null.
/// - `len` must be the length returned by `unhwp_get_resource_data`.
/// - After calling this function, the pointer is invalid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn unhwp_free_bytes(data: *mut u8, len: usize) {
    if !data.is_null() && len > 0 {
        let _ = Box::from_raw(std::ptr::slice_from_raw_parts_mut(data, len));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
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
    }

    #[test]
    fn test_parse_invalid_path() {
        let path = CString::new("nonexistent.hwp").unwrap();
        let doc = unsafe { unhwp_parse_file(path.as_ptr()) };
        assert!(doc.is_null());

        let error = unhwp_last_error();
        assert!(!error.is_null());
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

    #[test]
    fn test_free_null() {
        // Should not crash
        unsafe {
            unhwp_free_document(ptr::null_mut());
            unhwp_free_string(ptr::null_mut());
        }
    }
}
