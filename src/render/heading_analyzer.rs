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
}

impl Default for HeadingConfig {
    fn default() -> Self {
        Self {
            max_heading_level: 4,
            max_text_length: 80,
            trust_explicit_styles: true,
            analyze_sequences: true,
            min_sequence_count: 2,
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
        matches!(self, HeadingDecision::Explicit(_) | HeadingDecision::Inferred(_))
    }

    /// Get the heading level if this is a heading.
    pub fn level(&self) -> Option<u8> {
        match self {
            HeadingDecision::Explicit(level) | HeadingDecision::Inferred(level) => Some(*level),
            _ => None,
        }
    }
}

/// Analyzer for sophisticated heading detection.
pub struct HeadingAnalyzer {
    config: HeadingConfig,
}

impl HeadingAnalyzer {
    /// Create a new heading analyzer with the given configuration.
    pub fn new(config: HeadingConfig) -> Self {
        Self { config }
    }

    /// Create a heading analyzer with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(HeadingConfig::default())
    }

    /// Analyze a document and return heading decisions for all paragraphs.
    ///
    /// Returns a vector of decisions, one per paragraph block in document order.
    /// Table blocks are skipped (not included in the output).
    pub fn analyze(&self, doc: &Document) -> Vec<HeadingDecision> {
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

        self.analyze_paragraphs(&paragraphs)
    }

    /// Analyze a sequence of paragraphs with context awareness.
    pub fn analyze_paragraphs(&self, paragraphs: &[&Paragraph]) -> Vec<HeadingDecision> {
        let mut decisions = Vec::with_capacity(paragraphs.len());

        // First pass: make initial decisions
        for para in paragraphs {
            decisions.push(self.decide_heading(para));
        }

        // Second pass: check sequential patterns if enabled
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

        // P1: Explicit style with full trust (skip all exclusion checks)
        if style.heading_level > 0 && self.config.trust_explicit_styles {
            let level = self.cap_heading_level(style.heading_level);
            return HeadingDecision::Explicit(level);
        }

        // P2: Exclusion conditions - bullet markers
        if self.looks_like_bullet_item(trimmed) {
            return if style.heading_level > 0 {
                HeadingDecision::Demoted
            } else {
                HeadingDecision::None
            };
        }

        // Check text length
        if trimmed.chars().count() > self.config.max_text_length {
            return if style.heading_level > 0 {
                HeadingDecision::Demoted
            } else {
                HeadingDecision::None
            };
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

    /// Check if text looks like a bullet list item.
    ///
    /// Note: Numbered patterns (1., 가., a.) are NOT checked here.
    /// They are handled separately in sequence analysis.
    fn looks_like_bullet_item(&self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }

        // Common bullet/symbol markers (NOT numbered patterns)
        const BULLET_MARKERS: &[char] = &[
            'ㅇ', 'ㆍ', '○', '●', '◎', '■', '□', '▪', '▫', '◆', '◇', '★', '☆', '※', '•', '-', '–',
            '—', '→', '▶', '►', '▷', '▹', '◁', '◀', '◃', '◂', '·', '∙',
        ];

        let first_char = text.chars().next().unwrap();
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
    if chars.len() >= 2
        && is_korean_sequence_char(chars[0])
        && (chars[1] == '.' || chars[1] == ')')
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
                '장' => Some(KoreanChapterType::Jang), // Chapter
                '절' => Some(KoreanChapterType::Jeol), // Section
                '조' => Some(KoreanChapterType::Jo),   // Article (legal)
                '항' => Some(KoreanChapterType::Hang), // Paragraph (legal)
                '편' => Some(KoreanChapterType::Pyeon), // Part
                '부' => Some(KoreanChapterType::Bu),   // Division
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
                            Some(chars[num_end + 1..].iter().collect::<String>().trim().to_string())
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
            let has_separator = chars.get(1).map(|c| *c == '.' || *c == '-' || c.is_whitespace()).unwrap_or(false);
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
    use crate::model::{InlineContent, ParagraphStyle, TextRun};

    fn make_paragraph(text: &str, heading_level: u8) -> Paragraph {
        let mut para = Paragraph::with_style(ParagraphStyle {
            heading_level,
            ..Default::default()
        });
        para.content.push(InlineContent::Text(TextRun::new(text)));
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
        assert!(decisions.iter().all(|d| matches!(d, HeadingDecision::Demoted)));
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
        assert!(decisions.iter().all(|d| matches!(d, HeadingDecision::Demoted)));
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
}
