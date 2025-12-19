#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! # FFI Bindings for C# Interop
//!
//! This module provides C-compatible FFI functions for integrating unhwp
//! with C#, .NET, and other languages via P/Invoke.
//!
//! # Safety
//! All functions that accept raw pointers require the caller to ensure
//! the pointers are valid. This is enforced by the FFI contract.
//!
//! ## Usage Pattern
//!
//! ```c
//! // Parse document
//! UnhwpResult* result = unhwp_parse("document.hwp", NULL);
//!
//! // Access results
//! const char* markdown = unhwp_result_get_markdown(result);
//! const char* text = unhwp_result_get_text(result);
//!
//! // Access images
//! int count = unhwp_result_get_image_count(result);
//! for (int i = 0; i < count; i++) {
//!     UnhwpImage img = unhwp_result_get_image(result, i);
//!     // use img.name, img.data, img.data_len
//! }
//!
//! // Free when done
//! unhwp_result_free(result);
//! ```
//!
//! ## Memory Management
//!
//! - Call `unhwp_result_free()` to free the result and all associated data.
//! - Call `unhwp_free_string()` for strings from simple functions.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use crate::cleanup::CleanupOptions;
use crate::model::Document;
use crate::render::RenderOptions;
use crate::{detect_format_from_path, parse_file, render, FormatType};

// ============================================================================
// Result Codes
// ============================================================================

/// Success
pub const UNHWP_OK: i32 = 0;
/// File not found
pub const UNHWP_ERR_FILE_NOT_FOUND: i32 = -1;
/// Parse error
pub const UNHWP_ERR_PARSE: i32 = -2;
/// Render error
pub const UNHWP_ERR_RENDER: i32 = -3;
/// Invalid argument (null pointer)
pub const UNHWP_ERR_INVALID_ARG: i32 = -4;
/// Unsupported format
pub const UNHWP_ERR_UNSUPPORTED: i32 = -5;
/// Unknown error
pub const UNHWP_ERR_UNKNOWN: i32 = -99;

// ============================================================================
// Format Detection
// ============================================================================

/// Format type constants
pub const FORMAT_UNKNOWN: i32 = 0;
pub const FORMAT_HWP5: i32 = 1;
pub const FORMAT_HWPX: i32 = 2;
pub const FORMAT_HWP3: i32 = 3;

/// Detects the format of an HWP/HWPX file.
///
/// # Parameters
/// - `path`: Null-terminated UTF-8 file path
///
/// # Returns
/// - FORMAT_HWP5 (1): HWP 5.0 binary format
/// - FORMAT_HWPX (2): HWPX XML format
/// - FORMAT_HWP3 (3): Legacy HWP 3.x format
/// - FORMAT_UNKNOWN (0): Unknown or error
/// # Safety
/// The `path` pointer must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn unhwp_detect_format(path: *const c_char) -> i32 {
    if path.is_null() {
        return FORMAT_UNKNOWN;
    }

    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return FORMAT_UNKNOWN,
    };

    match detect_format_from_path(path_str) {
        Ok(FormatType::Hwp5) => FORMAT_HWP5,
        Ok(FormatType::Hwpx) => FORMAT_HWPX,
        Ok(FormatType::Hwp3) => FORMAT_HWP3,
        _ => FORMAT_UNKNOWN,
    }
}

// ============================================================================
// Cleanup Options (C-compatible struct)
// ============================================================================

/// C-compatible cleanup options structure.
#[repr(C)]
pub struct UnhwpCleanupOptions {
    /// Enable cleanup pipeline (0 = disabled, 1 = enabled)
    pub enabled: i32,
    /// Cleanup preset (0 = default, 1 = minimal, 2 = aggressive)
    pub preset: i32,
    /// Enable mojibake detection (0 = disabled, 1 = enabled)
    pub detect_mojibake: i32,
    /// Preserve YAML frontmatter (0 = disabled, 1 = enabled)
    pub preserve_frontmatter: i32,
}

