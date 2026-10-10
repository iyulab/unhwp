# unhwp

High-performance Python library for extracting HWP/HWPX Korean word processor documents to Markdown.

## Installation

```bash
pip install unhwp
```

## Quick Start

```python
import unhwp

# Simple conversion
markdown = unhwp.to_markdown("document.hwp")
print(markdown)

# Extract plain text
text = unhwp.extract_text("document.hwp")

# Full parsing with images
with unhwp.parse("document.hwp") as result:
    print(result.markdown)
    print(f"Sections: {result.section_count}")

    # Save images
    for img in result.images:
        img.save(f"output/{img.name}")
```

## Handling Failures

`UnhwpError.kind` says *why* a call failed, so you can react to the reason instead of
matching on message text:

```python
from unhwp import ErrorKind, ParseError

try:
    with unhwp.parse(path) as result:
        print(result.markdown)
except ParseError as err:
    if err.kind in (ErrorKind.OLE_CONTAINER, ErrorKind.ZIP_ARCHIVE):
        print("The file is damaged.")
    elif err.kind in (ErrorKind.UNKNOWN_FORMAT, ErrorKind.UNSUPPORTED_FORMAT):
        print("Not a supported HWP/HWPX document.")
    elif err.kind in (ErrorKind.ENCRYPTED, ErrorKind.DISTRIBUTION_RESTRICTED):
        print("The document cannot be opened without authorization.")
    else:
        # Also the right branch for a reason this build has no name for.
        print(f"Extraction failed ({err.kind}): {err}")
```

The numbers behind `ErrorKind` are a stable ABI contract: a new reason takes the next free
number and existing ones are never renumbered. Always keep a final `else` — an
unrecognised value arrives as a plain `int` rather than an `ErrorKind`, so that a newer
native library stays usable. `kind` is `ErrorKind.OTHER` for failures raised by the wrapper
itself, and never `ErrorKind.NONE` (which means success).

## Features

- **Fast**: Native Rust library with zero-copy parsing
- **Complete**: Extracts text, tables, images, and document structure
- **Clean Output**: Optional cleanup pipeline for polished Markdown
- **Format Support**: HWP 5.0, HWPX, and HWP 3.x (legacy)

## API Reference

### Functions

#### `to_markdown(path) -> str`
Convert an HWP/HWPX document to Markdown.

```python
markdown = unhwp.to_markdown("document.hwp")
```

#### `to_markdown_with_cleanup(path, cleanup_options=None) -> str`
Convert and run the cleanup pipeline over the Markdown — standard cleanup when
`cleanup_options` is omitted.

```python
markdown = unhwp.to_markdown_with_cleanup(
    "document.hwp",
    cleanup_options=unhwp.CleanupOptions.aggressive()
)
```

#### `extract_text(path) -> str`
Extract plain text content.

```python
text = unhwp.extract_text("document.hwp")
```

#### `parse(path, render_options=None) -> ParseResult`
Parse a document with full access to content and images. `result.markdown` is rendered
with `render_options`; `parse_bytes(data, render_options=None)` does the same for a
document in memory.

```python
with unhwp.parse("document.hwp") as result:
    print(result.markdown)
    print(result.text)
    for img in result.images:
        print(img.name, len(img.data))
```

#### `detect_format(path) -> int`
Detect the document format.

```python
fmt = unhwp.detect_format("document.hwp")
if fmt == unhwp.FORMAT_HWP5:
    print("HWP 5.0 format")
elif fmt == unhwp.FORMAT_HWPX:
    print("HWPX format")
```

### Classes

#### `ParseResult`
What `parse()` and `parse_bytes()` return; use it as a context manager or call `close()`.

- `markdown`, `text`, `plain_text`, `json` - The document rendered
- `section_count`, `image_count`, `title`, `author` - About the document
- `images`, `iter_images()` - Embedded images
- `get_tables(tsv=False)` - Every table as CSV (RFC 4180), or tab-separated with `tsv=True`, in reading order: `{"section", "index", "text"}` — the section's number (from 1), the table's place among that section's tables (from 1), and the text. A merged cell's text is in its top-left position and the positions it covers are empty, so every record has the same number of fields: `pandas.read_csv(io.StringIO(t["text"]))` reads one.

#### `RenderOptions`
Options for Markdown rendering. Every field reaches the native library, and the defaults
are the library's own.

```python
opts = unhwp.RenderOptions(
    include_frontmatter=True,
    image_path_prefix="images/",
    table_fallback="html",
    cleanup=unhwp.CleanupOptions.default(),
)
with unhwp.parse("document.hwp", render_options=opts) as result:
    print(result.markdown)
```

| Field | Default | Meaning |
|---|---|---|
| `image_path_prefix` | `"assets/"` | Prefix of the image paths in the Markdown |
| `table_fallback` | `"simplified_markdown"` | A table with merged cells: `"simplified_markdown"` (merges dropped), `"html"` (rowspan/colspan kept) or `"skip"` |
| `max_heading_level` | `4` | Deepest heading level written (1-6) |
| `include_frontmatter` | `False` | Document metadata as YAML frontmatter |
| `preserve_line_breaks` | `True` | Line breaks inside a paragraph as Markdown hard breaks; `False` joins the lines |
| `include_empty_paragraphs` | `False` | Keep empty paragraphs as blank lines |
| `list_marker` | `"-"` | Marker of an unordered list item (one character) |
| `paragraph_spacing` | `True` | A blank line after each paragraph |
| `escape_special_chars` | `True` | Escape text that would read as Markdown syntax |
| `section_markers` | `"none"` | `"comment"` writes `<!-- section N -->` before each section |
| `cleanup` | `None` | A `CleanupOptions` to run over the output |
| `refine` | `False` | The lossless shape-refinement pass (table shape, list numbering, link/image paths, frontmatter, section anchors) |

A value the library cannot honour — an unknown `table_fallback`, a `max_heading_level`
outside 1-6 — raises `RenderError` with `kind == ErrorKind.INVALID_ARGUMENT` when the
Markdown is produced.

#### `CleanupOptions`
The cleanup pipeline: string normalization, line cleaning (page numbers, repeated
headers and footers), structural filtering, whitespace normalization.

```python
opts = unhwp.CleanupOptions.minimal()     # normalization only
opts = unhwp.CleanupOptions.default()     # every stage ("standard")
opts = unhwp.CleanupOptions.aggressive()  # every stage, headers and footers removed more eagerly
opts = unhwp.CleanupOptions.disabled()    # no cleanup

opts = unhwp.CleanupOptions(preset="aggressive")
```

### Constants

- `FORMAT_UNKNOWN` - Unknown format
- `FORMAT_HWP5` - HWP 5.0 binary format
- `FORMAT_HWPX` - HWPX XML format
- `FORMAT_HWP3` - HWP 3.x legacy format

## Platform Support

- Windows (x64)
- Linux (x64)
- macOS (x64, ARM64)

## Native library

The package ships the native library for your platform. To load another build of it — one
you compiled from the Rust crate, say — set `UNHWP_LIB_PATH` to that file. A path that names no file
is an error: importing raises `OSError` naming the path, rather than quietly loading the
packaged library instead.

## License

MIT License - see [LICENSE](../../LICENSE) for details.

## Links

- [GitHub Repository](https://github.com/iyulab/unhwp)
- [Rust Crate](https://crates.io/crates/unhwp)
- [NuGet Package](https://www.nuget.org/packages/Unhwp)
