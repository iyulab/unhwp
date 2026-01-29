"""
Native library loader for unhwp.

Handles loading the correct native library for the current platform.
"""

import ctypes
import os
import platform
import sys
from ctypes import (
    POINTER, Structure, c_char_p, c_int, c_uint8, c_size_t, c_void_p, c_bool
)
from pathlib import Path
from typing import Optional


def _get_lib_name() -> str:
    """Get the library filename for the current platform."""
    system = platform.system()
    if system == "Windows":
        return "unhwp.dll"
    elif system == "Darwin":
        return "libunhwp.dylib"
    else:  # Linux and others
        return "libunhwp.so"


def _get_platform_dir() -> str:
    """Get the platform-specific directory name."""
    system = platform.system()
    machine = platform.machine().lower()

    if system == "Windows":
        return "win-x64"
    elif system == "Darwin":
        if machine in ("arm64", "aarch64"):
            return "osx-arm64"
        return "osx-x64"
    else:  # Linux and others
        return "linux-x64"


def _find_library() -> Optional[Path]:
    """Find the native library in various locations."""
    lib_name = _get_lib_name()
    platform_dir = _get_platform_dir()
    pkg_dir = Path(__file__).parent

    # Search locations in order of priority
    search_paths = [
        # 1. Platform-specific subdirectory (packaged wheel)
        pkg_dir / "lib" / platform_dir / lib_name,
        # 2. Flat lib directory
        pkg_dir / "lib" / lib_name,
        # 3. Same directory as this module
        pkg_dir / lib_name,
        # 4. Package data directory
        pkg_dir / "data" / lib_name,
        # 5. System paths (for development)
        Path.cwd() / "target" / "release" / lib_name,
        Path.cwd() / "target" / "debug" / lib_name,
        # 6. Development: relative to bindings directory
        pkg_dir.parent.parent.parent.parent / "target" / "release" / lib_name,
    ]

    # Check UNHWP_LIB_PATH environment variable
    env_path = os.environ.get("UNHWP_LIB_PATH")
    if env_path:
        search_paths.insert(0, Path(env_path))

    for path in search_paths:
        if path.exists():
            return path

    return None


def _load_library() -> ctypes.CDLL:
    """Load the native library."""
    lib_path = _find_library()

    if lib_path is None:
        # Try system library path as last resort
        lib_name = _get_lib_name()
        try:
            return ctypes.CDLL(lib_name)
        except OSError:
            raise OSError(
                f"Could not find unhwp native library ({lib_name}). "
                f"Please ensure the library is installed or set UNHWP_LIB_PATH environment variable."
            )

    return ctypes.CDLL(str(lib_path))


# Load the library
_lib = _load_library()


# =============================================================================
# Structures
# =============================================================================

class UnhwpCleanupOptions(Structure):
    """Cleanup options structure."""
    _fields_ = [
        ("enabled", c_bool),
        ("preset", c_int),  # 0=minimal, 1=default, 2=aggressive
        ("detect_mojibake", c_bool),
        ("preserve_frontmatter", c_bool),
    ]


class UnhwpRenderOptions(Structure):
    """Render options structure."""
    _fields_ = [
        ("include_frontmatter", c_bool),
        ("image_path_prefix", c_char_p),
        ("table_fallback", c_int),  # 0=markdown, 1=html, 2=text
        ("preserve_line_breaks", c_bool),
        ("escape_special_chars", c_bool),
    ]


class UnhwpImage(Structure):
    """Image data structure."""
    _fields_ = [
        ("name", c_char_p),
        ("data", POINTER(c_uint8)),
        ("data_len", c_size_t),
    ]


# Opaque pointer for UnhwpResult
UnhwpResultPtr = c_void_p


# =============================================================================
# Error codes
# =============================================================================

UNHWP_OK = 0
UNHWP_ERR_FILE_NOT_FOUND = 1
UNHWP_ERR_PARSE = 2
UNHWP_ERR_RENDER = 3
UNHWP_ERR_INVALID_ARG = 4
UNHWP_ERR_UNSUPPORTED = 5
UNHWP_ERR_UNKNOWN = 99

# Format constants
FORMAT_UNKNOWN = 0
FORMAT_HWP5 = 1
FORMAT_HWPX = 2
FORMAT_HWP3 = 3


