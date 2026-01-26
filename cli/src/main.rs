//! unhwp CLI - Convert HWP/HWPX documents to Markdown
//!
//! A command-line tool for converting Korean HWP/HWPX word processor documents
//! into structured Markdown with extracted assets.

mod update;

use clap::{Parser, Subcommand};
use colored::Colorize;
use std::fs;
use std::path::PathBuf;
use unhwp::{parse_file, render, Document, RenderOptions};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "unhwp")]
#[command(author = "iyulab")]
#[command(version = VERSION)]
#[command(about = "Convert HWP/HWPX documents to Markdown", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Input HWP or HWPX file (for direct conversion without subcommand)
    #[arg(value_name = "INPUT")]
    input: Option<PathBuf>,

    /// Output directory (default: <input>_output)
    #[arg(value_name = "OUTPUT_DIR")]
    output_dir: Option<PathBuf>,

    /// Enable default cleanup for LLM training data
    #[arg(long)]
    cleanup: bool,

    /// Enable minimal cleanup (essential normalization only)
    #[arg(long)]
    cleanup_minimal: bool,

    /// Enable aggressive cleanup (maximum purification)
    #[arg(long)]
    cleanup_aggressive: bool,

    /// Check for updates before extraction, update if available
    #[arg(long)]
    update: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert HWP/HWPX file to Markdown
    Convert {
        /// Input HWP or HWPX file
        input: PathBuf,

        /// Output directory (default: <input>_output)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Enable default cleanup for LLM training data
        #[arg(long)]
        cleanup: bool,

        /// Enable minimal cleanup (essential normalization only)
        #[arg(long)]
        cleanup_minimal: bool,

        /// Enable aggressive cleanup (maximum purification)
        #[arg(long)]
        cleanup_aggressive: bool,
    },

    /// Check for updates and self-update if available
    Update {
        /// Check only, don't install
        #[arg(long)]
        check: bool,

        /// Force update even if on latest version
        #[arg(long)]
        force: bool,
    },

    /// Show version information
    Version,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Convert {
            input,
            output,
            cleanup,
            cleanup_minimal,
            cleanup_aggressive,
        }) => {
            // Start async update check
            let update_rx = update::check_update_async();

            convert_document(&input, output, cleanup, cleanup_minimal, cleanup_aggressive);

            // Check for update result after conversion completes
            if let Some(result) = update::try_get_update_result(&update_rx) {
                update::print_update_notification(&result);
            }
        }
        Some(Commands::Update { check, force }) => {
            if let Err(e) = update::run_update(check, force) {
                eprintln!("{} {}", "Error:".red().bold(), e);
                std::process::exit(1);
            }
        }
        Some(Commands::Version) => {
            print_version();
        }
        None => {
            // Handle direct invocation without subcommand
            if let Some(input) = cli.input {
                // If --update flag is set, update first then extract
                if cli.update {
                    println!("{}", "Checking for updates before extraction...".cyan());
                    if let Err(e) = update::run_update(false, false) {
                        eprintln!("{} Update check failed: {}", "Warning:".yellow().bold(), e);
                    }
                    println!();
                }

                // Start async update check (only if not already updating)
                let update_rx = if !cli.update {
                    Some(update::check_update_async())
                } else {
                    None
                };

                convert_document(
                    &input,
                    cli.output_dir,
                    cli.cleanup,
                    cli.cleanup_minimal,
                    cli.cleanup_aggressive,
                );

                // Check for update result after conversion completes
                if let Some(rx) = update_rx {
                    if let Some(result) = update::try_get_update_result(&rx) {
                        update::print_update_notification(&result);
                    }
                }
            } else {
                // No input provided, show help
                eprintln!("{}", "Usage: unhwp <INPUT> [OUTPUT_DIR] [OPTIONS]".yellow());
                eprintln!();
                eprintln!("Try 'unhwp --help' for more information.");
                std::process::exit(1);
            }
        }
    }
}

fn print_version() {
    println!("{} {}", "unhwp".green().bold(), VERSION);
    println!("A high-performance HWP/HWPX to Markdown converter");
    println!();
    println!("Repository: https://github.com/iyulab/unhwp");
}

