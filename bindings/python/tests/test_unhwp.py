"""Tests for the unhwp Python bindings."""

import dataclasses
import json
import os
import sys
import pytest
from pathlib import Path

# Imported unconditionally: a native library that does not load is a failure, not a reason to
# skip. Skipped, every test would report green with nothing checked — and this suite is what
# stands between a build and its release. conftest.py points UNHWP_LIB_PATH at a local build.
import unhwp

# The repository's own committed sample, also used by the Rust suite. It ships with the
# source, so its absence is a broken checkout rather than an expected condition — tests
# that need it fail loudly instead of quietly passing over a missing fixture.
SAMPLE_DOCUMENT = (
    Path(__file__).parent.parent.parent.parent / "tests" / "fixtures" / "two_sections.hwpx"
)


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

    def test_every_render_option_is_sent(self):
        """Each field of RenderOptions has its key in the JSON the native library reads —
        a field added here and left out of the JSON would be accepted and ignored."""
        sent = json.loads(unhwp.RenderOptions()._to_json())
        declared = {f.name for f in dataclasses.fields(unhwp.RenderOptions)}
        assert set(sent) == (declared - {"cleanup"}) | {"cleanup_preset"}

    def test_cleanup_is_sent_as_its_preset(self):
        def preset(cleanup):
            return json.loads(unhwp.RenderOptions(cleanup=cleanup)._to_json())["cleanup_preset"]

        assert preset(None) is None
        assert preset(unhwp.CleanupOptions.disabled()) is None
        assert preset(unhwp.CleanupOptions.minimal()) == "minimal"
        assert preset(unhwp.CleanupOptions.default()) == "standard"
        assert preset(unhwp.CleanupOptions()) == "standard"
        assert preset(unhwp.CleanupOptions.aggressive()) == "aggressive"


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
        with unhwp.parse(str(SAMPLE_DOCUMENT)) as result:
            _ = result.markdown

        from unhwp import _native as native
        assert native.lib.unhwp_last_error_kind() == unhwp.ErrorKind.NONE


def _cell(text: str, span: str = "") -> str:
    return f"<hp:tc><hp:subList><hp:p><hp:run><hp:t>{text}</hp:t></hp:run></hp:p></hp:subList>{span}</hp:tc>"


# ┌────────┬───────────────┐
# │ Region │ Sales, total  │   «Region» merged down, «Sales, total» across two columns
# │        ├───────┬───────┤
# │        │ 2024  │ 2025  │
# ├────────┼───────┼───────┤
# │ North  │  10   │  12   │
# └────────┴───────┴───────┘
_TABLE_SECTION = (
    '<?xml version="1.0" encoding="UTF-8"?>'
    '<hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section"'
    ' xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph">'
    "<hp:tbl>"
    "<hp:tr>"
    + _cell("Region", '<hp:cellSpan colSpan="1" rowSpan="2"/>')
    + _cell("Sales, total", '<hp:cellSpan colSpan="2" rowSpan="1"/>')
    + "</hp:tr>"
    "<hp:tr>" + _cell("2024") + _cell("2025") + "</hp:tr>"
    "<hp:tr>" + _cell("North") + _cell("10") + _cell("12") + "</hp:tr>"
    "</hp:tbl>"
    "</hs:sec>"
)


def _hwpx(section0: str, title: "str | None" = None) -> bytes:
    """The committed sample with its first section replaced by ``section0``, and a
    title in its package metadata when ``title`` is given."""
    import io
    import zipfile

    out = io.BytesIO()
    with zipfile.ZipFile(SAMPLE_DOCUMENT) as source, zipfile.ZipFile(out, "w") as target:
        for entry in source.infolist():
            body = source.read(entry)
            if entry.filename == "Contents/section0.xml":
                body = section0.encode("utf-8")
            elif entry.filename == "Contents/content.hpf" and title is not None:
                package = body.decode("utf-8")
                metadata = f"<opf:metadata><opf:title>{title}</opf:title></opf:metadata>"
                body = package.replace("<opf:manifest>", metadata + "<opf:manifest>").encode()
            target.writestr(entry.filename, body, compress_type=zipfile.ZIP_STORED)
    return out.getvalue()


def _hwpx_with_a_table() -> bytes:
    """The committed sample with its first section replaced by ``_TABLE_SECTION``."""
    return _hwpx(_TABLE_SECTION)


def _paragraph(text: str) -> str:
    return f"<hp:p><hp:run><hp:t>{text}</hp:t></hp:run></hp:p>"


