# unhwp

A Rust library for extracting HWP/HWPX documents into structured Markdown with assets.

## Features

- **Multi-version support**: HWP 2.x, 3.x, 5.0, and HWPX
- **Structure preservation**: Maintains document hierarchy, headings, lists, and tables
- **Asset extraction**: Images, OLE objects, and embedded files
- **Clean Markdown output**: Well-formatted, readable Markdown

## Supported Formats

| Format | Version | Status |
|--------|---------|--------|
| HWP | 2.x | 🚧 Planned |
| HWP | 3.x | 🚧 Planned |
| HWP | 5.0 | 🚧 In Progress |
| HWPX | 1.x | 🚧 In Progress |

## Structure Preservation

unhwp maintains document structure during conversion:

- **Headings**: Outline levels → `#`, `##`, `###`
- **Lists**: Bullets and numbered lists
- **Tables**: Cell spans and alignment
- **Images**: Extracted with Markdown references
- **Styles**: Bold, italic, underline, strikethrough

---

이 정도면 기본 틀이 될 것 같고, 개발 진행하면서 API가 확정되면 예제 코드 부분 업데이트하면 됩니다. 뱃지나 CI 설정 추가할까요?
