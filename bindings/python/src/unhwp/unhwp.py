"""
High-level Python API for unhwp.

Provides a Pythonic interface to the unhwp native library.
"""

import ctypes
import json
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional, Union, Iterator

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

class UnhwpError(Exception):
    """Base exception for unhwp errors."""
    pass


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
class RenderOptions:
    """Options for rendering documents to Markdown."""
    include_frontmatter: bool = False
    image_path_prefix: str = ""
    table_fallback: int = 0  # 0=markdown, 1=html, 2=text
    preserve_line_breaks: bool = False
    escape_special_chars: bool = True

    def _to_flags(self) -> int:
        """Convert to native flags bitmask."""
        flags = 0
        if self.include_frontmatter:
            flags |= native.UNHWP_FLAG_FRONTMATTER
        if self.escape_special_chars:
            flags |= native.UNHWP_FLAG_ESCAPE_SPECIAL
        if self.preserve_line_breaks:
            flags |= native.UNHWP_FLAG_PARAGRAPH_SPACING
        return flags


@dataclass
class CleanupOptions:
    """Options for cleaning up extracted Markdown."""
    enabled: bool = True
    preset: int = 1  # 0=minimal, 1=default, 2=aggressive
    detect_mojibake: bool = True
    preserve_frontmatter: bool = True

    @classmethod
    def minimal(cls) -> "CleanupOptions":
        """Create minimal cleanup options."""
        return cls(enabled=True, preset=0)

    @classmethod
    def default(cls) -> "CleanupOptions":
        """Create default cleanup options."""
        return cls(enabled=True, preset=1)

    @classmethod
    def aggressive(cls) -> "CleanupOptions":
        """Create aggressive cleanup options."""
        return cls(enabled=True, preset=2)

    @classmethod
    def disabled(cls) -> "CleanupOptions":
        """Create disabled cleanup options."""
        return cls(enabled=False)


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

    def __init__(self, handle: int, flags: int = 0):
        self._handle = handle
        self._flags = flags
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
        """Get the rendered Markdown content."""
        self._ensure_open()
        ptr = native.lib.unhwp_to_markdown(self._handle, self._flags)
        if not ptr:
            raise RenderError(f"Failed to convert to markdown: {_get_last_error()}")
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
            raise RenderError(f"Failed to convert to text: {_get_last_error()}")
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
            raise RenderError(f"Failed to get plain text: {_get_last_error()}")
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
            raise RenderError(f"Failed to convert to JSON: {_get_last_error()}")
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
            raise UnhwpError(f"Failed to get section count: {_get_last_error()}")
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
            raise UnhwpError(f"Failed to get resource count: {_get_last_error()}")
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
    flags = (render_options or RenderOptions())._to_flags()

    handle = native.lib.unhwp_parse_file(path_bytes)
    if not handle:
        raise ParseError(f"Failed to parse {path}: {_get_last_error()}")

    return ParseResult(handle, flags)


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
    flags = (render_options or RenderOptions())._to_flags()
    data_ptr = (ctypes.c_uint8 * len(data)).from_buffer_copy(data)

    handle = native.lib.unhwp_parse_bytes(data_ptr, len(data))
    if not handle:
        raise ParseError(f"Failed to parse bytes: {_get_last_error()}")

    return ParseResult(handle, flags)


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

    Note: Cleanup is performed client-side as the native library does not
    expose a dedicated cleanup API. Currently returns the same result as
    to_markdown().

    Args:
        path: Path to the document file.
        cleanup_options: Optional cleanup options (reserved for future use).

    Returns:
        Cleaned Markdown content as a string.

    Example:
        >>> markdown = unhwp.to_markdown_with_cleanup(
        ...     "document.hwp",
        ...     cleanup_options=unhwp.CleanupOptions.aggressive()
        ... )
    """
    # The native library does not have a cleanup API; return markdown as-is
    return to_markdown(path)


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
