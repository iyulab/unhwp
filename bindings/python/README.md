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
    print(f"Paragraphs: {result.paragraph_count}")

    # Save images
    for img in result.images:
        img.save(f"output/{img.name}")
```

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
Convert with optional cleanup.

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
Parse a document with full access to content and images.

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

#### `RenderOptions`
Options for Markdown rendering.

```python
opts = unhwp.RenderOptions(
    include_frontmatter=True,
    image_path_prefix="images/",
    preserve_line_breaks=False,
)
```

#### `CleanupOptions`
Options for output cleanup.

```python
# Presets
opts = unhwp.CleanupOptions.minimal()
opts = unhwp.CleanupOptions.default()
opts = unhwp.CleanupOptions.aggressive()
opts = unhwp.CleanupOptions.disabled()

# Custom
opts = unhwp.CleanupOptions(
    enabled=True,
    preset=1,
    detect_mojibake=True,
)
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

## License

MIT License - see [LICENSE](../../LICENSE) for details.

## Links

- [GitHub Repository](https://github.com/iyulab/unhwp)
- [Rust Crate](https://crates.io/crates/unhwp)
- [NuGet Package](https://www.nuget.org/packages/Unhwp)
