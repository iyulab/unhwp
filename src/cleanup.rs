//! # Cleanup Pipeline
//!
//! A 4-stage pipeline for purifying markdown output for LLM training data.
//!
//! ## Pipeline Stages
//!
//! 1. **Stage 1: String Normalization** - Unicode NFC normalization, bullet mapping, control character removal
//! 2. **Stage 2: Line-Based Cleaning** - Page numbers, TOC, headers/footers removal
//! 3. **Stage 3: Structural Filtering** - Empty tags, orphan lines, captions
//! 4. **Stage 4: Final Normalization** - Consecutive newlines, whitespace cleanup

use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

/// Cleanup configuration options
#[derive(Debug, Clone)]
pub struct CleanupOptions {
    /// Enable Stage 1: String normalization
    pub normalize_strings: bool,
    /// Enable Stage 2: Line-based cleaning
    pub clean_lines: bool,
    /// Enable Stage 3: Structural filtering
    pub filter_structure: bool,
    /// Enable Stage 4: Final normalization
    pub final_normalize: bool,
    /// Remove PUA (Private Use Area) characters
    pub remove_pua: bool,
    /// Remove HWP placeholders like [EQ], <TABLE>
    pub remove_hwp_placeholders: bool,
    /// Threshold for header/footer detection (0.0-1.0)
    pub header_footer_threshold: f64,
    /// Maximum line length to consider for header/footer detection
    pub max_header_footer_length: usize,
    /// Enable heuristic mojibake detection
    pub detect_mojibake: bool,
    /// Preserve YAML frontmatter during structural filtering
    pub preserve_frontmatter: bool,
}

impl Default for CleanupOptions {
    fn default() -> Self {
        Self {
            normalize_strings: true,
            clean_lines: true,
            filter_structure: true,
            final_normalize: true,
            remove_pua: true,
            remove_hwp_placeholders: true,
            header_footer_threshold: 0.8,
            max_header_footer_length: 100,
            detect_mojibake: true,
            preserve_frontmatter: true,
        }
    }
}

impl CleanupOptions {
    /// Creates options for minimal cleanup (only essential normalization)
    pub fn minimal() -> Self {
        Self {
            normalize_strings: true,
            clean_lines: false,
            filter_structure: false,
            final_normalize: true,
            remove_pua: true,
            remove_hwp_placeholders: true,
            header_footer_threshold: 0.8,
            max_header_footer_length: 100,
            detect_mojibake: true,
            preserve_frontmatter: true,
        }
    }

    /// Creates options for aggressive cleanup (maximum purification)
    pub fn aggressive() -> Self {
        Self {
            normalize_strings: true,
            clean_lines: true,
            filter_structure: true,
            final_normalize: true,
            remove_pua: true,
            remove_hwp_placeholders: true,
            header_footer_threshold: 0.7, // Lower threshold = more aggressive
            max_header_footer_length: 150,
            detect_mojibake: true,
            preserve_frontmatter: true,
        }
    }
}

// ============================================================================
// Stage 1: String Normalization
// ============================================================================

/// Bullet character mapping table
const BULLET_MAPPINGS: &[(char, &str)] = &[
    // Filled bullets -> standard list marker
    ('●', "- "),
    ('■', "- "),
    ('◆', "- "),
    ('▶', "- "),
    ('►', "- "),
    ('➢', "- "),
    ('➤', "- "),
    ('•', "- "),
    ('·', "- "), // Middle dot (common in Korean docs)
    // Hollow bullets -> standard list marker (could use indentation)
    ('○', "- "),
    ('□', "- "),
    ('◇', "- "),
    ('▷', "- "),
    // Arrows
    ('→', "- "),
    ('⇒', "- "),
    ('➔', "- "),
    // Special markers
    ('※', "> ※ "), // Note marker -> blockquote
    ('★', "- "),
    ('☆', "- "),
    ('✓', "- [x] "), // Checkmark -> task list
    ('✔', "- [x] "),
    ('✗', "- [ ] "),
    ('✘', "- [ ] "),
];

/// Stage 1: Normalize raw string
///
/// - Unicode NFC normalization
/// - Bullet character mapping
/// - Control character removal
/// - PUA character handling
/// - Fullwidth space normalization
pub fn stage1_normalize_string(input: &str, options: &CleanupOptions) -> String {
    let mut result = String::with_capacity(input.len());

    // Unicode NFC normalization
    for c in input.nfc() {
        // Check control characters
        if is_control_char(c) {
            continue;
        }

        // Check PUA characters
        if options.remove_pua && is_pua_char(c) {
            continue;
        }

        // Map bullet characters
        if let Some(replacement) = get_bullet_replacement(c) {
            result.push_str(replacement);
            continue;
        }

        // Normalize fullwidth characters
        if let Some(normalized) = normalize_fullwidth(c) {
            result.push(normalized);
            continue;
        }

        result.push(c);
    }

    result
}

