//! unhwp CLI - HWP/HWPX document extraction tool
//!
//! A command-line tool for extracting content from HWP and HWPX files.

mod update;

use clap::{Parser, Subcommand, ValueEnum};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use unhwp::{parse_file, render, RenderOptions, TableFallback};

/// HWP/HWPX document extraction to Markdown, text, and JSON
#[derive(Parser)]
#[command(
    name = "unhwp",
    author = "iyulab",
    version,
    about = "Extract content from HWP/HWPX documents",
    long_about = "unhwp - High-performance HWP/HWPX document extraction tool.\n\n\
                  Converts HWP and HWPX files to Markdown, plain text, or JSON.\n\n\
                  Usage:\n  \
                  unhwp <file>              Extract all formats to output directory\n  \
                  unhwp <file> <output>     Extract to specified directory\n  \
                  unhwp md <file>           Convert to Markdown only"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Input file path (for default conversion)
    #[arg(global = false)]
    input: Option<PathBuf>,

    /// Output directory (for default conversion)
    #[arg(global = false)]
    output: Option<PathBuf>,

    /// Apply text cleanup preset
    #[arg(long, global = true)]
    cleanup: Option<CleanupMode>,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert a document (default command - extracts all formats)
    Convert {
        /// Input file path
        input: PathBuf,

        /// Output directory (default: <filename>_output)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Apply text cleanup
        #[arg(long)]
        cleanup: Option<CleanupMode>,
    },

    /// Convert a document to Markdown
    #[command(visible_alias = "md")]
    Markdown {
        /// Input file path
        input: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Include YAML frontmatter with metadata
        #[arg(short, long)]
        frontmatter: bool,

        /// Table rendering mode
        #[arg(long, default_value = "markdown")]
        table_mode: TableMode,

        /// Apply text cleanup
        #[arg(long)]
        cleanup: Option<CleanupMode>,

        /// Maximum heading level (1-6, default: 4)
        #[arg(long, default_value = "4")]
        max_heading: u8,
    },

    /// Convert a document to plain text
    Text {
        /// Input file path
        input: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Apply text cleanup
        #[arg(long)]
        cleanup: Option<CleanupMode>,
    },

    /// Convert a document to JSON
    Json {
        /// Input file path
        input: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output compact JSON (no indentation)
        #[arg(long)]
        compact: bool,
    },

    /// Show document information and metadata
    Info {
        /// Input file path
        input: PathBuf,
    },

    /// Extract resources (images, media) from a document
    Extract {
        /// Input file path
        input: PathBuf,

        /// Output directory for resources
        #[arg(short, long, default_value = ".")]
        output: PathBuf,
    },

    /// Update unhwp to the latest version
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

/// Table rendering mode
#[derive(Clone, ValueEnum)]
enum TableMode {
    /// Standard Markdown tables
    Markdown,
    /// HTML tables (for complex layouts)
    Html,
    /// Skip tables
    Skip,
}

impl From<TableMode> for TableFallback {
    fn from(mode: TableMode) -> Self {
        match mode {
            TableMode::Markdown => TableFallback::SimplifiedMarkdown,
            TableMode::Html => TableFallback::Html,
            TableMode::Skip => TableFallback::Skip,
        }
    }
}

/// Cleanup mode
#[derive(Clone, ValueEnum)]
enum CleanupMode {
    /// No cleanup
    None,
    /// Minimal cleanup
    Minimal,
    /// Standard cleanup (default)
    Standard,
    /// Aggressive cleanup
    Aggressive,
}

/// Check if we should perform background update check.
/// Skip for update/version commands to avoid redundant checks.
fn should_check_update(cli: &Cli) -> bool {
    !matches!(
        &cli.command,
        Some(Commands::Update { .. }) | Some(Commands::Version)
    )
}