impl Default for UnhwpCleanupOptions {
    fn default() -> Self {
        Self {
            enabled: 0,
            preset: 0,
            detect_mojibake: 1,
            preserve_frontmatter: 1,
        }
    }
}

impl UnhwpCleanupOptions {
    fn to_rust_options(&self) -> Option<CleanupOptions> {
        if self.enabled == 0 {
            return None;
        }

        let mut options = match self.preset {
            1 => CleanupOptions::minimal(),
            2 => CleanupOptions::aggressive(),
            _ => CleanupOptions::default(),
        };

        options.detect_mojibake = self.detect_mojibake != 0;
        options.preserve_frontmatter = self.preserve_frontmatter != 0;

        Some(options)
    }
}

/// Creates default cleanup options.
#[no_mangle]
pub extern "C" fn unhwp_cleanup_options_default() -> UnhwpCleanupOptions {
    UnhwpCleanupOptions::default()
}

/// Creates cleanup options with enabled cleanup.
#[no_mangle]
pub extern "C" fn unhwp_cleanup_options_enabled() -> UnhwpCleanupOptions {
    UnhwpCleanupOptions {
        enabled: 1,
        preset: 0,
        detect_mojibake: 1,
        preserve_frontmatter: 1,
    }
}

// ============================================================================
// Render Options (C-compatible struct)
// ============================================================================

/// C-compatible render options structure.
#[repr(C)]
pub struct UnhwpRenderOptions {
    /// Include YAML frontmatter (0 = disabled, 1 = enabled)
    pub include_frontmatter: i32,
    /// Image path prefix (null-terminated string, or NULL for default)
    pub image_path_prefix: *const c_char,
    /// Table fallback mode (0 = simplified markdown, 1 = HTML, 2 = skip)
    pub table_fallback: i32,
    /// Preserve line breaks (0 = disabled, 1 = enabled)
    pub preserve_line_breaks: i32,
    /// Escape special markdown characters (0 = disabled, 1 = enabled)
    pub escape_special_chars: i32,
}

impl Default for UnhwpRenderOptions {
    fn default() -> Self {
        Self {
            include_frontmatter: 1,
            image_path_prefix: ptr::null(),
            table_fallback: 0,
            preserve_line_breaks: 0,
            escape_special_chars: 1,
        }
    }
}

impl UnhwpRenderOptions {
    fn to_rust_options(&self) -> RenderOptions {
        let image_path_prefix = if !self.image_path_prefix.is_null() {
            unsafe { CStr::from_ptr(self.image_path_prefix) }
                .to_str()
                .map(|s| s.to_string())
                .unwrap_or_default()
        } else {
            String::new()
        };

        let table_fallback = match self.table_fallback {
            1 => crate::render::TableFallback::Html,
            2 => crate::render::TableFallback::Skip,
            _ => crate::render::TableFallback::SimplifiedMarkdown,
        };

        RenderOptions {
            include_frontmatter: self.include_frontmatter != 0,
            preserve_line_breaks: self.preserve_line_breaks != 0,
            escape_special_chars: self.escape_special_chars != 0,
            image_path_prefix,
            table_fallback,
            ..Default::default()
        }
    }
}

/// Creates default render options.
#[no_mangle]
pub extern "C" fn unhwp_render_options_default() -> UnhwpRenderOptions {
    UnhwpRenderOptions::default()
}

// ============================================================================
// Core Functions
// ============================================================================

