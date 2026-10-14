//! The XML of an HWPX section: the fuzzed bytes replace the first section of the committed
//! two-section fixture, whose other parts stay well formed, so the mutations reach the section
//! parser instead of stopping at the ZIP container.
#![no_main]

use std::io::{Cursor, Read, Write};

use libfuzzer_sys::fuzz_target;
use zip::write::SimpleFileOptions;

const FIXTURE: &[u8] = include_bytes!("../../tests/fixtures/two_sections.hwpx");
const FUZZED_PART: &str = "Contents/section0.xml";

fn with_section(section: &[u8]) -> Vec<u8> {
    let mut source = zip::ZipArchive::new(Cursor::new(FIXTURE)).unwrap();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for i in 0..source.len() {
        let mut entry = source.by_index(i).unwrap();
        let name = entry.name().unwrap().to_string();
        let mut body = Vec::new();
        entry.read_to_end(&mut body).unwrap();
        out.start_file(name.as_str(), options).unwrap();
        out.write_all(if name == FUZZED_PART { section } else { &body })
            .unwrap();
    }
    out.finish().unwrap().into_inner()
}

fuzz_target!(|data: &[u8]| {
    if let Ok(doc) = unhwp::parse_bytes(&with_section(data)) {
        let _ = unhwp::render::render_markdown(&doc, &unhwp::render::RenderOptions::default());
    }
});
