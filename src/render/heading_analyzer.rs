//! Sophisticated heading detection with multi-level priority analysis.
//!
//! This module implements heading analysis for accurate heading vs list discrimination:
//! - Priority-based heading decisions
//! - Sequence analysis for consecutive numbered items
//! - HWP-specific Korean document patterns
//!
//! # Priority Order
//!
//! 1. **Explicit styles** (`outline_level`) - always trusted when enabled
//! 2. **Exclusion conditions** - bullet markers, excessive length
//! 3. **Sequence analysis** - consecutive numbered items demoted to lists
//!
//! # Key Insight
//!
//! The same pattern "1. 항목" can be either a heading or a list item:
//! - Standalone "1. 서론" → likely a heading
//! - Consecutive "1. 항목", "2. 항목", "3. 항목" → likely a list

use crate::model::{Block, Document, Paragraph};

/// Configuration for heading analysis.
#[derive(Debug, Clone)]
pub struct HeadingConfig {
    /// Maximum heading level to emit (1-6).
    pub max_heading_level: u8,

    /// Maximum text length for a paragraph to be considered a heading.
    pub max_text_length: usize,

    /// Trust explicit styles (`heading_level` from outline) unconditionally.
    /// When true, explicit headings skip all exclusion checks.
    pub trust_explicit_styles: bool,

    /// Analyze sequential patterns to detect lists.
    pub analyze_sequences: bool,

    /// Minimum consecutive items to consider as a list.
    pub min_sequence_count: usize,

    /// Enable statistical inference (font size + bold → heading).
    /// Requires font_size information in TextRuns.
    pub enable_statistical_inference: bool,

    /// Font size ratio threshold for statistical inference.
    /// Default: 1.2 (120% of base font size).
    pub size_threshold_ratio: f32,

    /// Normalize heading levels so the minimum is H1 or H2.
    /// If true and document starts with H4, levels are shifted up (H4→H1, H5→H2, etc.).
    /// Default: true
    pub normalize_levels: bool,

    /// Target minimum heading level after normalization (1 or 2).
    /// Default: 2 (first heading becomes H2, allows room for document title)
    pub normalize_min_level: u8,
}

impl Default for HeadingConfig {
    fn default() -> Self {
        Self {
            max_heading_level: 4,
            max_text_length: 80,
            trust_explicit_styles: true,
            analyze_sequences: true,
            min_sequence_count: 2,
            enable_statistical_inference: true, // Enabled by default for font-size based detection
            size_threshold_ratio: 1.15,         // 115% of base font size = heading candidate
            normalize_levels: true,             // Normalize so min heading is H1/H2
            normalize_min_level: 2,             // Target H2 (leaves room for title)
        }
    }
}

impl HeadingConfig {
    /// Create a new heading config with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum heading level.
    pub fn with_max_level(mut self, level: u8) -> Self {
        self.max_heading_level = level.clamp(1, 6);
        self
    }

    /// Set the maximum text length for headings.
    pub fn with_max_text_length(mut self, length: usize) -> Self {
        self.max_text_length = length;
        self
    }

    /// Set whether to trust explicit styles.
    pub fn with_trust_explicit(mut self, trust: bool) -> Self {
        self.trust_explicit_styles = trust;
        self
    }

    /// Set whether to analyze sequences.
    pub fn with_sequence_analysis(mut self, analyze: bool) -> Self {
        self.analyze_sequences = analyze;
        self
    }

    /// Set minimum sequence count.
    pub fn with_min_sequence_count(mut self, count: usize) -> Self {
        self.min_sequence_count = count.max(2);
        self
    }

    /// Enable statistical inference (font size + bold → heading).
    pub fn with_statistical_inference(mut self, enable: bool) -> Self {
        self.enable_statistical_inference = enable;
        self
    }

    /// Set the font size ratio threshold for statistical inference.
    pub fn with_size_ratio(mut self, ratio: f32) -> Self {
        self.size_threshold_ratio = ratio.max(1.0);
        self
    }

    /// Enable heading level normalization.
    /// When enabled, heading levels are shifted so minimum becomes normalize_min_level.
    pub fn with_normalize_levels(mut self, enable: bool) -> Self {
        self.normalize_levels = enable;
        self
    }

    /// Set the target minimum heading level after normalization.
    /// Default: 2 (H2, leaves room for document title as H1)
    pub fn with_normalize_min_level(mut self, level: u8) -> Self {
        self.normalize_min_level = level.clamp(1, 3);
        self
    }
}

/// Result of heading analysis for a paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingDecision {
    /// Heading from explicit style (`heading_level` in ParagraphStyle).
    Explicit(u8),

    /// Heading inferred from formatting or section markers.
    Inferred(u8),

    /// Demoted from heading to normal paragraph (e.g., part of numbered list).
    Demoted,

    /// Not a heading.
    None,
}

impl HeadingDecision {
    /// Check if this decision results in a heading.
    pub fn is_heading(&self) -> bool {
        matches!(
            self,
            HeadingDecision::Explicit(_) | HeadingDecision::Inferred(_)
        )
    }