/// Converts an HWP/HWPX file to Markdown.
///
/// # Parameters
/// - `path`: Null-terminated UTF-8 file path
/// - `out_markdown`: Pointer to receive the resulting markdown string
/// - `out_error`: Pointer to receive error message (optional, can be NULL)
///
/// # Returns
/// - UNHWP_OK (0) on success
/// - Error code on failure
///
/// # Memory
/// - On success, `*out_markdown` is set to a newly allocated string.
///   Caller must free it using `unhwp_free_string`.
/// - On failure, `*out_error` may be set to an error message.
///   Caller must free it using `unhwp_free_string`.
///
/// # Safety
/// All pointer parameters must be valid or null where allowed.
#[no_mangle]
pub unsafe extern "C" fn unhwp_to_markdown(
    path: *const c_char,
    out_markdown: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    let options = UnhwpRenderOptions::default();
    let cleanup = UnhwpCleanupOptions::default();
    unhwp_to_markdown_ex(path, &options, &cleanup, out_markdown, out_error)
}

/// Converts an HWP/HWPX file to Markdown with cleanup enabled.
///
/// # Parameters
/// - `path`: Null-terminated UTF-8 file path
/// - `out_markdown`: Pointer to receive the resulting markdown string
/// - `out_error`: Pointer to receive error message (optional, can be NULL)
///
/// # Returns
/// - UNHWP_OK (0) on success
/// - Error code on failure
///
/// # Safety
/// All pointer parameters must be valid or null where allowed.
#[no_mangle]
pub unsafe extern "C" fn unhwp_to_markdown_with_cleanup(
    path: *const c_char,
    out_markdown: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    let options = UnhwpRenderOptions::default();
    let cleanup = unhwp_cleanup_options_enabled();
    unhwp_to_markdown_ex(path, &options, &cleanup, out_markdown, out_error)
}

/// Converts an HWP/HWPX file to Markdown with full options.
///
/// # Parameters
/// - `path`: Null-terminated UTF-8 file path
/// - `render_options`: Render options structure
/// - `cleanup_options`: Cleanup options structure
/// - `out_markdown`: Pointer to receive the resulting markdown string
/// - `out_error`: Pointer to receive error message (optional, can be NULL)
///
/// # Returns
/// - UNHWP_OK (0) on success
/// - Error code on failure
///
/// # Safety
/// All pointer parameters must be valid or null where allowed.
#[no_mangle]
pub unsafe extern "C" fn unhwp_to_markdown_ex(
    path: *const c_char,
    render_options: *const UnhwpRenderOptions,
    cleanup_options: *const UnhwpCleanupOptions,
    out_markdown: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    // Validate inputs
    if path.is_null() || out_markdown.is_null() {
        return UNHWP_ERR_INVALID_ARG;
    }

    // Convert path
    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(e) => {
            set_error(out_error, &format!("Invalid UTF-8 path: {}", e));
            return UNHWP_ERR_INVALID_ARG;
        }
    };

    // Check if file exists
    if !std::path::Path::new(path_str).exists() {
        set_error(out_error, &format!("File not found: {}", path_str));
        return UNHWP_ERR_FILE_NOT_FOUND;
    }

    // Parse document
    let document = match parse_file(path_str) {
        Ok(doc) => doc,
        Err(e) => {
            set_error(out_error, &format!("Parse error: {}", e));
            return UNHWP_ERR_PARSE;
        }
    };

    // Build render options
    let mut rust_render_options = if render_options.is_null() {
        RenderOptions::default()
    } else {
        (*render_options).to_rust_options()
    };

    // Apply cleanup options
    if !cleanup_options.is_null() {
        let cleanup = &*cleanup_options;
        rust_render_options.cleanup = cleanup.to_rust_options();
    }

    // Render to markdown
    let markdown = match render::render_markdown(&document, &rust_render_options) {
        Ok(md) => md,
        Err(e) => {
            set_error(out_error, &format!("Render error: {}", e));
            return UNHWP_ERR_RENDER;
        }
    };

    // Return result
    match CString::new(markdown) {
        Ok(cstr) => {
            *out_markdown = cstr.into_raw();
            UNHWP_OK
        }
        Err(e) => {
            set_error(out_error, &format!("String conversion error: {}", e));
            UNHWP_ERR_UNKNOWN
        }
    }
}

