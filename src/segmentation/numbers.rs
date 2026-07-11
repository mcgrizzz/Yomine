//! Numeric-token handling (mission objective #3; the "8月22日 → がつにち" bug).
//!
//! UniDic tokenizes multi-digit Arabic numerals one digit at a time (22 → 「2」ニ
//! + 「2」ニ), so naive pron concatenation reads 22 as ニニ. `merge_digit_runs`
//! folds each run of consecutive digit 数詞 tokens into one synthetic token whose
//! readings come from `number_to_katakana` — a place-value reading (22 →
//! ニジュウニ, 300 → サンビャク, 8000 → ハッセン, 10000 → イチマン). Runs it
//! can't read (>16 digits) keep their concatenated per-digit prons.

use super::{
    token_models::UnidicToken,
    unidic_tags::UnidicTag,
};

/// Fold runs of consecutive Arabic-digit 数詞 tokens into single tokens with a
/// synthesized number reading. Every other token passes through untouched.
/// Called at the head of the rule matcher so words, lemmas, terms, and phrase
/// detection all see the corrected token.
pub fn merge_digit_runs(tokens: Vec<UnidicToken>) -> Vec<UnidicToken> {
    let mut out: Vec<UnidicToken> = Vec::with_capacity(tokens.len());
    let mut run: Vec<UnidicToken> = Vec::new();

    for token in tokens {
        if is_digit_token(&token) {
            run.push(token);
        } else {
            flush_run(&mut run, &mut out);
            out.push(token);
        }
    }
    flush_run(&mut run, &mut out);
    out
}

fn flush_run(run: &mut Vec<UnidicToken>, out: &mut Vec<UnidicToken>) {
    if run.is_empty() {
        return;
    }
    let surface: String = run.iter().map(|t| t.surface.as_str()).collect();
    // A lone single-digit token already reads correctly from the lexicon
    // (8 → ハチ). Everything else needs synthesis: multi-token runs read
    // digit-by-digit (22 → ニニ), and a single multi-digit OOV token's
    // "reading" is just its surface digits (3000 → "3000").
    if run.len() == 1 && surface.chars().count() == 1 {
        out.push(run.pop().unwrap());
        return;
    }
    let digits: String = surface.chars().map(normalize_digit).collect();
    match number_to_katakana(&digits) {
        Some(reading) => {
            let end_byte = run.last().unwrap().end_byte;
            let first = run.first().unwrap();
            out.push(UnidicToken {
                surface: surface.clone(),
                pos1: first.pos1.clone(),
                pos2: first.pos2.clone(),
                pos3: first.pos3.clone(),
                pos4: first.pos4.clone(),
                conjugation_type: first.conjugation_type.clone(),
                conjugation_form: first.conjugation_form.clone(),
                surface_hatsuon: reading.clone(),
                // UniDic lemmatizes digits to kanji numerals (2 → 二); for a merged
                // number the Arabic surface is the sensible dictionary form.
                lemma_form: surface,
                lemma_hatsuon: reading,
                start_byte: first.start_byte,
                end_byte,
            });
            run.clear();
        }
        // Unreadable (>16 digits): pass the original tokens through untouched.
        None => out.append(run),
    }
}

/// A token that is purely Arabic digits (ASCII or fullwidth) tagged 数詞.
fn is_digit_token(token: &UnidicToken) -> bool {
    token.pos2 == UnidicTag::Suushi
        && !token.surface.is_empty()
        && token.surface.chars().all(|c| c.is_ascii_digit() || ('０'..='９').contains(&c))
}

fn normalize_digit(c: char) -> char {
    if ('０'..='９').contains(&c) {
        char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap()
    } else {
        c
    }
}

const DIGITS: [&str; 10] =
    ["", "イチ", "ニ", "サン", "ヨン", "ゴ", "ロク", "ナナ", "ハチ", "キュウ"];
