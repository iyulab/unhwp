# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**unhwp** is a Rust library for extracting HWP/HWPX Korean word processor documents into structured Markdown with assets. The project targets four format versions: HWP 2.x, 3.x, 5.0 (binary), and HWPX (XML-based).

## Quality & Performance Goals

This library aims for **industry-leading quality and performance**:

- **Correctness**: 100% fidelity in text extraction; zero data loss
- **Structure Preservation**: Maintain document hierarchy, not just flat text
- **Performance**: Process 100MB+ documents efficiently; streaming where possible
- **Robustness**: Graceful degradation on malformed/partial files; never panic
- **Zero-copy**: Minimize allocations; use borrowed data where feasible
- **Async-ready**: Design for optional async I/O integration

## Build Commands

```bash
cargo build                    # Build library
cargo build --release          # Release build
cargo test                     # Run all tests
cargo test <test_name>         # Run specific test
cargo clippy                   # Lint
cargo fmt                      # Format code
cargo doc --open               # Generate and view documentation
```

## Architecture

### Format Detection Strategy

Files are identified by magic bytes:
- **HWP 5.x**: OLE container starting with `D0 CF 11 E0 A1 B1 1A E1`
- **HWPX**: ZIP archive (standard ZIP magic bytes)
- **HWP 3.x**: ASCII signature `HWP Document File V`

### HWP 5.0 Binary Format

Uses OLE Compound File Binary (CFB) container with these critical streams:
- `FileHeader`: 256-byte header with version, compression flags (bit 0), encryption flags (bit 1)
- `DocInfo`: Fonts, styles, paragraph/character shapes (usually zlib compressed)
- `BodyText/Section0...N`: Document content (usually zlib compressed)
- `BinData/`: Embedded images and OLE objects

**Record structure**: 4-byte headers with Tag ID (bits 0-9), Level (bits 10-19), Size (bits 20-31). Extended size uses `0xFFF` sentinel with following DWORD for actual size.

**Text encoding**: UTF-16LE with control characters 0x00-0x1F. Key codes: 0x0B (extended control for tables/images), 0x0D (paragraph break).

### HWPX XML Format

ZIP archive following OWPML (KS X 6101) standard:
- `Contents/content.hpf`: Package manifest
- `Contents/section*.xml`: Body content with `hp:` namespace
- `Contents/styles.xml`: Style definitions
- `BinData/`: Binary resources

Text extraction: Parse `<hp:p>` paragraphs, `<hp:run>` formatting runs, `<hp:t>` text content.

### Key Dependencies (Planned)

| Crate | Purpose |
|-------|---------|
| `cfb` | OLE container parsing |
| `flate2` | Raw deflate decompression (no zlib headers) |
| `encoding_rs` | EUC-KR/CP949 for HWP 3.x |
| `zip` | HWPX archive extraction |
| `quick-xml` or `roxmltree` | HWPX XML parsing |

### Markdown Conversion Strategy

- **Headings**: Map outline styles to `#`, `##`, `###`
- **Inline styles**: Bold (`**`), italic (`*`), strikethrough (`~~`)
- **Tables**: Simple grids use pipe syntax; merged cells fall back to HTML
- **Images**: Extract to assets folder, reference with `![alt](path)`
- **Equations**: HWP uses EQEdit script; may require LaTeX conversion

## Version Bump Checklist

When bumping version, **ALL** of the following files must be updated simultaneously:

```
Cargo.toml                           # Root library version
cli/Cargo.toml                       # CLI version (must match)
bindings/python/pyproject.toml       # Python package version
bindings/csharp/Unhwp/Unhwp.csproj   # C# package version
```

**Important**: CLI version mismatch causes "update available" message to appear even after updating. All versions must be in sync before creating a GitHub release tag.

## Key Implementation Notes

- Compression: Use raw deflate mode (`flate2` with `Decompress::new(false)`)
- Extended records: Handle size field `0xFFF` as sentinel for 4-byte extended size
- Control chars in text: Skip 8-WCHAR (16 bytes) regions after extended control (0x0B)
- Reference resolution: Both formats use ID references to style definitions; build style map before parsing body