/// Check if character is a control character that should be removed
fn is_control_char(c: char) -> bool {
    matches!(
        c,
        '\0'        // Null
        | '\x0B'    // Vertical Tab
        | '\x0C'    // Form Feed
        | '\u{FEFF}' // BOM
        | '\u{FFFD}' // Replacement character
        | '\u{00AD}' // Soft hyphen
    )
}

/// Check if character is in Private Use Area
fn is_pua_char(c: char) -> bool {
    let code = c as u32;
    // Basic PUA: U+E000 to U+F8FF
    // Supplementary PUA-A: U+F0000 to U+FFFFD
    // Supplementary PUA-B: U+100000 to U+10FFFD
    (0xE000..=0xF8FF).contains(&code)
        || (0xF0000..=0xFFFFD).contains(&code)
        || (0x100000..=0x10FFFD).contains(&code)
}

/// Get bullet replacement if applicable
fn get_bullet_replacement(c: char) -> Option<&'static str> {
    BULLET_MAPPINGS
        .iter()
        .find(|(bullet, _)| *bullet == c)
        .map(|(_, replacement)| *replacement)
}

/// Normalize fullwidth characters to ASCII equivalents
fn normalize_fullwidth(c: char) -> Option<char> {
    match c {
        '\u{3000}' => Some(' '), // Ideographic space -> regular space
        '\u{FF01}'..='\u{FF5E}' => {
            // Fullwidth ASCII variants (！to ～)
            let offset = c as u32 - 0xFF01;
            char::from_u32(0x21 + offset) // Map to ASCII ! to ~
        }
        _ => None,
    }
}

// ============================================================================
// Mojibake Detection (Heuristic-based, minimal hardcoding)
// ============================================================================

/// Detects and removes trailing mojibake from lines.
///
/// Mojibake typically appears when:
/// 1. CJK characters appear at the end of ASCII/URL lines
/// 2. Isolated rare CJK characters follow common text
/// 3. Characters from incompatible encoding ranges mix
/// 4. Mixed CJK + ASCII garbage at line endings (e.g., "汫h")
///
/// This uses Unicode category analysis rather than character lists.
pub fn clean_line_trailing_mojibake(line: &str) -> String {
    if line.is_empty() {
        return line.to_string();
    }

    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();

    // First pass: detect if line ends with suspicious mixed garbage
    // Pattern: URL or ASCII content followed by isolated CJK + optional ASCII
    if let Some(end_pos) = detect_trailing_garbage(&chars) {
        return chars[..end_pos].iter().collect();
    }

    // Second pass: scan from end for suspicious trailing characters
    let mut end_pos = len;

    for i in (0..len).rev() {
        let c = chars[i];

        // If we hit normal content, stop
        if is_normal_content_char(c) {
            break;
        }

        // Check if this is a suspicious trailing character
        if is_suspicious_trailing_char(c, &chars, i) {
            end_pos = i;
        } else {
            break;
        }
    }

    if end_pos < len {
        chars[..end_pos].iter().collect()
    } else {
        line.to_string()
    }
}

/// Detects trailing garbage pattern: content followed by isolated CJK + optional ASCII
/// Returns the position where garbage starts, or None if no garbage detected
fn detect_trailing_garbage(chars: &[char]) -> Option<usize> {
    let len = chars.len();
    if len < 3 {
        return None;
    }

    // Check if line has meaningful content (Korean, URL, or ASCII text)
    let has_korean = chars.iter().any(|&c| is_hangul(c));
    let text: String = chars.iter().collect();
    let has_url = text.contains("://") || text.contains("http");
    let has_content = has_korean || has_url || chars.iter().any(|c| c.is_ascii_alphanumeric());

    if !has_content {
        return None;
    }

    // Scan from end: look for pattern [CJK][optional short ASCII] at the end
    let mut garbage_start = None;
    let mut cjk_count = 0;
    let mut trailing_ascii_count = 0;

    for i in (0..len).rev() {
        let c = chars[i];

        if is_cjk_ideograph(c) {
            cjk_count += 1;
            garbage_start = Some(i);
        } else if cjk_count > 0 {
            // We've passed the trailing CJK chars
            // Check if the char before CJK is normal content
            if is_normal_content_char(c) || c.is_ascii_punctuation() {
                // Isolated CJK (1-3 chars) at the end of content is suspicious
                if cjk_count <= 3 {
                    return garbage_start;
                }
            }
            break;
        } else if c.is_ascii_alphabetic() && trailing_ascii_count < 3 {
            // Short trailing ASCII might be part of garbage (e.g., "汫h")
            // Continue scanning to check for CJK before it
            trailing_ascii_count += 1;
        } else {
            // No suspicious pattern at the end
            break;
        }
    }

    None
}

