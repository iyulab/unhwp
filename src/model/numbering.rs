//! Paragraph numbering: the number or bullet a paragraph is shown with.
//!
//! A numbered paragraph holds no number in its text: its paragraph shape says it is numbered
//! (or outlined, or bulleted), at a level, by a numbering definition, and the number is
//! counted by reading the paragraphs in order. The definitions come from the document's
//! header -- HWP 5.0 `HWPTAG_NUMBERING` and `HWPTAG_BULLET`, HWPX `hh:numbering` and
//! `hh:bullet` -- and [`NumberingState`] does the counting.

use super::StyleRegistry;
use serde::Serialize;
use std::collections::HashMap;

/// How many levels a numbering definition holds.
pub const NUMBERING_LEVELS: usize = 7;

/// What kind of head a paragraph shape gives its paragraphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum HeadKind {
    /// Counted by the section's outline numbering.
    Outline,
    /// Counted by the numbering definition the paragraph shape names.
    Number,
    /// A bullet character, from the bullet definition the paragraph shape names.
    Bullet,
}

/// The head a paragraph shape gives its paragraphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ParaHead {
    /// The kind of head.
    pub kind: HeadKind,
    /// The level, from 0 (the first level).
    pub level: u8,
    /// The numbering or bullet definition, from 1; 0 for none (an outline paragraph is
    /// counted by its section's outline numbering instead).
    pub id: u32,
}

/// How a counter is written: the number shapes of HWP 5.0 (table 134) and of the HWPX
/// `numFormat` names, which list them in the same order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum NumberShape {
    /// 1, 2, 3
    #[default]
    Digit,
    /// ①, ②, ③
    CircledDigit,
    /// I, II, III
    RomanCapital,
    /// i, ii, iii
    RomanSmall,
    /// A, B, C
    LatinCapital,
    /// a, b, c
    LatinSmall,
    /// Ⓐ, Ⓑ, Ⓒ
    CircledLatinCapital,
    /// ⓐ, ⓑ, ⓒ
    CircledLatinSmall,
    /// 가, 나, 다
    HangulSyllable,
    /// ㉮, ㉯, ㉰
    CircledHangulSyllable,
    /// ㄱ, ㄴ, ㄷ
    HangulJamo,
    /// ㉠, ㉡, ㉢
    CircledHangulJamo,
    /// 일, 이, 삼
    HangulPhonetic,
    /// 一, 二, 三
    Ideograph,
    /// ㊀, ㊁, ㊂
    CircledIdeograph,
    /// 갑, 을, 병
    HeavenlyStem,
    /// 甲, 乙, 丙
    HeavenlyStemHanja,
}

impl NumberShape {
    /// The shape of an HWP 5.0 number-shape code; a code outside the table (a symbol or a
    /// user character) is written as digits, the way the editor writes it.
    pub fn from_code(code: u32) -> Self {
        match code {
            1 => Self::CircledDigit,
            2 => Self::RomanCapital,
            3 => Self::RomanSmall,
            4 => Self::LatinCapital,
            5 => Self::LatinSmall,
            6 => Self::CircledLatinCapital,
            7 => Self::CircledLatinSmall,
            8 => Self::HangulSyllable,
            9 => Self::CircledHangulSyllable,
            10 => Self::HangulJamo,
            11 => Self::CircledHangulJamo,
            12 => Self::HangulPhonetic,
            13 => Self::Ideograph,
            14 => Self::CircledIdeograph,
            15 => Self::HeavenlyStem,
            16 => Self::HeavenlyStemHanja,
            _ => Self::Digit,
        }
    }

