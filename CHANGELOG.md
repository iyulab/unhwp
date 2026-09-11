# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.11.0] - 2026-09-11

### Changed

- **Breaking:** the shared plumbing — error-kind numbering, the C-ABI helpers and the markdown
  shape-refinement pass — now comes from one crate, `unparser-shared`, instead of two
  (`uncore` and `unrefine`). `refine` and `RefineOptions` are still re-exported from this crate
  at the same paths, so code that names them through `unhwp` is unaffected. Code that named
  `unrefine::RefineOptions` directly has to switch, because `RenderOptions::refine` now holds
  `unparser_shared::refine::RefineOptions`. Error kinds, the C ABI and the bindings are unchanged.
  The dependency tree also loses its second copy of `pulldown-cmark` (0.12 next to 0.13) and
  `pulldown-cmark-to-cmark` (18 next to 22), which `unrefine` had been pulling in alongside the
  versions this crate already uses.

## [0.10.0] - 2026-09-09

### Changed

- Refreshed the declared minimum for `colored` (2 → 3), `indicatif` (0.17 → 0.18),
  `criterion` (0.5 → 0.8), `base64` (0.22 → 0.23), `pulldown-cmark` (0.12 → 0.13) and
  `pulldown-cmark-to-cmark` (18 → 22). The `indicatif` bump is the one that matters: 0.17
  pulled in `number_prefix`, which is unmaintained (RUSTSEC-2025-0119), and 0.18 does not
  depend on it. `criterion` 0.8 deprecates its `black_box` re-export in favour of
  `std::hint::black_box`, which the benchmark now uses.

- Updated `quick-xml` to 0.42, which replaces its byte-oriented event API with a string-oriented
  one (`QName`, `LocalName` and `Attribute::value` now carry `str`/`Cow<str>` instead of bytes).
  Parsing behaviour is unchanged: every reader in this crate is constructed from a `&str`, so the
  UTF-8 validation the new API performs sits on input that is already valid by construction. The
  decode-and-fall-back code the old API required around each element name and attribute was
  therefore unreachable, and has been removed rather than translated — it suggested a tolerance for
  malformed bytes that the parser has never actually had. Where invalid UTF-8 does enter, it is
  still rejected at the point it is read, as `ErrorKind::Encoding`.

- The declared minimum supported Rust version is now 1.88. It had said 1.87, which the crate
  has not actually built on for some time — a dependency in the tree raised its own floor —
  so anyone taking the manifest at its word got a compile error rather than a clear refusal.
  CI now builds the workspace on exactly the declared version, so the two cannot drift apart
  again.
  The CLI crate, published alongside the library, now declares it as well — it named no
  minimum at all, which reads as "any version" to anyone checking.

### Fixed

- The WebAssembly usage examples no longer open with `import init` and `await init()`. The npm
  package is built for bundlers, where the module initialises itself and no `init` export exists,
  so anyone following the first example stopped on its first line. The `ParseOptions` example also
  called `parseWithOptions` without importing it.
- ⚠️ **A damaged section in an HWPX package no longer disappears silently.** `ErrorMode`
  documents `Strict` — "fail immediately on any error" — as its default, and the streaming
  parser honoured it, but the batch parser (`parse_file`, `parse_bytes`, and their
  `_with_options` forms) skipped any section it could not read or parse regardless of the
  setting. A package with an unreadable section therefore parsed "successfully" into a
  document with that section's content missing, indistinguishable from a package that never
  had it. Both paths now honour the option: under the default `Strict` such a package fails,
  and under `Lenient` the readable sections are returned as before. Callers that relied on
  the previous behaviour should pass `ErrorMode::Lenient` explicitly.
- `header.xml` being *unreadable* is no longer treated as it being *absent*. The part is
  optional, so a missing one is still not an error, but every other failure — a malformed
  part included — used to be discarded along with it.
- Bytes in an XML part that are not valid UTF-8 are now reported as `ErrorKind::Encoding`
  instead of `ErrorKind::Io`. The condition was classified by where it was noticed (the
  reader raising `InvalidData`) rather than by what went wrong, so a consumer branching on
  the kind saw an I/O problem where the cause was a malformed document. This matches what
  the crate already did for the UTF-16 paths, and what `From<std::str::Utf8Error>` states.

## [0.9.1] - 2026-08-21

### Fixed

- A figure/table caption paragraph (e.g. `[표 1]`, `[Figure 2]`) carrying an explicit heading
  style is no longer promoted to a heading. Previously, under the default
  `trust_explicit_styles`, an explicit style always won even for a caption, so a caption with
  any heading style attached fragmented the document's heading structure.

## [0.9.0] - 2026-08-20

### Added