fn convert_document(
    input: &PathBuf,
    output: Option<PathBuf>,
    cleanup: bool,
    cleanup_minimal: bool,
    cleanup_aggressive: bool,
) {
    if !input.exists() {
        eprintln!(
            "{} Input file not found: {}",
            "Error:".red().bold(),
            input.display()
        );
        std::process::exit(1);
    }

    // Determine output directory
    let output_dir = output.unwrap_or_else(|| {
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        input
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join(format!("{}_output", stem))
    });

    // Create output directory
    if let Err(e) = fs::create_dir_all(&output_dir) {
        eprintln!(
            "{} Failed to create output directory: {}",
            "Error:".red().bold(),
            e
        );
        std::process::exit(1);
    }

    // Create images subdirectory
    let images_dir = output_dir.join("images");
    if let Err(e) = fs::create_dir_all(&images_dir) {
        eprintln!(
            "{} Failed to create images directory: {}",
            "Error:".red().bold(),
            e
        );
        std::process::exit(1);
    }

    println!("{} {}", "Parsing:".cyan().bold(), input.display());

    // Parse the document
    let document = match parse_file(input) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("{} Failed to parse document: {}", "Error:".red().bold(), e);
            std::process::exit(1);
        }
    };

    // Extract images
    let mut image_count = 0;
    for (name, resource) in &document.resources {
        let image_path = images_dir.join(name);
        if let Err(e) = fs::write(&image_path, &resource.data) {
            eprintln!(
                "{} Failed to write image {}: {}",
                "Warning:".yellow().bold(),
                name,
                e
            );
        } else {
            image_count += 1;
        }
    }

    // Render options
    let mut options = RenderOptions::default()
        .with_image_dir("images/")
        .with_image_prefix("images/")
        .with_frontmatter();

    // Apply cleanup options
    if cleanup_aggressive {
        options = options.with_aggressive_cleanup();
        println!("{} aggressive mode enabled", "Cleanup:".cyan().bold());
    } else if cleanup_minimal {
        options = options.with_minimal_cleanup();
        println!("{} minimal mode enabled", "Cleanup:".cyan().bold());
    } else if cleanup {
        options = options.with_cleanup();
        println!("{} default mode enabled", "Cleanup:".cyan().bold());
    }

    // Generate outputs
    let markdown = match render::render_markdown(&document, &options) {
        Ok(md) => md,
        Err(e) => {
            eprintln!("{} Failed to render markdown: {}", "Error:".red().bold(), e);
            std::process::exit(1);
        }
    };

    let plain_text = extract_plain_text(&document);
    let content_json = match serde_json::to_string_pretty(&document) {
        Ok(json) => json,
        Err(e) => {
            eprintln!(
                "{} Failed to serialize document to JSON: {}",
                "Warning:".yellow().bold(),
                e
            );
            String::new()
        }
    };

    // Write extract.md
    let md_path = output_dir.join("extract.md");
    if let Err(e) = fs::write(&md_path, &markdown) {
        eprintln!(
            "{} Failed to write extract.md: {}",
            "Error:".red().bold(),
            e
        );
        std::process::exit(1);
    }

    // Write extract.txt
    let txt_path = output_dir.join("extract.txt");
    if let Err(e) = fs::write(&txt_path, &plain_text) {
        eprintln!(
            "{} Failed to write extract.txt: {}",
            "Error:".red().bold(),
            e
        );
        std::process::exit(1);
    }

    // Write content.json
    if !content_json.is_empty() {
        let json_path = output_dir.join("content.json");
        if let Err(e) = fs::write(&json_path, &content_json) {
            eprintln!(
                "{} Failed to write content.json: {}",
                "Warning:".yellow().bold(),
                e
            );
        }
    }

    println!();
    println!("{} {}", "Output:".green().bold(), output_dir.display());
    println!("  {} extract.md ({} bytes)", "→".cyan(), markdown.len());
    println!("  {} extract.txt ({} bytes)", "→".cyan(), plain_text.len());
    if !content_json.is_empty() {
        println!(
            "  {} content.json ({} bytes)",
            "→".cyan(),
            content_json.len()
        );
    }
    println!("  {} images/ ({} files)", "→".cyan(), image_count);
    println!("{}", "Done!".green().bold());
}

/// Extract plain text from document (no formatting)
fn extract_plain_text(document: &Document) -> String {
    use unhwp::model::{Block, InlineContent};

    fn extract_paragraph_text(paragraph: &unhwp::model::Paragraph) -> String {
        let mut text = String::new();
        for inline in &paragraph.content {
            match inline {
                InlineContent::Text(run) => text.push_str(&run.text),
                InlineContent::LineBreak => text.push('\n'),
                InlineContent::Link {
                    text: link_text, ..
                } => text.push_str(link_text),
                InlineContent::Footnote(note) => {
                    text.push('[');
                    text.push_str(note);
                    text.push(']');
                }
                _ => {}
            }
        }
        text
    }

    let mut text = String::new();

    for section in &document.sections {
        for block in &section.content {
            match block {
                Block::Paragraph(paragraph) => {
                    text.push_str(&extract_paragraph_text(paragraph));
                    text.push('\n');
                }
                Block::Table(table) => {
                    for row in &table.rows {
                        for cell in &row.cells {
                            for para in &cell.content {
                                text.push_str(&extract_paragraph_text(para));
                                text.push('\t');
                            }
                        }
                        text.push('\n');
                    }
                }
            }
        }
        text.push('\n');
    }

    text.trim().to_string()
}