    /// The shape of an HWPX `numFormat` name; a name this does not know is digits.
    pub fn from_name(name: &str) -> Self {
        match name {
            "CIRCLED_DIGIT" => Self::CircledDigit,
            "ROMAN_CAPITAL" => Self::RomanCapital,
            "ROMAN_SMALL" => Self::RomanSmall,
            "LATIN_CAPITAL" => Self::LatinCapital,
            "LATIN_SMALL" => Self::LatinSmall,
            "CIRCLED_LATIN_CAPITAL" => Self::CircledLatinCapital,
            "CIRCLED_LATIN_SMALL" => Self::CircledLatinSmall,
            "HANGUL_SYLLABLE" => Self::HangulSyllable,
            "CIRCLED_HANGUL_SYLLABLE" => Self::CircledHangulSyllable,
            "HANGUL_JAMO" => Self::HangulJamo,
            "CIRCLED_HANGUL_JAMO" => Self::CircledHangulJamo,
            "HANGUL_PHONETIC" => Self::HangulPhonetic,
            "IDEOGRAPH" => Self::Ideograph,
            "CIRCLED_IDEOGRAPH" => Self::CircledIdeograph,
            "DECAGON_CIRCLE" => Self::HeavenlyStem,
            "DECAGON_CIRCLE_HANJA" => Self::HeavenlyStemHanja,
            _ => Self::Digit,
        }
    }

    /// `n` (from 1) written in this shape. A number past the end of a shape's run of
    /// characters (the 14 Hangul syllables, the 10 stems) is written as digits.
    pub fn format(self, n: u32) -> String {
        let index = n.saturating_sub(1) as usize;
        let nth = |run: &[char]| run.get(index).map(|c| c.to_string());
        let from = |base: u32, count: usize| -> Option<String> {
            if index < count {
                char::from_u32(base + index as u32).map(String::from)
            } else {
                None
            }
        };
        let written = match self {
            Self::Digit => None,
            // ① to ⑳, ㉑ to ㉟, ㊱ to ㊿
            Self::CircledDigit => match n {
                1..=20 => from(0x2460, 20),
                21..=35 => char::from_u32(0x3251 + (n - 21)).map(String::from),
                36..=50 => char::from_u32(0x32B1 + (n - 36)).map(String::from),
                _ => None,
            },
            Self::RomanCapital => roman(n, false),
            Self::RomanSmall => roman(n, true),
            Self::LatinCapital => latin(n, b'A'),
            Self::LatinSmall => latin(n, b'a'),
            Self::CircledLatinCapital => from(0x24B6, 26),
            Self::CircledLatinSmall => from(0x24D0, 26),
            Self::HangulSyllable => nth(&[
                '가', '나', '다', '라', '마', '바', '사', '아', '자', '차', '카', '타', '파', '하',
            ]),
            Self::CircledHangulSyllable => from(0x326E, 14),
            Self::HangulJamo => nth(&[
                'ㄱ', 'ㄴ', 'ㄷ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅅ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
            ]),
            Self::CircledHangulJamo => from(0x3260, 14),
            Self::HangulPhonetic => {
                nth(&['일', '이', '삼', '사', '오', '육', '칠', '팔', '구', '십'])
            }
            Self::Ideograph => nth(&['一', '二', '三', '四', '五', '六', '七', '八', '九', '十']),
            Self::CircledIdeograph => from(0x3280, 10),
            Self::HeavenlyStem => {
                nth(&['갑', '을', '병', '정', '무', '기', '경', '신', '임', '계'])
            }
            Self::HeavenlyStemHanja => {
                nth(&['甲', '乙', '丙', '丁', '戊', '己', '庚', '辛', '壬', '癸'])
            }
        };
        written.unwrap_or_else(|| n.to_string())
    }
}

/// `n` as a Roman numeral, for 1 to 3999.
fn roman(n: u32, small: bool) -> Option<String> {
    if !(1..=3999).contains(&n) {
        return None;
    }
    const PARTS: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut rest = n;
    let mut out = String::new();
    for (value, text) in PARTS {
        while rest >= value {
            out.push_str(text);
            rest -= value;
        }
    }
    Some(if small { out.to_lowercase() } else { out })
}

