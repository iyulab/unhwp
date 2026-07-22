# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0] - 2026-07-22

### Added

- **HWP 5.0 equation extraction** — equations authored with the equation
  editor (EQEdit) are now extracted from HWP 5.0 binary documents and
  rendered as inline LaTeX (`$...$`), matching the existing HWPX behavior.
  Previously they were **silently dropped**, producing markdown that looked
  complete but was missing values (reported from legal/technical documents
  where thresholds are typeset as equations). The inline `0x0B` "eqed"
  control now reserves a position slot and the `HWPTAG_EQ_EDIT` record fills
  in the script — in body paragraphs and table cells alike.
- Equation scripts that cannot be extracted render a visible
  `[unhwp:equation-unsupported]` placeholder instead of disappearing.
- `Paragraph::plain_text()` now includes equation scripts, so the plain-text
  surface (`unhwp text`) no longer loses equation content either.
- **WASM `toImages()`** — `HwpDocument.toImages()` returns the document's
  embedded images as an array of `HwpImage` objects (`id`, `filename`,
  `mimeType`, `bytes` as `Uint8Array`, `base64` for `data:` URLs), sorted by
  resource ID.

### Fixed

- Equation script → LaTeX converter fidelity, validated against a real
  equation-heavy HWP file:
  - `DIVIDE` → `\div`, `LEFT`/`RIGHT` → `\left`/`\right`, `+-`/`-+` →
    `\pm`/`\mp`, backtick small-space → `\,`.
  - `bar` is now the overline accent (`\bar{}`) per the Hancom equation
    spec — previously mis-mapped to a vertical line (that is `vert`).
  - `a-{1} over {2}` no longer pulls preceding operators into the fraction
    numerator, and single-group numerators no longer double their braces
    (`\frac{{4b}}{a}` → `\frac{4b}{a}`).
  - `rm`/`it` font toggles are consumed (content kept) instead of leaking
    into the output.
- Lines starting with enclosed enumeration markers (①, ⓵, ⑴, ㉠, ❶ …) are
  never promoted to headings, even when the source document assigns them an
  outline style (multiple-choice/clause lines rendered as `## …`).

### Removed

- Orphan `src/hwp5/control.rs` stub module (dead code superseded by the
  actual implementations in `bodytext.rs`).

## [0.5.3] - 2026-07-07

### Security

- Bump `crossbeam-epoch` to 0.9.20 (**RUSTSEC-2026-0204**).

### Fixed

- Publish the npm package under its scoped name `@iyulab/unhwp`.

## [0.5.2] - 2026-07-05

### Security

