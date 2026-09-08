# @iyulab/unhwp

WebAssembly bindings for [unhwp](https://github.com/iyulab/unhwp) — HWP/HWPX Korean document extraction.

## Install

```bash
npm install @iyulab/unhwp
```

## Usage (ES Module / browser)

> 이 패키지는 `wasm-pack --target bundler` 로 빌드된 ES 모듈입니다. WebAssembly 바이너리가
> 자동으로 초기화되므로 `await init()` 이 필요 없습니다 — 함수를 그대로 import 해 호출하세요.
> 번들러(webpack, Vite, Rollup, esbuild)를 통해 사용합니다.

```js
import { parse } from '@iyulab/unhwp';

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
import { parseWithOptions, ParseOptions } from '@iyulab/unhwp';

const opts = new ParseOptions().lenient().textOnly();
const doc = parseWithOptions(data, opts);
```

| Method | Description |
|--------|-------------|
| `lenient()` | 잘못된 섹션을 건너뛰고 파싱 계속 |
| `textOnly()` | 텍스트만 추출 (이미지 제외, 빠른 처리) |
