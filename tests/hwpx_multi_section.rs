/// Integration tests for HWPX multi-section parsing.
///
/// These tests use fixture files (compact single-line XML, as real HWPX files
/// are formatted) to catch regressions that unit tests with pretty-printed XML
/// would miss — such as the section1 drop bug fixed in container.rs.
use unhwp::{parse_bytes_with_options, parse_file, to_markdown, ErrorMode, ParseOptions};

const FIXTURE_TWO_SECTIONS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/two_sections.hwpx"
);

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

/// The HWPX path reports skipped sections the same way the HWP 5.0 path does. Damaging the
/// fixture's second section entry in the archive is enough: the container still lists it, the
/// read or parse of it fails, and lenient parsing has to say which index went missing rather
/// than quietly handing back a shorter document.
#[test]
fn hwpx_lenient_reports_the_section_it_skipped() {
    let original = std::fs::read(FIXTURE_TWO_SECTIONS).expect("fixture must be readable");
    let damaged = damage_second_section(&original);

    let opts = ParseOptions {
        error_mode: ErrorMode::Lenient,
        ..ParseOptions::default()
    };
    let doc = parse_bytes_with_options(&damaged, &opts).expect("lenient parsing must not fail");

    assert_eq!(doc.sections.len(), 1, "only the intact section survives");
    assert_eq!(
        doc.skipped_sections,
        [1],
        "the damaged section's index has to reach the caller"
    );
}

/// And strict still refuses the same bytes -- the report is the lenient half of the contract,
/// not a replacement for failing.
#[test]
fn hwpx_strict_still_fails_on_the_same_damage() {
    let original = std::fs::read(FIXTURE_TWO_SECTIONS).expect("fixture must be readable");
    let damaged = damage_second_section(&original);

    assert!(
        unhwp::parse_bytes(&damaged).is_err(),
        "strict must fail rather than report the loss"
    );
}

/// An intact fixture must report nothing, or the assertions above would pass against a
/// parser that always claims a loss.
#[test]
fn hwpx_an_intact_document_reports_no_skips() {
    let doc = parse_file(FIXTURE_TWO_SECTIONS).expect("parse should succeed");
    assert!(
        doc.skipped_sections.is_empty(),
        "{:?}",
        doc.skipped_sections
    );
}

/// Rewrites the archive so the second section's XML is no longer well formed, leaving every
/// other entry -- including the spine that lists it -- untouched.
fn damage_second_section(archive: &[u8]) -> Vec<u8> {
    use std::io::{Cursor, Read, Write};

    let mut reader = zip::ZipArchive::new(Cursor::new(archive)).expect("fixture must be a zip");
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    let mut damaged_any = false;
    for i in 0..reader.len() {
        let mut entry = reader.by_index(i).expect("entry must be readable");
        let name = entry.name().to_string();
        let mut body = Vec::new();
        entry
            .read_to_end(&mut body)
            .expect("entry must be readable");

        if name.contains("section1") {
            body = b"<not-well-formed".to_vec();
            damaged_any = true;
        }

        out.start_file(name, options)
            .expect("entry must be writable");
        out.write_all(&body).expect("entry must be writable");
    }
    assert!(
        damaged_any,
        "the fixture no longer has a section1 entry to damage"
    );

    out.finish().expect("archive must close").into_inner()
}
