"""
High-level Python API for unhwp.

Provides a Pythonic interface to the unhwp native library.
"""

import ctypes
import json
from dataclasses import dataclass
from enum import IntEnum
from pathlib import Path
from typing import Dict, List, Optional, Union, Iterator

from . import _native as native


# =============================================================================
# Constants
# =============================================================================

FORMAT_UNKNOWN = native.FORMAT_UNKNOWN
FORMAT_HWP5 = native.FORMAT_HWP5
FORMAT_HWPX = native.FORMAT_HWPX
FORMAT_HWP3 = native.FORMAT_HWP3

_FORMAT_NAMES = {
    FORMAT_UNKNOWN: "Unknown",
    FORMAT_HWP5: "HWP 5.0",
    FORMAT_HWPX: "HWPX",
    FORMAT_HWP3: "HWP 3.x",
}


# =============================================================================
# Exceptions
# =============================================================================

class ErrorKind(IntEnum):
    """Why an unhwp call failed, so callers can branch on the reason.

    Values 1-13 and 400-403 mirror the library's own failure reasons; values 100+
    are raised at the interop boundary and have no library-side counterpart. The
    numbers are part of the native ABI (``UnhwpErrorKind`` in ``unhwp.h``): a new
    reason takes the next free number and existing ones are never renumbered, so an
    unrecognised value is kept as a plain :class:`int` rather than rejected.
    """

    NONE = 0
    OTHER = 1
    IO = 2
    UNKNOWN_FORMAT = 3
    UNSUPPORTED_FORMAT = 4
    ZIP_ARCHIVE = 5
    XML_PARSE = 6
    INVALID_DATA = 7
    MISSING_COMPONENT = 8
    ENCODING = 9
    STYLE_NOT_FOUND = 10
    RESOURCE_NOT_FOUND = 11
    ENCRYPTED = 12
    RENDER = 13
    DECOMPRESSION = 400
    OLE_CONTAINER = 401
    RECORD_PARSE = 402
    DISTRIBUTION_RESTRICTED = 403
    INVALID_ARGUMENT = 100
    PANIC = 101
    INVALID_OUTPUT = 102


class UnhwpError(Exception):
    """Base exception for unhwp errors.

    Attributes:
        kind: An :class:`ErrorKind`, or the raw integer if the native library
            reported a reason this build does not know about. Never
            :attr:`ErrorKind.NONE`, which means success. Defaults to
            :attr:`ErrorKind.OTHER` for failures that did not come from the native
            library.
    """

    def __init__(self, message: str, kind: int = ErrorKind.OTHER) -> None:
        super().__init__(message)
        try:
            self.kind: int = ErrorKind(kind)
        except ValueError:
            # Forward compatibility: a newer native library may report a number
            # this build has no name for. Keep it rather than losing the
            # classification.
            self.kind = kind


class FileNotFoundError(UnhwpError):
    """File not found error."""
    pass


class ParseError(UnhwpError):
    """Document parsing error."""
    pass


class RenderError(UnhwpError):
    """Markdown rendering error."""
    pass


class UnsupportedFormatError(UnhwpError):
    """Unsupported document format error."""
    pass


def _get_last_error() -> str:
    """Get the last error message from the native library."""
    err = native.lib.unhwp_last_error()
    if err:
        return err.decode("utf-8")
    return "Unknown error"


def _get_last_error_kind() -> int:
    """Get the classification of the last error from the native library.

    An unrecognised number passes through unchanged so a newer native library
    stays usable. Zero is the one value that cannot stand: it means success, and
    we only ask while building a failure.
    """
    kind = native.lib.unhwp_last_error_kind()
    return ErrorKind.OTHER if kind == ErrorKind.NONE else kind


def _native_failure(exc_type, action: str) -> UnhwpError:
    """Build the exception for a failed native call, with message + classification.

    Every native failure goes through here so no raise site can quietly drop the
    classification and leave the caller with ``OTHER``.
    """
    return exc_type(f"{action}: {_get_last_error()}", _get_last_error_kind())


