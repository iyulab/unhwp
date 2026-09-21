//! What `table_fallback` does to a table Markdown cannot express.
//!
//! The option existed with three documented variants, a builder, and a doctest example that
//! set it -- and no code anywhere read it, so every table rendered the same way whatever the
//! caller chose (cycle-156 found it mechanically; cycle-157 implemented it). These assertions
//! are what keeps that from being true again: each variant is checked by the output it
//! produces, not by the field it sets.

use unhwp::model::{Block, Document, Section, Table, TableCell, TableRow};
use unhwp::render::render_markdown;
use unhwp::{RenderOptions, TableFallback};

/// A two-row table whose first cell spans both columns. Markdown has no way to say that.
fn document_with_a_merged_table() -> Document {
    let mut header = TableRow::header();
    let mut spanning = TableCell::text("Merged heading");
    spanning.colspan = 2;
    header.cells.push(spanning);

    let mut body = TableRow::new();
    body.cells.push(TableCell::text("left"));
    body.cells.push(TableCell::text("right"));

    let mut table = Table::new();
    table.has_header = true;
    table.rows.push(header);
    table.rows.push(body);

    let mut section = Section::new(0);
    section.content.push(Block::Table(table));

    let mut document = Document::new();
    document.sections.push(section);
    document
}

fn render(fallback: TableFallback) -> String {
    let options = RenderOptions::default().with_table_fallback(fallback);
    render_markdown(&document_with_a_merged_table(), &options).expect("rendering must succeed")
}

/// The baseline: without it, a renderer that emitted nothing would satisfy `Skip` and make
/// the difference between the other two look like a difference.
#[test]
fn the_fixture_really_has_a_merge() {
    let document = document_with_a_merged_table();
    let Block::Table(table) = &document.sections[0].content[0] else {
        panic!("the fixture must contain a table");
    };
    assert!(table.has_merged_cells(), "the fixture must contain a merge");
}

#[test]
fn html_keeps_the_span_that_markdown_cannot_express() {
    let out = render(TableFallback::Html);
    assert!(
        out.contains("<table>"),
        "expected an HTML table, got:\n{out}"
    );
    assert!(
        out.contains("colspan=\"2\""),
        "the span is the whole reason for this mode, got:\n{out}"
    );
    assert!(out.contains("Merged heading"));
    assert!(
        out.contains("<th"),
        "a header row renders as th, got:\n{out}"
    );
}

#[test]
fn simplified_markdown_flattens_the_merge_into_pipes() {
    let out = render(TableFallback::SimplifiedMarkdown);
    assert!(out.contains('|'), "expected a pipe table, got:\n{out}");
    assert!(!out.contains("<table>"), "must not emit HTML, got:\n{out}");
    assert!(out.contains("Merged heading"));
}

#[test]
fn skip_leaves_the_table_out_entirely() {
    let out = render(TableFallback::Skip);
    assert!(!out.contains("<table>"), "got:\n{out}");
    assert!(
        !out.contains("Merged heading"),
        "skip must drop the table's content, got:\n{out}"
    );
    assert!(!out.contains("left"), "got:\n{out}");
}

/// A table with nothing to fall back from renders the same way in every mode -- the option
/// is about merges, and reading it as a general table switch would be a different feature.
#[test]
fn a_table_without_merges_is_unaffected_by_the_mode() {
    let mut row = TableRow::new();
    row.cells.push(TableCell::text("a"));
    row.cells.push(TableCell::text("b"));
    let mut second = TableRow::new();
    second.cells.push(TableCell::text("c"));
    second.cells.push(TableCell::text("d"));

    let mut table = Table::new();
    table.rows.push(row);
    table.rows.push(second);

    let mut section = Section::new(0);
    section.content.push(Block::Table(table));
    let mut document = Document::new();
    document.sections.push(section);

    let renders: Vec<String> = [
        TableFallback::Html,
        TableFallback::SimplifiedMarkdown,
        TableFallback::Skip,
    ]
    .into_iter()
    .map(|fallback| {
        let options = RenderOptions::default().with_table_fallback(fallback);
        render_markdown(&document, &options).expect("rendering must succeed")
    })
    .collect();

    assert!(renders[0].contains('c'), "the table must still render");
    assert_eq!(renders[0], renders[1]);
    assert_eq!(renders[1], renders[2]);
}