const ZERO_DIGITS: [&str; 10] =
    ["ゼロ", "イチ", "ニ", "サン", "ヨン", "ゴ", "ロク", "ナナ", "ハチ", "キュウ"];

/// Place-value katakana reading of an unsigned decimal string (ASCII digits).
/// `0` → ゼロ; numbers with leading zeros are read digit-by-digit (007 →
/// ゼロゼロナナ); anything longer than 16 digits (>兆 range) → `None`.
pub fn number_to_katakana(digits: &str) -> Option<String> {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) || digits.len() > 16 {
        return None;
    }
    // Leading zeros: a code/ID, not a quantity — read each digit.
    if digits.len() > 1 && digits.starts_with('0') {
        return Some(digits.bytes().map(|b| ZERO_DIGITS[(b - b'0') as usize]).collect::<String>());
    }
    let n: u64 = digits.parse().ok()?;
    if n == 0 {
        return Some("ゼロ".to_string());
    }

    // Myriad grouping: 兆 / 億 / 万 / (units), each group read as 1–9999.
    let mut out = String::new();
    let groups: [(u64, &str); 3] =
        [(1_0000_0000_0000, "チョウ"), (1_0000_0000, "オク"), (1_0000, "マン")];
    let mut rest = n;
    for (unit, name) in groups {
        let g = rest / unit;
        if g > 0 {
            out.push_str(&small_group(g as u16));
            out.push_str(name);
        }
        rest %= unit;
    }
    if rest > 0 {
        out.push_str(&small_group(rest as u16));
    }
    Some(out)
}

/// Reading of 1..=9999 with the standard euphonic irregulars
/// (300 サンビャク, 600 ロッピャク, 800 ハッピャク, 3000 サンゼン, 8000 ハッセン).
fn small_group(n: u16) -> String {
    let mut out = String::new();
    let (thousands, rest) = (n / 1000, n % 1000);
    let (hundreds, rest) = (rest / 100, rest % 100);
    let (tens, units) = (rest / 10, rest % 10);

    match thousands {
        0 => {}
        1 => out.push_str("セン"),
        3 => out.push_str("サンゼン"),
        8 => out.push_str("ハッセン"),
        d => {
            out.push_str(DIGITS[d as usize]);
            out.push_str("セン");
        }
    }
    match hundreds {
        0 => {}
        1 => out.push_str("ヒャク"),
        3 => out.push_str("サンビャク"),
        6 => out.push_str("ロッピャク"),
        8 => out.push_str("ハッピャク"),
        d => {
            out.push_str(DIGITS[d as usize]);
            out.push_str("ヒャク");
        }
    }
    match tens {
        0 => {}
        1 => out.push_str("ジュウ"),
        d => {
            out.push_str(DIGITS[d as usize]);
            out.push_str("ジュウ");
        }
    }
    out.push_str(DIGITS[units as usize]);
    out
}

#[cfg(test)]
mod tests {
    use super::number_to_katakana;

    #[test]
    fn place_value_readings() {
        let cases = [
            ("0", "ゼロ"),
            ("7", "ナナ"),
            ("10", "ジュウ"),
            ("22", "ニジュウニ"),
            ("100", "ヒャク"),
            ("300", "サンビャク"),
            ("600", "ロッピャク"),
            ("816", "ハッピャクジュウロク"),
            ("1000", "セン"),
            ("3000", "サンゼン"),
            ("8000", "ハッセン"),
            ("10000", "イチマン"),
            ("12500", "イチマンニセンゴヒャク"),
            ("100000000", "イチオク"),
            ("2024", "ニセンニジュウヨン"),
            ("007", "ゼロゼロナナ"),
        ];
        for (digits, expected) in cases {
            assert_eq!(number_to_katakana(digits).as_deref(), Some(expected), "for {digits}");
        }
        assert_eq!(number_to_katakana("12345678901234567"), None); // 17 digits
        assert_eq!(number_to_katakana("12a"), None);
    }
}