fn main() {
    let cli = Cli::parse();

    // Start background update check (except for update/version commands)
    let update_rx = if should_check_update(&cli) {
        Some(update::check_update_async())
    } else {
        None
    };

    // Run the main command
    let result = run(cli);

    // Check for update result and show notification if available
    if let Some(rx) = update_rx {
        if let Some(update_result) = update::try_get_update_result(&rx) {
            update::print_update_notification(&update_result);
        }
    }

    // Handle errors
    if let Err(e) = result {
        eprintln!("{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    // Handle default command (unhwp <file> [output])
    if cli.command.is_none() {
        if let Some(input) = cli.input {
            return run_convert(&input, cli.output.as_ref(), cli.cleanup);
        } else {
            // No input provided, show help
            use clap::CommandFactory;
            Cli::command().print_help()?;
            return Ok(());
        }
    }

    match cli.command.unwrap() {
        Commands::Convert {
            input,
            output,
            cleanup,
        } => {
            run_convert(&input, output.as_ref(), cleanup)?;
        }

        Commands::Markdown {
            input,
            output,
            frontmatter,
            table_mode,
            cleanup,
            max_heading,
        } => {
            let pb = create_spinner("Parsing document...");

            let doc = parse_file(&input)?;
            pb.set_message("Rendering to Markdown...");

            let mut options = RenderOptions::default()
                .with_table_fallback(table_mode.into())
                .with_max_heading_level(max_heading);

            if frontmatter {
                options = options.with_frontmatter();
            }

            apply_cleanup(&mut options, cleanup);

            let markdown = render::render_markdown(&doc, &options)?;

            pb.finish_and_clear();
            write_output(output.as_ref(), &markdown)?;

            if output.is_some() {
                println!(
                    "{} Converted to Markdown: {}",
                    "✓".green().bold(),
                    output.unwrap().display()
                );
            }
        }

        Commands::Text {
            input,
            output,
            cleanup,
        } => {
            let pb = create_spinner("Parsing document...");

            let doc = parse_file(&input)?;
            pb.set_message("Extracting text...");

            let text = doc.plain_text();

            pb.finish_and_clear();
            write_output(output.as_ref(), &text)?;

            if output.is_some() {
                println!(
                    "{} Converted to text: {}",
                    "✓".green().bold(),
                    output.unwrap().display()
                );
            }

            // Suppress unused variable warning - cleanup is accepted for API consistency
            let _ = cleanup;
        }

        Commands::Json {
            input,
            output,
            compact,
        } => {
            let pb = create_spinner("Parsing document...");

            let doc = parse_file(&input)?;
            pb.set_message("Rendering to JSON...");

            let json = if compact {
                serde_json::to_string(&doc)?
            } else {
                serde_json::to_string_pretty(&doc)?
            };

            pb.finish_and_clear();
            write_output(output.as_ref(), &json)?;

            if output.is_some() {
                println!(
                    "{} Converted to JSON: {}",
                    "✓".green().bold(),
                    output.unwrap().display()
                );
            }
        }

        Commands::Info { input } => {
            let pb = create_spinner("Analyzing document...");

            let format = unhwp::detect_format_from_path(&input)?;
            let doc = parse_file(&input)?;

            pb.finish_and_clear();

            println!("{}", "Document Information".cyan().bold());
            println!("{}", "─".repeat(40));
            println!(
                "{}: {}",
                "File".bold(),
                input.file_name().unwrap_or_default().to_string_lossy()
            );
            println!("{}: {:?}", "Format".bold(), format);
            println!("{}: {}", "Sections".bold(), doc.sections.len());
            println!("{}: {}", "Resources".bold(), doc.resources.len());

            if let Some(ref title) = doc.metadata.title {
                println!("{}: {}", "Title".bold(), title);
            }
            if let Some(ref author) = doc.metadata.author {
                println!("{}: {}", "Author".bold(), author);
            }
            if let Some(ref created) = doc.metadata.created {
                println!("{}: {}", "Created".bold(), created);
            }
            if let Some(ref modified) = doc.metadata.modified {
                println!("{}: {}", "Modified".bold(), modified);
            }
            if doc.metadata.is_distribution {
                println!("{}: {}", "Distribution".bold(), "Yes (DRM protected)");
            }

            let text = doc.plain_text();
            let word_count = text.split_whitespace().count();
            let char_count = text.len();
            println!("\n{}", "Content Statistics".cyan().bold());
            println!("{}", "─".repeat(40));
            println!("{}: {}", "Words".bold(), word_count);
            println!("{}: {}", "Characters".bold(), char_count);
            println!("{}: {}", "Paragraphs".bold(), doc.paragraph_count());
        }

        Commands::Extract { input, output } => {
            let pb = create_spinner("Extracting resources...");

            let doc = parse_file(&input)?;

            fs::create_dir_all(&output)?;

            let mut count = 0;
            for (name, resource) in &doc.resources {
                let path = output.join(name);
                fs::write(&path, &resource.data)?;
                count += 1;
            }

            pb.finish_and_clear();

            if count > 0 {
                println!(
                    "{} Extracted {} resources to {}",
                    "✓".green().bold(),
                    count,
                    output.display()
                );
            } else {
                println!("{} No resources found in document", "!".yellow().bold());
            }
        }

        Commands::Update { check, force } => {
            if let Err(e) = update::run_update(check, force) {
                eprintln!("{}: {}", "Error".red().bold(), e);
                std::process::exit(1);
            }
        }

        Commands::Version => {
            print_version();
        }
    }

    Ok(())
}

/// Run the default convert command - extracts all formats to output directory
fn run_convert(
    input: &PathBuf,
    output: Option<&PathBuf>,
    cleanup: Option<CleanupMode>,
) -> Result<(), Box<dyn std::error::Error>> {
    let pb = create_spinner("Parsing document...");

    // Determine output directory
    let output_dir = match output {
        Some(p) => p.clone(),
        None => {
            let stem = input
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let parent = input.parent().unwrap_or(std::path::Path::new("."));
            parent.join(format!("{}_output", stem))
        }
    };

    // Create output directory
    fs::create_dir_all(&output_dir)?;

    // Parse document
    let doc = parse_file(input)?;

    // Prepare render options
    let mut options = RenderOptions::default().with_frontmatter();

    apply_cleanup(&mut options, cleanup);

    // Generate Markdown
    pb.set_message("Generating Markdown...");
    let markdown = render::render_markdown(&doc, &options)?;
    let md_path = output_dir.join("extract.md");
    fs::write(&md_path, &markdown)?;

    // Generate plain text
    pb.set_message("Generating text...");
    let text = doc.plain_text();
    let txt_path = output_dir.join("extract.txt");
    fs::write(&txt_path, &text)?;

    // Generate JSON
    pb.set_message("Generating JSON...");
    let json = serde_json::to_string_pretty(&doc)?;
    let json_path = output_dir.join("content.json");
    fs::write(&json_path, &json)?;

    // Extract resources
    let mut image_count = 0;
    if !doc.resources.is_empty() {
        pb.set_message("Extracting resources...");
        let images_dir = output_dir.join("images");
        fs::create_dir_all(&images_dir)?;

        for (name, resource) in &doc.resources {
            fs::write(images_dir.join(name), &resource.data)?;
            image_count += 1;
        }
    }

    pb.finish_and_clear();

    // Print summary
    println!("{}", "Conversion Complete".green().bold());
    println!("{}", "─".repeat(40));
    println!("{}: {}", "Output".bold(), output_dir.display());
    println!("  {} extract.md", "✓".green());
    println!("  {} extract.txt", "✓".green());
    println!("  {} content.json", "✓".green());
    if image_count > 0 {
        println!("  {} images/ ({} files)", "✓".green(), image_count);
    }

    // Print statistics
    let word_count = text.split_whitespace().count();
    println!("\n{}", "Statistics".cyan().bold());
    println!("{}", "─".repeat(40));
    println!("{}: {}", "Sections".bold(), doc.sections.len());
    println!("{}: {}", "Words".bold(), word_count);
    println!("{}: {}", "Resources".bold(), image_count);

    Ok(())
}

fn apply_cleanup(options: &mut RenderOptions, cleanup: Option<CleanupMode>) {
    if let Some(mode) = cleanup {
        match mode {
            CleanupMode::None => {}
            CleanupMode::Minimal => *options = options.clone().with_minimal_cleanup(),
            CleanupMode::Standard => *options = options.clone().with_cleanup(),
            CleanupMode::Aggressive => *options = options.clone().with_aggressive_cleanup(),
        }
    }
}

fn print_version() {
    println!("{} {}", "unhwp".green().bold(), env!("CARGO_PKG_VERSION"));
    println!("High-performance HWP/HWPX document extraction to Markdown");
    println!();
    println!("Supported formats: HWP 5.0, HWPX, HWP 3.x");
    println!("Repository: https://github.com/iyulab/unhwp");
}

fn create_spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}

fn write_output(path: Option<&PathBuf>, content: &str) -> Result<(), Box<dyn std::error::Error>> {
    match path {
        Some(p) => {
            fs::write(p, content)?;
        }
        None => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            writeln!(handle, "{}", content)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