    /// Get the heading level if this is a heading.
    pub fn level(&self) -> Option<u8> {
        match self {
            HeadingDecision::Explicit(level) | HeadingDecision::Inferred(level) => Some(*level),
            _ => None,
        }
    }
}

/// Statistics collected from a document for heading analysis.
#[derive(Debug, Clone, Default)]
pub struct DocumentStats {
    /// Font size distribution (size in tenths of a point → occurrence weight).
    pub font_sizes: std::collections::HashMap<u32, usize>,

    /// Detected base font size (most frequent, in points).
    pub base_font_size: Option<f32>,

    /// Number of bold paragraphs.
    pub bold_paragraphs: usize,

    /// Total number of paragraphs.
    pub total_paragraphs: usize,

    /// Number of paragraphs with explicit heading styles.
    pub explicit_heading_count: usize,
}

impl DocumentStats {
    /// Calculate the base (body) font size from the distribution.
    /// Uses the most weighted font size as the baseline.
    pub fn calculate_base_font_size(&mut self) {
        self.base_font_size = self
            .font_sizes
            .iter()
            .max_by_key(|(_, weight)| *weight)
            .map(|(size, _)| *size as f32 / 10.0);
    }

    /// Check if a font size is significantly larger than the base.
    pub fn is_larger_than_base(&self, size: f32, ratio: f32) -> bool {
        if let Some(base) = self.base_font_size {
            size >= base * ratio
        } else {
            false
        }
    }
}

/// Analyzer for sophisticated heading detection.
pub struct HeadingAnalyzer {
    config: HeadingConfig,
    stats: DocumentStats,
}

impl HeadingAnalyzer {
    /// Create a new heading analyzer with the given configuration.
    pub fn new(config: HeadingConfig) -> Self {
        Self {
            config,
            stats: DocumentStats::default(),
        }
    }

