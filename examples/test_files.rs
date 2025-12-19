//! Test parsing of real HWP/HWPX files.

use std::path::Path;
use std::time::Instant;

fn main() {
    let test_dir = Path::new("test-files");

    if !test_dir.exists() {
        eprintln!("Test directory not found: {:?}", test_dir);
        return;
    }

    println!("=== unhwp File Parsing Test ===\n");

    let files: Vec<_> = std::fs::read_dir(test_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            ext == "hwp" || ext == "hwpx"
        })
        .collect();

    for entry in files {
        let path = entry.path();
        let filename = path.file_name().unwrap().to_string_lossy();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📄 File: {}", filename);
        println!("📊 Size: {:.2} KB", size as f64 / 1024.0);

        // Detect format
        match unhwp::detect_format_from_path(&path) {
            Ok(format) => println!("🔍 Format: {}", format),
            Err(e) => {
                println!("❌ Format detection failed: {}", e);
                continue;
            }
        }

        // Parse document
        let start = Instant::now();
        match unhwp::parse_file(&path) {
            Ok(doc) => {
                let elapsed = start.elapsed();
                println!("✅ Parse: Success ({:.2?})", elapsed);
                println!("📑 Sections: {}", doc.sections.len());
                println!("📝 Paragraphs: {}", doc.paragraph_count());
                println!("🖼️  Resources: {}", doc.resources.len());

                // Show first 200 chars of text
                let text = doc.plain_text();
                let preview: String = text.chars().take(200).collect();
                let preview = preview.replace('\n', " ").replace('\r', "");
                println!("📖 Preview: {}...", preview.trim());

                // Try markdown conversion
                match unhwp::render::render_markdown(&doc, &unhwp::RenderOptions::default()) {
                    Ok(md) => {
                        println!("📝 Markdown: {} chars", md.len());
                    }
                    Err(e) => {
                        println!("⚠️  Markdown failed: {}", e);
                    }
                }
            }
            Err(e) => {
                let elapsed = start.elapsed();
                println!("❌ Parse failed ({:.2?}): {}", elapsed, e);
            }
        }
        println!();
    }

    println!("=== Test Complete ===");
}