/// `n` as a Latin letter, A to Z; past Z the letter repeats (AA, BB, ...), as a word
/// processor's list does.
fn latin(n: u32, first: u8) -> Option<String> {
    if n == 0 {
        return None;
    }
    let letter = char::from(first + ((n - 1) % 26) as u8);
    let times = (n - 1) / 26 + 1;
    Some(std::iter::repeat_n(letter, times as usize).collect())
}

/// One level of a numbering definition.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct NumberLevel {
    /// How the counter of this level is written.
    pub shape: NumberShape,
    /// The text the head is written from: `^n` stands for the counter of level `n` (from 1),
    /// any other character for itself -- `^1.` is "1.", `(^5)` is "(1)".
    pub format: String,
    /// The number the first paragraph of the level is given.
    pub start: u32,
}

/// A numbering definition: the format of each of its levels.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Numbering {
    /// The levels, the first level first. Fewer than [`NUMBERING_LEVELS`] when the
    /// definition holds fewer.
    pub levels: Vec<NumberLevel>,
}

/// Counts the numbered paragraphs of one section, in reading order, and writes their heads.
///
/// The counters of a numbering definition run on from paragraph to paragraph: a paragraph
/// takes the next number of its level and starts the levels below it over. A section is
/// counted on its own, so the heads do not depend on whether the document is read whole or
/// section by section.
#[derive(Debug, Clone, Default)]
pub struct NumberingState {
    /// The numbering definition outline paragraphs are counted by; 0 for none.
    outline_id: u32,
    /// The current number of each level, by definition; `None` for a level not begun.
    counters: HashMap<u32, [Option<u32>; NUMBERING_LEVELS]>,
}

impl NumberingState {
    /// A state with no paragraph counted yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the numbering definition the section's outline paragraphs are counted by.
    pub fn set_outline_id(&mut self, id: u32) {
        self.outline_id = id;
    }

