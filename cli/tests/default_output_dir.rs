//! Without `-o`, `convert` writes beside the input into a directory named by the
//! input's stem *and* extension.
//!
//! With the stem alone, `A2.hwp` and `A2.hwpx` shared one directory and the second
//! conversion silently overwrote the first one's files.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn convert_without_output_writes_next_to_the_input_named_by_its_extension() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/two_sections.hwpx");
    let dir = std::env::temp_dir().join(format!("unhwp-default-output-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("A2.hwpx");
    std::fs::copy(&fixture, &input).expect("the committed fixture must exist");

    let status = Command::new(env!("CARGO_BIN_EXE_unhwp"))
        .args(["convert"])
        .arg(&input)
        .arg("--quiet")
        .current_dir(std::env::temp_dir())
        .status()
        .unwrap();
    let wrote = dir.join("A2_hwpx_output").join("extract.md").exists();
    let old_name = dir.join("A2_output").exists();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(status.success(), "convert failed: {status:?}");
    assert!(
        wrote,
        "default output must sit beside the input as A2_hwpx_output/"
    );
    assert!(!old_name);
}
