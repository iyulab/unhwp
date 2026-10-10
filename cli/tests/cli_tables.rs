//! `unhwp tables` writes each table of a document as CSV, merged cells on their grid.
//!
//! The HWPX is the committed two-section fixture with its first section replaced by a table,
//! so the test runs everywhere instead of quietly skipping when a fixture is absent.

use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_unhwp")
}

// ┌────────┬───────────────┐
// │ Region │ Sales, total  │   «Region» merged down, «Sales, total» across two columns
// │        ├───────┬───────┤
// │        │ 2024  │ 2025  │
// ├────────┼───────┼───────┤
// │ North  │  10   │  12   │
// └────────┴───────┴───────┘
const SECTION0: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    r#"<hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section" xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph">"#,
    r#"<hp:tbl>"#,
    r#"<hp:tr>"#,
    r#"<hp:tc><hp:subList><hp:p><hp:run><hp:t>Region</hp:t></hp:run></hp:p></hp:subList><hp:cellSpan colSpan="1" rowSpan="2"/></hp:tc>"#,
    r#"<hp:tc><hp:subList><hp:p><hp:run><hp:t>Sales, total</hp:t></hp:run></hp:p></hp:subList><hp:cellSpan colSpan="2" rowSpan="1"/></hp:tc>"#,
    r#"</hp:tr>"#,
    r#"<hp:tr>"#,
    r#"<hp:tc><hp:subList><hp:p><hp:run><hp:t>2024</hp:t></hp:run></hp:p></hp:subList></hp:tc>"#,
    r#"<hp:tc><hp:subList><hp:p><hp:run><hp:t>2025</hp:t></hp:run></hp:p></hp:subList></hp:tc>"#,
    r#"</hp:tr>"#,
    r#"<hp:tr>"#,
    r#"<hp:tc><hp:subList><hp:p><hp:run><hp:t>North</hp:t></hp:run></hp:p></hp:subList></hp:tc>"#,
    r#"<hp:tc><hp:subList><hp:p><hp:run><hp:t>10</hp:t></hp:run></hp:p></hp:subList></hp:tc>"#,
    r#"<hp:tc><hp:subList><hp:p><hp:run><hp:t>12</hp:t></hp:run></hp:p></hp:subList></hp:tc>"#,
    r#"</hp:tr>"#,
    r#"</hp:tbl>"#,
    r#"</hs:sec>"#,
);

const TABLE: &str = "Region,\"Sales, total\",\r\n,2024,2025\r\nNorth,10,12\r\n";

/// The fixture with `Contents/section0.xml` swapped for [`SECTION0`].
fn hwpx_with_a_merged_table() -> Vec<u8> {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/two_sections.hwpx");
    let original = std::fs::read(&fixture).expect("the committed fixture must exist");
    let mut archive = zip::ZipArchive::new(Cursor::new(original)).unwrap();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let name = entry.name().unwrap().into_owned();
        let mut body = Vec::new();
        entry.read_to_end(&mut body).unwrap();
        if name == "Contents/section0.xml" {
            body = SECTION0.as_bytes().to_vec();
        }
        out.start_file(name, options).unwrap();
        out.write_all(&body).unwrap();
    }
    out.finish().unwrap().into_inner()
}

fn input(dir: &Path) -> PathBuf {
    let path = dir.join("tables.hwpx");
    std::fs::write(&path, hwpx_with_a_merged_table()).unwrap();
    path
}

#[test]
fn tables_to_stdout_are_csv() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .arg("tables")
        .arg(input(tmp.path()))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8(out.stdout).unwrap(), TABLE);
}

#[test]
fn tables_to_a_directory_are_named_for_their_section_and_place() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("out");
    let status = Command::new(bin())
        .arg("tables")
        .arg(input(tmp.path()))
        .arg("-o")
        .arg(&dir)
        .status()
        .unwrap();
    assert!(status.success());

    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["s1-t1.csv"]);
    assert_eq!(
        std::fs::read_to_string(dir.join("s1-t1.csv")).unwrap(),
        TABLE
    );
}

#[test]
fn tsv_separates_fields_with_tabs() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .args(["tables", "--tsv"])
        .arg(input(tmp.path()))
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "Region\tSales, total\t\r\n\t2024\t2025\r\nNorth\t10\t12\r\n"
    );
}
