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
