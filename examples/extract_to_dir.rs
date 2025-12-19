//! Extract HWP files to output directories
use std::fs;
use std::path::Path;

fn main() {
    let files = [
        "test-files/(공고_제2025-288호)_2025년도 창업성장기술개발사업(디딤돌) 제2차 시행계획 공고.hwp",
        "test-files/1. 2026년 정부일반형 사업계획서_ver_0.3.hwp",
        "test-files/최종보고서_엣지컴퓨팅 클라우드기반.hwpx",
    ];

    for file_path in &files {
        let path = Path::new(file_path);
        if !path.exists() {
            println!("❌ File not found: {}", file_path);
            continue;
        }

        let file_stem = path.file_stem().unwrap().to_string_lossy();
        let output_dir = format!("test-files/{}_output", file_stem);

        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📄 File: {}", path.file_name().unwrap().to_string_lossy());
        println!("📁 Output: {}", output_dir);

        // Create output directory
        if let Err(e) = fs::create_dir_all(&output_dir) {
            println!("❌ Failed to create output dir: {}", e);
            continue;
        }

        // Parse document
        let start = std::time::Instant::now();
        match unhwp::parse_file(path) {
            Ok(doc) => {
                let elapsed = start.elapsed();
                println!("✅ Parse: Success ({:.2?})", elapsed);
                println!("📑 Sections: {}", doc.sections.len());
                println!("📝 Paragraphs: {}", doc.paragraph_count());
                println!("🖼️  Resources: {}", doc.resources.len());

                // Extract markdown with correct image path
                let options = unhwp::RenderOptions::default()
                    .with_image_dir(format!("{}/images", output_dir))
                    .with_image_prefix("images/");

                match unhwp::render::render_markdown(&doc, &options) {
                    Ok(markdown) => {
                        let md_path = format!("{}/content.md", output_dir);
                        if let Err(e) = fs::write(&md_path, &markdown) {
                            println!("❌ Failed to write markdown: {}", e);
                        } else {
                            println!("📝 Markdown: {} chars → {}", markdown.len(), md_path);
                        }
                    }
                    Err(e) => println!("❌ Render failed: {}", e),
                }

                // Extract plain text
                let text = doc.plain_text();
                let txt_path = format!("{}/content.txt", output_dir);
                if let Err(e) = fs::write(&txt_path, &text) {
                    println!("❌ Failed to write text: {}", e);
                } else {
                    println!("📄 Plain text: {} chars → {}", text.len(), txt_path);
                }

                // Extract images if any
                if !doc.resources.is_empty() {
                    let images_dir = format!("{}/images", output_dir);
                    fs::create_dir_all(&images_dir).ok();

                    for (name, resource) in &doc.resources {
                        let img_path = format!("{}/{}", images_dir, name);
                        if let Err(e) = fs::write(&img_path, &resource.data) {
                            println!("❌ Failed to write image {}: {}", name, e);
                        }
                    }
                    println!("🖼️  Extracted {} images → {}/", doc.resources.len(), images_dir);
                }

                // Show preview
                let preview: String = text.chars().take(200).collect();
                println!("📖 Preview: {}...", preview.replace('\n', " ").trim());
            }
            Err(e) => {
                println!("❌ Parse failed: {}", e);
            }
        }
    }

    println!("\n=== Extraction Complete ===");
}