- **Markdown shape-refinement pass** (`RenderOptions.refine`, CLI `--refine`, C-ABI
  `UNHWP_FLAG_REFINE`, C# `MarkdownOptions.Refine`, Python `RenderOptions.refine`) — a
  lossless, idempotent post-processing pass ([`unrefine`](https://crates.io/crates/unrefine))
  that normalizes table shape (missing separator-row recovery, ragged-row widening),
  ordered-list numbering, link/image path separators, frontmatter formatting, and assigns
  a GitHub-compatible slug to every heading. Never deletes visible text. Off by default —
  existing output is unaffected. The streaming CLI path applies it as a whole-file pass
  after the last section is written, since its passes need whole-document scope that a
  single streamed section doesn't have.

## [0.8.1] - 2026-08-20

### Fixed

- A hyperlink or image destination containing a space is now wrapped in `<...>` —
  previously a raw space produced Markdown that is not valid CommonMark outside
  `<...>` at all, so a downstream consumer read the literal brackets and
  parentheses as text instead of a link. A destination containing a literal `<`
  or `>` is now backslash-escaped rather than percent-encoded, so the escaped
  target still resolves to the original path.

## [0.8.0] - 2026-07-31

### Added
- **Korean chapter and section markers are recognized as headings** — 제N편 / 제N부 / 제N장 /
  제N절 / 제N조 / 제N항, and Roman-numeral sections (Ⅰ. Ⅱ. Ⅲ. …). Legal and regulatory
  documents state their structure in the text itself, frequently with no heading style and no
  font change to go with it, so such a document previously came out as flat prose however
  clearly it was organised. The marker's type sets the level — 편 outranks 장 outranks 절
  outranks 조 — which is an ordering the document states in words rather than in typography.

  This ranks below the document's own heading styles, so `trust_explicit_styles` still wins
  where both are present, and above font-size inference. A consecutive run of chapters is not
  treated as a numbered list to demote. Set `HeadingConfig::detect_korean_chapters` to `false`
  for documents where such a line is ordinary body text.

- **`ErrorKind::Render` (13)** — producing output can fail, and until now that was reported
  as `Other`. `Other` means "this failure carries no classification", which is worth
  keeping true, and the sibling libraries already use this number for the same reason.

### Changed
- **A JSON serialisation failure is now reported as `Render` (13) rather than `Other` (1).**
  Serialising a rendered result is rendering, so it is attributed to rendering. A caller
  branching on `Other` for this case sees a different number; a caller reading the message
  sees no change. Existing discriminants are untouched.

### Fixed
- **Nested lists were flattened by whitespace normalization.** The final cleanup stage
  trimmed every line, and in Markdown leading whitespace is the only expression of list
  nesting — so sub-items came back out at the top level. It also undid the stage that maps
  HWP's hollow bullet to an *indented* marker: one stage of the pipeline expressed nesting
  and the next removed it. Leading whitespace is now preserved; only trailing whitespace
  and runs inside a line are normalized.

  Like the bullet fix below, this affects output only where cleanup is enabled.

- **HWP's own bullet glyphs were deleted instead of converted.** All seven of them live in
  the Private Use Area, and PUA removal ran before the bullet table was consulted — so
  under every shipped cleanup preset (`remove_pua` is set in all three) the glyph was
  discarded and the list item lost its marker entirely. The table is now consulted first:
  a PUA codepoint it names is a glyph whose meaning is known, and stripping PUA is the
  fallback for the ones that carry none. Unmapped PUA is still removed.

  This affects output only where cleanup is enabled (`RenderOptions.cleanup`, or the CLI's
  `--cleanup`); it is not applied by default.

## [0.7.0] - 2026-07-30

### Added
- **Structured error classification.** Failures now carry a machine-readable reason
  alongside their message, so consumers can branch on *why* a call failed instead of
  matching on message text.
  - Rust: `ErrorKind` (`#[repr(i32)]`) and `Error::kind()`.
  - C ABI: `unhwp_last_error_kind()` and the `UnhwpErrorKind` numbering. Written and
    cleared in lockstep with `unhwp_last_error()`, so a message is never paired with a
    stale reason.
  - C#: `UnhwpException.Kind` and `UnhwpErrorKind`.
  - Python: `UnhwpError.kind` (and its subclasses `ParseError`, `RenderError`,
    `UnsupportedFormatError`) plus `ErrorKind`, now exported from the package root.
  - The numbers are a stable ABI contract shared with the sibling `un*` extraction
    libraries: a new reason takes the next free number and existing ones are never
    reused or renumbered, so an unrecognised value can safely be treated as a generic
    failure. Unknown values pass through unchanged in every binding rather than being
    collapsed or rejected. `ErrorKind` is `#[non_exhaustive]`, so Rust callers should
    match it with a `_ =>` arm for the same reason.

### Fixed
- `unhwp_get_title`/`unhwp_get_author` no longer collapse "no title/author" and "the
  value holds an interior NUL byte" into the same silent `null` — the latter now
  reports `UNHWP_ERROR_INVALID_OUTPUT`.
- A resource lookup by an unknown ID now reports a classified failure
  (`ResourceNotFound`) instead of an unclassified message string.
- `unhwp_section_count`/`unhwp_resource_count` did not clear the last-error slot on
  entry, so a stale error `kind` from an earlier failed call could still be read
  after a later, successful call to these two functions.
- The C# bindings' `GetLastError()`/`Version` decoded the native string as ASCII;
  non-ASCII error messages and version strings are now decoded as UTF-8.

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

[Unreleased]: https://github.com/iyulab/unhwp/compare/v0.11.0...HEAD
[0.11.0]: https://github.com/iyulab/unhwp/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/iyulab/unhwp/compare/v0.9.1...v0.10.0
[0.9.1]: https://github.com/iyulab/unhwp/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/iyulab/unhwp/compare/v0.8.1...v0.9.0
[0.8.1]: https://github.com/iyulab/unhwp/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/iyulab/unhwp/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/iyulab/unhwp/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/iyulab/unhwp/compare/v0.5.3...v0.6.0
[0.5.3]: https://github.com/iyulab/unhwp/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/iyulab/unhwp/compare/v0.5.1...v0.5.2
[0.4.0]: https://github.com/iyulab/unhwp/compare/v0.1.4...v0.4.0
[0.3.0]: https://github.com/iyulab/unhwp/compare/v0.2.5...v0.3.0
[0.2.5]: https://github.com/iyulab/unhwp/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/iyulab/unhwp/compare/v0.1.3...v0.2.4
[0.1.3]: https://github.com/iyulab/unhwp/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/iyulab/unhwp/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/iyulab/unhwp/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/iyulab/unhwp/releases/tag/v0.1.0
