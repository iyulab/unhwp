# Unhwp

High-performance .NET library for extracting HWP/HWPX Korean word processor documents to Markdown.

## Installation

```bash
dotnet add package Unhwp
```

Or via NuGet Package Manager:
```
Install-Package Unhwp
```

## Quick Start

```csharp
using Unhwp;

// Parse a document
using var doc = UnhwpDocument.ParseFile("document.hwp");

// Convert to Markdown
string markdown = doc.ToMarkdown();
Console.WriteLine(markdown);

// Plain text and JSON
string text = doc.ToText();
string json = doc.ToJson();

Console.WriteLine($"Sections: {doc.SectionCount}");
```

### With Markdown Options

```csharp
using var doc = UnhwpDocument.ParseFile("document.hwpx");

var markdown = doc.ToMarkdown(new MarkdownOptions
{
    IncludeFrontmatter = true,
    ImagePathPrefix = "images/",
    TableFallback = TableFallback.Html,
    Cleanup = CleanupPreset.Standard,
});
```

### Parse from Bytes

```csharp
byte[] data = File.ReadAllBytes("document.hwp");
using var doc = UnhwpDocument.ParseBytes(data);
```

### Tables as CSV

```csharp
using var doc = UnhwpDocument.ParseFile("document.hwp");

foreach (var table in doc.GetTables())
{
    File.WriteAllText($"s{table.Section}-t{table.Index}.csv", table.Text);
}
```

### Extract Images

```csharp
using var doc = UnhwpDocument.ParseFile("document.hwp");

foreach (var id in doc.GetResourceIds())
{
    var data = doc.GetResourceData(id);
    if (data != null)
        File.WriteAllBytes(Path.Combine("output", id), data);
}
```

### Handling Failures

`UnhwpException.Kind` says *why* a call failed, so you can react to the reason instead of
matching on message text:

```csharp
try
{
    using var doc = UnhwpDocument.ParseFile(path);
}
catch (UnhwpException ex)
{
    switch (ex.Kind)
    {
        case UnhwpErrorKind.OleContainer:
        case UnhwpErrorKind.ZipArchive:
            Console.Error.WriteLine("The file is damaged.");
            break;
        case UnhwpErrorKind.UnknownFormat:
        case UnhwpErrorKind.UnsupportedFormat:
            Console.Error.WriteLine("Not a supported HWP/HWPX document.");
            break;
        default:
            // Also the right branch for a reason this build has no name for.
            Console.Error.WriteLine($"Extraction failed ({ex.Kind}): {ex.Message}");
            break;
    }
}
```

The numbers behind `UnhwpErrorKind` are a stable ABI contract: a new reason takes the next
free number and existing ones are never renumbered. Always keep a `default` branch so an
unrecognised value degrades to a generic failure rather than going unhandled. `Kind` is
`Other` for failures raised by the wrapper itself, and never `None` (which means success).

## Features

- **Fast**: Native Rust library
- **Complete**: Extracts text, tables, images, and document structure
- **Format Support**: HWP 5.0, HWPX, and HWP 3.x (legacy)
- **Trim and Native AOT compatible**: no reflection-based serialization

## API Reference

### UnhwpDocument Class

Implements `IDisposable`; dispose it to release the native document.

#### Static Members

- `ParseFile(string path)` - Parse a document from a file path
- `ParseBytes(byte[] data)` - Parse a document from bytes
- `Version` - Library version

#### Instance Methods

- `ToMarkdown(MarkdownOptions? options = null)` - Convert to Markdown
- `ToText()` - Convert to plain text
- `ToJson(bool compact = false)` - Convert to JSON
- `PlainText()` - Get plain text (fast extraction)
- `GetTables(bool tsv = false)` - Every table as CSV (RFC 4180), or tab-separated, in reading order (`IReadOnlyList<TableText>`): `Section`, `Index` (its place in the section, from 1) and `Text`. A merged cell's text is in its top-left position and the positions it covers are empty, so every record has the same number of fields.
- `GetResourceIds()` - List of resource IDs
- `GetResourceInfo(string id)` - Resource metadata as `JsonDocument`, or null when absent
- `GetResourceData(string id)` - Resource binary data, or null when absent

#### Properties

- `Title` - Document title, or null
- `Author` - Document author, or null
- `SectionCount` - Number of sections
- `ResourceCount` - Number of resources

### MarkdownOptions Class

Every property reaches the native library, and the defaults are the library's own.

| Property | Default | Meaning |
|---|---|---|
| `ImagePathPrefix` | `"assets/"` | Prefix of the image paths in the Markdown |
| `TableFallback` | `SimplifiedMarkdown` | A table with merged cells: `SimplifiedMarkdown` (merges dropped), `Html` (rowspan/colspan kept) or `Skip` |
| `MaxHeadingLevel` | `4` | Deepest heading level written (1-6) |
| `IncludeFrontmatter` | `false` | Document metadata as YAML frontmatter |
| `PreserveLineBreaks` | `true` | Line breaks inside a paragraph as Markdown hard breaks; `false` joins the lines |
| `IncludeEmptyParagraphs` | `false` | Keep empty paragraphs as blank lines |
| `ListMarker` | `'-'` | Marker of an unordered list item |
| `ParagraphSpacing` | `true` | A blank line after each paragraph |
| `EscapeSpecialChars` | `true` | Escape text that would read as Markdown syntax |
| `SectionMarkers` | `None` | `Comment` writes `<!-- section N -->` before each section |
| `Cleanup` | `null` | A `CleanupPreset` to run over the output: `Minimal` (normalization only), `Standard` (every stage) or `Aggressive` (headers and footers removed more eagerly) |
| `Refine` | `false` | The lossless shape-refinement pass (table shape, list numbering, link/image paths, frontmatter, section anchors) |

A value the library cannot honour, such as a `MaxHeadingLevel` outside 1-6, makes
`ToMarkdown` throw `UnhwpException` with `Kind == UnhwpErrorKind.InvalidArgument`.

## Platform Support

- Windows (x64)
- Linux (x64, glibc and musl)
- macOS (x64, ARM64)

## Target Frameworks

- .NET 10.0

## License

MIT License - see [LICENSE](https://github.com/iyulab/unhwp/blob/main/LICENSE) for details.

## Links

- [GitHub Repository](https://github.com/iyulab/unhwp)
- [Rust Crate](https://crates.io/crates/unhwp)
- [Python Package](https://pypi.org/project/unhwp)