/// Check if character is Korean Hangul
fn is_hangul(c: char) -> bool {
    let code = c as u32;
    (0xAC00..=0xD7AF).contains(&code)  // Hangul Syllables
        || (0x1100..=0x11FF).contains(&code)  // Hangul Jamo
        || (0x3130..=0x318F).contains(&code) // Hangul Compatibility Jamo
}

/// Determines if a character is "normal" content (not potential mojibake)
fn is_normal_content_char(c: char) -> bool {
    // ASCII printable
    if c.is_ascii() {
        return true;
    }

    // Korean Hangul (common in HWP documents)
    let code = c as u32;
    if (0xAC00..=0xD7AF).contains(&code) // Hangul Syllables
        || (0x1100..=0x11FF).contains(&code) // Hangul Jamo
        || (0x3130..=0x318F).contains(&code)
    // Hangul Compatibility Jamo
    {
        return true;
    }

    // Common punctuation and symbols used in Korean documents
    if (0x2000..=0x206F).contains(&code) // General Punctuation
        || (0x3000..=0x303F).contains(&code)
    // CJK Symbols and Punctuation
    {
        return true;
    }

    false
}

/// Determines if a character at a given position is suspicious as a trailing character
fn is_suspicious_trailing_char(c: char, chars: &[char], pos: usize) -> bool {
    let code = c as u32;

    // CJK Unified Ideographs - suspicious at line endings after non-CJK content
    if (0x4E00..=0x9FFF).contains(&code) {
        // Check context: is there ASCII/URL content before this?
        let has_ascii_before = chars[..pos].iter().any(|&ch| ch.is_ascii_alphanumeric());
        let has_url_pattern = chars[..pos].iter().collect::<String>().contains("://");

        if has_ascii_before || has_url_pattern {
            // Check if this CJK char is isolated (not part of Chinese/Japanese text)
            let cjk_neighbors = count_cjk_neighbors(chars, pos);
            if cjk_neighbors < 2 {
                return true; // Isolated CJK after ASCII is suspicious
            }
        }
    }

    // CJK Extension ranges - rarely used in normal text
    if (0x3400..=0x4DBF).contains(&code)   // CJK Extension A
        || (0x20000..=0x2A6DF).contains(&code) // CJK Extension B
        || (0x2A700..=0x2B73F).contains(&code)
    // CJK Extension C
    {
        return true;
    }

    // Unassigned or rarely used ranges
    if (0xFFF0..=0xFFFF).contains(&code)
    // Specials
    {
        return true;
    }

    false
}

/// Counts neighboring CJK characters around a position
fn count_cjk_neighbors(chars: &[char], pos: usize) -> usize {
    let mut count = 0;

    // Check 3 chars before
    for c in chars.iter().take(pos).skip(pos.saturating_sub(3)) {
        if is_cjk_ideograph(*c) {
            count += 1;
        }
    }

    // Check 3 chars after
    for c in chars.iter().skip(pos + 1).take(3) {
        if is_cjk_ideograph(*c) {
            count += 1;
        }
    }

    count
}

/// Check if character is a CJK ideograph
fn is_cjk_ideograph(c: char) -> bool {
    let code = c as u32;
    (0x4E00..=0x9FFF).contains(&code)
        || (0x3400..=0x4DBF).contains(&code)
        || (0x20000..=0x2A6DF).contains(&code)
}

/// Check if a line consists entirely of likely mojibake (isolated CJK without context)
///
/// Mojibake lines typically:
/// 1. Are short (1-5 characters)
/// 2. Contain only CJK ideographs with no Korean/punctuation
/// 3. Have no meaningful word structure
fn is_entirely_mojibake(line: &str) -> bool {
    let trimmed = line.trim();
    let char_count = trimmed.chars().count();

    // Empty or too long to be mojibake
    if char_count == 0 || char_count > 5 {
        return false;
    }

    // Must be all CJK ideographs (not Hangul, not punctuation)
    let all_cjk_ideographs = trimmed.chars().all(is_cjk_ideograph);
    if !all_cjk_ideographs {
        return false;
    }

    // If it contains common CJK words/patterns, it might be legitimate
    // For now, short sequences of pure CJK ideographs without Hangul context
    // in a Korean document are suspicious
    true
}