def _ptr_to_string(ptr: Optional[int]) -> Optional[str]:
    """Convert a c_void_p (int) to a Python string via UTF-8 decoding.

    Returns None if the pointer is null/None.
    The caller is responsible for freeing the pointer after this call.
    """
    if not ptr:
        return None
    return ctypes.string_at(ptr).decode("utf-8")


# =============================================================================
# Data Classes
# =============================================================================

@dataclass
class Image:
    """Represents an extracted image from the document."""
    name: str
    data: bytes

    def save(self, path: Union[str, Path]) -> None:
        """Save the image to a file."""
        Path(path).write_bytes(self.data)


@dataclass
class CleanupOptions:
    """The cleanup pipeline run over the rendered Markdown: string normalization,
    line cleaning (page numbers, repeated headers and footers), structural
    filtering and final whitespace normalization.

    Attributes:
        enabled: Run the pipeline. ``False`` renders without cleanup.
        preset: ``"minimal"`` (normalization only), ``"standard"`` (every stage)
            or ``"aggressive"`` (every stage, removing headers and footers more
            eagerly).
    """
    enabled: bool = True
    preset: str = "standard"

    @classmethod
    def minimal(cls) -> "CleanupOptions":
        """Create minimal cleanup options."""
        return cls(enabled=True, preset="minimal")

    @classmethod
    def default(cls) -> "CleanupOptions":
        """Create default (standard) cleanup options."""
        return cls(enabled=True, preset="standard")

    @classmethod
    def aggressive(cls) -> "CleanupOptions":
        """Create aggressive cleanup options."""
        return cls(enabled=True, preset="aggressive")

    @classmethod
    def disabled(cls) -> "CleanupOptions":
        """Create disabled cleanup options."""
        return cls(enabled=False)


@dataclass
class RenderOptions:
    """Options for rendering documents to Markdown.

    Every field reaches the native library; the defaults are the library's own.
    A value the library cannot honour (an unknown ``table_fallback``, a
    ``max_heading_level`` outside 1-6, a ``list_marker`` that is not one
    character) raises :class:`RenderError` with kind
    :attr:`ErrorKind.INVALID_ARGUMENT` when the Markdown is produced.

    Attributes:
        image_path_prefix: Prefix of the image paths written in the Markdown.
        table_fallback: What becomes of a table with merged cells, which
            Markdown cannot express: ``"simplified_markdown"`` (a Markdown table,
            merges dropped), ``"html"`` (an HTML table with rowspan/colspan) or
            ``"skip"`` (left out).
        max_heading_level: The deepest heading level written (1-6); deeper
            headings take this level.
        include_frontmatter: Write the document's metadata as YAML frontmatter.
        preserve_line_breaks: Keep line breaks inside paragraphs as Markdown
            hard breaks; ``False`` joins the lines with a space.
        include_empty_paragraphs: Keep empty paragraphs as blank lines.
        list_marker: The character that marks an unordered list item.
        paragraph_spacing: A blank line after each paragraph.
        escape_special_chars: Escape text that would read as Markdown syntax.
        section_markers: ``"none"``, or ``"comment"`` for an
            ``<!-- section N -->`` comment before each section.
        cleanup: Run the cleanup pipeline over the output; ``None`` for none.
        refine: Apply the lossless, idempotent markdown shape-refinement pass
            (table shape, ordered-list numbering, link/image paths, frontmatter,
            section anchors) after rendering.
    """
    image_path_prefix: str = "assets/"
    table_fallback: str = "simplified_markdown"
    max_heading_level: int = 4
    include_frontmatter: bool = False
    preserve_line_breaks: bool = True
    include_empty_paragraphs: bool = False
    list_marker: str = "-"
    paragraph_spacing: bool = True
    escape_special_chars: bool = True
    section_markers: str = "none"
    cleanup: Optional[CleanupOptions] = None
    refine: bool = False

    def _to_json(self) -> str:
        """The options as ``unhwp_to_markdown_with_options`` reads them — every field."""
        cleanup = self.cleanup
        return json.dumps({
            "image_path_prefix": self.image_path_prefix,
            "table_fallback": self.table_fallback,
            "max_heading_level": self.max_heading_level,
            "include_frontmatter": self.include_frontmatter,
            "preserve_line_breaks": self.preserve_line_breaks,
            "include_empty_paragraphs": self.include_empty_paragraphs,
            "list_marker": self.list_marker,
            "paragraph_spacing": self.paragraph_spacing,
            "escape_special_chars": self.escape_special_chars,
            "section_markers": self.section_markers,
            "cleanup_preset": cleanup.preset if cleanup and cleanup.enabled else None,
            "refine": self.refine,
        })


