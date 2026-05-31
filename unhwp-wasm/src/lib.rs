mod document;
mod options;

pub use document::HwpDocument;
pub use options::ParseOptions;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn parse(data: &[u8]) -> Result<HwpDocument, JsValue> {
    unhwp::parse_bytes(data)
        .map(|inner| HwpDocument { inner })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Parse with options.
#[wasm_bindgen(js_name = parseWithOptions)]
pub fn parse_with_options(data: &[u8], opts: &ParseOptions) -> Result<HwpDocument, JsValue> {
    unhwp::parse_bytes_with_options(data, &opts.to_parse_options())
        .map(|inner| HwpDocument { inner })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_node_experimental);

    #[wasm_bindgen_test]
    fn test_parse_invalid_returns_error() {
        let result = parse(b"garbage data");
        assert!(result.is_err());
    }

    #[wasm_bindgen_test]
    fn test_parse_with_options_invalid_returns_error() {
        let opts = ParseOptions::new().lenient();
        let result = parse_with_options(b"garbage data", &opts);
        assert!(result.is_err());
    }
}
