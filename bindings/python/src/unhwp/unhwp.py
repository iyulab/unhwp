"""
High-level Python API for unhwp.

Provides a Pythonic interface to the unhwp native library.
"""

import ctypes
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional, Union, Iterator
from contextlib import contextmanager

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


_ERROR_MAP = {
    native.UNHWP_ERR_FILE_NOT_FOUND: FileNotFoundError,
    native.UNHWP_ERR_PARSE: ParseError,
    native.UNHWP_ERR_RENDER: RenderError,
    native.UNHWP_ERR_UNSUPPORTED: UnsupportedFormatError,
}


def _check_error(result_ptr: native.UnhwpResultPtr) -> None:
    """Check for errors in the result and raise appropriate exception."""
    if result_ptr is None:
        raise UnhwpError("Failed to parse document: unknown error")

    error = native.lib.unhwp_result_get_error(result_ptr)
    if error:
        error_msg = error.decode("utf-8")
        raise ParseError(error_msg)


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

    def _to_native(self) -> native.UnhwpRenderOptions:
        opts = native.UnhwpRenderOptions()
        opts.include_frontmatter = self.include_frontmatter
        opts.image_path_prefix = self.image_path_prefix.encode("utf-8") if self.image_path_prefix else None
        opts.table_fallback = self.table_fallback
        opts.preserve_line_breaks = self.preserve_line_breaks
        opts.escape_special_chars = self.escape_special_chars
        return opts


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

    def _to_native(self) -> native.UnhwpCleanupOptions:
        opts = native.UnhwpCleanupOptions()
        opts.enabled = self.enabled
        opts.preset = self.preset
        opts.detect_mojibake = self.detect_mojibake
        opts.preserve_frontmatter = self.preserve_frontmatter
        return opts


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

    def __init__(self, result_ptr: native.UnhwpResultPtr):
        self._ptr = result_ptr
        self._closed = False
        _check_error(result_ptr)

    def __enter__(self) -> "ParseResult":
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.close()

    def __del__(self) -> None:
        self.close()

    def close(self) -> None:
        """Release native resources."""
        if not self._closed and self._ptr:
            native.lib.unhwp_result_free(self._ptr)
            self._ptr = None
            self._closed = True

    def _ensure_open(self) -> None:
        if self._closed:
            raise ValueError("ParseResult has been closed")

    @property
    def markdown(self) -> str:
        """Get the rendered Markdown content."""
        self._ensure_open()
        result = native.lib.unhwp_result_get_markdown(self._ptr)
        return result.decode("utf-8") if result else ""

    @property
    def text(self) -> str:
        """Get the plain text content."""
        self._ensure_open()
        result = native.lib.unhwp_result_get_text(self._ptr)
        return result.decode("utf-8") if result else ""

    @property
    def raw_content(self) -> str:
        """Get the raw content (without cleanup)."""
        self._ensure_open()
        result = native.lib.unhwp_result_get_raw_content(self._ptr)
        return result.decode("utf-8") if result else ""

    @property
    def section_count(self) -> int:
        """Get the number of sections in the document."""
        self._ensure_open()
        return native.lib.unhwp_result_get_section_count(self._ptr)

    @property
    def paragraph_count(self) -> int:
        """Get the number of paragraphs in the document."""
        self._ensure_open()
        return native.lib.unhwp_result_get_paragraph_count(self._ptr)

    @property
    def is_distribution(self) -> bool:
        """Check if the document is a distribution (protected) document.

        Distribution documents are protected by DRM and may have restrictions
        on editing, copying, and printing.
        """
        self._ensure_open()
        return native.lib.unhwp_result_is_distribution(self._ptr) == 1

    @property
    def image_count(self) -> int:
        """Get the number of images in the document."""
        self._ensure_open()
        return native.lib.unhwp_result_get_image_count(self._ptr)

    @property
    def images(self) -> List[Image]:
        """Get all images from the document."""
        self._ensure_open()
        count = self.image_count
        images = []

        for i in range(count):
            img_struct = native.UnhwpImage()
            if native.lib.unhwp_result_get_image(self._ptr, i, ctypes.byref(img_struct)) == 0:
                name = img_struct.name.decode("utf-8") if img_struct.name else f"image_{i}"
                data = bytes(img_struct.data[:img_struct.data_len])
                images.append(Image(name=name, data=data))

        return images

    def iter_images(self) -> Iterator[Image]:
        """Iterate over images in the document."""
        self._ensure_open()
        count = self.image_count

        for i in range(count):
            img_struct = native.UnhwpImage()
            if native.lib.unhwp_result_get_image(self._ptr, i, ctypes.byref(img_struct)) == 0:
                name = img_struct.name.decode("utf-8") if img_struct.name else f"image_{i}"
                data = bytes(img_struct.data[:img_struct.data_len])
                yield Image(name=name, data=data)


# =============================================================================
# Public Functions
# =============================================================================

def version() -> str:
    """Get the unhwp library version."""
    result = native.lib.unhwp_version()
    return result.decode("utf-8") if result else "unknown"


def supported_formats() -> str:
    """Get the supported document formats as a descriptive string."""
    flags = native.lib.unhwp_supported_formats()
    formats = []
    if flags & 0x01:
        formats.append("HWP 5.0")
    if flags & 0x02:
        formats.append("HWPX")
    if flags & 0x04:
        formats.append("HWP 3.x")
    return ", ".join(formats) if formats else "None"


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
    path_bytes = str(path).encode("utf-8")
    return native.lib.unhwp_detect_format(path_bytes)


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
    opts = (render_options or RenderOptions())._to_native()

    result_ptr = native.lib.unhwp_parse(path_bytes, opts)
    return ParseResult(result_ptr)


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
    opts = (render_options or RenderOptions())._to_native()
    data_ptr = (ctypes.c_uint8 * len(data)).from_buffer_copy(data)

    result_ptr = native.lib.unhwp_parse_bytes(data_ptr, len(data), opts)
    return ParseResult(result_ptr)


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
    path_bytes = str(path).encode("utf-8")
    result = native.lib.unhwp_to_markdown(path_bytes)

    if result is None:
        raise ParseError(f"Failed to convert {path} to markdown")

    return result.decode("utf-8")


def to_markdown_with_cleanup(
    path: Union[str, Path],
    cleanup_options: Optional[CleanupOptions] = None,
) -> str:
    """
    Convert an HWP/HWPX document to Markdown with cleanup.

    Args:
        path: Path to the document file.
        cleanup_options: Optional cleanup options.

    Returns:
        Cleaned Markdown content as a string.

    Example:
        >>> markdown = unhwp.to_markdown_with_cleanup(
        ...     "document.hwp",
        ...     cleanup_options=unhwp.CleanupOptions.aggressive()
        ... )
    """
    path_bytes = str(path).encode("utf-8")
    opts = (cleanup_options or CleanupOptions())._to_native()
    result = native.lib.unhwp_to_markdown_with_cleanup(path_bytes, opts)

    if result is None:
        raise ParseError(f"Failed to convert {path} to markdown")

    return result.decode("utf-8")


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
    path_bytes = str(path).encode("utf-8")
    result = native.lib.unhwp_extract_text(path_bytes)

    if result is None:
        raise ParseError(f"Failed to extract text from {path}")

    return result.decode("utf-8")