- Bump `quick-xml` from 0.37 to 0.41, remediating **RUSTSEC-2026-0194**
  (quadratic runtime on duplicate attribute-name checks) and
  **RUSTSEC-2026-0195** (unbounded namespace-declaration allocation), both DoS
  advisories affecting consumers that parse untrusted HWPX documents.
  ([#5](https://github.com/iyulab/unhwp/issues/5))
- Bump transitive `bytes`, `tar`, and `time` to their patched releases.
- Add a `cargo audit` job to CI so future advisories are caught automatically.

### Fixed

- Preserve XML entity references (`&amp;`, `&lt;`, `&#NN;`, `&#xNN;`) in HWPX
  body text, equations, footnotes, and metadata. quick-xml 0.40+ emits entities
  as separate `GeneralRef` events; the text-accumulation loops now fold them
  back in, and metadata parsing accumulates per element (dispatching on the
  closing tag) so values such as a title containing `&amp;` are no longer
  truncated to their last fragment.

## [0.4.0] - 2026-05-31

### Added

#### WASM Support
- `unhwp-wasm` crate — WebAssembly bindings via `wasm-bindgen`
- `parse(data: &[u8])` — parse HWP/HWPX from bytes (WASM entry point)
- `parseWithOptions(data, opts)` — parse with options (lenient, textOnly)
- `HwpDocument` — WASM struct with `toMarkdown()`, `toText()`, `toJson()`, `sectionCount()`, `paragraphCount()`
- `ParseOptions` — WASM struct with `lenient()`, `textOnly()` builder methods
- npm package `@iyulab/unhwp` — ES Module for browser and bundler targets

#### GitHub Pages Playground
- `docs/index.html` — single-file SPA: drag-and-drop HWP/HWPX → Markdown/text conversion
- Automatic deployment via `pages.yml` GitHub Actions workflow

#### CI/CD
- `build-wasm` job in `ci.yml` — validates WASM compilation + wasm-pack tests on every push
- `pages.yml` — GitHub Pages deployment on main branch changes
- `publish-npm` job in `release.yml` — publishes `@iyulab/unhwp` to npm on release

### Changed
- `rayon` moved to native-only dependency (not available under `wasm32-unknown-unknown`)
- `zip` uses deflate-only feature set under `wasm32` (bzip2/lzma/xz require C libraries)
- `parse_file`, `extract_text`, `to_markdown`, `to_markdown_with_options`, `parse_file_streaming`, `detect_format_from_path` gated as `#[cfg(not(target_arch = "wasm32"))]`

## [0.3.0] - 2026-05-09

### Added

#### Streaming API
- `parse_file_streaming()` — processes large documents section-by-section with bounded memory
- `ParseEvent` enum: `DocumentStart`, `SectionParsed`, `SectionFailed`, `DocumentEnd`, `ResourceExtracted`
- `SectionStreamOptions` — configure error mode and resource extraction for streaming

#### Section Boundary Markers
- `SectionMarkerStyle` enum (`None`, `Comment`) — insert `<!-- section N -->` before each section
- `RenderOptions::with_section_markers()` builder method
- CLI `--section-markers` flag on `convert` subcommand

#### CLI Improvements
- `--formats <md,txt,json>` — select output formats (default: `md` only)
- `--all` — shorthand for `--formats md,txt,json`
- `--no-images` — skip binary resource extraction
- `--quiet` / `-q` — suppress progress output

### Changed
- `convert` default output is now **Markdown only** (`extract.md` + `images/`); use `--all` or `--formats` for additional formats
- `--cleanup` without a preset argument defaults to `standard`
- `cmd_convert` rewired to streaming pipeline — sections are processed and written one at a time; peak memory no longer scales with document size

### Fixed
- CLI path sanitization in resource extraction (prevent path traversal)
- Removed silent-drop warning on render result
- `render_section_standalone` no longer clones the section (performance)
- Dead `extract_mode` field removed from streaming options

## [0.2.5] - 2026-04-xx

### Changed
- CI: opt all JS Actions into Node.js 24 ahead of GitHub's forced migration

## [0.2.4] - 2026-04-14

### Fixed
- `unhwp update` failed on Windows with "Compression method not supported" because
  `self_update`'s `archive-zip` feature only handles stored-only zips. PowerShell's
  `Compress-Archive` (used by the release workflow) emits Deflate. Added the
  `compression-zip-deflate` feature so self-updating binaries can extract the
  downloaded archive. Users on 0.2.3 or earlier must install 0.2.4 manually once.

### Added
- CI `version-check` job fails fast when the four canonical version files
  (root `Cargo.toml`, `cli/Cargo.toml` + its `unhwp` dep, `pyproject.toml`,
  `Unhwp.csproj`) drift out of sync, preventing partial releases.
- Release workflow `cleanup-old-releases` job keeps the 10 newest GitHub
  Releases and deletes the rest with `--cleanup-tag`, honoring the global
  GitHub Actions resource-management policy.

## [0.1.3] - 2024-12-19

### Fixed
- Fixed crates.io publish workflow with `--allow-dirty` flag

## [0.1.2] - 2024-12-19

### Fixed
- Fixed all Clippy warnings for cleaner code
  - Replaced manual range loops with iterators
  - Used `strip_prefix()` instead of manual string slicing
  - Replaced `map_or()` with `is_some_and()` for cleaner boolean checks
  - Changed `push_str()` to `push()` for single character appends
  - Used `derive(Default)` with `#[default]` attribute for enum defaults
  - Replaced manual `% 2 != 0` checks with `is_multiple_of(2)`
- Fixed FFI safety by wrapping unsafe function calls in `unsafe` blocks

### Changed
- Added `#![allow(clippy::not_unsafe_ptr_arg_deref)]` for FFI module (intentional raw pointer handling)
- CI workflow now uses `dtolnay/rust-toolchain` action correctly

## [0.1.1] - 2024-12-19

### Added

#### RawContent JSON API
- `Document::raw_content()` method returning JSON with full document structure
- `unhwp_result_get_raw_content()` FFI function for C#/Python integration
- Complete metadata, styles, and formatting in JSON output

#### Markdown Renderer Improvements
- Underline support (`<u>text</u>`)

#### C# Integration
- `HwpDocument.RawContent` property for accessing structured JSON
- Updated documentation with JSON parsing examples

#### CI/CD
- GitHub Actions workflow for CI (test, lint)
- Automated release on Cargo.toml version change
- Multi-platform binary builds (Windows, Linux, macOS Intel/ARM)
- Automatic publishing to crates.io and GitHub Releases

### Changed
- Renamed CLI binary from `unhwp` to `unhwp-cli` to avoid name collision with library

### Fixed
- Resolved unused code warnings with `#[allow(dead_code)]` for reserved code
- Fixed unused imports in hwpx and hwp5 modules

#### HWP 3.x Legacy Support (feature: `hwp3`)
- HWP 3.x binary format parser with 128-byte header parsing
- EUC-KR/CP949 text encoding support via `encoding_rs`
- Version detection from signature string (V3.0, V3.1, etc.)
- Compressed body support with zlib decompression
- Control code handling (bold, italic, underline, line break)
- Body text parsing with Korean character handling

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

[Unreleased]: https://github.com/iyulab/unhwp/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/iyulab/unhwp/compare/v0.2.5...v0.3.0
[0.2.5]: https://github.com/iyulab/unhwp/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/iyulab/unhwp/compare/v0.1.3...v0.2.4
[0.1.3]: https://github.com/iyulab/unhwp/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/iyulab/unhwp/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/iyulab/unhwp/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/iyulab/unhwp/releases/tag/v0.1.0