/// Extracts plain text from an HWP/HWPX file.
///
/// # Parameters
/// - `path`: Null-terminated UTF-8 file path
/// - `out_text`: Pointer to receive the resulting text string
/// - `out_error`: Pointer to receive error message (optional, can be NULL)
///
/// # Returns
/// - UNHWP_OK (0) on success
/// - Error code on failure
///
/// # Safety
/// All pointer parameters must be valid or null where allowed.
#[no_mangle]
pub unsafe extern "C" fn unhwp_extract_text(
    path: *const c_char,
    out_text: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    // Validate inputs
    if path.is_null() || out_text.is_null() {
        return UNHWP_ERR_INVALID_ARG;
    }

    // Convert path
    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(e) => {
            set_error(out_error, &format!("Invalid UTF-8 path: {}", e));
            return UNHWP_ERR_INVALID_ARG;
        }
    };

    // Parse and extract text
    match crate::extract_text(path_str) {
        Ok(text) => match CString::new(text) {
            Ok(cstr) => {
                *out_text = cstr.into_raw();
                UNHWP_OK
            }
            Err(e) => {
                set_error(out_error, &format!("String conversion error: {}", e));
                UNHWP_ERR_UNKNOWN
            }
        },
        Err(e) => {
            set_error(out_error, &format!("Extract error: {}", e));
            UNHWP_ERR_PARSE
        }
    }
}

// ============================================================================
// Byte Array Functions (for in-memory processing)
// ============================================================================

/// Converts HWP/HWPX bytes to Markdown.
///
/// # Parameters
/// - `data`: Pointer to file data
/// - `data_len`: Length of file data in bytes
/// - `out_markdown`: Pointer to receive the resulting markdown string
/// - `out_error`: Pointer to receive error message (optional, can be NULL)
///
/// # Returns
/// - UNHWP_OK (0) on success
/// - Error code on failure
#[no_mangle]
pub extern "C" fn unhwp_bytes_to_markdown(
    data: *const u8,
    data_len: usize,
    out_markdown: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    let options = UnhwpRenderOptions::default();
    let cleanup = UnhwpCleanupOptions::default();
    unhwp_bytes_to_markdown_ex(data, data_len, &options, &cleanup, out_markdown, out_error)
}

/// Converts HWP/HWPX bytes to Markdown with full options.
#[no_mangle]
pub extern "C" fn unhwp_bytes_to_markdown_ex(
    data: *const u8,
    data_len: usize,
    render_options: *const UnhwpRenderOptions,
    cleanup_options: *const UnhwpCleanupOptions,
    out_markdown: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    // Validate inputs
    if data.is_null() || data_len == 0 || out_markdown.is_null() {
        return UNHWP_ERR_INVALID_ARG;
    }

    // Create slice from raw pointer
    let bytes = unsafe { std::slice::from_raw_parts(data, data_len) };

    // Parse document
    let document = match crate::parse_bytes(bytes) {
        Ok(doc) => doc,
        Err(e) => {
            set_error(out_error, &format!("Parse error: {}", e));
            return UNHWP_ERR_PARSE;
        }
    };

    // Build render options
    let mut rust_render_options = if render_options.is_null() {
        RenderOptions::default()
    } else {
        unsafe { &*render_options }.to_rust_options()
    };

    // Apply cleanup options
    if !cleanup_options.is_null() {
        let cleanup = unsafe { &*cleanup_options };
        rust_render_options.cleanup = cleanup.to_rust_options();
    }

    // Render to markdown
    let markdown = match render::render_markdown(&document, &rust_render_options) {
        Ok(md) => md,
        Err(e) => {
            set_error(out_error, &format!("Render error: {}", e));
            return UNHWP_ERR_RENDER;
        }
    };

    // Return result
    match CString::new(markdown) {
        Ok(cstr) => {
            unsafe { *out_markdown = cstr.into_raw() };
            UNHWP_OK
        }
        Err(e) => {
            set_error(out_error, &format!("String conversion error: {}", e));
            UNHWP_ERR_UNKNOWN
        }
    }
}