    /// Create a heading analyzer with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(HeadingConfig::default())
    }

    /// Analyze a document and return heading decisions for all paragraphs.
    ///
    /// Performs a multi-pass analysis:
    /// 1. Collect document statistics (font sizes, patterns)
    /// 2. Apply priority-based heading decisions
    /// 3. Normalize heading levels (if enabled)
    ///
    /// Returns a vector of decisions, one per paragraph block in document order.
    /// Table blocks are skipped (not included in the output).
    pub fn analyze(&mut self, doc: &Document) -> Vec<HeadingDecision> {
        // Collect all paragraphs from all sections
        let paragraphs: Vec<&Paragraph> = doc
            .sections
            .iter()
            .flat_map(|section| {
                section.content.iter().filter_map(|block| {
                    if let Block::Paragraph(para) = block {
                        Some(para)
                    } else {
                        None
                    }
                })
            })
            .collect();

        // Pass 1: Collect statistics (if statistical inference enabled)
        if self.config.enable_statistical_inference {
            self.collect_stats(&paragraphs);
        }

        // Pass 2: Make heading decisions
        let mut decisions = self.analyze_paragraphs(&paragraphs);

        // Pass 3: Normalize heading levels (if enabled)
        if self.config.normalize_levels {
            self.normalize_heading_levels(&mut decisions);
        }

        decisions
    }

    /// Normalize heading levels so the minimum becomes the target level.
    ///
    /// If document starts with H4, shift all levels up (H4→H2, H5→H3, etc.)
    /// This ensures proper heading hierarchy in output.
    fn normalize_heading_levels(&self, decisions: &mut [HeadingDecision]) {
        // Find minimum heading level in the document
        let min_level = decisions.iter().filter_map(|d| d.level()).min();

        let min_level = match min_level {
            Some(level) => level,
            None => return, // No headings to normalize
        };

        // Calculate shift needed
        let target = self.config.normalize_min_level;
        if min_level <= target {
            return; // Already at or above target, no normalization needed
        }

        let shift = min_level - target;

        // Apply shift to all heading decisions
        for decision in decisions.iter_mut() {
            *decision = match *decision {
                HeadingDecision::Explicit(level) => {
                    let new_level = level.saturating_sub(shift).max(1);
                    HeadingDecision::Explicit(new_level)
                }
                HeadingDecision::Inferred(level) => {
                    let new_level = level.saturating_sub(shift).max(1);
                    HeadingDecision::Inferred(new_level)
                }
                other => other,
            };
        }
    }

    /// Collect statistics from paragraphs (Pass 1).
    fn collect_stats(&mut self, paragraphs: &[&Paragraph]) {
        self.stats = DocumentStats::default();

        for para in paragraphs {
            self.stats.total_paragraphs += 1;

            // Check for explicit heading
            if para.style.heading_level > 0 {
                self.stats.explicit_heading_count += 1;
            }

            // Collect font sizes
            if let Some(size) = para.dominant_font_size() {
                let key = (size * 10.0) as u32;
                let text_len = para.plain_text().chars().count();
                *self.stats.font_sizes.entry(key).or_insert(0) += text_len;
            }

            // Count bold paragraphs
            if para.is_all_bold() {
                self.stats.bold_paragraphs += 1;
            }
        }

        self.stats.calculate_base_font_size();
    }

    /// Analyze a sequence of paragraphs with context awareness.
    pub fn analyze_paragraphs(&self, paragraphs: &[&Paragraph]) -> Vec<HeadingDecision> {
        let mut decisions = Vec::with_capacity(paragraphs.len());

        // Make initial decisions
        for para in paragraphs {
            decisions.push(self.decide_heading(para));
        }

        // Check sequential patterns if enabled
        if self.config.analyze_sequences {
            self.apply_sequence_analysis(paragraphs, &mut decisions);
        }

        decisions
    }

    /// Make a heading decision for a single paragraph.
    fn decide_heading(&self, para: &Paragraph) -> HeadingDecision {
        let plain_text = para.plain_text();
        let trimmed = plain_text.trim();
        let style = &para.style;

        // P1: Hard exclusions - these ALWAYS prevent heading regardless of styles
        // Bullet markers are never headings
        if self.looks_like_bullet_item(trimmed) {
            return if style.heading_level > 0 {
                HeadingDecision::Demoted
            } else {
                HeadingDecision::None
            };
        }

        // Text too long is never a heading
        if trimmed.chars().count() > self.config.max_text_length {
            return if style.heading_level > 0 {
                HeadingDecision::Demoted
            } else {
                HeadingDecision::None
            };
        }

        // P2: Explicit style - trust document's heading markers (after exclusions)
        if style.heading_level > 0 && self.config.trust_explicit_styles {
            let level = self.cap_heading_level(style.heading_level);
            return HeadingDecision::Explicit(level);
        }

        // P3: Statistical inference (font size based)
        if self.config.enable_statistical_inference {
            if let Some(inferred) = self.infer_heading_from_style(para) {
                return HeadingDecision::Inferred(inferred);
            }
        }

        // Fallback: Use explicit style if present (even when trust=false)
        // This handles numbered headings like "1. 서론" with explicit H1 style
        // Sequence analysis may still demote them if they form a consecutive pattern
        if style.heading_level > 0 {
            let level = self.cap_heading_level(style.heading_level);
            return HeadingDecision::Explicit(level);
        }

        HeadingDecision::None
    }

    /// Infer heading level from text style (primarily font size).
    ///
    /// For a paragraph to be inferred as a heading:
    /// - Font size must be >= base_font_size * size_threshold_ratio
    /// - Bold formatting is a bonus signal but not required
    fn infer_heading_from_style(&self, para: &Paragraph) -> Option<u8> {
        // Must have text content
        let text = para.plain_text();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Get dominant font size
        let font_size = para.dominant_font_size()?;

        // Check if font is larger than base
        if !self
            .stats
            .is_larger_than_base(font_size, self.config.size_threshold_ratio)
        {
            return None;
        }

        // Additional validation: heading text shouldn't be too long
        if trimmed.chars().count() > self.config.max_text_length {
            return None;
        }

        // Infer level based on size ratio (bold gets higher priority)
        let level = self.infer_level_from_size(font_size, para.is_all_bold());
        Some(self.cap_heading_level(level))
    }

    /// Infer heading level from font size ratio.
    /// Bold text gets a level boost (one level higher).
    fn infer_level_from_size(&self, size: f32, is_bold: bool) -> u8 {
        let base = self.stats.base_font_size.unwrap_or(12.0);
        let ratio = size / base;

        let base_level = if ratio >= 1.8 {
            1 // H1: 180%+ of base
        } else if ratio >= 1.5 {
            2 // H2: 150-180%
        } else if ratio >= 1.3 {
            3 // H3: 130-150%
        } else {
            4 // H4: 115-130%
        };

        // Bold text can promote by one level (but not beyond H1)
        if is_bold && base_level > 1 {
            base_level - 1
        } else {
            base_level
        }
    }

    /// Check if text looks like a bullet list item.
    ///
    /// Note: Numbered patterns (1., 가., a.) are NOT checked here.
    /// They are handled separately in sequence analysis.
    fn looks_like_bullet_item(&self, text: &str) -> bool {
        let trimmed = text.trim_start();
        if trimmed.is_empty() {
            return false;
        }

        // Common bullet/symbol markers (NOT numbered patterns)
        const BULLET_MARKERS: &[char] = &[
            'ㅇ', 'ㆍ', '○', '●', '◎', '■', '□', '▪', '▫', '◆', '◇', '★', '☆', '※', '•', '-', '–',
            '—', '→', '▶', '►', '▷', '▹', '◁', '◀', '◃', '◂', '·', '∙',
        ];

        let first_char = trimmed.chars().next().unwrap();
        BULLET_MARKERS.contains(&first_char)
    }

    /// Cap heading level to configured maximum.
    fn cap_heading_level(&self, level: u8) -> u8 {
        if level > self.config.max_heading_level {
            self.config.max_heading_level
        } else {
            level
        }
    }

    /// Apply sequence analysis to detect list patterns.
    fn apply_sequence_analysis(
        &self,
        paragraphs: &[&Paragraph],
        decisions: &mut [HeadingDecision],
    ) {
        if paragraphs.len() < self.config.min_sequence_count {
            return;
        }

        // Find sequences of numbered paragraphs
        let mut i = 0;
        while i < paragraphs.len() {
            if let Some(seq_len) = self.detect_sequence_at(paragraphs, i) {
                if seq_len >= self.config.min_sequence_count {
                    // Demote all items in the sequence
                    for decision in decisions.iter_mut().skip(i).take(seq_len) {
                        if decision.is_heading() {
                            *decision = HeadingDecision::Demoted;
                        }
                    }
                    i += seq_len;
                    continue;
                }
            }
            i += 1;
        }
    }

    /// Detect a numbered sequence starting at the given index.
    /// Returns the length of the sequence if found.
    fn detect_sequence_at(&self, paragraphs: &[&Paragraph], start: usize) -> Option<usize> {
        let first_text = paragraphs[start].plain_text();
        let first_trimmed = first_text.trim();

        // Try to parse the first number/marker
        let first_marker = extract_sequence_marker(first_trimmed)?;

        let mut seq_len = 1;
        let mut expected_next = next_marker(&first_marker)?;

        for para in paragraphs.iter().skip(start + 1) {
            let text = para.plain_text();
            let trimmed = text.trim();

            if let Some(marker) = extract_sequence_marker(trimmed) {
                if marker == expected_next {
                    seq_len += 1;
                    if let Some(next) = next_marker(&marker) {
                        expected_next = next;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if seq_len >= 2 {
            Some(seq_len)
        } else {
            None
        }
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &HeadingConfig {
        &self.config
    }
}

/// Extract a sequence marker from text (e.g., "1", "가", "a").
///
/// Supports:
/// - Numeric: "1.", "2)", "(3)"
/// - Korean 가나다: "가.", "나)", "(다)"
/// - Alphabetic: "a.", "b)"
fn extract_sequence_marker(text: &str) -> Option<String> {
    let text = text.trim_start();
    if text.is_empty() {
        return None;
    }

    let chars: Vec<char> = text.chars().take(10).collect();

    // Check "(N)" pattern - parenthesized
    if chars.first() == Some(&'(') {
        if let Some(close_idx) = chars.iter().position(|&c| c == ')') {
            let inner: String = chars[1..close_idx].iter().collect();
            if !inner.is_empty()
                && (inner.chars().all(|c| c.is_ascii_digit())
                    || (inner.chars().count() == 1
                        && inner.chars().next().is_some_and(|c| c.is_ascii_lowercase()))
                    || (inner.chars().count() == 1
                        && inner.chars().next().is_some_and(is_korean_sequence_char)))
            {
                return Some(inner);
            }
        }
    }

    // Check "N." or "N)" pattern for numbers
    let mut num_end = 0;
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_digit() {
            num_end = i + 1;
        } else {
            break;
        }
    }

    if num_end > 0 && num_end < chars.len() {
        let next = chars[num_end];
        if next == '.' || next == ')' {
            return Some(chars[..num_end].iter().collect());
        }
    }

    // Check Korean "가." pattern
    if chars.len() >= 2 && is_korean_sequence_char(chars[0]) && (chars[1] == '.' || chars[1] == ')')
    {
        return Some(chars[0].to_string());
    }

    // Check "a." or "a)" pattern
    if chars.len() >= 2 && chars[0].is_ascii_lowercase() && (chars[1] == '.' || chars[1] == ')') {
        return Some(chars[0].to_string());
    }

    None
}

/// Get the next expected marker in a sequence.
fn next_marker(marker: &str) -> Option<String> {
    // Number sequence
    if let Ok(n) = marker.parse::<u32>() {
        return Some((n + 1).to_string());
    }

    // Single character sequences (Korean, alphabetic)
    if marker.chars().count() == 1 {
        let c = marker.chars().next()?;

        // Korean "가나다라..." sequence
        const KOREAN_SEQ: &[char] = &[
            '가', '나', '다', '라', '마', '바', '사', '아', '자', '차', '카', '타', '파', '하',
        ];
        if let Some(idx) = KOREAN_SEQ.iter().position(|&x| x == c) {
            if idx + 1 < KOREAN_SEQ.len() {
                return Some(KOREAN_SEQ[idx + 1].to_string());
            }
        }

        // Alphabetic sequence
        if c.is_ascii_lowercase() && c != 'z' {
            return Some(((c as u8) + 1) as char).map(|c| c.to_string());
        }
    }

    None
}

/// Check if a character is part of the Korean sequence (가나다라...).
fn is_korean_sequence_char(c: char) -> bool {
    const KOREAN_SEQ: &[char] = &[
        '가', '나', '다', '라', '마', '바', '사', '아', '자', '차', '카', '타', '파', '하',
    ];
    KOREAN_SEQ.contains(&c)
}

// ============================================================================
// HWP-Specific Korean Heading Patterns
// ============================================================================

/// Roman numeral characters (uppercase).
const ROMAN_NUMERALS: &[char] = &['Ⅰ', 'Ⅱ', 'Ⅲ', 'Ⅳ', 'Ⅴ', 'Ⅵ', 'Ⅶ', 'Ⅷ', 'Ⅸ', 'Ⅹ'];

/// Check if text starts with a Korean chapter/section pattern.
///
/// Patterns recognized:
/// - "제N장", "제N절", "제N조", "제N항" (Korean legal/document numbering)
/// - "Ⅰ.", "Ⅱ.", "Ⅲ." (Roman numerals)
/// - "제1장 서론", "Ⅱ. 본론" (with following title)
pub fn is_korean_chapter_pattern(text: &str) -> Option<KoreanChapterInfo> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    let chars: Vec<char> = text.chars().collect();

    // Pattern 1: "제N장/절/조/항" (Korean legal numbering)
    if chars.first() == Some(&'제') && chars.len() >= 3 {
        // Find the number part
        let mut num_end = 1;
        while num_end < chars.len() && chars[num_end].is_ascii_digit() {
            num_end += 1;
        }

        if num_end > 1 && num_end < chars.len() {
            let suffix = chars[num_end];
            let chapter_type = match suffix {
                '장' => Some(KoreanChapterType::Jang),  // Chapter
                '절' => Some(KoreanChapterType::Jeol),  // Section
                '조' => Some(KoreanChapterType::Jo),    // Article (legal)
                '항' => Some(KoreanChapterType::Hang),  // Paragraph (legal)
                '편' => Some(KoreanChapterType::Pyeon), // Part
                '부' => Some(KoreanChapterType::Bu),    // Division
                _ => None,
            };

            if let Some(ct) = chapter_type {
                let number: String = chars[1..num_end].iter().collect();
                if let Ok(n) = number.parse::<u32>() {
                    return Some(KoreanChapterInfo {
                        chapter_type: ct,
                        number: n,
                        // Title is everything after the chapter marker (if any)
                        title: if num_end + 1 < chars.len() {
                            Some(
                                chars[num_end + 1..]
                                    .iter()
                                    .collect::<String>()
                                    .trim()
                                    .to_string(),
                            )
                        } else {
                            None
                        },
                    });
                }
            }
        }
    }

    // Pattern 2: Roman numerals (Ⅰ, Ⅱ, Ⅲ...)
    if let Some(first) = chars.first() {
        if let Some(roman_idx) = ROMAN_NUMERALS.iter().position(|&c| c == *first) {
            // Check for separator (., -, space, or direct title)
            let has_separator = chars
                .get(1)
                .map(|c| *c == '.' || *c == '-' || c.is_whitespace())
                .unwrap_or(false);
            if has_separator || chars.len() == 1 {
                return Some(KoreanChapterInfo {
                    chapter_type: KoreanChapterType::Roman,
                    number: (roman_idx + 1) as u32,
                    title: if chars.len() > 2 {
                        Some(chars[2..].iter().collect::<String>().trim().to_string())
                    } else {
                        None
                    },
                });
            }
        }
    }

    None
}

/// Type of Korean chapter/section marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KoreanChapterType {
    /// 장 (Chapter)
    Jang,
    /// 절 (Section)
    Jeol,
    /// 조 (Article - legal)
    Jo,
    /// 항 (Paragraph - legal)
    Hang,
    /// 편 (Part)
    Pyeon,
    /// 부 (Division)
    Bu,
    /// Roman numeral (Ⅰ, Ⅱ, Ⅲ...)
    Roman,
}

impl KoreanChapterType {
    /// Get suggested heading level for this chapter type.
    pub fn suggested_heading_level(&self) -> u8 {
        match self {
            KoreanChapterType::Pyeon | KoreanChapterType::Bu => 1,
            KoreanChapterType::Jang | KoreanChapterType::Roman => 2,
            KoreanChapterType::Jeol => 3,
            KoreanChapterType::Jo | KoreanChapterType::Hang => 4,
        }
    }
}

/// Information extracted from Korean chapter pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KoreanChapterInfo {
    /// Type of chapter marker.
    pub chapter_type: KoreanChapterType,
    /// Chapter number.
    pub number: u32,
    /// Optional title following the chapter marker.
    pub title: Option<String>,
}

