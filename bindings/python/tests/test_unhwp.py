"""Tests for the unhwp Python bindings."""

import os
import pytest
from pathlib import Path

# Skip tests if native library is not available
try:
    import unhwp
    HAS_NATIVE = True
except OSError:
    HAS_NATIVE = False

pytestmark = pytest.mark.skipif(not HAS_NATIVE, reason="Native library not available")


class TestVersion:
    """Test version and info functions."""

    def test_version_returns_string(self):
        """Version should return a non-empty string."""
        version = unhwp.version()
        assert isinstance(version, str)
        assert len(version) > 0

    def test_version_format(self):
        """Version should be in semver format."""
        version = unhwp.version()
        parts = version.split(".")
        assert len(parts) >= 2

    def test_supported_formats(self):
        """Should return supported formats string."""
        formats = unhwp.supported_formats()
        assert isinstance(formats, str)
        assert "HWP" in formats or "hwp" in formats.lower()


class TestFormatDetection:
    """Test format detection."""

    def test_detect_format_unknown_file(self, tmp_path):
        """Should return FORMAT_UNKNOWN for non-HWP files."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("Hello, World!")

        fmt = unhwp.detect_format(str(test_file))
        assert fmt == unhwp.FORMAT_UNKNOWN

    def test_detect_format_nonexistent_file(self, tmp_path):
        """Should return FORMAT_UNKNOWN for nonexistent files."""
        fmt = unhwp.detect_format(str(tmp_path / "nonexistent.hwp"))
        assert fmt == unhwp.FORMAT_UNKNOWN


class TestOptions:
    """Test options classes."""

    def test_render_options_defaults(self):
        """RenderOptions should have sensible defaults."""
        opts = unhwp.RenderOptions()
        assert opts.include_frontmatter == False
        assert opts.image_path_prefix == ""
        assert opts.escape_special_chars == True

    def test_cleanup_options_presets(self):
        """CleanupOptions should have working presets."""
        minimal = unhwp.CleanupOptions.minimal()
        assert minimal.preset == 0
        assert minimal.enabled == True

        default = unhwp.CleanupOptions.default()
        assert default.preset == 1

        aggressive = unhwp.CleanupOptions.aggressive()
        assert aggressive.preset == 2

        disabled = unhwp.CleanupOptions.disabled()
        assert disabled.enabled == False


class TestConstants:
    """Test module constants."""

    def test_format_constants(self):
        """Format constants should be defined."""
        assert hasattr(unhwp, "FORMAT_UNKNOWN")
        assert hasattr(unhwp, "FORMAT_HWP5")
        assert hasattr(unhwp, "FORMAT_HWPX")
        assert hasattr(unhwp, "FORMAT_HWP3")

    def test_format_constants_values(self):
        """Format constants should have distinct values."""
        formats = [
            unhwp.FORMAT_UNKNOWN,
            unhwp.FORMAT_HWP5,
            unhwp.FORMAT_HWPX,
            unhwp.FORMAT_HWP3,
        ]
        assert len(formats) == len(set(formats))


class TestErrorKind:
    """Test the failure classification carried by unhwp exceptions."""

    def test_message_only_exception_is_other_not_none(self):
        """An exception built without a kind did not come from the native
        library, so it must default to OTHER — never NONE, which means
        success."""
        err = unhwp.UnhwpError("wrapper-side failure")
        assert err.kind == unhwp.ErrorKind.OTHER
        assert err.kind != unhwp.ErrorKind.NONE

    def test_unknown_kind_value_passes_through_and_keeps_its_number(self):
        """Forward compatibility: a newer native library may report a reason
        this build has no name for. The number has to survive rather than
        raise or collapse."""
        err = unhwp.ParseError("from the future", 9999)
        assert int(err.kind) == 9999

    def test_discriminants_match_the_native_abi(self):
        """The Python numbering is only useful if it agrees with the native
        ABI, so pin it here too — these values are what cross the boundary."""
        assert unhwp.ErrorKind.NONE == 0
        assert unhwp.ErrorKind.OTHER == 1
        assert unhwp.ErrorKind.IO == 2
        assert unhwp.ErrorKind.UNKNOWN_FORMAT == 3
        assert unhwp.ErrorKind.UNSUPPORTED_FORMAT == 4
        assert unhwp.ErrorKind.ZIP_ARCHIVE == 5
        assert unhwp.ErrorKind.XML_PARSE == 6
        assert unhwp.ErrorKind.INVALID_DATA == 7
        assert unhwp.ErrorKind.MISSING_COMPONENT == 8
        assert unhwp.ErrorKind.ENCODING == 9
        assert unhwp.ErrorKind.STYLE_NOT_FOUND == 10
        assert unhwp.ErrorKind.RESOURCE_NOT_FOUND == 11
        assert unhwp.ErrorKind.ENCRYPTED == 12
        assert unhwp.ErrorKind.RENDER == 13
        assert unhwp.ErrorKind.DECOMPRESSION == 400
        assert unhwp.ErrorKind.OLE_CONTAINER == 401
        assert unhwp.ErrorKind.RECORD_PARSE == 402
        assert unhwp.ErrorKind.DISTRIBUTION_RESTRICTED == 403
        assert unhwp.ErrorKind.INVALID_ARGUMENT == 100
        assert unhwp.ErrorKind.PANIC == 101
        assert unhwp.ErrorKind.INVALID_OUTPUT == 102


class TestNativeErrorKind:
    """End to end: a native failure must be recognisable from the exception
    without reading its message."""

    def test_parse_bytes_not_a_document_reports_unknown_format(self):
        with pytest.raises(unhwp.ParseError) as excinfo:
            unhwp.parse_bytes(b"not an hwp document at all")

        assert excinfo.value.kind == unhwp.ErrorKind.UNKNOWN_FORMAT

    def test_successful_call_leaves_no_recorded_kind(self, tmp_path):
        test_files_dir = Path(__file__).parent.parent.parent.parent / "test-files"
        sample = test_files_dir / "Sample.hwp"
        if not sample.exists():
            pytest.skip("Test file not available")

        with unhwp.parse(str(sample)) as result:
            _ = result.markdown

        from unhwp import _native as native
        assert native.lib.unhwp_last_error_kind() == unhwp.ErrorKind.NONE


@pytest.mark.integration
class TestIntegration:
    """Integration tests requiring actual HWP files."""

    @pytest.fixture
    def test_file(self):
        """Get path to test file if available."""
        test_files_dir = Path(__file__).parent.parent.parent.parent / "test-files"
        sample = test_files_dir / "Sample.hwp"
        if sample.exists():
            return sample
        pytest.skip("Test file not available")

    def test_to_markdown(self, test_file):
        """Should convert HWP to markdown."""
        markdown = unhwp.to_markdown(str(test_file))
        assert isinstance(markdown, str)
        assert len(markdown) > 0

    def test_extract_text(self, test_file):
        """Should extract plain text."""
        text = unhwp.extract_text(str(test_file))
        assert isinstance(text, str)
        assert len(text) > 0

    def test_parse_result(self, test_file):
        """Should parse and return result object."""
        with unhwp.parse(str(test_file)) as result:
            assert isinstance(result.markdown, str)
            assert isinstance(result.text, str)
            assert result.section_count >= 0
            assert result.paragraph_count >= 0
            assert result.image_count >= 0

    def test_parse_with_options(self, test_file):
        """Should respect render options."""
        opts = unhwp.RenderOptions(include_frontmatter=True)
        with unhwp.parse(str(test_file), render_options=opts) as result:
            markdown = result.markdown
            # Frontmatter starts with ---
            # (may or may not be present depending on document)
            assert isinstance(markdown, str)

    def test_to_markdown_with_cleanup(self, test_file):
        """Should apply cleanup options."""
        clean = unhwp.to_markdown_with_cleanup(
            str(test_file),
            cleanup_options=unhwp.CleanupOptions.aggressive()
        )
        raw = unhwp.to_markdown(str(test_file))

        # Cleanup should generally reduce or equal size
        assert len(clean) <= len(raw) + 100  # Allow small increase from formatting
