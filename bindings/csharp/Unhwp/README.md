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

// Simple conversion
string markdown = UnhwpConverter.ToMarkdown("document.hwp");
Console.WriteLine(markdown);

// Extract plain text
string text = UnhwpConverter.ExtractText("document.hwp");

// Full parsing with images
using var result = UnhwpConverter.Parse("document.hwp");
Console.WriteLine(result.Markdown);
Console.WriteLine($"Sections: {result.SectionCount}");
Console.WriteLine($"Paragraphs: {result.ParagraphCount}");

// Save images
foreach (var img in result.Images)
{
    img.Save($"output/{img.Name}");
}
```

## Features

- **Fast**: Native Rust library with zero-copy parsing
- **Complete**: Extracts text, tables, images, and document structure
- **Clean Output**: Optional cleanup pipeline for polished Markdown
- **Format Support**: HWP 5.0, HWPX, and HWP 3.x (legacy)

## API Reference

### UnhwpConverter (Static Class)

#### Properties

- `Version` - Gets the library version string
- `SupportedFormats` - Gets a description of supported formats

#### Methods

##### `DetectFormat(string path) -> DocumentFormat`
Detect the format of a document file.

```csharp
var format = UnhwpConverter.DetectFormat("document.hwp");
if (format == DocumentFormat.Hwp5)
    Console.WriteLine("HWP 5.0 format");
```

##### `Parse(string path, RenderOptions? options = null) -> ParseResult`
Parse a document with full access to content and images.

```csharp
using var result = UnhwpConverter.Parse("document.hwp");
Console.WriteLine(result.Markdown);
Console.WriteLine(result.Text);
foreach (var img in result.Images)
    Console.WriteLine($"{img.Name}: {img.Data.Length} bytes");
```

##### `ParseBytes(byte[] data, RenderOptions? options = null) -> ParseResult`
Parse a document from byte array.

```csharp
byte[] documentBytes = File.ReadAllBytes("document.hwp");
using var result = UnhwpConverter.ParseBytes(documentBytes);
Console.WriteLine(result.Markdown);
```

##### `ToMarkdown(string path) -> string`
Convert an HWP/HWPX document to Markdown.

```csharp
string markdown = UnhwpConverter.ToMarkdown("document.hwp");
```

##### `ToMarkdownWithCleanup(string path, CleanupOptions? options = null) -> string`
Convert with optional cleanup.

```csharp
string markdown = UnhwpConverter.ToMarkdownWithCleanup(
    "document.hwp",
    CleanupOptions.Aggressive
);
```

##### `ExtractText(string path) -> string`
Extract plain text content.

```csharp
string text = UnhwpConverter.ExtractText("document.hwp");
```

### Classes

#### `ParseResult`
Result of parsing a document. Implements `IDisposable`.

Properties:
- `Markdown` - Rendered Markdown content
- `Text` - Plain text content
- `RawContent` - Content without cleanup
- `SectionCount` - Number of sections
- `ParagraphCount` - Number of paragraphs
- `ImageCount` - Number of images
- `Images` - List of extracted images

#### `RenderOptions`
Options for Markdown rendering.

```csharp
var opts = new RenderOptions
{
    IncludeFrontmatter = true,
    ImagePathPrefix = "images/",
    TableFallback = TableFallback.Html,
    PreserveLineBreaks = false,
    EscapeSpecialChars = true
};
```

#### `CleanupOptions`
Options for output cleanup.

```csharp
// Presets
var minimal = CleanupOptions.Minimal;
var defaultOpts = CleanupOptions.Default;
var aggressive = CleanupOptions.Aggressive;
var disabled = CleanupOptions.Disabled;

// Custom
var custom = new CleanupOptions
{
    Enabled = true,
    Preset = CleanupPreset.Default,
    DetectMojibake = true,
    PreserveFrontmatter = true
};
```

#### `UnhwpImage`
Represents an extracted image.

Properties:
- `Name` - Image filename
- `Data` - Image data as byte array

Methods:
- `Save(string path)` - Save image to file

### Enums

#### `DocumentFormat`
- `Unknown` - Unknown format
- `Hwp5` - HWP 5.0 binary format
- `Hwpx` - HWPX XML format
- `Hwp3` - HWP 3.x legacy format

#### `TableFallback`
- `Markdown` - Render as Markdown tables
- `Html` - Render as HTML tables
- `Text` - Render as plain text

#### `CleanupPreset`
- `Minimal` - Minimal cleanup
- `Default` - Balanced cleanup
- `Aggressive` - Maximum cleanup

## Platform Support

- Windows (x64)
- Linux (x64)
- macOS (x64, ARM64)

## Target Frameworks

- .NET 6.0, 7.0, 8.0, 10.0
- .NET Standard 2.0, 2.1

## License

MIT License - see [LICENSE](../../../LICENSE) for details.

## Links

- [GitHub Repository](https://github.com/iyulab/unhwp)
- [Rust Crate](https://crates.io/crates/unhwp)
- [Python Package](https://pypi.org/project/unhwp)
