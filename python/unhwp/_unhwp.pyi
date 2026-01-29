from __future__ import annotations

from enum import IntEnum
from typing import Union, overload, Optional


class FormatType(IntEnum):
    """Document format returned by :func:`detect_format`.

    Values:
        Hwp5: HWP5 binary format
        Hwpx: HWPX (XML) format
        Hwp3: legacy HWP3 format
    """
    Hwp5 = 1
    Hwpx = 2
    Hwp3 = 3


def detect_format(data: Union[bytes, bytearray, memoryview]) -> FormatType:
    """Detect the document format from raw bytes.

    Args:
        data: Binary contents of an HWP/HWPX file.

    Returns:
        FormatType: detected format.

    Raises:
        ValueError: when the data cannot be parsed or format is unsupported.
    """
    ...


class Document:
    """Opaque handle for a parsed document returned by :func:`parse`.

    The low-level extension exposes this type as an opaque object (no
    attributes/methods on the raw module).
    """
    ...


def parse(data: Union[bytes, bytearray, memoryview]) -> Document:
    """Parse HWP/HWPX bytes and return a Document handle.

    Args:
        data: Document bytes.

    Returns:
        Document: opaque parsed document handle (low-level).

    Raises:
        ValueError: on parse failure.
    """
    ...


@overload
def convert_to_markdown(data: Union[bytes, bytearray, memoryview]) -> str: ...

@overload
def convert_to_markdown(document: Document) -> str: ...

def convert_to_markdown(
    data: Optional[Union[bytes, bytearray, memoryview]] = None,
    document: Optional[Document] = None,
) -> str:
    """Render a document (or raw bytes) to Markdown using default options.

    Exactly one of data or document must be provided.

    Args:
        data: Raw document bytes (optional).
        document: Parsed Document handle (optional).
    Returns:
        str: Markdown result.

    Raises:
        ValueError: when both/neither arguments are provided or rendering fails.
    """
    ...


@overload
def is_distribution(data: Union[bytes, bytearray, memoryview]) -> bool: ...

@overload
def is_distribution(document: Document) -> bool: ...

def is_distribution(
    data: Optional[Union[bytes, bytearray, memoryview]] = None,
    document: Optional[Document] = None,
) -> bool:
    """Return True if the document is a distribution (CDF) document.

    Exactly one of ``data`` or ``document`` must be provided.

    Raises:
        ValueError: when both/neither arguments are provided or parsing fails.
    """
    ...


__all__ = [
    "detect_format",
    "convert_to_markdown",
    "is_distribution",
    "FormatType",
    "Document",
    "parse",
]
