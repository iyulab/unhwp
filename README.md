# unhwp

[![Crates.io](https://img.shields.io/crates/v/unhwp.svg)](https://crates.io/crates/unhwp)
[![PyPI](https://img.shields.io/pypi/v/unhwp.svg)](https://pypi.org/project/unhwp/)
[![NuGet](https://img.shields.io/nuget/v/Unhwp.svg)](https://www.nuget.org/packages/Unhwp/)
[![Documentation](https://docs.rs/unhwp/badge.svg)](https://docs.rs/unhwp)
[![CI](https://github.com/iyulab/unhwp/actions/workflows/ci.yml/badge.svg)](https://github.com/iyulab/unhwp/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A high-performance Rust library for extracting HWP/HWPX Korean word processor documents into structured Markdown with assets.

## Features

- **Multi-format support**: HWP 5.0 (OLE) and HWPX (XML/ZIP)
- **Multiple output formats**: Markdown, Plain Text, JSON (with full metadata)
- **Structure preservation**: Headings, lists, tables, inline formatting
- **Asset extraction**: Images and binary resources
- **Streaming API**: Memory-efficient section-by-section processing for large documents
- **Section markers**: Optional `<!-- section N -->` boundary markers in output
- **Self-update**: Built-in update mechanism via GitHub releases
- **C-ABI FFI**: Native library for C#, Python, and other languages
- **Parallel processing**: HWPX sections parsed in parallel via Rayon
- **Async support**: Optional Tokio integration

---

## Table of Contents

- [Installation](#installation)
  - [Pre-built Binaries (Recommended)](#pre-built-binaries-recommended)
  - [Updating](#updating)
  - [Install via Cargo](#install-via-cargo)
- [CLI Usage](#cli-usage)
- [Rust Library Usage](#rust-library-usage)
  - [Quick Start](#quick-start)
  - [Streaming API](#streaming-api)
  - [Builder API](#builder-api)
- [C# / .NET Integration](#c--net-integration)
- [Output Formats](#output-formats)
- [Feature Flags](#feature-flags)
- [License](#license)

---

## Installation

### Pre-built Binaries (Recommended)

Download the latest release from [GitHub Releases](https://github.com/iyulab/unhwp/releases/latest).

#### Windows (x64)

```powershell
# Download and extract
Invoke-WebRequest -Uri "https://github.com/iyulab/unhwp/releases/latest/download/unhwp-cli-x86_64-pc-windows-msvc.zip" -OutFile "unhwp.zip"
Expand-Archive -Path "unhwp.zip" -DestinationPath "."

# Move to a directory in PATH (optional)
Move-Item -Path "unhwp.exe" -Destination "$env:LOCALAPPDATA\Microsoft\WindowsApps\"

# Verify installation
unhwp --version
```

Or manually:
1. Download `unhwp-cli-x86_64-pc-windows-msvc.zip`
2. Extract `unhwp.exe`
3. Add to PATH or run directly

#### Linux (x64)

```bash
# Download and extract
curl -LO https://github.com/iyulab/unhwp/releases/latest/download/unhwp-cli-x86_64-unknown-linux-gnu.tar.gz
tar -xzf unhwp-cli-x86_64-unknown-linux-gnu.tar.gz

# Install to /usr/local/bin (requires sudo)
sudo mv unhwp /usr/local/bin/

# Or install to user directory
mkdir -p ~/.local/bin
mv unhwp ~/.local/bin/
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc

# Verify installation
unhwp --version
```

#### macOS

```bash
# Intel Mac
curl -LO https://github.com/iyulab/unhwp/releases/latest/download/unhwp-cli-x86_64-apple-darwin.tar.gz
tar -xzf unhwp-cli-x86_64-apple-darwin.tar.gz

# Apple Silicon (M1/M2/M3)
curl -LO https://github.com/iyulab/unhwp/releases/latest/download/unhwp-cli-aarch64-apple-darwin.tar.gz
tar -xzf unhwp-cli-aarch64-apple-darwin.tar.gz

# Install
sudo mv unhwp /usr/local/bin/

# Verify
unhwp --version
```

#### Available Binaries

| Platform | Architecture | File |
|----------|--------------|------|
| Windows | x64 | `unhwp-cli-x86_64-pc-windows-msvc.zip` |
| Linux | x64 | `unhwp-cli-x86_64-unknown-linux-gnu.tar.gz` |
| macOS | Intel | `unhwp-cli-x86_64-apple-darwin.tar.gz` |
| macOS | Apple Silicon | `unhwp-cli-aarch64-apple-darwin.tar.gz` |

### Updating

unhwp includes a built-in self-update mechanism:

```bash
# Check for updates
unhwp update --check

# Update to latest version
unhwp update

# Force reinstall (even if on latest)
unhwp update --force
```

The update command automatically:
- Detects your platform
- Downloads the appropriate binary from GitHub Releases
- Replaces the current executable
- Preserves your settings

### Install via Cargo

If you have Rust installed:

```bash
# Install CLI
cargo install unhwp-cli

# Add library to your project
cargo add unhwp
```

---

## CLI Usage

### Basic Conversion

```bash
# Convert HWP/HWPX to Markdown (creates <filename>_output/ directory)
unhwp document.hwp

# Specify output directory
unhwp document.hwp ./output

# Using subcommand
unhwp convert document.hwp -o ./output
```

### Output Structure

By default, only Markdown is produced. Use `--formats` or `--all` to add more formats:

```bash
# Default: Markdown only
unhwp document.hwp
# → document_output/extract.md
# → document_output/images/

# All formats
unhwp convert document.hwp --all
# → document_output/extract.md
# → document_output/extract.txt
# → document_output/content.json
# → document_output/images/

# Specific formats
unhwp convert document.hwp --formats md,txt
```

### Cleanup Options (for LLM Training Data)

```bash
# Standard cleanup (default when --cleanup is specified without a value)
unhwp document.hwp --cleanup

# Preset selection
unhwp document.hwp --cleanup minimal      # Essential normalization only
unhwp document.hwp --cleanup standard     # Balanced (default)
unhwp document.hwp --cleanup aggressive   # Maximum purification
unhwp document.hwp --cleanup none         # Disable cleanup
```

### Section Markers

Insert `<!-- section N -->` boundaries to identify document sections in output:

```bash
unhwp convert document.hwp --section-markers
```

Output:

```markdown
<!-- section 0 -->

# Introduction

Lorem ipsum...

<!-- section 1 -->

## Appendix
```

### Commands

```bash
unhwp --help                              # Show help
unhwp --version                           # Show version
unhwp version                             # Show detailed version info
unhwp update --check                      # Check for updates
unhwp update                              # Self-update to latest version
unhwp convert FILE [OPTIONS]              # Convert to directory (default command)
unhwp md FILE [-o OUTPUT]                 # Convert to Markdown (stdout or file)
unhwp text FILE [-o OUTPUT]               # Extract plain text
unhwp json FILE [-o OUTPUT]               # Convert to JSON
unhwp info FILE                           # Show document metadata
unhwp extract FILE [-o DIR]               # Extract binary resources only
```

### Examples

```bash
# Basic conversion
unhwp report.hwp

# All formats + cleanup for AI training
unhwp convert report.hwp --all --cleanup aggressive

# Section-aware Markdown for downstream parsing
unhwp convert report.hwp --section-markers

# Skip image extraction (faster)
unhwp convert report.hwp --no-images

# Quiet batch conversion (shell)
for f in *.hwp; do unhwp "$f" -q; done

# Quiet batch conversion (PowerShell)
Get-ChildItem *.hwp | ForEach-Object { unhwp $_.FullName -q }
```

---

## Rust Library Usage

### Quick Start

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

### Streaming API

For large documents, process section-by-section without loading the entire document:

```rust
use std::ops::ControlFlow;
use unhwp::{parse_file_streaming, ParseEvent, SectionStreamOptions};

fn main() -> unhwp::Result<()> {
    parse_file_streaming(
        "large.hwp",
        SectionStreamOptions::default(),
        |event| {
            match event {
                ParseEvent::DocumentStart { metadata, section_count, .. } => {
                    println!("Title: {:?}, sections: {}", metadata.title, section_count);
                }
                ParseEvent::SectionParsed(section) => {
                    println!("Section {}: {} blocks", section.index, section.content.len());
                    // section memory is freed after this callback returns
                }
                ParseEvent::SectionFailed { index, error } => {
                    eprintln!("Section {} failed: {}", index, error);
                }
                ParseEvent::DocumentEnd => {}
                ParseEvent::ResourceExtracted { name, data } => {
                    std::fs::write(format!("images/{}", name), data).ok();
                }
            }
            ControlFlow::Continue(())
        },
    )?;
    Ok(())
}
```

Event order is always: `DocumentStart → (SectionParsed | SectionFailed)* → DocumentEnd → ResourceExtracted*`

### Section Markers

```rust
use unhwp::{to_markdown_with_options, RenderOptions, SectionMarkerStyle};

let options = RenderOptions::default()
    .with_section_markers(SectionMarkerStyle::Comment);

let markdown = to_markdown_with_options("document.hwp", &options)?;
// Each section preceded by <!-- section N -->
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

### Builder API

```rust
use unhwp::{Unhwp, TableFallback};

let markdown = Unhwp::new()
    .with_images(true)
    .with_image_dir("./assets")
    .with_table_fallback(TableFallback::Html)
    .with_frontmatter()
    .lenient()  // Continue past invalid sections
    .parse("document.hwp")?
    .to_markdown()?;
```

### RenderOptions

```rust
use unhwp::{to_markdown_with_options, RenderOptions, SectionMarkerStyle, TableFallback};

let options = RenderOptions::default()
    .with_frontmatter()
    .with_table_fallback(TableFallback::Html)
    .with_max_heading_level(3)
    .with_image_dir("./images")
    .with_image_prefix("images/")
    .with_cleanup()                                      // Standard cleanup
    .with_section_markers(SectionMarkerStyle::Comment);  // <!-- section N -->

let markdown = to_markdown_with_options("document.hwp", &options)?;
```

## C# / .NET Integration

unhwp provides C-ABI compatible bindings for seamless integration with C# and .NET applications.

### Getting the Native Library

Build from source or download from [GitHub Releases](https://github.com/iyulab/unhwp/releases):

| Platform | Library File |
|----------|-------------|
| Windows x64 | `unhwp.dll` |
| Linux x64 | `libunhwp.so` |
| macOS | `libunhwp.dylib` |

```bash
# Build native library from source
cargo build --release
# Output: target/release/unhwp.dll (Windows)
#         target/release/libunhwp.so (Linux)
#         target/release/libunhwp.dylib (macOS)
```

### Quick Start

```csharp
using Unhwp;

// Parse document once, access multiple outputs
using var doc = HwpDocument.Parse("document.hwp");

// Get Markdown
string markdown = doc.Markdown;
File.WriteAllText("output.md", markdown);

// Get plain text
string text = doc.RawText;

// Get full structured JSON (metadata, styles, formatting)
string json = doc.RawContent;

// Extract all images
foreach (var image in doc.Images)
{
    image.SaveTo($"./images/{image.Name}");
    Console.WriteLine($"Saved: {image.Name} ({image.Size} bytes)");
}

// Document statistics
Console.WriteLine($"Sections: {doc.SectionCount}");
Console.WriteLine($"Paragraphs: {doc.ParagraphCount}");
```

### With Cleanup Options

```csharp
var options = new ConversionOptions
{
    EnableCleanup = true,
    CleanupPreset = CleanupPreset.Aggressive,  // For LLM training
    IncludeFrontmatter = true,
    TableFallback = TableFallback.Html
};

using var doc = HwpDocument.Parse("document.hwp", options);
File.WriteAllText("cleaned.md", doc.Markdown);
```

### Static Methods (Simple API)

```csharp
// One-liner conversion
string markdown = HwpConverter.ToMarkdown("document.hwp");

// With cleanup
string cleanedMarkdown = HwpConverter.ToMarkdown("document.hwp", enableCleanup: true);

// Plain text extraction
string text = HwpConverter.ExtractText("document.hwp");

// From byte array or stream
byte[] data = File.ReadAllBytes("document.hwp");
string md = HwpConverter.BytesToMarkdown(data);
```

### ASP.NET Core Example

```csharp
[ApiController]
[Route("api/[controller]")]
public class DocumentController : ControllerBase
{
    [HttpPost("convert")]
    public async Task<IActionResult> ConvertHwp(IFormFile file)
    {
        if (file == null) return BadRequest("No file");

        using var ms = new MemoryStream();
        await file.CopyToAsync(ms);

        try
        {
            var markdown = HwpConverter.BytesToMarkdown(ms.ToArray(), enableCleanup: true);
            return Ok(new { markdown });
        }
        catch (HwpException ex)
        {
            return BadRequest(new { error = ex.Message });
        }
    }
}
```

See [C# Integration Guide](docs/csharp-integration.md) for complete documentation including:
- Full P/Invoke wrapper code
- NuGet package setup
- Error handling
- Memory management
- Thread safety

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
