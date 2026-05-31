# @iyulab/unhwp

WebAssembly bindings for [unhwp](https://github.com/iyulab/unhwp) — HWP/HWPX Korean document extraction.

## Install

```bash
npm install @iyulab/unhwp
```

## Usage (ES Module / browser)

```js
import init, { parse } from '@iyulab/unhwp';

await init();

const response = await fetch('document.hwp');
const data = new Uint8Array(await response.arrayBuffer());
const doc = parse(data);

console.log(doc.toMarkdown());
console.log(doc.toText());
console.log(doc.sectionCount(), doc.paragraphCount());
```

## API

### `parse(data: Uint8Array): HwpDocument`

HWP 또는 HWPX 파일 바이트를 파싱합니다. 파싱 실패 시 오류를 던집니다.

### `HwpDocument`

| Method | Returns | Description |
|--------|---------|-------------|
| `toMarkdown()` | `string` | Markdown 렌더링 |
| `toText()` | `string` | 평문 텍스트 |
| `toJson()` | `string` | 구조화된 JSON |
| `sectionCount()` | `number` | 섹션 수 |
| `paragraphCount()` | `number` | 단락 수 |

### `ParseOptions`

```js
import { parse, ParseOptions } from '@iyulab/unhwp';

const opts = new ParseOptions().lenient().textOnly();
const doc = parseWithOptions(data, opts);
```

| Method | Description |
|--------|-------------|
| `lenient()` | 잘못된 섹션을 건너뛰고 파싱 계속 |
| `textOnly()` | 텍스트만 추출 (이미지 제외, 빠른 처리) |
