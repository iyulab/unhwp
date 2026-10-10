/**
 * unhwp - HWP/HWPX Document Extraction Library
 *
 * Extracts content from Hangul Word Processor documents (HWP 5.0 and HWPX).
 * Converts documents to Markdown, plain text, or JSON.
 *
 * Memory: strings returned by this library are freed with unhwp_free_string(),
 * buffers from unhwp_get_resource_data() with unhwp_free_bytes(), and document
 * handles with unhwp_free_document().
 *
 * Copyright (c) 2025 iyulab
 * MIT License
 */

#ifndef UNHWP_H
#define UNHWP_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque document handle */
typedef struct UnhwpDocument UnhwpDocument;

/* Flags for unhwp_to_markdown */
#define UNHWP_FLAG_FRONTMATTER       1u /* Include YAML frontmatter */
#define UNHWP_FLAG_ESCAPE_SPECIAL    2u /* Accepted, no effect: escaping is the default (see NO_ESCAPE) */
#define UNHWP_FLAG_PARAGRAPH_SPACING 4u /* Keep line breaks inside paragraphs */
#define UNHWP_FLAG_REFINE            8u /* Apply the shape-refinement pass */
#define UNHWP_FLAG_NO_ESCAPE        16u /* Do not escape special Markdown characters */

/* Format selector for unhwp_to_json */
#define UNHWP_JSON_PRETTY   0  /* Pretty-printed JSON with indentation */
#define UNHWP_JSON_COMPACT  1  /* Compact JSON without whitespace */

/**
 * Why the last call failed, as returned by unhwp_last_error_kind().
 *
 * Values 1..=13 mirror the reasons the sibling libraries share, 400+ are reasons unique
 * to HWP, and 100+ are raised at the FFI boundary with no library-side counterpart.
 * These numbers are a stable ABI contract: a new reason takes the next free number and
 * existing ones are never reused or renumbered. Treat an unrecognised value as a generic
 * failure rather than as an error, so that a newer library stays usable by older callers.
 */
typedef enum UnhwpErrorKind {
    UNHWP_ERROR_NONE                    = 0,   /* The last call succeeded */
    UNHWP_ERROR_OTHER                   = 1,   /* Failure with no more specific reason */
    UNHWP_ERROR_IO                      = 2,   /* Missing or unreadable file */
    UNHWP_ERROR_UNKNOWN_FORMAT          = 3,   /* Not a recognised HWP document */
    UNHWP_ERROR_UNSUPPORTED_FORMAT      = 4,   /* Recognised but not supported */
    UNHWP_ERROR_ZIP_ARCHIVE             = 5,   /* The HWPX container could not be read */
    UNHWP_ERROR_XML_PARSE               = 6,   /* XML content could not be parsed */
    UNHWP_ERROR_INVALID_DATA            = 7,   /* Malformed data inside the document */
    UNHWP_ERROR_MISSING_COMPONENT       = 8,   /* A required document part is absent */
    UNHWP_ERROR_ENCODING                = 9,   /* Text encoding conversion failed */
    UNHWP_ERROR_STYLE_NOT_FOUND         = 10,  /* A referenced style is absent */
    UNHWP_ERROR_RESOURCE_NOT_FOUND      = 11,  /* A referenced resource is absent */
    UNHWP_ERROR_ENCRYPTED               = 12,  /* The document is encrypted */
    UNHWP_ERROR_RENDER                  = 13,  /* Producing the output failed */
    UNHWP_ERROR_INVALID_ARGUMENT        = 100, /* An argument was NULL or not valid UTF-8 */
    UNHWP_ERROR_PANIC                   = 101, /* A panic was caught at the boundary */
    UNHWP_ERROR_INVALID_OUTPUT          = 102, /* Output holds a NUL byte, cannot cross ABI */
    UNHWP_ERROR_DECOMPRESSION           = 400, /* A compressed stream could not be inflated */
    UNHWP_ERROR_OLE_CONTAINER           = 401, /* The HWP 5.0 compound file is damaged */
    UNHWP_ERROR_RECORD_PARSE            = 402, /* An HWP 5.0 record could not be parsed */
    UNHWP_ERROR_DISTRIBUTION_RESTRICTED = 403  /* A distribution-restricted document */
} UnhwpErrorKind;

/**
 * Get the library version.
 *
 * @return Static version string (do not free)
 */
const char* unhwp_version(void);

/**
 * Get the last error message.
 *
 * Call this after a function returns NULL to get the error description.
 *
 * @return Error message or NULL if no error. Do not free.
 */
const char* unhwp_last_error(void);

/**
 * Classify the last error without parsing its message.
 *
 * Returns UNHWP_ERROR_NONE (0) when the last call on this thread succeeded. Written
 * and cleared in lockstep with unhwp_last_error(), so a message is never paired with
 * a stale kind.
 *
 * @return A UnhwpErrorKind value; treat an unrecognised value as a generic failure.
 */
int unhwp_last_error_kind(void);

/**
 * Parse a document from a file path.
 *
 * Detects HWP 5.0 or HWPX from the content.
 *
 * @param path Path to the document file (UTF-8 encoded)
 * @return Document handle or NULL on error. Must be freed with unhwp_free_document().
 */
