/// Quality regression tests using real-world HWP/HWPX fixture files.
///
/// These tests verify that core rendering properties hold across known documents:
///   - Parse succeeds without panicking
///   - Markdown output is non-empty and structurally sound
///   - Bullet characters render as list markers without requiring cleanup opt-in
///
/// Files in test-files/ are committed but excluded from CI artifact uploads.
use unhwp::{parse_file, to_markdown, RenderOptions};

const SAMPLE_HWP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test-files/Sample.hwp");
const HWPX_2016: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test-files/HWP2016.hwpx");
const TIKA_HWP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test-files/tika-testHWP_5.0.hwp");

// ── Parse succeeds ────────────────────────────────────────────────────────────

#[test]
fn sample_hwp_parse_succeeds() {
    let doc = parse_file(SAMPLE_HWP).expect("Sample.hwp must parse without error");
    assert!(
        !doc.sections.is_empty(),
        "parsed document must have at least one section"
    );
}

#[test]
fn hwpx_2016_parse_succeeds() {
    let doc = parse_file(HWPX_2016).expect("HWP2016.hwpx must parse without error");
    assert!(
        !doc.sections.is_empty(),
        "parsed document must have at least one section"
    );
}

#[test]
fn tika_hwp_parse_succeeds() {
    let doc = parse_file(TIKA_HWP).expect("tika-testHWP_5.0.hwp must parse without error");
    assert!(
        !doc.sections.is_empty(),
        "parsed document must have at least one section"
    );
}

// ── Markdown output is non-empty ─────────────────────────────────────────────

#[test]
fn sample_hwp_markdown_non_empty() {
    let md = to_markdown(SAMPLE_HWP).expect("to_markdown must succeed");
    assert!(!md.trim().is_empty(), "markdown output must not be empty");
}

#[test]
fn hwpx_2016_markdown_non_empty() {
    let md = to_markdown(HWPX_2016).expect("to_markdown must succeed");
    assert!(!md.trim().is_empty(), "markdown output must not be empty");
}

// ── Bullet rendering without cleanup ─────────────────────────────────────────

#[test]
fn bullet_chars_render_as_list_marker_without_cleanup() {
    // Build a document from the HWPX fixture (no cleanup applied).
    // If the document contains any bullet paragraph (● ■ □ ○ etc.),
    // the renderer must convert it to "- " directly, not pass it through raw.
    let opts = RenderOptions::default();
    let doc = parse_file(HWPX_2016).expect("must parse");
    let md = unhwp::render::render_markdown(&doc, &opts).expect("must render");

    // At minimum: the markdown must not contain raw PUA bullet chars
    // (these would indicate the renderer failed to convert HWP private-use bullets)
    let pua_bullets = ['\u{F0A3}', '\u{F09F}', '\u{F09E}', '\u{F020}', '\u{F076}', '\u{F0A8}'];
    for ch in pua_bullets {
        assert!(
            !md.contains(ch),
            "PUA bullet char U+{:04X} must not appear in rendered output — should be converted to '- '",
            ch as u32
        );
    }
}

// ── Bold/italic bit ordering (end-to-end) ────────────────────────────────────

#[test]
fn tika_hwp_headings_render_correctly() {
    // tika-testHWP_5.0.hwp contains headings ("테스트", "test") — verify they render
    // as Markdown headings. This is an end-to-end guard for the CharShape parsing pipeline.
    let opts = RenderOptions::default().with_heading_analysis();
    let doc = parse_file(TIKA_HWP).expect("must parse");
    let md = unhwp::render::render_markdown(&doc, &opts).expect("must render");
    // Document has "테스트" and "test" as headings
    assert!(
        md.contains('#'),
        "document with heading styles must produce Markdown headings, got: {:?}",
        &md[..md.len().min(200)]
    );
}
