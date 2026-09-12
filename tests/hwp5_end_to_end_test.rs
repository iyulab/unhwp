//! HWP 5.0 documents, end to end: from the OLE container through the section records to
//! the document model.
//!
//! The unit tests in `hwp5::bodytext` start from a section's record bytes, and the only
//! committed fixture is an HWPX package, so nothing opened an actual HWP 5.0 container:
//! the container, decompression, section enumeration and error-mode paths never ran.
//! These tests assemble minimal containers in memory -- a `FileHeader`, an empty
//! `DocInfo`, and one `BodyText/Section{n}` stream per section.

use std::io::{Cursor, Write};
use std::ops::ControlFlow;

use unhwp::{
    parse_bytes, parse_bytes_with_options, parse_file_streaming, ErrorKind, ErrorMode, ParseEvent,
    ParseOptions, SectionStreamOptions,
};

const PARA_HEADER: u32 = 66;
const PARA_TEXT: u32 = 67;

const COMPRESSED: u32 = 1 << 0;
const ENCRYPTED: u32 = 1 << 1;

/// A record: 4-byte header (tag | level << 10 | size << 20) followed by its data.
fn record(tag: u32, level: u32, data: &[u8]) -> Vec<u8> {
    let header = tag | (level << 10) | ((data.len() as u32) << 20);
    let mut out = header.to_le_bytes().to_vec();
    out.extend_from_slice(data);
    out
}

/// A section holding one paragraph of `text`.
fn section(text: &str) -> Vec<u8> {
    let utf16: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut out = record(PARA_HEADER, 0, &[0u8; 8]);
    out.extend(record(PARA_TEXT, 1, &utf16));
    out
}