UnhwpDocument* unhwp_parse_file(const char* path);

/**
 * Parse a document from a byte buffer.
 *
 * @param data Pointer to document data
 * @param len Length of data in bytes
 * @return Document handle or NULL on error. Must be freed with unhwp_free_document().
 */
UnhwpDocument* unhwp_parse_bytes(const uint8_t* data, size_t len);

/**
 * Free a document handle.
 *
 * @param doc Document handle (may be NULL)
 */
void unhwp_free_document(UnhwpDocument* doc);

/**
 * Convert a document to Markdown.
 *
 * @param doc Document handle
 * @param flags Bitwise OR of UNHWP_FLAG_* constants
 * @return Markdown string or NULL on error. Must be freed with unhwp_free_string().
 */
char* unhwp_to_markdown(const UnhwpDocument* doc, uint32_t flags);

/**
 * Convert a document to plain text.
 *
 * @param doc Document handle
 * @return Plain text string or NULL on error. Must be freed with unhwp_free_string().
 */
char* unhwp_to_text(const UnhwpDocument* doc);

/**
 * Convert a document to JSON.
 *
 * @param doc Document handle
 * @param format UNHWP_JSON_PRETTY or UNHWP_JSON_COMPACT
 * @return JSON string or NULL on error. Must be freed with unhwp_free_string().
 */
char* unhwp_to_json(const UnhwpDocument* doc, int format);

/**
 * Get plain text content directly.
 *
 * @param doc Document handle
 * @return Plain text or NULL on error. Must be freed with unhwp_free_string().
 */
char* unhwp_plain_text(const UnhwpDocument* doc);

/**
 * Get the number of sections in a document.
 *
 * @param doc Document handle
 * @return Section count or -1 on error
 */
int unhwp_section_count(const UnhwpDocument* doc);

/**
 * Get the number of embedded resources (images and other binary data).
 *
 * @param doc Document handle
 * @return Resource count or -1 on error
 */
int unhwp_resource_count(const UnhwpDocument* doc);

/**
 * Get the document title.
 *
 * A NULL return with unhwp_last_error_kind() at UNHWP_ERROR_NONE means no title is
 * set; with a non-zero kind it means the title could not be produced.
 *
 * @param doc Document handle
 * @return Title or NULL. Must be freed with unhwp_free_string().
 */
char* unhwp_get_title(const UnhwpDocument* doc);

/**
 * Get the document author.
 *
 * A NULL return with unhwp_last_error_kind() at UNHWP_ERROR_NONE means no author is
 * set; with a non-zero kind it means the author could not be produced.
 *
 * @param doc Document handle
 * @return Author or NULL. Must be freed with unhwp_free_string().
 */
char* unhwp_get_author(const UnhwpDocument* doc);

/**
 * Every table of the document as delimited text, in reading order, as a JSON
 * array of {"section","index","text"}: the section's number (from 1), the
 * table's place among that section's tables (from 1), and the table as CSV
 * (RFC 4180) — tab-separated when tsv is non-zero. A merged cell's text is in
 * its top-left position and the positions it covers are empty; records end
 * with CRLF.
 *
 * @param doc Document handle
 * @param tsv Non-zero for tab-separated instead of comma-separated
 * @return JSON string, "[]" when the document has no tables, or NULL on error.
 *         Must be freed with unhwp_free_string().
 */
char* unhwp_tables(const UnhwpDocument* doc, int tsv);

/**
 * Get the ids of all embedded resources as a JSON array.
 *
 * @param doc Document handle
 * @return JSON array such as ["image1.png", "image2.jpg"], or NULL on error.
 *         Must be freed with unhwp_free_string().
 */
char* unhwp_get_resource_ids(const UnhwpDocument* doc);

/**
 * Get one resource's metadata as a JSON object, without its bytes.
 *
 * The object carries id, type, filename, mime_type and size.
 *
 * @param doc Document handle
 * @param resource_id Resource id (UTF-8, NUL-terminated)
 * @return JSON object, or NULL if the resource does not exist or on error.
 *         Must be freed with unhwp_free_string().
 */
char* unhwp_get_resource_info(const UnhwpDocument* doc, const char* resource_id);

/**
 * Get one resource's bytes.
 *
 * @param doc Document handle
 * @param resource_id Resource id (UTF-8, NUL-terminated)
 * @param out_len Receives the length of the returned buffer. Left untouched when an
 *                argument is rejected.
 * @return Buffer, or NULL if the resource does not exist or on error.
 *         Must be freed with unhwp_free_bytes() together with *out_len.
 */
uint8_t* unhwp_get_resource_data(const UnhwpDocument* doc, const char* resource_id, size_t* out_len);

/**
 * Free a string allocated by this library.
 *
 * @param str String pointer (may be NULL)
 */
void unhwp_free_string(char* str);

/**
 * Free a buffer returned by unhwp_get_resource_data().
 *
 * @param data Buffer pointer (may be NULL)
 * @param len The length unhwp_get_resource_data() wrote to out_len
 */
void unhwp_free_bytes(uint8_t* data, size_t len);

#ifdef __cplusplus
}
#endif

#endif /* UNHWP_H */
