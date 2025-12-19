//! unhwp CLI - Convert HWP/HWPX documents to Markdown
//!
//! Usage:
//!   unhwp <input.hwp> [output_dir] [--cleanup]
//!   unhwp <input.hwpx> [output_dir] [--cleanup]

use std::env;
use std::fs;
use std::path::Path;
use unhwp::{parse_file, render, RenderOptions};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <input.hwp|hwpx> [output_dir] [OPTIONS]", args[0]);
        eprintln!();
        eprintln!("Converts HWP/HWPX documents to Markdown with extracted images.");
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  input       Input HWP or HWPX file");
        eprintln!("  output_dir  Output directory (default: <input>_output)");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --cleanup            Enable default cleanup for LLM training data");
        eprintln!("  --cleanup-minimal    Enable minimal cleanup (essential normalization only)");
        eprintln!("  --cleanup-aggressive Enable aggressive cleanup (maximum purification)");
        std::process::exit(1);
    }

    // Parse flags
    let enable_cleanup = args.iter().any(|a| a == "--cleanup");
    let enable_cleanup_minimal = args.iter().any(|a| a == "--cleanup-minimal");
    let enable_cleanup_aggressive = args.iter().any(|a| a == "--cleanup-aggressive");

    // Filter out flags from args
    let positional_args: Vec<&String> = args.iter()
        .filter(|a| !a.starts_with("--"))
        .collect();

    let input_path = Path::new(&positional_args[1]);

    if !input_path.exists() {
        eprintln!("Error: Input file not found: {}", input_path.display());
        std::process::exit(1);
    }

    // Determine output directory
    let output_dir = if positional_args.len() > 2 {
        Path::new(positional_args[2]).to_path_buf()
    } else {
        let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
        input_path.parent().unwrap_or(Path::new(".")).join(format!("{}_output", stem))
    };

    // Create output directory
    if let Err(e) = fs::create_dir_all(&output_dir) {
        eprintln!("Error: Failed to create output directory: {}", e);
        std::process::exit(1);
    }

    // Create images subdirectory
    let images_dir = output_dir.join("images");
    if let Err(e) = fs::create_dir_all(&images_dir) {
        eprintln!("Error: Failed to create images directory: {}", e);
        std::process::exit(1);
    }

    println!("Parsing: {}", input_path.display());

    // Parse the document
    let document = match parse_file(input_path) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("Error: Failed to parse document: {}", e);
            std::process::exit(1);
        }
    };

    // Extract images
    let mut image_count = 0;
    for (name, resource) in &document.resources {
        let image_path = images_dir.join(name);
        if let Err(e) = fs::write(&image_path, &resource.data) {
            eprintln!("Warning: Failed to write image {}: {}", name, e);
        } else {
            image_count += 1;
        }
    }

    // Render to Markdown
    let mut options = RenderOptions::default()
        .with_image_dir("images/")
        .with_frontmatter();

    // Apply cleanup options
    if enable_cleanup_aggressive {
        options = options.with_aggressive_cleanup();
        println!("Cleanup: aggressive mode enabled");
    } else if enable_cleanup_minimal {
        options = options.with_minimal_cleanup();
        println!("Cleanup: minimal mode enabled");
    } else if enable_cleanup {
        options = options.with_cleanup();
        println!("Cleanup: default mode enabled");
    }

    let markdown = match render::render_markdown(&document, &options) {
        Ok(md) => md,
        Err(e) => {
            eprintln!("Error: Failed to render markdown: {}", e);
            std::process::exit(1);
        }
    };

    // Write markdown file
    let md_filename = input_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let md_path = output_dir.join(format!("{}.md", md_filename));

    if let Err(e) = fs::write(&md_path, &markdown) {
        eprintln!("Error: Failed to write markdown file: {}", e);
        std::process::exit(1);
    }

    println!("Output directory: {}", output_dir.display());
    println!("  - {}.md ({} bytes)", md_filename, markdown.len());
    println!("  - images/ ({} files)", image_count);
    println!("Done!");
}