// ============================================================================
// Stage 2: Line-Based Cleaning
// ============================================================================

// Regex patterns (compiled once using LazyLock)
static RE_PAGE_HYPHEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[-\[\(]\s*\d+\s*[-\]\)]\s*$").unwrap());

static RE_PAGE_RATIO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*(?:Page\s*)?\d+\s*(?:/|of)\s*\d+\s*$").unwrap());

static RE_PAGE_KOREAN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*(?:-\s*)?\d+\s*(?:쪽|페이지|Page)(?:\s*-)?(?:\s*/\s*\d+(?:쪽|페이지|Page)?)?\s*$",
    )
    .unwrap()
});

static RE_TOC_DOTS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*\.{3,}[\.\s]*\d+\s*$").unwrap());

static RE_SEPARATOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[-=*_]{3,}$").unwrap());

static RE_HWP_PLACEHOLDER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\[(?:EQ|수식|표|TABLE|그림|IMAGE)\]\s*$").unwrap());

static RE_EMPTY_BRACKETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\(\[\{<]\s*[\)\]\}>]$").unwrap());

// Reserved for future caption detection feature
#[allow(dead_code)]
static RE_FIGURE_CAPTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\[?(?:그림|Figure|Fig)\.?\s*\d+[^\]]*\]?\s*$").unwrap());

#[allow(dead_code)]
static RE_TABLE_CAPTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\[?(?:표|Table)\.?\s*\d+[^\]]*\]?\s*$").unwrap());

/// Stage 2: Line-based cleaning
///
/// - Remove page numbers
/// - Remove TOC lines with dots
/// - Remove HWP placeholders
/// - Remove repeated headers/footers
/// - Remove empty separators
pub fn stage2_clean_lines(input: &str, options: &CleanupOptions) -> String {
    let lines: Vec<&str> = input.lines().collect();

    // Analyze line frequencies for header/footer detection
    let frequent_lines = if options.clean_lines {
        analyze_line_frequencies(&lines, options)
    } else {
        HashMap::new()
    };

    let mut result = Vec::with_capacity(lines.len());

    for line in lines {
        let trimmed = line.trim();

        // Skip empty lines (will be normalized in Stage 4)
        if trimmed.is_empty() {
            result.push(line.to_string());
            continue;
        }

        // Check page number patterns
        if is_page_number(trimmed) {
            result.push(String::new());
            continue;
        }

        // Check TOC pattern
        if RE_TOC_DOTS.is_match(trimmed) {
            result.push(String::new());
            continue;
        }

        // Check HWP placeholders
        if options.remove_hwp_placeholders && RE_HWP_PLACEHOLDER.is_match(trimmed) {
            result.push(String::new());
            continue;
        }

        // Check empty brackets (OCR noise)
        if RE_EMPTY_BRACKETS.is_match(trimmed) {
            result.push(String::new());
            continue;
        }

        // Check separator lines (excessive use)
        if RE_SEPARATOR.is_match(trimmed) {
            // Keep one separator but mark for potential removal if excessive
            result.push("---".to_string());
            continue;
        }

        // Check for repeated headers/footers
        if frequent_lines.contains_key(trimmed) {
            result.push(String::new());
            continue;
        }

        // Check for lines that are entirely mojibake (isolated CJK garbage)
        if options.detect_mojibake && is_entirely_mojibake(trimmed) {
            result.push(String::new());
            continue;
        }

        // Apply mojibake detection to clean trailing garbage
        let cleaned_line = if options.detect_mojibake {
            clean_line_trailing_mojibake(line)
        } else {
            line.to_string()
        };

        result.push(cleaned_line);
    }

    result.join("\n")
}

/// Check if line matches page number patterns
fn is_page_number(line: &str) -> bool {
    RE_PAGE_HYPHEN.is_match(line) || RE_PAGE_RATIO.is_match(line) || RE_PAGE_KOREAN.is_match(line)
}