fn file_header(flags: u32) -> Vec<u8> {
    let mut header = vec![0u8; 256];
    header[..17].copy_from_slice(b"HWP Document File");
    header[32..36].copy_from_slice(&[0, 0, 1, 5]); // revision, build, minor, major
    header[36..40].copy_from_slice(&flags.to_le_bytes());
    header
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

/// What goes into one `BodyText/Section{n}` stream.
enum Body<'a> {
    /// Section records, deflated when the header says the document is compressed.
    Records(Vec<u8>),
    /// Bytes written verbatim -- a stream the reader cannot make sense of.
    Raw(&'a [u8]),
}

/// A minimal HWP 5.0 container.
fn hwp5(flags: u32, sections: &[Body]) -> Vec<u8> {
    let compressed = flags & COMPRESSED != 0;
    let pack = |data: &[u8]| {
        if compressed {
            deflate(data)
        } else {
            data.to_vec()
        }
    };

    let mut cfb = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    cfb.create_stream("/FileHeader")
        .unwrap()
        .write_all(&file_header(flags))
        .unwrap();
    cfb.create_stream("/DocInfo")
        .unwrap()
        .write_all(&pack(&[]))
        .unwrap();
    cfb.create_storage("/BodyText").unwrap();
    for (index, body) in sections.iter().enumerate() {
        let bytes = match body {
            Body::Records(records) => pack(records),
            Body::Raw(raw) => raw.to_vec(),
        };
        cfb.create_stream(format!("/BodyText/Section{index}"))
            .unwrap()
            .write_all(&bytes)
            .unwrap();
    }
    cfb.flush().unwrap();
    cfb.into_inner().into_inner()
}

/// `0xFF` opens a deflate block of type 3, which is reserved -- no decoder accepts it.
const NOT_DEFLATE: &[u8] = &[0xFF; 16];

fn two_sections(flags: u32) -> Vec<u8> {
    hwp5(
        flags,
        &[
            Body::Records(section("첫 번째 구역")),
            Body::Records(section("Second section")),
        ],
    )
}

fn damaged_second_section() -> Vec<u8> {
    hwp5(
        COMPRESSED,
        &[Body::Records(section("kept")), Body::Raw(NOT_DEFLATE)],
    )
}

#[test]
fn an_uncompressed_document_parses_end_to_end() {
    let doc = parse_bytes(&two_sections(0)).expect("a well-formed container must parse");

    assert_eq!(doc.sections.len(), 2);
    let text = doc.plain_text();
    let first = text.find("첫 번째 구역").expect("first section's text");
    let second = text.find("Second section").expect("second section's text");
    assert!(first < second, "sections must keep their order: {text:?}");
    assert_eq!(doc.metadata.format_version.as_deref(), Some("5.1.0.0"));
}

/// Most real documents set the compression bit, so the deflate path is the ordinary one.
#[test]
fn a_compressed_document_parses_to_the_same_content() {
    let plain = parse_bytes(&two_sections(0)).unwrap();
    let compressed = parse_bytes(&two_sections(COMPRESSED)).unwrap();

    assert_eq!(compressed.sections.len(), 2);
    assert_eq!(compressed.plain_text(), plain.plain_text());
}

#[test]
fn an_encrypted_document_is_refused_as_encrypted() {
    let err = parse_bytes(&two_sections(COMPRESSED | ENCRYPTED)).unwrap_err();

    assert_eq!(err.kind(), ErrorKind::Encrypted, "got: {err}");
}

/// `ErrorMode::Strict` is the default. A section that cannot be read must fail the
/// document rather than vanish from it -- otherwise a damaged file and a file that never
/// had that section are indistinguishable to the caller. The kind is pinned too: "it
/// failed" alone would also pass for a failure with an unrelated cause.
#[test]
fn a_damaged_section_fails_the_document_by_default() {
    match parse_bytes(&damaged_second_section()) {
        Err(err) => assert_eq!(err.kind(), ErrorKind::Decompression, "got: {err}"),
        Ok(doc) => panic!(
            "a document with an unreadable section parsed instead of failing; it produced \
             {} section(s)",
            doc.sections.len()
        ),
    }
}

/// A section that decompresses fine but whose last record claims more bytes than remain.
fn truncated_section() -> Vec<u8> {
    let mut out = section("before the cut");
    out.extend_from_slice(&(PARA_TEXT | (1 << 10) | (200 << 20)).to_le_bytes());
    out.extend_from_slice(&[0x41, 0x00, 0x42, 0x00]); // 4 of the 200 bytes promised
    out
}

fn truncated_second_section() -> Vec<u8> {
    hwp5(
        COMPRESSED,
        &[
            Body::Records(section("kept")),
            Body::Records(truncated_section()),
        ],
    )
}

/// A record that runs past the end of its section is a record-structure error, and it
/// reaches the caller as one -- `ErrorKind::RecordParse` exists for exactly this and is
/// part of the binding surface.
#[test]
fn a_truncated_record_fails_the_document_by_default() {
    match parse_bytes(&truncated_second_section()) {
        Err(err) => assert_eq!(err.kind(), ErrorKind::RecordParse, "got: {err}"),
        Ok(doc) => panic!(
            "a document with a truncated record parsed instead of failing: {:?}",
            doc.plain_text()
        ),
    }
}

/// Under `Lenient` the section holding the truncated record is skipped like any other
/// unparsable section -- whole, so none of its half-read content leaks into the output.
#[test]
fn lenient_mode_skips_a_section_with_a_truncated_record() {
    let opts = ParseOptions {
        error_mode: ErrorMode::Lenient,
        ..ParseOptions::default()
    };

    let doc = parse_bytes_with_options(&truncated_second_section(), &opts)
        .expect("lenient parsing must not fail on a truncated record");

    assert_eq!(doc.sections.len(), 1);
    let text = doc.plain_text();
    assert!(text.contains("kept"), "{text:?}");
    assert!(!text.contains("before the cut"), "{text:?}");
}

/// The other half of the option: a caller that asked to salvage what it can still gets the
/// readable sections. Pinning both halves keeps the option meaningful -- with only the
/// strict test, hard-coding a failure would pass.
#[test]
fn lenient_mode_skips_the_damaged_section_and_keeps_the_rest() {
    let opts = ParseOptions {
        error_mode: ErrorMode::Lenient,
        ..ParseOptions::default()
    };

    let doc = parse_bytes_with_options(&damaged_second_section(), &opts)
        .expect("lenient parsing must not fail on a damaged section");

    assert_eq!(doc.sections.len(), 1);
    assert!(doc.plain_text().contains("kept"));
}

/// The streaming parser already honoured the option; the batch parser must agree with it
/// on the same input, in both modes.
#[test]
fn the_streaming_parser_agrees_on_the_same_damaged_document() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("damaged.hwp");
    std::fs::write(&path, damaged_second_section()).unwrap();

    let strict = parse_file_streaming(&path, SectionStreamOptions::default(), |_| {
        ControlFlow::Continue(())
    });
    assert!(
        strict.is_err(),
        "strict streaming must fail on the damaged section"
    );

    let lenient = SectionStreamOptions {
        error_mode: ErrorMode::Lenient,
        ..SectionStreamOptions::default()
    };
    let (mut parsed, mut failed) = (Vec::new(), Vec::new());
    parse_file_streaming(&path, lenient, |event| {
        match event {
            ParseEvent::SectionParsed(section) => parsed.push(section.index),
            ParseEvent::SectionFailed { index, .. } => failed.push(index),
            _ => {}
        }
        ControlFlow::Continue(())
    })
    .expect("lenient streaming must not fail");
    assert_eq!(parsed, [0]);
    assert_eq!(failed, [1]);
}

