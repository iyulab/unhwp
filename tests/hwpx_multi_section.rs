/// Integration tests for HWPX multi-section parsing.
///
/// These tests use fixture files (compact single-line XML, as real HWPX files
/// are formatted) to catch regressions that unit tests with pretty-printed XML
/// would miss — such as the section1 drop bug fixed in container.rs.
use unhwp::{parse_file, to_markdown};

const FIXTURE_TWO_SECTIONS: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/two_sections.hwpx");

#[test]
fn hwpx_two_sections_both_parsed() {
    let doc = parse_file(FIXTURE_TWO_SECTIONS).expect("parse should succeed");
    assert_eq!(
        doc.sections.len(),
        2,
        "both sections in the spine must be parsed; got {} section(s). \
         This likely means the section-order parser only found section0.",
        doc.sections.len()
    );
}

#[test]
fn hwpx_two_sections_content_present() {
    let md = to_markdown(FIXTURE_TWO_SECTIONS).expect("conversion should succeed");
    assert!(
        md.contains("Section zero content"),
        "section0 text must appear in output"
    );
    assert!(
        md.contains("Section one content"),
        "section1 text must appear in output — was missing before the single-line XML fix"
    );
}