class ParseResult:
    """
    Result of parsing an HWP/HWPX document.

    Provides access to the extracted markdown, plain text, and images.
    This object manages the underlying native memory and should be used
    as a context manager or explicitly closed.

    Example:
        >>> with unhwp.parse("document.hwp") as result:
        ...     print(result.markdown)
        ...     for img in result.images:
        ...         img.save(f"output/{img.name}")
    """

    def __init__(self, handle: int, render_options: Optional[RenderOptions] = None):
        self._handle = handle
        self._options_json = (render_options or RenderOptions())._to_json().encode("utf-8")
        self._closed = False

    def __enter__(self) -> "ParseResult":
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.close()

    def __del__(self) -> None:
        self.close()

    def close(self) -> None:
        """Release native resources."""
        if not self._closed and self._handle:
            native.lib.unhwp_free_document(self._handle)
            self._handle = None
            self._closed = True

    def _ensure_open(self) -> None:
        if self._closed:
            raise ValueError("ParseResult has been closed")

    @property
    def markdown(self) -> str:
        """Get the Markdown content, rendered with the parse call's ``render_options``.

        Raises:
            RenderError: If rendering fails, or with kind
                :attr:`ErrorKind.INVALID_ARGUMENT` if an option cannot be honoured.
        """
        self._ensure_open()
        ptr = native.lib.unhwp_to_markdown_with_options(self._handle, self._options_json)
        if not ptr:
            raise _native_failure(RenderError, "Failed to convert to markdown")
        try:
            return _ptr_to_string(ptr) or ""
        finally:
            native.lib.unhwp_free_string(ptr)

    @property
    def text(self) -> str:
        """Get the plain text content."""
        self._ensure_open()
        ptr = native.lib.unhwp_to_text(self._handle)
        if not ptr:
            raise _native_failure(RenderError, "Failed to convert to text")
        try:
            return _ptr_to_string(ptr) or ""
        finally:
            native.lib.unhwp_free_string(ptr)

    @property
    def plain_text(self) -> str:
        """Get the plain text content (faster extraction)."""
        self._ensure_open()
        ptr = native.lib.unhwp_plain_text(self._handle)
        if not ptr:
            raise _native_failure(RenderError, "Failed to get plain text")
        try:
            return _ptr_to_string(ptr) or ""
        finally:
            native.lib.unhwp_free_string(ptr)

    @property
    def json(self) -> str:
        """Get the JSON representation."""
        self._ensure_open()
        ptr = native.lib.unhwp_to_json(self._handle, native.UNHWP_JSON_PRETTY)
        if not ptr:
            raise _native_failure(RenderError, "Failed to convert to JSON")
        try:
            return _ptr_to_string(ptr) or ""
        finally:
            native.lib.unhwp_free_string(ptr)

    @property
    def section_count(self) -> int:
        """Get the number of sections in the document."""
        self._ensure_open()
        count = native.lib.unhwp_section_count(self._handle)
        if count < 0:
            raise _native_failure(UnhwpError, "Failed to get section count")
        return count

    @property
    def paragraph_count(self) -> int:
        """Get the number of paragraphs in the document.

        Note: This returns section count as the native API does not expose
        a separate paragraph count.
        """
        self._ensure_open()
        return self.section_count

    @property
    def is_distribution(self) -> bool:
        """Check if the document is a distribution (protected) document.

        Note: Not available in current native API. Always returns False.
        """
        return False

    @property
    def image_count(self) -> int:
        """Get the number of images/resources in the document."""
        self._ensure_open()
        count = native.lib.unhwp_resource_count(self._handle)
        if count < 0:
            raise _native_failure(UnhwpError, "Failed to get resource count")
        return count

    @property
    def title(self) -> Optional[str]:
        """Get the document title, if set."""
        self._ensure_open()
        ptr = native.lib.unhwp_get_title(self._handle)
        if not ptr:
            return None
        try:
            return _ptr_to_string(ptr)
        finally:
            native.lib.unhwp_free_string(ptr)

    @property
    def author(self) -> Optional[str]:
        """Get the document author, if set."""
        self._ensure_open()
        ptr = native.lib.unhwp_get_author(self._handle)
        if not ptr:
            return None
        try:
            return _ptr_to_string(ptr)
        finally:
            native.lib.unhwp_free_string(ptr)

    def get_tables(self, tsv: bool = False) -> List[Dict]:
        """Every table of the document as delimited text, in reading order.

        Args:
            tsv: Tab-separated instead of comma-separated.

        Returns:
            A list of ``{"section", "index", "text"}``: the section's number
            (from 1), the table's place among that section's tables (from 1),
            and the table as CSV (RFC 4180) — tab-separated when ``tsv`` is
            true. A merged cell's text is in its top-left position and the
            positions it covers are empty, so every record has the same number
            of fields; records end with CRLF.
            ``pandas.read_csv(io.StringIO(t["text"]))`` reads one. An empty
            list when the document has no tables.

        Raises:
            UnhwpError: If the tables cannot be produced.
        """
        self._ensure_open()
        ptr = native.lib.unhwp_tables(self._handle, 1 if tsv else 0)
        if not ptr:
            raise _native_failure(UnhwpError, "Failed to get tables")
        try:
            return json.loads(_ptr_to_string(ptr) or "[]")
        finally:
            native.lib.unhwp_free_string(ptr)

    @property
    def images(self) -> List[Image]:
        """Get all images from the document."""
        self._ensure_open()
        images = []

        # Get resource IDs
        ids_ptr = native.lib.unhwp_get_resource_ids(self._handle)
        if not ids_ptr:
            return images

        try:
            ids_json = _ptr_to_string(ids_ptr) or "[]"
        finally:
            native.lib.unhwp_free_string(ids_ptr)

        resource_ids = json.loads(ids_json)

        for resource_id in resource_ids:
            rid_bytes = resource_id.encode("utf-8")
            out_len = ctypes.c_size_t(0)
            data_ptr = native.lib.unhwp_get_resource_data(
                self._handle, rid_bytes, ctypes.byref(out_len)
            )
            if data_ptr and out_len.value > 0:
                data = bytes(data_ptr[:out_len.value])
                native.lib.unhwp_free_bytes(data_ptr, out_len)
                images.append(Image(name=resource_id, data=data))

        return images

    def iter_images(self) -> Iterator[Image]:
        """Iterate over images in the document."""
        self._ensure_open()

        ids_ptr = native.lib.unhwp_get_resource_ids(self._handle)
        if not ids_ptr:
            return

        try:
            ids_json = _ptr_to_string(ids_ptr) or "[]"
        finally:
            native.lib.unhwp_free_string(ids_ptr)

        resource_ids = json.loads(ids_json)

        for resource_id in resource_ids:
            rid_bytes = resource_id.encode("utf-8")
            out_len = ctypes.c_size_t(0)
            data_ptr = native.lib.unhwp_get_resource_data(
                self._handle, rid_bytes, ctypes.byref(out_len)
            )
            if data_ptr and out_len.value > 0:
                data = bytes(data_ptr[:out_len.value])
                native.lib.unhwp_free_bytes(data_ptr, out_len)
                yield Image(name=resource_id, data=data)