// ============================================================================
// Memory Management
// ============================================================================

/// Frees a string allocated by unhwp functions.
///
/// # Safety
/// - The pointer must have been returned by an unhwp function.
/// - The pointer must not be NULL.
/// - The pointer must not have been freed already.
#[no_mangle]
pub extern "C" fn unhwp_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

// ============================================================================
// Version Info
// ============================================================================

/// Returns the library version string.
///
/// # Returns
/// - Null-terminated version string (e.g., "0.1.0")
/// - The returned string is statically allocated and must NOT be freed.
#[no_mangle]
pub extern "C" fn unhwp_version() -> *const c_char {
    static VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
    VERSION.as_ptr() as *const c_char
}

/// Returns the supported format flags.
///
/// # Returns
/// - Bitmask of supported formats:
///   - Bit 0 (0x01): HWP 5.0
///   - Bit 1 (0x02): HWPX
///   - Bit 2 (0x04): HWP 3.x
#[no_mangle]
pub extern "C" fn unhwp_supported_formats() -> i32 {
    let mut flags = 0i32;

    #[cfg(feature = "hwp5")]
    {
        flags |= 0x01;
    }

    #[cfg(feature = "hwpx")]
    {
        flags |= 0x02;
    }

    #[cfg(feature = "hwp3")]
    {
        flags |= 0x04;
    }

    flags
}

// ============================================================================
// Helper Functions
// ============================================================================

fn set_error(out_error: *mut *mut c_char, message: &str) {
    if !out_error.is_null() {
        if let Ok(cstr) = CString::new(message) {
            unsafe { *out_error = cstr.into_raw() };
        }
    }
}

// ============================================================================
// Structured Result API
// ============================================================================

/// C-compatible image structure.
#[repr(C)]
pub struct UnhwpImage {
    /// Image name (null-terminated UTF-8 string)
    pub name: *mut c_char,
    /// Image binary data
    pub data: *mut u8,
    /// Length of image data in bytes
    pub data_len: usize,
}

/// Opaque result handle containing parsed document and cached results.
pub struct UnhwpResult {
    document: Document,
    cached_markdown: Option<CString>,
    cached_text: Option<CString>,
    cached_raw_content: Option<CString>,
    render_options: RenderOptions,
    images: Vec<UnhwpImage>,
    last_error: Option<CString>,
}

impl UnhwpResult {
    fn new(document: Document, render_options: RenderOptions) -> Self {
        Self {
            document,
            cached_markdown: None,
            cached_text: None,
            cached_raw_content: None,
            render_options,
            images: Vec::new(),
            last_error: None,
        }
    }

    fn ensure_markdown(&mut self) -> Result<&CString, String> {
        if self.cached_markdown.is_none() {
            let markdown = render::render_markdown(&self.document, &self.render_options)
                .map_err(|e| format!("Render error: {}", e))?;
            self.cached_markdown = Some(
                CString::new(markdown).map_err(|e| format!("String conversion error: {}", e))?,
            );
        }
        Ok(self.cached_markdown.as_ref().unwrap())
    }

    fn ensure_text(&mut self) -> Result<&CString, String> {
        if self.cached_text.is_none() {
            let text = self.document.plain_text();
            self.cached_text =
                Some(CString::new(text).map_err(|e| format!("String conversion error: {}", e))?);
        }
        Ok(self.cached_text.as_ref().unwrap())
    }

    fn ensure_raw_content(&mut self) -> Result<&CString, String> {
        if self.cached_raw_content.is_none() {
            let raw = self.document.raw_content();
            self.cached_raw_content =
                Some(CString::new(raw).map_err(|e| format!("String conversion error: {}", e))?);
        }
        Ok(self.cached_raw_content.as_ref().unwrap())
    }

