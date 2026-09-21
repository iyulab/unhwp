use wasm_bindgen::prelude::*;

#[derive(Default)]
#[wasm_bindgen]
pub struct ParseOptions {
    lenient: bool,
    without_resources: bool,
}

#[wasm_bindgen]
impl ParseOptions {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lenient(mut self) -> Self {
        self.lenient = true;
        self
    }

    /// Skip binary resources (images, equations). Named for what it does: the
    /// structure and text of the document are produced either way.
    #[wasm_bindgen(js_name = withoutResources)]
    pub fn without_resources(mut self) -> Self {
        self.without_resources = true;
        self
    }

    pub(crate) fn to_parse_options(&self) -> unhwp::ParseOptions {
        let mut opts = unhwp::ParseOptions::default();
        if self.lenient {
            opts = opts.lenient();
        }
        if self.without_resources {
            opts = opts.without_resources();
        }
        opts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_node_experimental);

    #[wasm_bindgen_test]
    fn test_parse_options_default() {
        let opts = ParseOptions::new();
        assert!(!opts.lenient);
        assert!(!opts.without_resources);
    }

    #[wasm_bindgen_test]
    fn test_parse_options_lenient() {
        let opts = ParseOptions::new().lenient();
        assert!(opts.lenient);
    }

    #[wasm_bindgen_test]
    fn test_parse_options_without_resources() {
        let opts = ParseOptions::new().without_resources();
        assert!(opts.without_resources);
    }
}