    /// Counts a paragraph with `head` and returns its head as written -- "1.", "가)", "●" --
    /// or `None` when it has none (no such definition, a level the definition does not hold,
    /// a format that writes nothing).
    pub fn label(&mut self, head: &ParaHead, styles: &StyleRegistry) -> Option<String> {
        let level = usize::from(head.level);
        match head.kind {
            HeadKind::Bullet => {
                let bullet = styles.bullet(head.id)?;
                (!bullet.is_whitespace() && bullet != '\0').then(|| bullet.to_string())
            }
            HeadKind::Number | HeadKind::Outline => {
                let id = if head.kind == HeadKind::Outline {
                    self.outline_id
                } else {
                    head.id
                };
                let numbering = styles.numbering(id)?;
                let definition = numbering.levels.get(level)?;
                let counters = self.counters.entry(id).or_default();
                counters[level] = Some(match counters[level] {
                    Some(count) => count.saturating_add(1),
                    None => definition.start,
                });
                for deeper in &mut counters[level + 1..] {
                    *deeper = None;
                }

                let mut label = String::new();
                let mut chars = definition.format.chars().peekable();
                while let Some(c) = chars.next() {
                    let referenced = (c == '^')
                        .then(|| chars.peek().and_then(|d| d.to_digit(10)))
                        .flatten()
                        .filter(|d| (1..=NUMBERING_LEVELS as u32).contains(d));
                    match referenced {
                        Some(d) => {
                            chars.next();
                            let at = d as usize - 1;
                            // A level above this one that was never counted shows its first number.
                            let shown = numbering
                                .levels
                                .get(at)
                                .map(|l| l.shape.format(counters[at].unwrap_or(l.start)));
                            label.push_str(&shown.unwrap_or_default());
                        }
                        None => label.push(c),
                    }
                }
                let label = label.trim().to_string();
                (!label.is_empty()).then_some(label)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(shape: NumberShape, format: &str) -> NumberLevel {
        NumberLevel {
            shape,
            format: format.to_string(),
            start: 1,
        }
    }

    /// The default definition of the editor: `1.` `가.` `1)` `가)` `(1)` `(가)` `①`.
    fn default_numbering() -> Numbering {
        Numbering {
            levels: vec![
                level(NumberShape::Digit, "^1."),
                level(NumberShape::HangulSyllable, "^2."),
                level(NumberShape::Digit, "^3)"),
                level(NumberShape::HangulSyllable, "^4)"),
                level(NumberShape::Digit, "(^5)"),
                level(NumberShape::HangulSyllable, "(^6)"),
                level(NumberShape::CircledDigit, "^7"),
            ],
        }
    }

    fn registry_with(id: u32, numbering: Numbering) -> StyleRegistry {
        let mut styles = StyleRegistry::new();
        styles.register_numbering(id, numbering);
        styles
    }

    fn number(level: u8, id: u32) -> ParaHead {
        ParaHead {
            kind: HeadKind::Number,
            level,
            id,
        }
    }

    #[test]
    fn shapes_write_their_counters() {
        let cases = [
            (NumberShape::Digit, 12, "12"),
            (NumberShape::CircledDigit, 3, "③"),
            (NumberShape::CircledDigit, 20, "⑳"),
            (NumberShape::CircledDigit, 21, "㉑"),
            (NumberShape::CircledDigit, 51, "51"),
            (NumberShape::RomanCapital, 4, "IV"),
            (NumberShape::RomanSmall, 14, "xiv"),
            (NumberShape::LatinCapital, 3, "C"),
            (NumberShape::LatinCapital, 27, "AA"),
            (NumberShape::LatinSmall, 2, "b"),
            (NumberShape::CircledLatinCapital, 1, "Ⓐ"),
            (NumberShape::CircledLatinSmall, 26, "ⓩ"),
            (NumberShape::HangulSyllable, 3, "다"),
            (NumberShape::HangulSyllable, 14, "하"),
            (NumberShape::HangulSyllable, 15, "15"),
            (NumberShape::CircledHangulSyllable, 1, "㉮"),
            (NumberShape::HangulJamo, 2, "ㄴ"),
            (NumberShape::CircledHangulJamo, 3, "㉢"),
            (NumberShape::HangulPhonetic, 3, "삼"),
            (NumberShape::Ideograph, 10, "十"),
            (NumberShape::CircledIdeograph, 2, "㊁"),
            (NumberShape::HeavenlyStem, 3, "병"),
            (NumberShape::HeavenlyStemHanja, 1, "甲"),
        ];
        for (shape, n, expected) in cases {
            assert_eq!(shape.format(n), expected, "{shape:?} {n}");
        }
    }

    #[test]
    fn shape_codes_and_names_agree() {
        let names = [
            "DIGIT",
            "CIRCLED_DIGIT",
            "ROMAN_CAPITAL",
            "ROMAN_SMALL",
            "LATIN_CAPITAL",
            "LATIN_SMALL",
            "CIRCLED_LATIN_CAPITAL",
            "CIRCLED_LATIN_SMALL",
            "HANGUL_SYLLABLE",
            "CIRCLED_HANGUL_SYLLABLE",
            "HANGUL_JAMO",
            "CIRCLED_HANGUL_JAMO",
            "HANGUL_PHONETIC",
            "IDEOGRAPH",
            "CIRCLED_IDEOGRAPH",
            "DECAGON_CIRCLE",
            "DECAGON_CIRCLE_HANJA",
        ];
        for (code, name) in names.iter().enumerate() {
            assert_eq!(
                NumberShape::from_code(code as u32),
                NumberShape::from_name(name)
            );
        }
        // Outside the table: digits.
        assert_eq!(NumberShape::from_code(0x80), NumberShape::Digit);
        assert_eq!(NumberShape::from_name("SYMBOL"), NumberShape::Digit);
    }

    #[test]
    fn a_level_counts_on_and_starts_the_levels_below_it_over() {
        let styles = registry_with(1, default_numbering());
        let mut state = NumberingState::new();
        let mut labels = Vec::new();
        for level in [0, 1, 1, 2, 0, 1] {
            labels.push(state.label(&number(level, 1), &styles).unwrap());
        }
        assert_eq!(labels, ["1.", "가.", "나.", "1)", "2.", "가."]);
    }

    #[test]
    fn a_format_can_write_the_counters_of_several_levels() {
        let numbering = Numbering {
            levels: vec![
                level(NumberShape::Digit, "제^1장"),
                level(NumberShape::Digit, "^1.^2."),
            ],
        };
        let styles = registry_with(1, numbering);
        let mut state = NumberingState::new();
        assert_eq!(state.label(&number(0, 1), &styles).unwrap(), "제1장");
        assert_eq!(state.label(&number(1, 1), &styles).unwrap(), "1.1.");
        assert_eq!(state.label(&number(1, 1), &styles).unwrap(), "1.2.");
        assert_eq!(state.label(&number(0, 1), &styles).unwrap(), "제2장");
        assert_eq!(state.label(&number(1, 1), &styles).unwrap(), "2.1.");
    }

    #[test]
    fn a_level_begun_without_its_parents_shows_their_first_numbers() {
        let numbering = Numbering {
            levels: vec![
                level(NumberShape::Digit, "^1."),
                level(NumberShape::Digit, "^1.^2."),
            ],
        };
        let styles = registry_with(1, numbering);
        assert_eq!(
            NumberingState::new().label(&number(1, 1), &styles).unwrap(),
            "1.1."
        );
    }

    #[test]
    fn a_level_starts_from_its_start_number() {
        let mut numbering = default_numbering();
        numbering.levels[0].start = 5;
        let styles = registry_with(1, numbering);
        let mut state = NumberingState::new();
        assert_eq!(state.label(&number(0, 1), &styles).unwrap(), "5.");
        assert_eq!(state.label(&number(0, 1), &styles).unwrap(), "6.");
    }

    #[test]
    fn definitions_count_separately() {
        let mut styles = registry_with(1, default_numbering());
        styles.register_numbering(2, default_numbering());
        let mut state = NumberingState::new();
        assert_eq!(state.label(&number(0, 1), &styles).unwrap(), "1.");
        assert_eq!(state.label(&number(0, 2), &styles).unwrap(), "1.");
        assert_eq!(state.label(&number(0, 1), &styles).unwrap(), "2.");
    }

    #[test]
    fn outline_paragraphs_are_counted_by_the_sections_outline_numbering() {
        let styles = registry_with(3, default_numbering());
        let outline = ParaHead {
            kind: HeadKind::Outline,
            level: 0,
            id: 0,
        };
        let mut state = NumberingState::new();
        assert_eq!(
            state.label(&outline, &styles),
            None,
            "no outline numbering set"
        );
        state.set_outline_id(3);
        assert_eq!(state.label(&outline, &styles).unwrap(), "1.");
        assert_eq!(state.label(&outline, &styles).unwrap(), "2.");
    }

    #[test]
    fn a_bullet_is_its_character_and_a_missing_definition_is_no_head() {
        let mut styles = StyleRegistry::new();
        styles.register_bullet(1, '●');
        styles.register_bullet(2, '\0');
        let bullet = |id| ParaHead {
            kind: HeadKind::Bullet,
            level: 0,
            id,
        };
        let mut state = NumberingState::new();
        assert_eq!(state.label(&bullet(1), &styles).unwrap(), "●");
        assert_eq!(state.label(&bullet(2), &styles), None);
        assert_eq!(state.label(&bullet(9), &styles), None);
        assert_eq!(state.label(&number(0, 9), &styles), None);
    }

    #[test]
    fn a_level_the_definition_does_not_hold_has_no_head() {
        let styles = registry_with(1, default_numbering());
        assert_eq!(NumberingState::new().label(&number(7, 1), &styles), None);
    }
}