/// Analyze line frequencies to detect headers/footers
fn analyze_line_frequencies<'a>(
    lines: &[&'a str],
    options: &CleanupOptions,
) -> HashMap<&'a str, usize> {
    let mut freq: HashMap<&str, usize> = HashMap::new();

    // Count occurrences
    for line in lines {
        let trimmed = line.trim();
        if !trimmed.is_empty() && trimmed.len() <= options.max_header_footer_length {
            *freq.entry(trimmed).or_insert(0) += 1;
        }
    }

    // Estimate page count (rough: ~40 lines per page)
    let estimated_pages = (lines.len() as f64 / 40.0).ceil() as usize;
    let threshold = (estimated_pages as f64 * options.header_footer_threshold) as usize;

    // Filter to only frequent lines
    freq.retain(|_, count| *count >= threshold.max(3));

    freq
}

// ============================================================================
// Stage 3: Structural Filtering
// ============================================================================

/// Stage 3: Structural filtering using markdown parser
///
/// - Remove empty emphasis tags
/// - Clean orphan captions
/// - Filter empty table structures
/// - Preserve YAML frontmatter (pulldown-cmark doesn't handle it natively)
pub fn stage3_filter_structure(input: &str, options: &CleanupOptions) -> String {
    use pulldown_cmark::{Event, Options, Parser, Tag};

    // Extract and preserve YAML frontmatter if enabled
    let (frontmatter, content) = if options.preserve_frontmatter {
        extract_yaml_frontmatter(input)
    } else {
        (None, input)
    };

    let parser_options = Options::empty();
    let parser = Parser::new_ext(content, parser_options);

    let mut events: Vec<Event> = Vec::new();
    let mut tag_stack: Vec<(Tag, usize)> = Vec::new();

    for event in parser {
        match &event {
            Event::Start(tag) => {
                tag_stack.push((tag.clone(), events.len()));
                events.push(event);
            }
            Event::End(tag_end) => {
                if let Some((start_tag, start_idx)) = tag_stack.pop() {
                    // Check if this is an empty emphasis tag
                    if is_empty_emphasis(&start_tag, tag_end, &events, start_idx) {
                        // Remove the start tag
                        events.truncate(start_idx);
                    } else {
                        events.push(event);
                    }
                } else {
                    events.push(event);
                }
            }
            _ => {
                events.push(event);
            }
        }
    }

    // Convert back to markdown
    let mut output = String::new();
    if pulldown_cmark_to_cmark::cmark(events.into_iter(), &mut output).is_err() {
        // Fallback to original if conversion fails
        return input.to_string();
    }

    // Re-add frontmatter if it was extracted
    if let Some(fm) = frontmatter {
        format!("{}\n{}", fm, output.trim_start())
    } else {
        output
    }
}

/// Extract YAML frontmatter from markdown content.
/// Returns (frontmatter_string, remaining_content).
/// Frontmatter must start at the very beginning with `---` and end with `---`.
fn extract_yaml_frontmatter(input: &str) -> (Option<String>, &str) {
    let trimmed = input.trim_start();

    // Must start with ---
    if !trimmed.starts_with("---") {
        return (None, input);
    }

    // Find the end of the first line (the opening ---)
    let after_opening = match trimmed.strip_prefix("---") {
        Some(rest) => rest.trim_start_matches([' ', '\t']),
        None => return (None, input),
    };

    // The opening --- should be followed by a newline
    if !after_opening.starts_with('\n') && !after_opening.starts_with("\r\n") {
        return (None, input);
    }

    // Skip the newline
    let content_start = if let Some(stripped) = after_opening.strip_prefix("\r\n") {
        stripped
    } else if let Some(stripped) = after_opening.strip_prefix('\n') {
        stripped
    } else {
        return (None, input);
    };

    // Find the closing ---
    // Look for a line that is exactly --- (with optional trailing whitespace)
    let mut pos = 0;
    let mut found_end = false;
    let mut end_pos = 0;

    for line in content_start.lines() {
        let line_trimmed = line.trim();
        if line_trimmed == "---" || line_trimmed == "..." {
            found_end = true;
            end_pos = pos + line.len();
            break;
        }
        pos += line.len() + 1; // +1 for newline
    }

    if !found_end {
        return (None, input);
    }

    // Calculate positions in original input
    let opening_offset = input.len() - trimmed.len();
    let frontmatter_content = &content_start[..pos];
    let frontmatter_str = format!("---\n{}---", frontmatter_content);

    // Remaining content is after the closing ---
    let remaining_start = content_start.get(end_pos..).unwrap_or("");
    let remaining = remaining_start.trim_start_matches(['\n', '\r']);

    // If remaining is empty but there's leftover after the closing ---, use original slice
    let final_remaining = if remaining.is_empty() {
        let full_fm_len = opening_offset
            + 3
            + (content_start.as_ptr() as usize - trimmed.as_ptr() as usize - 3)
            + end_pos;
        input
            .get(full_fm_len..)
            .unwrap_or("")
            .trim_start_matches(['\n', '\r'])
    } else {
        remaining
    };

    (Some(frontmatter_str), final_remaining)
}