# =============================================================================
# Public Functions
# =============================================================================

def version() -> str:
    """Get the unhwp library version."""
    result = native.lib.unhwp_version()
    return result.decode("utf-8") if result else "unknown"


def supported_formats() -> str:
    """Get the supported document formats as a descriptive string."""
    # The native library supports HWP 5.0 and HWPX
    return "HWP 5.0, HWPX"


def detect_format(path: Union[str, Path]) -> int:
    """
    Detect the format of a document file.

    Args:
        path: Path to the document file.

    Returns:
        Format constant (FORMAT_HWP5, FORMAT_HWPX, FORMAT_HWP3, or FORMAT_UNKNOWN).

    Example:
        >>> fmt = unhwp.detect_format("document.hwp")
        >>> if fmt == unhwp.FORMAT_HWP5:
        ...     print("HWP 5.0 format")
    """
    path_obj = Path(str(path))

    # Return UNKNOWN for nonexistent files
    if not path_obj.exists():
        return FORMAT_UNKNOWN

    # The native API does not expose a dedicated format detection function.
    # Detect based on file extension.
    ext = path_obj.suffix.lower()
    if ext == ".hwp":
        return FORMAT_HWP5
    elif ext == ".hwpx":
        return FORMAT_HWPX
    else:
        return FORMAT_UNKNOWN