    fn ensure_images(&mut self) {
        if self.images.is_empty() && !self.document.resources.is_empty() {
            for (name, resource) in &self.document.resources {
                let name_cstr = match CString::new(name.clone()) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                // Allocate and copy image data
                let data_len = resource.data.len();
                let data_ptr = if data_len > 0 {
                    let layout = std::alloc::Layout::from_size_align(data_len, 1).unwrap();
                    let ptr = unsafe { std::alloc::alloc(layout) };
                    if !ptr.is_null() {
                        unsafe {
                            std::ptr::copy_nonoverlapping(resource.data.as_ptr(), ptr, data_len);
                        }
                    }
                    ptr
                } else {
                    ptr::null_mut()
                };

                self.images.push(UnhwpImage {
                    name: name_cstr.into_raw(),
                    data: data_ptr,
                    data_len,
                });
            }
        }
    }

    fn set_error(&mut self, message: &str) {
        self.last_error = CString::new(message).ok();
    }
}

/// Parses an HWP/HWPX file and returns a result handle.
///
/// # Parameters
/// - `path`: Null-terminated UTF-8 file path
/// - `render_options`: Render options (optional, can be NULL for defaults)
/// - `cleanup_options`: Cleanup options (optional, can be NULL for defaults)
///
/// # Returns
/// - Pointer to result handle on success
/// - NULL on failure (call unhwp_get_last_error for details)
///
/// # Memory
/// - Caller must free the result using `unhwp_result_free`.
#[no_mangle]
pub extern "C" fn unhwp_parse(
    path: *const c_char,
    render_options: *const UnhwpRenderOptions,
    cleanup_options: *const UnhwpCleanupOptions,
) -> *mut UnhwpResult {
    if path.is_null() {
        return ptr::null_mut();
    }

    let path_str = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    // Parse document
    let document = match parse_file(path_str) {
        Ok(doc) => doc,
        Err(_) => return ptr::null_mut(),
    };

    // Build render options
    let mut rust_render_options = if render_options.is_null() {
        RenderOptions::default()
    } else {
        unsafe { &*render_options }.to_rust_options()
    };

    // Apply cleanup options
    if !cleanup_options.is_null() {
        let cleanup = unsafe { &*cleanup_options };
        rust_render_options.cleanup = cleanup.to_rust_options();
    }

    let result = Box::new(UnhwpResult::new(document, rust_render_options));
    Box::into_raw(result)
}

/// Parses HWP/HWPX bytes and returns a result handle.
///
/// # Parameters
/// - `data`: Pointer to file data
/// - `data_len`: Length of file data in bytes
/// - `render_options`: Render options (optional, can be NULL for defaults)
/// - `cleanup_options`: Cleanup options (optional, can be NULL for defaults)
///
/// # Returns
/// - Pointer to result handle on success
/// - NULL on failure
#[no_mangle]
pub extern "C" fn unhwp_parse_bytes(
    data: *const u8,
    data_len: usize,
    render_options: *const UnhwpRenderOptions,
    cleanup_options: *const UnhwpCleanupOptions,
) -> *mut UnhwpResult {
    if data.is_null() || data_len == 0 {
        return ptr::null_mut();
    }

    let bytes = unsafe { std::slice::from_raw_parts(data, data_len) };

    // Parse document
    let document = match crate::parse_bytes(bytes) {
        Ok(doc) => doc,
        Err(_) => return ptr::null_mut(),
    };

    // Build render options
    let mut rust_render_options = if render_options.is_null() {
        RenderOptions::default()
    } else {
        unsafe { &*render_options }.to_rust_options()
    };

    // Apply cleanup options
    if !cleanup_options.is_null() {
        let cleanup = unsafe { &*cleanup_options };
        rust_render_options.cleanup = cleanup.to_rust_options();
    }

    let result = Box::new(UnhwpResult::new(document, rust_render_options));
    Box::into_raw(result)
}

