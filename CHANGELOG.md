# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2024-12-19

### Added

#### Core Features
- HWP 5.0 binary format parser with OLE/CFB container support
- HWPX XML format parser with ZIP container support
- Automatic format detection via magic bytes
- Unified document model (IR) for both formats

#### Document Model
- Section, Paragraph, and Block structures
- Inline content: Text, LineBreak, Image, Equation, Link, Footnote
- Style support: Bold, italic, underline, strikethrough, super/subscript
- Table model with row/colspan support
- Resource extraction (images, binary data)

#### Markdown Renderer
- ATX-style headings with configurable max level
- Ordered and unordered lists with nesting
- Table rendering with HTML fallback for merged cells
- Image references with configurable path prefix
- YAML frontmatter option
- Special character escaping

#### Performance
- Parallel section processing with Rayon
- Criterion benchmarks for parsing and rendering
- Efficient XML parsing with quick-xml
- Memory-efficient buffer handling

#### API
- `parse_file()`, `parse_reader()`, `parse_bytes()` functions
- `to_markdown()`, `extract_text()` convenience functions
- `Unhwp` builder with fluent configuration
- `RenderOptions` for customizing output
- `ParseOptions` for error handling modes

#### Async Support (feature: `async`)
- `async_api::parse_file()`, `to_markdown()` functions
- `AsyncUnhwp` builder for async workflows
- Tokio-based file I/O

### Technical Details
- HWP 5.0 record parsing (4-byte headers, extended size support)
- UTF-16LE text decoding for HWP 5.0
- Control character handling (line break, paragraph break, extended control)
- DocInfo style registry parsing
- HWPX OWPML XML namespace handling
- Style reference resolution

### Dependencies
- `cfb` for OLE container parsing
- `zip` for HWPX archive handling
- `quick-xml` for XML parsing
- `flate2` for deflate decompression
- `rayon` for parallel processing
- `tokio` (optional) for async I/O
- `thiserror` for error types
- `bytes` for buffer handling

[Unreleased]: https://github.com/iyulab/unhwp/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/iyulab/unhwp/releases/tag/v0.1.0
