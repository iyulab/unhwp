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

    /// Returns the document's embedded images as an array of [`HwpImage`],
    /// sorted by resource ID for deterministic ordering.
    #[wasm_bindgen(js_name = toImages)]
    pub fn to_images(&self) -> Vec<HwpImage> {
        let mut images: Vec<HwpImage> = self
            .inner
            .resources
            .iter()
            .filter(|(_, r)| r.resource_type == unhwp::model::ResourceType::Image)
            .map(|(id, r)| HwpImage {
                id: id.clone(),
                filename: r.filename.clone(),
                mime_type: r.mime_type.clone(),
                data: r.data.clone(),
            })
            .collect();
        images.sort_by(|a, b| a.id.cmp(&b.id));
        images
    }
}

/// An embedded image extracted from a document.
#[wasm_bindgen]
pub struct HwpImage {
    id: String,
    filename: Option<String>,
    mime_type: Option<String>,
    data: Vec<u8>,
}

#[wasm_bindgen]
impl HwpImage {
    /// Resource identifier (matches markdown image references).
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    /// Original filename, if known.
    #[wasm_bindgen(getter)]
    pub fn filename(&self) -> Option<String> {
        self.filename.clone()
    }

    /// MIME type, if known.
    #[wasm_bindgen(getter, js_name = mimeType)]
    pub fn mime_type(&self) -> Option<String> {
        self.mime_type.clone()
    }

    /// Raw image bytes (`Uint8Array` in JS).
    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Vec<u8> {
        self.data.clone()
    }

    /// Image bytes encoded as standard base64 (for `data:` URLs).
    #[wasm_bindgen(getter)]
    pub fn base64(&self) -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(&self.data)
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

    #[wasm_bindgen_test]
    fn test_to_images_empty_for_doc_without_images() {
        let fixture = include_bytes!("../../tests/fixtures/two_sections.hwpx");
        let doc = HwpDocument::from_bytes(fixture).expect("fixture should parse");
        assert_eq!(doc.to_images().len(), 0);
    }

    #[wasm_bindgen_test]
    fn test_hwp_image_base64_and_bytes() {
        let img = HwpImage {
            id: "BIN0001".into(),
            filename: Some("BIN0001.png".into()),
            mime_type: Some("image/png".into()),
            data: vec![1, 2, 3],
        };
        assert_eq!(img.base64(), "AQID");
        assert_eq!(img.bytes(), vec![1, 2, 3]);
        assert_eq!(img.id(), "BIN0001");
    }
}