/// Gets the rendered Markdown from a result.
///
/// # Parameters
/// - `result`: Result handle from unhwp_parse
///
/// # Returns
/// - Null-terminated Markdown string
/// - NULL on error
///
/// # Note
/// - The returned string is owned by the result and must NOT be freed separately.
/// - The string remains valid until the result is freed.
#[no_mangle]
pub extern "C" fn unhwp_result_get_markdown(result: *mut UnhwpResult) -> *const c_char {
    if result.is_null() {
        return ptr::null();
    }

    let result = unsafe { &mut *result };
    match result.ensure_markdown() {
        Ok(cstr) => cstr.as_ptr(),
        Err(e) => {
            result.set_error(&e);
            ptr::null()
        }
    }
}

/// Gets the plain text from a result.
///
/// # Parameters
/// - `result`: Result handle from unhwp_parse
///
/// # Returns
/// - Null-terminated plain text string
/// - NULL on error
///
/// # Note
/// - The returned string is owned by the result and must NOT be freed separately.
#[no_mangle]
pub extern "C" fn unhwp_result_get_text(result: *mut UnhwpResult) -> *const c_char {
    if result.is_null() {
        return ptr::null();
    }

    let result = unsafe { &mut *result };
    match result.ensure_text() {
        Ok(cstr) => cstr.as_ptr(),
        Err(e) => {
            result.set_error(&e);
            ptr::null()
        }
    }
}

/// Gets the structured content as JSON with full metadata.
///
/// This provides access to the full document structure including:
/// - Document metadata (title, author, dates)
/// - Paragraph styles (heading level, alignment, list type)
/// - Text formatting (bold, italic, underline, font, color, etc.)
/// - Table structure (rows, cells, colspan, rowspan)
/// - Equations, images, and links
///
/// # Parameters
/// - `result`: Result handle from unhwp_parse
///
/// # Returns
/// - Null-terminated JSON string
/// - NULL on error
///
/// # Note
/// - The returned string is owned by the result and must NOT be freed separately.
#[no_mangle]
pub extern "C" fn unhwp_result_get_raw_content(result: *mut UnhwpResult) -> *const c_char {
    if result.is_null() {
        return ptr::null();
    }

    let result = unsafe { &mut *result };
    match result.ensure_raw_content() {
        Ok(cstr) => cstr.as_ptr(),
        Err(e) => {
            result.set_error(&e);
            ptr::null()
        }
    }
}

/// Gets the number of images in the document.
///
/// # Parameters
/// - `result`: Result handle from unhwp_parse
///
/// # Returns
/// - Number of images (0 or more)
/// - -1 on error (null result)
#[no_mangle]
pub extern "C" fn unhwp_result_get_image_count(result: *mut UnhwpResult) -> i32 {
    if result.is_null() {
        return -1;
    }

    let result = unsafe { &mut *result };
    result.ensure_images();
    result.images.len() as i32
}

/// Gets an image from the result by index.
///
/// # Parameters
/// - `result`: Result handle from unhwp_parse
/// - `index`: Image index (0-based)
/// - `out_image`: Pointer to receive image data
///
/// # Returns
/// - UNHWP_OK on success
/// - UNHWP_ERR_INVALID_ARG on invalid index or null pointers
///
/// # Note
/// - The image data is owned by the result and must NOT be freed separately.
#[no_mangle]
pub extern "C" fn unhwp_result_get_image(
    result: *mut UnhwpResult,
    index: i32,
    out_image: *mut UnhwpImage,
) -> i32 {
    if result.is_null() || out_image.is_null() || index < 0 {
        return UNHWP_ERR_INVALID_ARG;
    }

    let result = unsafe { &mut *result };
    result.ensure_images();

    let index = index as usize;
    if index >= result.images.len() {
        return UNHWP_ERR_INVALID_ARG;
    }

    unsafe {
        *out_image = UnhwpImage {
            name: result.images[index].name,
            data: result.images[index].data,
            data_len: result.images[index].data_len,
        };
    }

    UNHWP_OK
}