def format_name(fmt: int) -> str:
    """Get the human-readable name of a format constant."""
    return _FORMAT_NAMES.get(fmt, "Unknown")


def parse(
    path: Union[str, Path],
    *,
    render_options: Optional[RenderOptions] = None,
) -> ParseResult:
    """
    Parse an HWP/HWPX document file.

    Args:
        path: Path to the document file.
        render_options: Optional rendering options.

    Returns:
        ParseResult containing the extracted content.

    Example:
        >>> with unhwp.parse("document.hwp") as result:
        ...     print(result.markdown)
        ...     print(f"Images: {result.image_count}")
    """
    path_bytes = str(path).encode("utf-8")

    handle = native.lib.unhwp_parse_file(path_bytes)
    if not handle:
        raise _native_failure(ParseError, f"Failed to parse {path}")

    return ParseResult(handle, render_options)


def parse_bytes(
    data: bytes,
    *,
    render_options: Optional[RenderOptions] = None,
) -> ParseResult:
    """
    Parse an HWP/HWPX document from bytes.

    Args:
        data: Document content as bytes.
        render_options: Optional rendering options.

    Returns:
        ParseResult containing the extracted content.

    Example:
        >>> data = open("document.hwp", "rb").read()
        >>> with unhwp.parse_bytes(data) as result:
        ...     print(result.markdown)
    """
    data_ptr = (ctypes.c_uint8 * len(data)).from_buffer_copy(data)

    handle = native.lib.unhwp_parse_bytes(data_ptr, len(data))
    if not handle:
        raise _native_failure(ParseError, "Failed to parse bytes")

    return ParseResult(handle, render_options)


def to_markdown(path: Union[str, Path]) -> str:
    """
    Convert an HWP/HWPX document to Markdown.

    This is a convenience function for simple conversions.
    For more control, use `parse()` instead.

    Args:
        path: Path to the document file.

    Returns:
        Markdown content as a string.

    Example:
        >>> markdown = unhwp.to_markdown("document.hwp")
        >>> print(markdown)
    """
    with parse(path) as result:
        return result.markdown


def to_markdown_with_cleanup(
    path: Union[str, Path],
    cleanup_options: Optional[CleanupOptions] = None,
) -> str:
    """
    Convert an HWP/HWPX document to Markdown with cleanup.

    The same as ``parse(path, render_options=RenderOptions(cleanup=...))``
    with the other options at their defaults.

    Args:
        path: Path to the document file.
        cleanup_options: The cleanup to run; standard cleanup when omitted.

    Returns:
        Cleaned Markdown content as a string.

    Example:
        >>> markdown = unhwp.to_markdown_with_cleanup(
        ...     "document.hwp",
        ...     cleanup_options=unhwp.CleanupOptions.aggressive()
        ... )
    """
    options = RenderOptions(cleanup=cleanup_options or CleanupOptions.default())
    with parse(path, render_options=options) as result:
        return result.markdown


def extract_text(path: Union[str, Path]) -> str:
    """
    Extract plain text from an HWP/HWPX document.

    Args:
        path: Path to the document file.

    Returns:
        Plain text content as a string.

    Example:
        >>> text = unhwp.extract_text("document.hwp")
        >>> print(text)
    """
    with parse(path) as result:
        return result.text