/// Check if emphasis tag pair is empty
fn is_empty_emphasis(
    start_tag: &pulldown_cmark::Tag,
    end_tag: &pulldown_cmark::TagEnd,
    events: &[pulldown_cmark::Event],
    start_idx: usize,
) -> bool {
    use pulldown_cmark::{Tag, TagEnd};

    // Only check emphasis tags
    let is_emphasis = matches!(
        (start_tag, end_tag),
        (Tag::Emphasis, TagEnd::Emphasis)
            | (Tag::Strong, TagEnd::Strong)
            | (Tag::Strikethrough, TagEnd::Strikethrough)
    );

    if !is_emphasis {
        return false;
    }

    // Check if there's only whitespace between start and end
    let content_events = &events[start_idx + 1..];
    for event in content_events {
        if let pulldown_cmark::Event::Text(text) = event {
            if !text.trim().is_empty() {
                return false;
            }
        }
    }

    true
}

// ============================================================================
// Stage 4: Final Normalization
// ============================================================================

static RE_MULTIPLE_NEWLINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());

static RE_MULTIPLE_SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]+").unwrap());

static RE_TRAILING_WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]+$").unwrap());

/// Stage 4: Final normalization
///
/// - Reduce consecutive newlines (3+ -> 2)
/// - Normalize multiple spaces
/// - Remove orphan lines
/// - Clean trailing whitespace
pub fn stage4_final_normalize(input: &str, _options: &CleanupOptions) -> String {
    let mut result = input.to_string();

    // Reduce consecutive newlines
    result = RE_MULTIPLE_NEWLINES
        .replace_all(&result, "\n\n")
        .to_string();

    // Process line by line
    let lines: Vec<&str> = result.lines().collect();
    let mut cleaned_lines: Vec<String> = Vec::with_capacity(lines.len());

    for line in lines {
        // Skip orphan lines (< 3 chars, not markers, not sentence endings)
        let trimmed = line.trim();
        if is_orphan_line(trimmed) {
            continue;
        }

        // Normalize multiple spaces within line (but preserve leading indentation for code)
        let cleaned = if trimmed.starts_with("    ") || trimmed.starts_with('\t') {
            // Preserve code block indentation
            let leading = &line[..line.len() - line.trim_start().len()];
            format!(
                "{}{}",
                leading,
                RE_MULTIPLE_SPACES.replace_all(trimmed, " ")
            )
        } else {
            RE_MULTIPLE_SPACES.replace_all(trimmed, " ").to_string()
        };

        // Remove trailing whitespace
        let final_line = RE_TRAILING_WHITESPACE.replace(&cleaned, "").to_string();

        cleaned_lines.push(final_line);
    }

    result = cleaned_lines.join("\n");

    // Final pass: reduce any remaining consecutive newlines
    RE_MULTIPLE_NEWLINES
        .replace_all(&result, "\n\n")
        .to_string()
}

/// Check if line is an orphan (meaningless fragment)
fn is_orphan_line(line: &str) -> bool {
    let len = line.chars().count();

    // Empty lines handled separately
    if len == 0 {
        return false;
    }

    // Preserve horizontal rules and YAML frontmatter delimiters
    // These are meaningful markdown structural elements
    if line == "---" || line == "..." || line == "***" || line == "___" {
        return false;
    }

    // Lines with 2 or more meaningful chars are considered meaningful
    // (threshold lowered to 2 to handle short Korean words like "첫번째")
    if len >= 2 {
        // But still check if it's only punctuation
        if !line
            .chars()
            .all(|c| c.is_ascii_punctuation() || c.is_whitespace())
        {
            return false;
        }
    }

    // Not a list marker
    if line.starts_with("- ")
        || line.starts_with("* ")
        || line.starts_with("+ ")
        || line.chars().next().is_some_and(|c| c.is_ascii_digit())
    {
        return false;
    }

    // Not a header
    if line.starts_with('#') {
        return false;
    }

    // Single character checks
    if len == 1 {
        let c = line.chars().next().unwrap();
        // Single punctuation is an orphan (OCR noise)
        if c.is_ascii_punctuation() || c == '.' || c == ',' || c == '。' || c == '、' {
            return true;
        }
        // Single non-punctuation char might be meaningful (e.g., "A", "가")
        return false;
    }

    // Short fragments that only contain punctuation are orphans
    if line
        .chars()
        .all(|c| c.is_ascii_punctuation() || c.is_whitespace())
    {
        return true;
    }

    // If we get here, the line is 2+ chars with non-punctuation content - keep it
    false
}