# A section that shows what the render options do: chapter, section and article markers
# (headings three levels deep), Markdown syntax in text with a private-use character, an
# empty paragraph, an image, and a table whose first row merges across.
_EVERY_OPTION_SECTION = (
    '<?xml version="1.0" encoding="UTF-8"?>'
    '<hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section"'
    ' xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"'
    ' xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">'
    + _paragraph("제1장 총칙")
    + _paragraph("제1절 목적")
    + _paragraph("제1조 정의")
    + _paragraph("see [x](y) and a*b*c\ue000")
    + "<hp:p></hp:p>"
    + _paragraph("plain words")
    + '<hp:p><hp:run><hp:pic><hc:img binaryItemIDRef="image1"/></hp:pic></hp:run></hp:p>'
    + "<hp:p><hp:run><hp:tbl>"
    + "<hp:tr>" + _cell("Region and sales", '<hp:cellSpan colSpan="2" rowSpan="1"/>') + "</hp:tr>"
    + "<hp:tr>" + _cell("North") + _cell("10") + "</hp:tr>"
    + "<hp:tr>" + _cell("South") + _cell("12") + "</hp:tr>"
    + "</hp:tbl></hp:run></hp:p>"
    + "</hs:sec>"
)


def _every_option_document() -> bytes:
    return _hwpx(_EVERY_OPTION_SECTION, title="Quarterly report")


def _markdown(options: "unhwp.RenderOptions | None" = None) -> str:
    with unhwp.parse_bytes(_every_option_document(), render_options=options) as result:
        return result.markdown


class TestRenderOptions:
    """``RenderOptions`` reach the native library: each one changes the Markdown."""

    def test_the_defaults_are_the_library_defaults(self):
        """``RenderOptions()`` renders what the native library renders with no options."""
        from unhwp import _native as native

        with unhwp.parse_bytes(_every_option_document()) as result:
            ptr = native.lib.unhwp_to_markdown(result._handle, 0)
            try:
                library_default = unhwp.unhwp._ptr_to_string(ptr)
            finally:
                native.lib.unhwp_free_string(ptr)
            assert result.markdown == library_default
        assert _markdown(unhwp.RenderOptions()) == library_default

    @pytest.mark.parametrize(
        "options, present, absent",
        [
            ({"image_path_prefix": "img/"}, "](img/image1)", "](assets/image1)"),
            ({"table_fallback": "html"}, "<table", "| North | 10 |"),
            ({"table_fallback": "skip"}, "plain words", "Region and sales"),
            ({"max_heading_level": 2}, "## ", "### "),
            ({"include_frontmatter": True}, 'title: "Quarterly report"', None),
            ({"include_empty_paragraphs": True}, "\n\n\n\nplain words", None),
            ({"paragraph_spacing": False}, "plain words\n![image]", None),
            ({"escape_special_chars": False}, "see [x](y) and a*b*c", "\\["),
            ({"section_markers": "comment"}, "<!-- section 1 -->", None),
            ({"cleanup": unhwp.CleanupOptions.minimal()}, "plain words", "\ue000"),
        ],
    )
    def test_option_changes_the_output(self, options, present, absent):
        default = _markdown()
        markdown = _markdown(unhwp.RenderOptions(**options))
        assert markdown != default
        assert present in markdown
        if absent is not None:
            assert absent in default
            assert absent not in markdown

    def test_refine_changes_the_output(self):
        unrefined = _markdown(unhwp.RenderOptions(image_path_prefix="img\\sub\\"))
        refined = _markdown(unhwp.RenderOptions(image_path_prefix="img\\sub\\", refine=True))
        assert "](img/sub/image1)" not in unrefined
        assert "](img/sub/image1)" in refined

    # One value per field that the native library refuses. A refusal proves the field
    # reached it; the native suite proves each one, when valid, changes the Markdown —
    # including the two this document cannot show (line breaks, list items).
    _UNHONOURABLE = {
        "image_path_prefix": 1,
        "table_fallback": "ascii",
        "max_heading_level": 7,
        "include_frontmatter": "yes",
        "preserve_line_breaks": "yes",
        "include_empty_paragraphs": "yes",
        "list_marker": "**",
        "paragraph_spacing": "yes",
        "escape_special_chars": "yes",
        "section_markers": "page",
        "cleanup": unhwp.CleanupOptions(preset="default"),
        "refine": "yes",
    }

    def test_every_field_has_an_unhonourable_value(self):
        declared = {f.name for f in dataclasses.fields(unhwp.RenderOptions)}
        assert set(self._UNHONOURABLE) == declared

    @pytest.mark.parametrize("name", sorted(_UNHONOURABLE))
    def test_every_field_reaches_the_native_library(self, name):
        options = unhwp.RenderOptions(**{name: self._UNHONOURABLE[name]})
        with unhwp.parse_bytes(_every_option_document(), render_options=options) as result:
            with pytest.raises(unhwp.RenderError) as raised:
                result.markdown
        assert raised.value.kind == unhwp.ErrorKind.INVALID_ARGUMENT
        assert "options_json" in str(raised.value)


