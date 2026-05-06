//! unhwp CLI - HWP/HWPX document extraction tool
//!
//! A command-line tool for extracting content from HWP and HWPX files.

mod update;
mod writer;

use clap::{Parser, Subcommand, ValueEnum};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::{self, Write};
use std::ops::ControlFlow;
use std::path::PathBuf;
use unhwp::{
    parse_file, parse_file_streaming, render, ErrorMode, ParseEvent, RenderOptions,
    SectionStreamOptions, TableFallback,
};
use writer::{MultiFormatWriter, OutputFormat};

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
                  unhwp <file>              Extract Markdown to output directory\n  \
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

/// Arguments for the `convert` subcommand.
#[derive(Parser, Debug)]
struct ConvertArgs {
    /// Input file path
    input: PathBuf,

    /// Output directory (default: <filename>_output)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Apply text cleanup preset
    #[arg(long)]
    cleanup: Option<CleanupMode>,

    /// Output formats to produce (comma-separated: md,txt,json)
    #[arg(long, value_delimiter = ',', default_value = "md")]
    formats: Vec<String>,

    /// Produce all output formats (md + txt + json)
    #[arg(long)]
    all: bool,

    /// Skip image extraction (images are extracted by default)
    #[arg(long)]
    no_images: bool,

    /// Suppress progress output
    #[arg(short, long)]
    quiet: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert a document (default command — Markdown output by default)
    Convert(ConvertArgs),

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
#[derive(Clone, ValueEnum, Debug)]
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
#[derive(Clone, ValueEnum, Debug)]
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
            // Synthesize ConvertArgs with defaults (MD-only, images on)
            let args = ConvertArgs {
                input,
                output: cli.output,
                cleanup: cli.cleanup,
                formats: vec!["md".to_string()],
                all: false,
                no_images: false,
                quiet: false,
            };
            return cmd_convert(args);
        } else {
            // No input provided, show help
            use clap::CommandFactory;
            Cli::command().print_help()?;
            return Ok(());
        }
    }

    match cli.command.unwrap() {
        Commands::Convert(args) => {
            cmd_convert(args)?;
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

/// Run the `convert` subcommand — streaming path.
///
/// Uses `parse_file_streaming` to process sections one at a time, keeping
/// memory bounded for large documents. The full `Document` is never
/// materialized in memory.
fn cmd_convert(args: ConvertArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Determine output directory
    let output_dir = match args.output {
        Some(ref p) => p.clone(),
        None => {
            let stem = args
                .input
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let parent = args.input.parent().unwrap_or(std::path::Path::new("."));
            parent.join(format!("{}_output", stem))
        }
    };

    // Create output directory
    fs::create_dir_all(&output_dir)?;

    // Resolve formats: --all overrides --formats
    let formats: Vec<OutputFormat> = if args.all {
        vec![
            OutputFormat::Markdown,
            OutputFormat::Text,
            OutputFormat::Json,
        ]
    } else {
        let mut fmts = Vec::new();
        for s in &args.formats {
            match OutputFormat::from_str(s) {
                Some(f) => fmts.push(f),
                None => return Err(format!("Unknown format '{}'. Use: md, txt, json", s).into()),
            }
        }
        if fmts.is_empty() {
            fmts.push(OutputFormat::Markdown);
        }
        fmts
    };

    // Build render options
    let mut render_opts = RenderOptions::default()
        .with_frontmatter()
        .with_image_prefix("images/");
    apply_cleanup(&mut render_opts, args.cleanup);

    // Images directory (None = skip)
    let images_dir: Option<PathBuf> = if args.no_images {
        None
    } else {
        Some(output_dir.join("images"))
    };

    // Streaming options: lenient for CLI (best-effort)
    let opts = SectionStreamOptions {
        error_mode: ErrorMode::Lenient,
        extract_resources: images_dir.is_some(),
    };

    // Progress bar — starts at 0; length set in DocumentStart handler
    let pb = if args.quiet {
        None
    } else {
        let pb = ProgressBar::new(0);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} sections {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );
        Some(pb)
    };

    // State accumulated across events
    let mut mfw: Option<MultiFormatWriter> = None;
    let mut summary_result: Option<writer::WriteSummary> = None;
    let mut section_count_total: usize = 0;
    let mut image_count: u32 = 0;

    // Carry errors out of the closure (FnMut cannot return Result)
    let mut cb_err: Option<Box<dyn std::error::Error>> = None;

    let quiet = args.quiet;
    let output_dir_clone = output_dir.clone();
    let images_dir_clone = images_dir.clone();

    // `styles` from DocumentStart has lifetime tied to the streaming call.
    // Pass an empty StyleRegistry to write_section — render_section_standalone
    // uses embedded heading_level from parsed data, not from the registry.
    use unhwp::model::StyleRegistry;
    let empty_styles = StyleRegistry::new();

    parse_file_streaming(&args.input, opts, |event| {
        match event {
            ParseEvent::DocumentStart {
                metadata,
                styles,
                section_count,
            } => {
                section_count_total = section_count;
                // Create MultiFormatWriter with styles from this event
                match MultiFormatWriter::new(
                    &output_dir_clone,
                    &formats,
                    render_opts.clone(),
                    styles,
                ) {
                    Err(e) => {
                        cb_err = Some(e.into());
                        return ControlFlow::Break(());
                    }
                    Ok(mut writer) => {
                        if let Err(e) = writer.write_document_start(metadata, styles) {
                            cb_err = Some(e.into());
                            return ControlFlow::Break(());
                        }
                        mfw = Some(writer);
                    }
                }
                if let Some(ref pb) = pb {
                    if section_count > 0 {
                        pb.set_length(section_count as u64);
                    } else {
                        // Unknown section count — switch to spinner style
                        pb.set_style(
                            ProgressStyle::default_spinner()
                                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                                .template("{spinner:.blue} {msg}")
                                .unwrap(),
                        );
                        pb.enable_steady_tick(std::time::Duration::from_millis(100));
                        pb.set_message("Processing sections...");
                    }
                }
            }

            ParseEvent::SectionParsed(section) => {
                if let Some(ref mut writer) = mfw {
                    // render_section_standalone uses embedded heading_level from
                    // parsed data; passing empty_styles is correct here.
                    if let Err(e) = writer.write_section(section, &empty_styles) {
                        cb_err = Some(e.into());
                        return ControlFlow::Break(());
                    }
                } else if !quiet {
                    eprintln!("Warning: section data arrived before DocumentStart — skipping");
                }
                if let Some(ref pb) = pb {
                    pb.inc(1);
                }
            }

            ParseEvent::SectionFailed { index, error } => {
                if !quiet {
                    eprintln!(
                        "{}: section {} failed: {}",
                        "Warning".yellow().bold(),
                        index,
                        error
                    );
                }
                if let Some(ref pb) = pb {
                    pb.inc(1);
                }
            }

            ParseEvent::DocumentEnd => {
                if let Some(writer) = mfw.take() {
                    match writer.finish() {
                        Err(e) => {
                            cb_err = Some(e.into());
                            return ControlFlow::Break(());
                        }
                        Ok(s) => {
                            summary_result = Some(s);
                        }
                    }
                } else {
                    // cb_err is set from DocumentStart failure; post-closure check handles it
                }
            }

            ParseEvent::ResourceExtracted { name, data } => {
                if let Some(ref dir) = images_dir_clone {
                    let result: io::Result<()> = (|| {
                        std::fs::create_dir_all(dir)?;
                        // Sanitize resource name to prevent path traversal attacks
                        let safe_name = std::path::Path::new(&name)
                            .file_name()
                            .unwrap_or_else(|| std::ffi::OsStr::new(&name));
                        std::fs::write(dir.join(safe_name), &data)?;
                        Ok(())
                    })();
                    match result {
                        Err(e) => {
                            cb_err = Some(e.into());
                            return ControlFlow::Break(());
                        }
                        Ok(()) => {
                            image_count += 1;
                        }
                    }
                }
            }
        }
        ControlFlow::Continue(())
    })?;

    // Propagate any error from inside the closure
    if let Some(e) = cb_err {
        return Err(e);
    }

    if let Some(ref pb) = pb {
        pb.finish_and_clear();
    }

    let summary = summary_result.unwrap_or_default();

    // Print summary
    println!("{}", "Conversion Complete".green().bold());
    println!("{}", "─".repeat(40));
    println!("{}: {}", "Output".bold(), output_dir.display());

    if let Some(ref p) = summary.md_path {
        println!(
            "  {} {}",
            "✓".green(),
            p.file_name().unwrap_or_default().to_string_lossy()
        );
    }
    if let Some(ref p) = summary.txt_path {
        println!(
            "  {} {}",
            "✓".green(),
            p.file_name().unwrap_or_default().to_string_lossy()
        );
    }
    if let Some(ref p) = summary.json_path {
        println!(
            "  {} {}",
            "✓".green(),
            p.file_name().unwrap_or_default().to_string_lossy()
        );
    }
    if image_count > 0 {
        println!("  {} images/ ({} files)", "✓".green(), image_count);
    }

    // Statistics
    println!("\n{}", "Statistics".cyan().bold());
    println!("{}", "─".repeat(40));
    println!("{}: {}", "Sections".bold(), section_count_total);
    println!("{}: {}", "Words".bold(), summary.word_count);
    if image_count > 0 {
        println!("{}: {}", "Resources".bold(), image_count);
    }

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

    #[test]
    fn test_output_format_from_str() {
        assert_eq!(OutputFormat::from_str("md"), Some(OutputFormat::Markdown));
        assert_eq!(
            OutputFormat::from_str("markdown"),
            Some(OutputFormat::Markdown)
        );
        assert_eq!(OutputFormat::from_str("txt"), Some(OutputFormat::Text));
        assert_eq!(OutputFormat::from_str("text"), Some(OutputFormat::Text));
        assert_eq!(OutputFormat::from_str("json"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::from_str("JSON"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::from_str("unknown"), None);
    }
}