/// Skipping is only half of what Lenient owes a caller. `sections.len()` is the same number
/// whether a document had three sections or had five and lost two, so a caller that asked to
/// keep going could not see what keeping going cost. The streaming parser has always reported
/// this as `SectionFailed`; the batch parser used to drop the section and say nothing.
#[test]
fn lenient_mode_reports_which_section_it_skipped() {
    let opts = ParseOptions {
        error_mode: ErrorMode::Lenient,
        ..ParseOptions::default()
    };

    let doc = parse_bytes_with_options(&damaged_second_section(), &opts)
        .expect("lenient parsing must not fail on a damaged section");

    assert_eq!(doc.sections.len(), 1);
    assert_eq!(
        doc.skipped_sections,
        [1],
        "the damaged section's index has to reach the caller"
    );
}

/// The same for a section that reads but will not parse -- the two failure stages have to
/// reach the caller the same way, or the report means different things for different damage.
#[test]
fn lenient_mode_reports_a_section_lost_to_a_truncated_record() {
    let opts = ParseOptions {
        error_mode: ErrorMode::Lenient,
        ..ParseOptions::default()
    };

    let doc = parse_bytes_with_options(&truncated_second_section(), &opts)
        .expect("lenient parsing must not fail on a truncated record");

    assert_eq!(doc.skipped_sections, [1]);
}

/// A report that is never empty is not a report. An intact document must say so, in both
/// modes -- otherwise "nothing was lost" and "this build does not measure loss" look alike.
#[test]
fn an_intact_document_reports_no_skipped_sections() {
    let doc = parse_bytes(&two_sections(0)).expect("an intact document must parse");
    assert!(
        doc.skipped_sections.is_empty(),
        "{:?}",
        doc.skipped_sections
    );

    let lenient = ParseOptions {
        error_mode: ErrorMode::Lenient,
        ..ParseOptions::default()
    };
    let doc = parse_bytes_with_options(&two_sections(0), &lenient)
        .expect("an intact document must parse under lenient too");
    assert!(
        doc.skipped_sections.is_empty(),
        "{:?}",
        doc.skipped_sections
    );
}

/// The point of the change, stated as an equality: the two parsers now answer the same
/// question with the same indices on the same bytes. Asserting each against a literal would
/// pass even if they drifted apart again.
#[test]
fn the_batch_parser_reports_what_the_streaming_parser_reports() {
    let bytes = damaged_second_section();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("damaged.hwp");
    std::fs::write(&path, &bytes).unwrap();

    let opts = ParseOptions {
        error_mode: ErrorMode::Lenient,
        ..ParseOptions::default()
    };
    let batch = parse_bytes_with_options(&bytes, &opts).expect("lenient batch must not fail");

    let stream_opts = SectionStreamOptions {
        error_mode: ErrorMode::Lenient,
        ..SectionStreamOptions::default()
    };
    let mut streamed = Vec::new();
    parse_file_streaming(&path, stream_opts, |event| {
        if let ParseEvent::SectionFailed { index, .. } = event {
            streamed.push(index);
        }
        ControlFlow::Continue(())
    })
    .expect("lenient streaming must not fail");

    assert_eq!(batch.skipped_sections, streamed);
}

/// Strict does not skip, so it has nothing to report -- and a document it accepts must not
/// carry a phantom loss.
#[test]
fn strict_mode_never_reports_a_skip() {
    let doc = parse_bytes(&two_sections(0)).expect("an intact document must parse");
    assert!(doc.skipped_sections.is_empty());

    assert!(
        parse_bytes(&damaged_second_section()).is_err(),
        "strict must still fail rather than report the loss"
    );
}