// ============================================================================
// Main Pipeline
// ============================================================================

/// Run the full cleanup pipeline on markdown content
///
/// # Example
///
/// ```
/// use unhwp::cleanup::{cleanup, CleanupOptions};
///
/// let dirty_markdown = "Some text\n\n\n\n- 15 -\n\nMore text●item";
/// let options = CleanupOptions::default();
/// let clean = cleanup(dirty_markdown, &options);
/// ```
pub fn cleanup(input: &str, options: &CleanupOptions) -> String {
    let mut result = input.to_string();

    // Stage 1: String normalization
    if options.normalize_strings {
        result = stage1_normalize_string(&result, options);
    }

    // Stage 2: Line-based cleaning
    if options.clean_lines {
        result = stage2_clean_lines(&result, options);
    }

    // Stage 3: Structural filtering
    if options.filter_structure {
        result = stage3_filter_structure(&result, options);
    }

    // Stage 4: Final normalization
    if options.final_normalize {
        result = stage4_final_normalize(&result, options);
    }

    result
}

/// Run cleanup with default options
pub fn cleanup_default(input: &str) -> String {
    cleanup(input, &CleanupOptions::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bullet_mapping() {
        let input = "●첫번째 항목\n■두번째 항목\n▶세번째 항목";
        let result = stage1_normalize_string(input, &CleanupOptions::default());
        assert!(result.contains("- 첫번째"));
        assert!(result.contains("- 두번째"));
        assert!(result.contains("- 세번째"));
    }

    #[test]
    fn test_pua_removal() {
        let input = "정상텍스트\u{E000}PUA문자\u{F000}끝";
        let options = CleanupOptions::default();
        let result = stage1_normalize_string(input, &options);
        assert_eq!(result, "정상텍스트PUA문자끝");
    }

    #[test]
    fn test_control_char_removal() {
        let input = "텍스트\x0B수직탭\x0C폼피드\u{FEFF}BOM끝";
        let result = stage1_normalize_string(input, &CleanupOptions::default());
        assert_eq!(result, "텍스트수직탭폼피드BOM끝");
    }

    #[test]
    fn test_fullwidth_normalization() {
        let input = "전각　스페이스Ａ～Ｚ";
        let result = stage1_normalize_string(input, &CleanupOptions::default());
        assert!(result.contains(' ')); // Ideographic space normalized
    }

    #[test]
    fn test_page_number_hyphen() {
        let input = "본문\n\n- 15 -\n\n다음 내용";
        let result = stage2_clean_lines(input, &CleanupOptions::default());
        assert!(!result.contains("- 15 -"));
    }

    #[test]
    fn test_page_number_ratio() {
        let input = "본문\n\n1 / 20\n\n다음";
        let result = stage2_clean_lines(input, &CleanupOptions::default());
        assert!(!result.contains("1 / 20"));
    }

    #[test]
    fn test_page_number_korean() {
        let input = "본문\n\n12 쪽\n\n다음";
        let result = stage2_clean_lines(input, &CleanupOptions::default());
        assert!(!result.contains("12 쪽"));
    }

    #[test]
    fn test_toc_removal() {
        let input = "목차\n\n서론.......... 5\n제1장 개요...... 12\n\n본문 시작";
        let result = stage2_clean_lines(input, &CleanupOptions::default());
        assert!(!result.contains("서론.......... 5"));
        assert!(!result.contains("제1장 개요...... 12"));
    }

    #[test]
    fn test_hwp_placeholder_removal() {
        let input = "수식:\n\n[EQ]\n\n다음 내용";
        let result = stage2_clean_lines(input, &CleanupOptions::default());
        assert!(!result.contains("[EQ]"));
    }

    #[test]
    fn test_multiple_newlines() {
        let input = "첫번째\n\n\n\n\n두번째";
        let result = stage4_final_normalize(input, &CleanupOptions::default());
        assert!(!result.contains("\n\n\n"));
        assert!(result.contains("\n\n"));
    }

    #[test]
    fn test_orphan_line_removal() {
        let input = "정상 문장입니다.\n.\n,\n다음 문장입니다.";
        let result = stage4_final_normalize(input, &CleanupOptions::default());
        assert!(!result.lines().any(|l| l.trim() == "."));
        assert!(!result.lines().any(|l| l.trim() == ","));
    }

    #[test]
    fn test_full_pipeline() {
        let input = concat!(
            "●첫번째 항목\n",
            "- 15 -\n",
            "\u{E000}PUA문자\n",
            "\n\n\n\n",
            "정상 내용입니다.\n",
            "서론.......... 5\n",
            "마지막 내용."
        );

        let result = cleanup(input, &CleanupOptions::default());

        // Bullet should be mapped (pulldown-cmark may convert list markers)
        assert!(
            result.contains("첫번째 항목"),
            "Expected '첫번째 항목' in result"
        );
        // Page number should be removed
        assert!(!result.contains("- 15 -"), "Page number should be removed");
        // PUA should be removed
        assert!(!result.contains('\u{E000}'), "PUA char should be removed");
        // Excessive newlines should be reduced
        assert!(
            !result.contains("\n\n\n\n"),
            "Excessive newlines should be reduced"
        );
        // TOC should be removed
        assert!(
            !result.contains("서론.......... 5"),
            "TOC should be removed"
        );
    }

    #[test]
    fn test_cleanup_options_minimal() {
        let options = CleanupOptions::minimal();
        assert!(options.normalize_strings);
        assert!(!options.clean_lines);
        assert!(!options.filter_structure);
        assert!(options.final_normalize);
    }

    #[test]
    fn test_cleanup_options_aggressive() {
        let options = CleanupOptions::aggressive();
        assert!(options.normalize_strings);
        assert!(options.clean_lines);
        assert!(options.filter_structure);
        assert!(options.final_normalize);
        assert!(options.header_footer_threshold < 0.8); // More aggressive
    }

    #[test]
    fn test_mojibake_detection_isolated_cjk() {
        // CJK ideograph at end of URL-like line is suspicious
        let input = "https://example.com湰灧";
        let result = clean_line_trailing_mojibake(input);
        assert_eq!(result, "https://example.com");
    }

    #[test]
    fn test_mojibake_preserves_legitimate_cjk() {
        // Multiple CJK characters together should be preserved
        let input = "다음은 중국어입니다: 你好世界";
        let result = clean_line_trailing_mojibake(input);
        assert_eq!(result, input); // Should not modify legitimate Chinese text
    }

    #[test]
    fn test_mojibake_preserves_korean() {
        // Korean text should be fully preserved
        let input = "한글 테스트 문장입니다";
        let result = clean_line_trailing_mojibake(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_mojibake_single_trailing_cjk() {
        // Single CJK after ASCII is suspicious
        let input = "Normal text 湰";
        let result = clean_line_trailing_mojibake(input);
        assert_eq!(result, "Normal text ");
    }

    #[test]
    fn test_frontmatter_extraction_simple() {
        let input = "---\ntitle: \"Test\"\n---\n\nContent here";
        let (fm, content) = extract_yaml_frontmatter(input);
        assert!(fm.is_some());
        assert!(fm.unwrap().contains("title: \"Test\""));
        assert!(content.contains("Content here"));
    }

    #[test]
    fn test_frontmatter_extraction_no_frontmatter() {
        let input = "Just regular content\nNo frontmatter here";
        let (fm, content) = extract_yaml_frontmatter(input);
        assert!(fm.is_none());
        assert_eq!(content, input);
    }

    #[test]
    fn test_frontmatter_preservation_in_pipeline() {
        let input =
            "---\ntitle: \"My Document\"\nformat: \"5.0.4.0\"\n---\n\nDocument content here.";
        let options = CleanupOptions::default();
        let result = stage3_filter_structure(input, &options);

        // Frontmatter should be preserved
        assert!(result.starts_with("---"), "Should start with ---");
        assert!(
            result.contains("title: \"My Document\""),
            "Title should be preserved"
        );
        assert!(
            result.contains("format: \"5.0.4.0\""),
            "Format should be preserved"
        );
        assert!(result.contains("---\n"), "Closing --- should exist");
    }

    #[test]
    fn test_frontmatter_not_corrupted() {
        // This was the original bug: pulldown-cmark converts --- to ## heading
        let input = "---\nformat: \"5.0.4.0\"\n---\n\n## Heading\n\nParagraph text.";
        let options = CleanupOptions::default();
        let result = stage3_filter_structure(input, &options);

        // Should NOT convert frontmatter format field to a heading
        assert!(
            !result.contains("## format"),
            "Should not convert format to heading"
        );
        assert!(
            result.contains("format: \"5.0.4.0\""),
            "Format value should be preserved"
        );
    }
}