# =============================================================================
# Function signatures
# =============================================================================

# Version and info
_lib.unhwp_version.argtypes = []
_lib.unhwp_version.restype = c_char_p

_lib.unhwp_supported_formats.argtypes = []
_lib.unhwp_supported_formats.restype = c_int

# Format detection
_lib.unhwp_detect_format.argtypes = [c_char_p]
_lib.unhwp_detect_format.restype = c_int

# Options
_lib.unhwp_cleanup_options_default.argtypes = []
_lib.unhwp_cleanup_options_default.restype = UnhwpCleanupOptions

_lib.unhwp_cleanup_options_enabled.argtypes = [c_int]
_lib.unhwp_cleanup_options_enabled.restype = UnhwpCleanupOptions

_lib.unhwp_render_options_default.argtypes = []
_lib.unhwp_render_options_default.restype = UnhwpRenderOptions

# Simple conversion functions
_lib.unhwp_to_markdown.argtypes = [c_char_p]
_lib.unhwp_to_markdown.restype = c_char_p

_lib.unhwp_to_markdown_with_cleanup.argtypes = [c_char_p, UnhwpCleanupOptions]
_lib.unhwp_to_markdown_with_cleanup.restype = c_char_p

_lib.unhwp_to_markdown_ex.argtypes = [c_char_p, UnhwpRenderOptions, UnhwpCleanupOptions]
_lib.unhwp_to_markdown_ex.restype = c_char_p

_lib.unhwp_extract_text.argtypes = [c_char_p]
_lib.unhwp_extract_text.restype = c_char_p

_lib.unhwp_bytes_to_markdown.argtypes = [POINTER(c_uint8), c_size_t]
_lib.unhwp_bytes_to_markdown.restype = c_char_p

_lib.unhwp_bytes_to_markdown_ex.argtypes = [POINTER(c_uint8), c_size_t, UnhwpRenderOptions, UnhwpCleanupOptions]
_lib.unhwp_bytes_to_markdown_ex.restype = c_char_p

# Parse functions (returning result handle)
_lib.unhwp_parse.argtypes = [c_char_p, UnhwpRenderOptions]
_lib.unhwp_parse.restype = UnhwpResultPtr

_lib.unhwp_parse_bytes.argtypes = [POINTER(c_uint8), c_size_t, UnhwpRenderOptions]
_lib.unhwp_parse_bytes.restype = UnhwpResultPtr

# Result accessors
_lib.unhwp_result_get_markdown.argtypes = [UnhwpResultPtr]
_lib.unhwp_result_get_markdown.restype = c_char_p

_lib.unhwp_result_get_text.argtypes = [UnhwpResultPtr]
_lib.unhwp_result_get_text.restype = c_char_p

_lib.unhwp_result_get_raw_content.argtypes = [UnhwpResultPtr]
_lib.unhwp_result_get_raw_content.restype = c_char_p

_lib.unhwp_result_get_image_count.argtypes = [UnhwpResultPtr]
_lib.unhwp_result_get_image_count.restype = c_size_t

_lib.unhwp_result_get_image.argtypes = [UnhwpResultPtr, c_size_t, POINTER(UnhwpImage)]
_lib.unhwp_result_get_image.restype = c_int

_lib.unhwp_result_get_section_count.argtypes = [UnhwpResultPtr]
_lib.unhwp_result_get_section_count.restype = c_size_t

_lib.unhwp_result_get_paragraph_count.argtypes = [UnhwpResultPtr]
_lib.unhwp_result_get_paragraph_count.restype = c_size_t

_lib.unhwp_result_is_distribution.argtypes = [UnhwpResultPtr]
_lib.unhwp_result_is_distribution.restype = c_int

_lib.unhwp_result_get_error.argtypes = [UnhwpResultPtr]
_lib.unhwp_result_get_error.restype = c_char_p

_lib.unhwp_result_free.argtypes = [UnhwpResultPtr]
_lib.unhwp_result_free.restype = None

# Memory management
_lib.unhwp_free_string.argtypes = [c_char_p]
_lib.unhwp_free_string.restype = None


# =============================================================================
# Export
# =============================================================================

lib = _lib