/// Get the next expected Korean chapter marker.
///
/// Used for sequence detection in Korean legal/formal documents.
pub fn next_korean_chapter(info: &KoreanChapterInfo) -> Option<KoreanChapterInfo> {
    Some(KoreanChapterInfo {
        chapter_type: info.chapter_type,
        number: info.number + 1,
        title: None, // Title will be different
    })
}

/// Check if text looks like a Korean chapter heading (not part of a list).
///
/// Returns true if the pattern suggests a standalone heading rather than
/// a list item. This considers:
/// - Korean chapter patterns (제1장, Ⅰ.)
/// - Text length (shorter = more likely heading)
/// - Following content structure
pub fn looks_like_korean_heading(text: &str) -> bool {
    let trimmed = text.trim();
    let char_count = trimmed.chars().count();

    // Korean chapter patterns are almost always headings
    if is_korean_chapter_pattern(trimmed).is_some() {
        // Unless it's very long (probably a list item with description)
        return char_count <= 60;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Block, Document, InlineContent, ParagraphStyle, Section, TextRun, TextStyle,
    };

    fn make_paragraph(text: &str, heading_level: u8) -> Paragraph {
        let mut para = Paragraph::with_style(ParagraphStyle {
            heading_level,
            ..Default::default()
        });
        para.content.push(InlineContent::Text(TextRun::new(text)));
        para
    }

    fn make_styled_paragraph(text: &str, heading_level: u8, style: TextStyle) -> Paragraph {
        let mut para = Paragraph::with_style(ParagraphStyle {
            heading_level,
            ..Default::default()
        });
        para.content
            .push(InlineContent::Text(TextRun::with_style(text, style)));
        para
    }

    #[test]
    fn test_explicit_heading_trusted() {
        let config = HeadingConfig::default();
        let analyzer = HeadingAnalyzer::new(config);
        let para = make_paragraph("제목", 1);

        let paras = vec![&para];
        let decisions = analyzer.analyze_paragraphs(&paras);

        assert!(matches!(decisions[0], HeadingDecision::Explicit(1)));
    }

    #[test]
    fn test_bullet_marker_demoted_when_untrusted() {
        let config = HeadingConfig::default().with_trust_explicit(false);
        let analyzer = HeadingAnalyzer::new(config);
        let para = make_paragraph("ㅇ 항목 내용", 2);

        let paras = vec![&para];
        let decisions = analyzer.analyze_paragraphs(&paras);

        assert_eq!(decisions[0], HeadingDecision::Demoted);
    }

    #[test]
    fn test_sequence_marker_extraction() {
        assert_eq!(extract_sequence_marker("1. 항목"), Some("1".to_string()));
        assert_eq!(extract_sequence_marker("2) 항목"), Some("2".to_string()));
        assert_eq!(extract_sequence_marker("(3) 항목"), Some("3".to_string()));
        assert_eq!(extract_sequence_marker("가. 항목"), Some("가".to_string()));
        assert_eq!(extract_sequence_marker("a. 항목"), Some("a".to_string()));
        assert_eq!(extract_sequence_marker("일반 텍스트"), None);
    }

    #[test]
    fn test_next_marker() {
        assert_eq!(next_marker("1"), Some("2".to_string()));
        assert_eq!(next_marker("9"), Some("10".to_string()));
        assert_eq!(next_marker("가"), Some("나".to_string()));
        assert_eq!(next_marker("a"), Some("b".to_string()));
        assert_eq!(next_marker("하"), None); // End of Korean sequence
    }

    #[test]
    fn test_sequence_analysis_demotes_consecutive() {
        let config = HeadingConfig::default().with_trust_explicit(false);
        let analyzer = HeadingAnalyzer::new(config);

        let para1 = make_paragraph("1. 첫째", 2);
        let para2 = make_paragraph("2. 둘째", 2);
        let para3 = make_paragraph("3. 셋째", 2);
        let paras = vec![&para1, &para2, &para3];

        let decisions = analyzer.analyze_paragraphs(&paras);

        // All should be demoted due to sequential pattern
        assert!(decisions
            .iter()
            .all(|d| matches!(d, HeadingDecision::Demoted)));
    }

    #[test]
    fn test_standalone_numbered_heading_preserved() {
        let config = HeadingConfig::default().with_trust_explicit(false);
        let analyzer = HeadingAnalyzer::new(config);

        // Numbered headings separated by plain text (not consecutive)
        let para1 = make_paragraph("1. 서론", 2);
        let para2 = make_paragraph("본문 내용입니다.", 0);
        let para3 = make_paragraph("2. 본론", 2);
        let paras = vec![&para1, &para2, &para3];

        let decisions = analyzer.analyze_paragraphs(&paras);

        // "1. 서론" and "2. 본론" are NOT consecutive (separated by plain text)
        // So they should be preserved as headings
        assert!(
            matches!(decisions[0], HeadingDecision::Explicit(2)),
            "First heading should be preserved: {:?}",
            decisions[0]
        );
        assert!(
            matches!(decisions[2], HeadingDecision::Explicit(2)),
            "Third heading should be preserved: {:?}",
            decisions[2]
        );
    }

    #[test]
    fn test_long_text_demoted() {
        let config = HeadingConfig::default().with_trust_explicit(false);
        let analyzer = HeadingAnalyzer::new(config);

        let long_text = "이것은 매우 긴 텍스트입니다. ".repeat(5);
        let para = make_paragraph(&long_text, 2);
        let paras = vec![&para];

        let decisions = analyzer.analyze_paragraphs(&paras);

        assert_eq!(decisions[0], HeadingDecision::Demoted);
    }

    #[test]
    fn test_korean_sequence_patterns() {
        let config = HeadingConfig::default().with_trust_explicit(false);
        let analyzer = HeadingAnalyzer::new(config);

        let para1 = make_paragraph("가. 첫째", 2);
        let para2 = make_paragraph("나. 둘째", 2);
        let para3 = make_paragraph("다. 셋째", 2);
        let paras = vec![&para1, &para2, &para3];

        let decisions = analyzer.analyze_paragraphs(&paras);

        // Korean sequence should also be demoted
        assert!(decisions
            .iter()
            .all(|d| matches!(d, HeadingDecision::Demoted)));
    }

    #[test]
    fn test_max_heading_level_capped() {
        let config = HeadingConfig::default().with_max_level(2);
        let analyzer = HeadingAnalyzer::new(config);
        let para = make_paragraph("제목", 4);

        let paras = vec![&para];
        let decisions = analyzer.analyze_paragraphs(&paras);

        assert!(matches!(decisions[0], HeadingDecision::Explicit(2)));
    }

    #[test]
    fn test_heading_decision_helpers() {
        assert!(HeadingDecision::Explicit(1).is_heading());
        assert!(HeadingDecision::Inferred(2).is_heading());
        assert!(!HeadingDecision::Demoted.is_heading());
        assert!(!HeadingDecision::None.is_heading());

        assert_eq!(HeadingDecision::Explicit(3).level(), Some(3));
        assert_eq!(HeadingDecision::Demoted.level(), None);
    }

    // ========================================================================
    // HWP-Specific Korean Pattern Tests
    // ========================================================================

    #[test]
    fn test_korean_chapter_pattern_jang() {
        // "제1장 서론" pattern
        let info = is_korean_chapter_pattern("제1장 서론").unwrap();
        assert_eq!(info.chapter_type, KoreanChapterType::Jang);
        assert_eq!(info.number, 1);
        assert_eq!(info.title, Some("서론".to_string()));

        // Without title
        let info2 = is_korean_chapter_pattern("제2장").unwrap();
        assert_eq!(info2.chapter_type, KoreanChapterType::Jang);
        assert_eq!(info2.number, 2);
        assert_eq!(info2.title, None);
    }

    #[test]
    fn test_korean_chapter_pattern_jeol() {
        // "제1절" pattern
        let info = is_korean_chapter_pattern("제1절 개요").unwrap();
        assert_eq!(info.chapter_type, KoreanChapterType::Jeol);
        assert_eq!(info.number, 1);
        assert_eq!(info.title, Some("개요".to_string()));
    }

    #[test]
    fn test_korean_chapter_pattern_jo() {
        // "제1조" pattern (legal article)
        let info = is_korean_chapter_pattern("제15조 적용범위").unwrap();
        assert_eq!(info.chapter_type, KoreanChapterType::Jo);
        assert_eq!(info.number, 15);
    }

    #[test]
    fn test_korean_chapter_pattern_roman() {
        // "Ⅰ. 서론" pattern
        let info = is_korean_chapter_pattern("Ⅰ. 서론").unwrap();
        assert_eq!(info.chapter_type, KoreanChapterType::Roman);
        assert_eq!(info.number, 1);
        assert_eq!(info.title, Some("서론".to_string()));

        // "Ⅲ-" pattern
        let info2 = is_korean_chapter_pattern("Ⅲ- 결론").unwrap();
        assert_eq!(info2.chapter_type, KoreanChapterType::Roman);
        assert_eq!(info2.number, 3);
    }

    #[test]
    fn test_korean_chapter_pattern_none() {
        // Regular text should return None
        assert!(is_korean_chapter_pattern("일반 텍스트").is_none());
        assert!(is_korean_chapter_pattern("1. 항목").is_none());
        assert!(is_korean_chapter_pattern("가. 내용").is_none());
    }

    #[test]
    fn test_korean_chapter_suggested_level() {
        assert_eq!(KoreanChapterType::Jang.suggested_heading_level(), 2);
        assert_eq!(KoreanChapterType::Jeol.suggested_heading_level(), 3);
        assert_eq!(KoreanChapterType::Jo.suggested_heading_level(), 4);
        assert_eq!(KoreanChapterType::Roman.suggested_heading_level(), 2);
        assert_eq!(KoreanChapterType::Pyeon.suggested_heading_level(), 1);
    }

    #[test]
    fn test_looks_like_korean_heading() {
        // Chapter patterns are headings
        assert!(looks_like_korean_heading("제1장 서론"));
        assert!(looks_like_korean_heading("Ⅱ. 본론"));

        // Regular numbered items are not
        assert!(!looks_like_korean_heading("1. 첫 번째 항목"));
        assert!(!looks_like_korean_heading("가. 내용"));
    }

    #[test]
    fn test_next_korean_chapter() {
        let info = KoreanChapterInfo {
            chapter_type: KoreanChapterType::Jang,
            number: 1,
            title: Some("서론".to_string()),
        };

        let next = next_korean_chapter(&info).unwrap();
        assert_eq!(next.chapter_type, KoreanChapterType::Jang);
        assert_eq!(next.number, 2);
        assert_eq!(next.title, None);
    }

    // ========================================================================
    // Statistical Inference Tests
    // ========================================================================

    #[test]
    fn test_statistical_inference_bold_large_font() {
        // Enable statistical inference
        let config = HeadingConfig::default()
            .with_statistical_inference(true)
            .with_trust_explicit(false);

        let mut analyzer = HeadingAnalyzer::new(config);

        // Create body paragraphs with 12pt font
        let body1 = make_styled_paragraph(
            "This is body text that should establish the baseline.",
            0,
            TextStyle {
                font_size: Some(12.0),
                ..Default::default()
            },
        );
        let body2 = make_styled_paragraph(
            "More body text to strengthen the baseline determination.",
            0,
            TextStyle {
                font_size: Some(12.0),
                ..Default::default()
            },
        );

        // Create heading paragraph: bold + 16pt (1.33x larger)
        let heading = make_styled_paragraph(
            "Chapter Title",
            0, // No explicit heading level
            TextStyle {
                bold: true,
                font_size: Some(16.0),
                ..Default::default()
            },
        );

        // Build document manually
        let doc = Document {
            sections: vec![Section {
                content: vec![
                    Block::Paragraph(body1),
                    Block::Paragraph(heading),
                    Block::Paragraph(body2),
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        let decisions = analyzer.analyze(&doc);

        // The heading (index 1) should be inferred
        assert!(
            matches!(decisions[1], HeadingDecision::Inferred(_)),
            "Bold + large font should be inferred as heading: {:?}",
            decisions[1]
        );
    }

    #[test]
    fn test_statistical_inference_large_font_without_bold() {
        // Large font IS now a heading even without bold
        // Bold only provides a level boost (one level higher)
        let config = HeadingConfig::default()
            .with_statistical_inference(true)
            .with_trust_explicit(false);

        let mut analyzer = HeadingAnalyzer::new(config);

        // Body text at 12pt
        let body = make_styled_paragraph(
            "Body text establishes baseline.",
            0,
            TextStyle {
                font_size: Some(12.0),
                ..Default::default()
            },
        );

        // Large font without bold - should still be inferred as heading
        let large_not_bold = make_styled_paragraph(
            "Large Title",
            0,
            TextStyle {
                bold: false,
                font_size: Some(16.0), // 133% of 12pt
                ..Default::default()
            },
        );

        let doc = Document {
            sections: vec![Section {
                content: vec![Block::Paragraph(body), Block::Paragraph(large_not_bold)],
                ..Default::default()
            }],
            ..Default::default()
        };

        let decisions = analyzer.analyze(&doc);

        // Should be inferred as heading (large font is sufficient)
        assert!(
            matches!(decisions[1], HeadingDecision::Inferred(_)),
            "Large font should be heading even without bold: {:?}",
            decisions[1]
        );
    }

    #[test]
    fn test_statistical_inference_enabled_by_default() {
        // Statistical inference is now ENABLED by default
        let config = HeadingConfig::default();
        let mut analyzer = HeadingAnalyzer::new(config);

        // Body text to establish baseline
        let body = make_styled_paragraph(
            "This is body text at normal size.",
            0,
            TextStyle {
                font_size: Some(12.0),
                ..Default::default()
            },
        );

        let heading = make_styled_paragraph(
            "Bold Large Title",
            0,
            TextStyle {
                bold: true,
                font_size: Some(20.0), // 167% of 12pt
                ..Default::default()
            },
        );

        let doc = Document {
            sections: vec![Section {
                content: vec![Block::Paragraph(body), Block::Paragraph(heading)],
                ..Default::default()
            }],
            ..Default::default()
        };

        let decisions = analyzer.analyze(&doc);

        // Should be inferred (statistical inference enabled by default)
        assert!(
            matches!(decisions[1], HeadingDecision::Inferred(_)),
            "Statistical inference should be enabled by default: {:?}",
            decisions[1]
        );
    }

    #[test]
    fn test_document_stats_base_font_calculation() {
        let mut stats = DocumentStats::default();

        // 12pt appears most (weighted by character count)
        stats.font_sizes.insert(120, 500); // 12.0pt * 10 = 120, weight 500
        stats.font_sizes.insert(160, 50); // 16.0pt * 10 = 160, weight 50
        stats.font_sizes.insert(100, 100); // 10.0pt * 10 = 100, weight 100

        stats.calculate_base_font_size();

        assert_eq!(stats.base_font_size, Some(12.0));
    }

    #[test]
    fn test_is_larger_than_base() {
        let stats = DocumentStats {
            base_font_size: Some(12.0),
            ..Default::default()
        };

        // 1.2x threshold (12.0 * 1.2 = 14.4)
        assert!(stats.is_larger_than_base(14.5, 1.2)); // slightly above 1.2x
        assert!(stats.is_larger_than_base(16.0, 1.2)); // 1.33x
        assert!(!stats.is_larger_than_base(12.0, 1.2)); // 1.0x
        assert!(!stats.is_larger_than_base(14.0, 1.2)); // 1.16x (under threshold)
    }
}
