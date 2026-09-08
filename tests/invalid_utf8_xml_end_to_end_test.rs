//! An HWPX part whose bytes are not valid UTF-8 must fail, and be classified as an
//! encoding failure.
//!
//! This crate already declares that policy — `From<std::str::Utf8Error>` maps to
//! `Error::Encoding`, and the HWP 5 UTF-16 paths return it. The HWPX path did not: it
//! read straight into a `String`, so the reader's `InvalidData` arrived as
//! `Error::Io` and a consumer branching on the kind saw a disk problem instead of a
//! malformed document. These tests pin the corrected classification at the layer that
//! can observe it, and pin that a valid package still parses.
//!
//! They also fix the *policy* in place ahead of any change to how XML is decoded. The
//! reader is handed a `&str`, so the bytes are valid by construction by the time it
//! runs — the decision about invalid input is made here, upstream, and must stay here.

use std::io::{Cursor, Read, Write};

use unhwp::{parse_bytes, parse_bytes_with_options, ErrorKind, ErrorMode, ParseOptions};
use zip::write::SimpleFileOptions;

const FIXTURE: &[u8] = include_bytes!("fixtures/two_sections.hwpx");

/// `0x80` is a UTF-8 continuation byte with no leading byte in front of it — invalid
/// wherever it appears, and not part of any BOM the readers probe for.
const BAD: u8 = 0x80;

/// Rebuilds the fixture package, optionally splicing an invalid byte into one part.
///
/// Rewriting a known-good package rather than hand-rolling one keeps the test about the
/// encoding policy instead of about whether the fixture is a well-formed HWPX.
fn repack(corrupt: Option<(&str, &str)>) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(FIXTURE)).expect("fixture is a zip");
    let names: Vec<String> = archive.file_names().map(str::to_string).collect();

    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();

    for name in names {
        let mut bytes = Vec::new();
        archive
            .by_name(&name)
            .expect("entry listed by the archive must open")
            .read_to_end(&mut bytes)
            .expect("read zip entry");

        if let Some((target, anchor)) = corrupt {
            if name == target {
                let at = find(&bytes, anchor.as_bytes())
                    .unwrap_or_else(|| panic!("{anchor:?} must appear in {name}"));
                bytes.insert(at, BAD);
            }
        }

        out.start_file(&name, options).expect("start zip entry");
        out.write_all(&bytes).expect("write zip entry");
    }

    out.finish().expect("finish zip").into_inner()
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|at| at + needle.len())
}

/// The three parts below are read by three different readers — the section parser, the
/// style parser, and the OPF manifest parser. A policy that only held for one of them
/// would be invisible in a test that checked a single part.
#[test]
fn invalid_utf8_in_any_part_fails_the_package_and_is_classified() {
    for (part, anchor) in [
        ("Contents/section0.xml", "Section zero"),
        ("Contents/header.xml", "<hh:refList>"),
        ("Contents/content.hpf", "<opf:manifest>"),
    ] {
        let bytes = repack(Some((part, anchor)));

        let err = match parse_bytes(&bytes) {
            Err(e) => e,
            Ok(doc) => panic!(
                "a package with an invalid UTF-8 byte in {part} parsed instead of failing; \
                 it produced {} section(s)",
                doc.sections.len()
            ),
        };

        assert_eq!(
            err.kind(),
            ErrorKind::Encoding,
            "invalid UTF-8 in {part} must be reported as an encoding failure, not by where \
             it was noticed; got: {err}"
        );
    }
}

/// Without this, "everything fails" would also satisfy the test above.
#[test]
fn a_repacked_valid_package_still_parses() {
    let doc = parse_bytes(&repack(None)).expect("the untouched fixture must still parse");

    assert_eq!(
        doc.sections.len(),
        2,
        "repacking must not change what the package contains"
    );
}

/// The other half of the option. Failing is the *default*, not the only behaviour —
/// a caller that asked to salvage what it can still gets the readable sections.
///
/// Pinning both branches is what keeps the option meaningful: with only the strict test,
/// hard-coding a failure would pass, which is the mirror image of the defect these tests
/// were written to catch (hard-coded skipping, with `ErrorMode::Strict` declared as the
/// default and never honoured on this path).
#[test]
fn lenient_mode_skips_the_damaged_section_and_keeps_the_rest() {
    let bytes = repack(Some(("Contents/section0.xml", "Section zero")));
    let opts = ParseOptions {
        error_mode: ErrorMode::Lenient,
        ..ParseOptions::default()
    };

    let doc = parse_bytes_with_options(&bytes, &opts)
        .expect("lenient parsing must not fail on a damaged section");

    assert_eq!(
        doc.sections.len(),
        1,
        "the undamaged section must survive and the damaged one must be dropped"
    );
}

/// Strict is the declared default, so a caller that passes no options gets it.
#[test]
fn strict_is_what_a_caller_gets_without_asking() {
    assert_eq!(ParseOptions::default().error_mode, ErrorMode::Strict);
}
