use wasm_bindgen::prelude::*;

#[derive(Default)]
#[wasm_bindgen]
pub struct ParseOptions {
    lenient: bool,
    text_only: bool,
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

    #[wasm_bindgen(js_name = textOnly)]
    pub fn text_only(mut self) -> Self {
        self.text_only = true;
        self
    }

    pub(crate) fn to_parse_options(&self) -> unhwp::ParseOptions {
        let mut opts = unhwp::ParseOptions::default();
        if self.lenient {
            opts = opts.lenient();
        }
        if self.text_only {
            opts = opts.text_only();
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
        assert!(!opts.text_only);
    }

    #[wasm_bindgen_test]
    fn test_parse_options_lenient() {
        let opts = ParseOptions::new().lenient();
        assert!(opts.lenient);
    }

    #[wasm_bindgen_test]
    fn test_parse_options_text_only() {
        let opts = ParseOptions::new().text_only();
        assert!(opts.text_only);
    }
}
