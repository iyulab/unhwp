use unhwp::render::RenderOptions;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct HwpDocument {
    #[allow(dead_code)]
    pub(crate) inner: unhwp::Document,
}

#[wasm_bindgen]
impl HwpDocument {
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(data: &[u8]) -> Result<HwpDocument, JsValue> {
        unhwp::parse_bytes(data)
            .map(|inner| HwpDocument { inner })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = toMarkdown)]
    pub fn to_markdown(&self) -> Result<String, JsValue> {
        let opts = RenderOptions::default();
        unhwp::render::render_markdown(&self.inner, &opts)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = toText)]
    pub fn to_text(&self) -> String {
        self.inner.plain_text()
    }

    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = sectionCount)]
    pub fn section_count(&self) -> u32 {
        self.inner.sections.len() as u32
    }

    #[wasm_bindgen(js_name = paragraphCount)]
    pub fn paragraph_count(&self) -> u32 {
        self.inner.paragraph_count() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_node_experimental);

    #[wasm_bindgen_test]
    fn test_from_bytes_invalid_returns_error() {
        let result = HwpDocument::from_bytes(b"garbage data not hwp");
        assert!(result.is_err());
    }

    #[wasm_bindgen_test]
    fn test_from_bytes_truncated_hwp5_returns_error() {
        // 매직 바이트는 맞지만 OLE 구조가 없으면 파싱 오류
        let hwp5_magic = &[0xD0u8, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0x00, 0x00];
        let result = HwpDocument::from_bytes(hwp5_magic);
        assert!(result.is_err());
    }
}
