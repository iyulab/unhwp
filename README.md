# unhwp

[![Crates.io](https://img.shields.io/crates/v/unhwp.svg)](https://crates.io/crates/unhwp)
[![Documentation](https://docs.rs/unhwp/badge.svg)](https://docs.rs/unhwp)
[![CI](https://github.com/iyulab/unhwp/actions/workflows/ci.yml/badge.svg)](https://github.com/iyulab/unhwp/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A high-performance Rust library for extracting HWP/HWPX Korean word processor documents into structured Markdown with assets.

## Features

- **Multi-format support**: HWP 5.0 (OLE) and HWPX (XML/ZIP)
- **Multiple output formats**: Markdown, Plain Text, JSON (with full metadata)
- **Structure preservation**: Headings, lists, tables, inline formatting
- **Asset extraction**: Images and binary resources
- **C-ABI FFI**: Native library for C#, Python, and other languages
- **Parallel processing**: Uses Rayon for multi-section documents
- **Async support**: Optional Tokio integration

## Installation

### Rust

```bash
cargo add unhwp
```

### CLI

```bash
cargo install unhwp
```

### Pre-built Binaries

Download from [GitHub Releases](https://github.com/iyulab/unhwp/releases):

| Platform | Architecture | File |
|----------|--------------|------|
| Windows | x64 | `unhwp-x86_64-pc-windows-msvc.zip` |
| Linux | x64 | `unhwp-x86_64-unknown-linux-gnu.tar.gz` |
| macOS | Intel | `unhwp-x86_64-apple-darwin.tar.gz` |
| macOS | Apple Silicon | `unhwp-aarch64-apple-darwin.tar.gz` |

## Quick Start

```rust
use unhwp::{parse_file, to_markdown};

fn main() -> unhwp::Result<()> {
    // Simple text extraction
    let text = unhwp::extract_text("document.hwp")?;
    println!("{}", text);

    // Convert to Markdown
    let markdown = to_markdown("document.hwp")?;
    std::fs::write("output.md", markdown)?;

    Ok(())
}
```

## Output Formats

unhwp provides four complementary output formats:

| Format | Method | Description |
|--------|--------|-------------|
| **RawContent** | `doc.raw_content()` | JSON with full metadata, styles, structure |
| **RawText** | `doc.plain_text()` | Pure text without formatting |
| **Markdown** | `to_markdown()` | Structured Markdown |
| **Images** | `doc.resources` | Extracted binary assets |

### RawContent (JSON)

Get the complete document structure with all metadata:

```rust
let doc = unhwp::parse_file("document.hwp")?;
let json = doc.raw_content();

// JSON includes:
// - metadata: title, author, created, modified
// - sections: paragraphs, tables
// - styles: bold, italic, underline, font, color
// - tables: rows, cells, colspan, rowspan
// - images, equations, links
```

## Builder API

```rust
use unhwp::{Unhwp, TableFallback};

let markdown = Unhwp::new()
    .with_images(true)
    .with_image_dir("./assets")
    .with_table_fallback(TableFallback::Html)
    .with_frontmatter()
    .lenient()  // Skip invalid sections
    .parse("document.hwp")?
    .to_markdown()?;
```

## C# / .NET Integration

unhwp provides C-ABI compatible bindings for use with P/Invoke:

```csharp
using var doc = HwpDocument.Parse("document.hwp");

// Access multiple output formats
string markdown = doc.Markdown;
string text = doc.RawText;
string json = doc.RawContent;  // Full structured JSON

// Extract images
foreach (var image in doc.Images)
{
    image.SaveTo($"./images/{image.Name}");
}
```

See [C# Integration Guide](docs/csharp-integration.md) for complete documentation.

## Supported Formats

| Format | Container | Status |
|--------|-----------|--------|
| HWP 5.0+ | OLE/CFB | ✅ Supported |
| HWPX | ZIP/XML | ✅ Supported |
| HWP 3.x | Binary | ✅ Supported (feature: `hwp3`) |

## Structure Preservation

unhwp maintains document structure during conversion:

- **Headings**: Outline levels → `#`, `##`, `###`
- **Lists**: Ordered and unordered with nesting
- **Tables**: Cell spans, alignment, HTML fallback for complex tables
- **Images**: Extracted with Markdown references
- **Inline styles**: Bold (`**`), italic (`*`), underline (`<u>`), strikethrough (`~~`)
- **Equations**: LaTeX or script format

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `hwp5` | HWP 5.0 binary format support | ✅ |
| `hwpx` | HWPX XML format support | ✅ |
| `hwp3` | Legacy HWP 3.x support (EUC-KR) | ❌ |
| `async` | Async I/O with Tokio | ❌ |

## CLI Usage

```bash
# Convert to Markdown
unhwp-cli document.hwp -o output.md

# Extract plain text
unhwp-cli document.hwp --text

# Extract with cleanup (for LLM training)
unhwp-cli document.hwp --cleanup
```

## Performance

- Parallel section processing with Rayon
- Zero-copy parsing where possible
- Memory-efficient streaming for large documents

Run benchmarks:
```bash
cargo bench
```

## License

MIT License - see [LICENSE](LICENSE) for details.