/// Gets the number of sections in the document.
///
/// # Parameters
/// - `result`: Result handle from unhwp_parse
///
/// # Returns
/// - Number of sections
/// - -1 on error
#[no_mangle]
pub extern "C" fn unhwp_result_get_section_count(result: *mut UnhwpResult) -> i32 {
    if result.is_null() {
        return -1;
    }

    let result = unsafe { &*result };
    result.document.sections.len() as i32
}

/// Gets the number of paragraphs in the document.
///
/// # Parameters
/// - `result`: Result handle from unhwp_parse
///
/// # Returns
/// - Number of paragraphs
/// - -1 on error
#[no_mangle]
pub extern "C" fn unhwp_result_get_paragraph_count(result: *mut UnhwpResult) -> i32 {
    if result.is_null() {
        return -1;
    }

    let result = unsafe { &*result };
    result.document.paragraph_count() as i32
}

/// Gets the last error message from a result.
///
/// # Parameters
/// - `result`: Result handle from unhwp_parse
///
/// # Returns
/// - Null-terminated error string, or NULL if no error
#[no_mangle]
pub extern "C" fn unhwp_result_get_error(result: *mut UnhwpResult) -> *const c_char {
    if result.is_null() {
        return ptr::null();
    }

    let result = unsafe { &*result };
    match &result.last_error {
        Some(err) => err.as_ptr(),
        None => ptr::null(),
    }
}

/// Frees a result handle and all associated resources.
///
/// # Parameters
/// - `result`: Result handle from unhwp_parse
///
/// # Safety
/// - The result must have been returned by unhwp_parse or unhwp_parse_bytes.
/// - The result must not be NULL.
/// - The result must not have been freed already.
#[no_mangle]
pub extern "C" fn unhwp_result_free(result: *mut UnhwpResult) {
    if result.is_null() {
        return;
    }

    unsafe {
        let mut result = Box::from_raw(result);

        // Free cached strings (handled by Drop)
        // Free image data
        for image in &mut result.images {
            if !image.name.is_null() {
                drop(CString::from_raw(image.name));
            }
            if !image.data.is_null() && image.data_len > 0 {
                let layout = std::alloc::Layout::from_size_align(image.data_len, 1).unwrap();
                std::alloc::dealloc(image.data, layout);
            }
        }

        // result is dropped here, freeing Document and cached strings
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let version = unhwp_version();
        assert!(!version.is_null());
        let version_str = unsafe { CStr::from_ptr(version) }.to_str().unwrap();
        assert!(!version_str.is_empty());
    }

    #[test]
    fn test_supported_formats() {
        let flags = unhwp_supported_formats();
        // At minimum, HWP5 and HWPX should be supported (default features)
        assert!(flags & 0x01 != 0); // HWP5
        assert!(flags & 0x02 != 0); // HWPX
    }

    #[test]
    fn test_default_options() {
        let render_opts = unhwp_render_options_default();
        assert_eq!(render_opts.include_frontmatter, 1);
        assert_eq!(render_opts.escape_special_chars, 1);

        let cleanup_opts = unhwp_cleanup_options_default();
        assert_eq!(cleanup_opts.enabled, 0);
        assert_eq!(cleanup_opts.detect_mojibake, 1);
    }

    #[test]
    fn test_null_handling() {
        unsafe {
            // Test null path
            assert_eq!(unhwp_detect_format(ptr::null()), FORMAT_UNKNOWN);

            // Test null output pointer
            let path = CString::new("test.hwp").unwrap();
            let result = unhwp_to_markdown(path.as_ptr(), ptr::null_mut(), ptr::null_mut());
            assert_eq!(result, UNHWP_ERR_INVALID_ARG);
        }
    }
}
