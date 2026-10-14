//! Whole files: format detection, the HWP 5 compound file (records, compressed streams), the
//! HWPX package and HWP 3, then the Markdown render.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(doc) = unhwp::parse_bytes(data) {
        let _ = unhwp::render::render_markdown(&doc, &unhwp::render::RenderOptions::default());
    }
});
