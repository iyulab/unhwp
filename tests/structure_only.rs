//! What "structure only" leaves out, and what it keeps.
//!
//! `ParseOptions::with_text(false)` is an output contract. Until it was implemented, the
//! option it replaced set a field nothing read — parsing produced the whole document
//! whatever the caller asked for — and no test anywhere would have noticed. These
//! assertions pin the shape the sibling PDF parser already uses for a page: **the section
//! survives, its content blocks are not built.**

use std::ops::ControlFlow;

use unhwp::{
    parse_bytes, parse_bytes_with_options, parse_file_streaming, ParseEvent, ParseOptions,
    SectionStreamOptions,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/two_sections.hwpx");

/// The baseline the structure-only assertions are measured against. Without it, a build
/// that produced nothing at all would satisfy every "is empty" assertion below.
#[test]
fn a_full_parse_produces_content_blocks() {
    let doc = parse_bytes(FIXTURE).expect("the committed fixture must parse");

    assert_eq!(doc.sections.len(), 2, "the fixture has two sections");
    assert!(
        doc.sections.iter().any(|s| !s.content.is_empty()),
        "the baseline document must carry content, or the structure-only test is vacuous"
    );
}

/// The contract: the sections are still there, with their identity intact. Only their
/// content is not.
#[test]
fn structure_only_keeps_the_sections_and_drops_their_content() {
    let full = parse_bytes(FIXTURE).expect("the committed fixture must parse");
    let structure = parse_bytes_with_options(FIXTURE, &ParseOptions::new().with_text(false))
        .expect("the fixture must parse without text extraction");

    assert_eq!(
        structure.sections.len(),
        full.sections.len(),
        "structure-only must not change how many sections a document has"
    );

    for (s, f) in structure.sections.iter().zip(full.sections.iter()) {
        assert_eq!(s.index, f.index, "the section keeps its index");
        assert!(
            s.content.is_empty(),
            "structure-only must not build content blocks, got {} in section {}",
            s.content.len(),
            s.index
        );
    }
}

/// `structure_only()` is a preset over two independent axes, and the axes can be set on
/// their own. Asking for structure decides nothing about resources by itself.
#[test]
fn text_and_resource_extraction_are_separate_axes() {
    let preset = ParseOptions::new().structure_only();
    assert!(!preset.extract_text);
    assert!(!preset.extract_resources);

    let text_off_only = ParseOptions::new().with_text(false);
    assert!(!text_off_only.extract_text);
    assert!(
        text_off_only.extract_resources,
        "turning text off must not turn resources off behind the caller's back"
    );
}

/// The streaming API gives the same contract as the batch one. It did not always have the
/// option at all: `SectionStreamOptions` carried an error mode and a resource flag, so a
/// caller who asked the batch API for structure only and then switched to streaming silently
/// got the whole document back.
#[test]
fn streaming_structure_only_emits_sections_without_content() {
    let opts = SectionStreamOptions {
        extract_text: false,
        ..SectionStreamOptions::default()
    };

    // The streaming entry point takes a path, so the committed fixture is staged in a
    // temporary directory rather than read from one that may not exist.
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("two_sections.hwpx");
    std::fs::write(&path, FIXTURE).expect("staging the fixture");

    let mut indices = Vec::new();
    let mut blocks = 0usize;
    parse_file_streaming(&path, opts, |event| {
        if let ParseEvent::SectionParsed(section) = event {
            indices.push(section.index);
            blocks += section.content.len();
        }
        ControlFlow::Continue(())
    })
    .expect("the fixture must stream without text extraction");

    assert_eq!(
        indices,
        vec![0, 1],
        "every section is still announced, in order"
    );
    assert_eq!(
        blocks, 0,
        "structure-only streaming must not build content blocks"
    );
}

/// The batch options convert to streaming options without dropping the text axis -- the
/// conversion is where it went missing before.
#[test]
fn batch_options_carry_the_text_axis_into_streaming_options() {
    let streaming: SectionStreamOptions = (&ParseOptions::new().with_text(false)).into();
    assert!(!streaming.extract_text);
}
