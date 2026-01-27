"""
unhwp - High-performance HWP/HWPX document extraction library

This package provides Python bindings for the unhwp Rust library,
enabling fast extraction of Korean HWP/HWPX documents to Markdown.

Example:
    >>> import unhwp
    >>> markdown = unhwp.to_markdown("document.hwp")
    >>> print(markdown)

    >>> # With options
    >>> result = unhwp.parse("document.hwp")
    >>> print(result.markdown)
    >>> print(result.text)
    >>> for img in result.images:
    ...     print(img.name, len(img.data))
"""

from .unhwp import (
    # Core functions
    parse,
    parse_bytes,
    to_markdown,
    to_markdown_with_cleanup,
    extract_text,
    detect_format,

    # Classes
    ParseResult,
    Image,
    RenderOptions,
    CleanupOptions,

    # Constants
    FORMAT_UNKNOWN,
    FORMAT_HWP5,
    FORMAT_HWPX,
    FORMAT_HWP3,

    # Utilities
    version,
    supported_formats,
)

__version__ = "0.1.10"
__all__ = [
    # Functions
    "parse",
    "parse_bytes",
    "to_markdown",
    "to_markdown_with_cleanup",
    "extract_text",
    "detect_format",
    "version",
    "supported_formats",
    # Classes
    "ParseResult",
    "Image",
    "RenderOptions",
    "CleanupOptions",
    # Constants
    "FORMAT_UNKNOWN",
    "FORMAT_HWP5",
    "FORMAT_HWPX",
    "FORMAT_HWP3",
]
