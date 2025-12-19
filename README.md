# unhwp

[![Crates.io](https://img.shields.io/crates/v/unhwp.svg)](https://crates.io/crates/unhwp)
[![Documentation](https://docs.rs/unhwp/badge.svg)](https://docs.rs/unhwp)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A high-performance Rust library for extracting HWP/HWPX Korean word processor documents into structured Markdown with assets.

## Features

- **Multi-format support**: HWP 5.0 (OLE) and HWPX (XML/ZIP)
- **Structure preservation**: Headings, lists, tables, inline formatting
- **Asset extraction**: Images and binary resources
- **Parallel processing**: Uses Rayon for multi-section documents
- **Async support**: Optional Tokio integration
- **Builder API**: Fluent configuration interface

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

## Async Support

Enable the `async` feature for non-blocking operations:

```toml
[dependencies]
unhwp = { version = "0.1", features = ["async"] }
```

```rust
use unhwp::async_api::{parse_file, to_markdown, AsyncUnhwp};

#[tokio::main]
async fn main() -> unhwp::Result<()> {
    // Simple async parsing
    let document = parse_file("document.hwp").await?;

    // Async builder
    let markdown = AsyncUnhwp::new()
        .with_frontmatter()
        .parse("document.hwp")
        .await?
        .to_markdown()
        .await?;

    Ok(())
}
```

## Supported Formats

| Format | Container | Status |
|--------|-----------|--------|
| HWP 5.0+ | OLE/CFB | ✅ Supported |
| HWPX | ZIP/XML | ✅ Supported |
| HWP 3.x | Binary | 🚧 Planned |

## Structure Preservation

unhwp maintains document structure during conversion:

- **Headings**: Outline levels → `#`, `##`, `###`
- **Lists**: Ordered and unordered with nesting
- **Tables**: Cell spans, alignment, HTML fallback for complex tables
- **Images**: Extracted with Markdown references
- **Inline styles**: Bold (`**`), italic (`*`), strikethrough (`~~`)
- **Equations**: LaTeX or script format

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `hwp5` | HWP 5.0 binary format support | ✅ |
| `hwpx` | HWPX XML format support | ✅ |
| `hwp3` | Legacy HWP 3.x support | ❌ |
| `async` | Async I/O with Tokio | ❌ |

## Performance

- Parallel section processing with Rayon
- Zero-copy parsing where possible
- Memory-efficient streaming for large documents

Run benchmarks:
```bash
cargo bench
```

## API Documentation

```rust
// Format detection
let format = unhwp::detect_format_from_path("document.hwp")?;

// Full document model access
let document = unhwp::parse_file("document.hwp")?;
println!("Sections: {}", document.sections.len());
println!("Paragraphs: {}", document.paragraph_count());

// Custom render options
use unhwp::RenderOptions;
let options = RenderOptions::default()
    .with_image_dir("./images")
    .with_frontmatter();
let markdown = unhwp::render::render_markdown(&document, &options)?;
```

## License

MIT License - see [LICENSE](LICENSE) for details.