class TestCleanup:
    """``to_markdown_with_cleanup`` runs the cleanup it is given."""

    @pytest.fixture
    def document(self, tmp_path):
        path = tmp_path / "every-option.hwpx"
        path.write_bytes(_every_option_document())
        return str(path)

    def test_each_preset_is_the_render_option(self, document):
        for cleanup in (
            unhwp.CleanupOptions.minimal(),
            unhwp.CleanupOptions.default(),
            unhwp.CleanupOptions.aggressive(),
        ):
            with unhwp.parse(document, render_options=unhwp.RenderOptions(cleanup=cleanup)) as r:
                assert unhwp.to_markdown_with_cleanup(document, cleanup) == r.markdown

    def test_cleanup_changes_the_output(self, document):
        raw = unhwp.to_markdown(document)
        minimal = unhwp.to_markdown_with_cleanup(document, unhwp.CleanupOptions.minimal())
        standard = unhwp.to_markdown_with_cleanup(document, unhwp.CleanupOptions.default())
        assert "\ue000" in raw
        assert "\ue000" not in minimal
        # Standard cleanup also compacts the table; minimal leaves its cells padded.
        assert "| North | 10 |" in minimal
        assert "|North|10|" in standard

    def test_standard_is_the_default_and_disabled_is_none(self, document):
        assert unhwp.to_markdown_with_cleanup(document) == unhwp.to_markdown_with_cleanup(
            document, unhwp.CleanupOptions.default()
        )
        assert unhwp.to_markdown_with_cleanup(
            document, unhwp.CleanupOptions.disabled()
        ) == unhwp.to_markdown(document)


class TestGetTables:
    """Tables as delimited text: ``ParseResult.get_tables``."""

    def test_tables_come_as_csv_with_their_place(self):
        with unhwp.parse_bytes(_hwpx_with_a_table()) as result:
            assert result.get_tables() == [
                {
                    "section": 1,
                    "index": 1,
                    "text": 'Region,"Sales, total",\r\n,2024,2025\r\nNorth,10,12\r\n',
                },
            ]

    def test_tsv_separates_fields_with_tabs(self):
        with unhwp.parse_bytes(_hwpx_with_a_table()) as result:
            assert (
                result.get_tables(tsv=True)[0]["text"]
                == "Region\tSales, total\t\r\n\t2024\t2025\r\nNorth\t10\t12\r\n"
            )

    def test_a_document_without_tables_has_none(self):
        with unhwp.parse(str(SAMPLE_DOCUMENT)) as result:
            assert result.get_tables() == []


@pytest.mark.integration
class TestIntegration:
    """End-to-end tests over the native library, run against the repository's own
    sample document. They mirror what the Rust suite asserts about that same file, so
    content lost on its way through the binding fails here rather than in a consumer."""

    @pytest.fixture
    def test_file(self):
        return SAMPLE_DOCUMENT

    def test_to_markdown(self, test_file):
        """Both sections must survive the round trip, not just the first one."""
        markdown = unhwp.to_markdown(str(test_file))
        assert "Section zero content" in markdown
        assert "Section one content" in markdown

    def test_extract_text(self, test_file):
        """Should extract plain text from every section."""
        text = unhwp.extract_text(str(test_file))
        assert "Section zero content" in text
        assert "Section one content" in text

    def test_parse_result(self, test_file):
        """Should parse and return result object."""
        with unhwp.parse(str(test_file)) as result:
            assert isinstance(result.markdown, str)
            assert isinstance(result.text, str)
            # A count of 1 is the signature of a section-order parser that stopped
            # after section0.
            assert result.section_count == 2
            assert result.image_count >= 0

    def test_parse_with_options(self, test_file):
        """Render options apply to a document parsed from a path."""
        opts = unhwp.RenderOptions(section_markers="comment")
        with unhwp.parse(str(test_file), render_options=opts) as result:
            markdown = result.markdown
        assert "<!-- section 0 -->" in markdown
        assert "<!-- section 1 -->" in markdown
        assert "<!--" not in unhwp.to_markdown(str(test_file))


def test_a_library_path_naming_no_file_is_an_error(tmp_path):
    """UNHWP_LIB_PATH pointing at nothing fails the import instead of loading the packaged library."""
    import subprocess

    missing = tmp_path / "missing-library"
    env = dict(os.environ, UNHWP_LIB_PATH=str(missing))
    env["PYTHONPATH"] = str(Path(__file__).resolve().parents[1] / "src")
    result = subprocess.run(
        [sys.executable, "-c", "import unhwp; unhwp.version()"],
        capture_output=True,
        text=True,
        env=env,
        check=False,
    )
    assert result.returncode != 0
    assert "UNHWP_LIB_PATH" in result.stderr and missing.name in result.stderr
