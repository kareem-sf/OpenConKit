//! Deterministic bilingual value normalization for BOQ inference.

use std::collections::BTreeSet;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Parsed numeric text plus interpretation metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedNumber {
    pub value: Decimal,
    pub is_percent: bool,
    /// Integer confidence percentage, avoiding floating-point persistence.
    pub confidence_percent: u8,
    pub normalized: String,
}

/// Canonical unit identity used for consistency checks (never conversion).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedUnit {
    pub canonical: String,
    pub dimension: String,
}

/// Explicit currency evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedCurrency {
    pub code: String,
    pub confidence_percent: u8,
}

/// Parse common Western/Arabic numeric forms without using the OS locale.
pub fn parse_number(raw: &str) -> Option<ParsedNumber> {
    let mut text = normalize_digits_and_signs(raw);
    text.retain(|character| !character.is_whitespace() && character != '\u{00a0}');
    if text.is_empty() {
        return None;
    }

    let mut negative = false;
    if text.starts_with('(') && text.ends_with(')') {
        negative = true;
        text = text[1..text.len().saturating_sub(1)].to_string();
    }
    if text.ends_with('-') {
        negative = true;
        text.pop();
    }
    let is_percent = text.ends_with('%');
    if is_percent {
        text.pop();
    }
    text = trim_numeric_affixes(&text);
    if text.is_empty() {
        return None;
    }
    if !text.chars().all(|character| {
        character.is_ascii_digit() || matches!(character, '.' | ',' | '+' | '-' | 'e' | 'E')
    }) {
        return None;
    }

    let (normalized, confidence_percent) = normalize_separators(&text)?;
    let mut value = if normalized.contains(['e', 'E']) {
        Decimal::from_scientific(&normalized).ok()?
    } else {
        Decimal::from_str(&normalized).ok()?
    };
    if negative {
        value.set_sign_negative(true);
    }
    if is_percent {
        value /= Decimal::from(100u32);
    }
    Some(ParsedNumber {
        value,
        is_percent,
        confidence_percent,
        normalized,
    })
}

fn normalize_digits_and_signs(raw: &str) -> String {
    raw.trim()
        .chars()
        .map(|character| match character {
            '٠' | '۰' => '0',
            '١' | '۱' => '1',
            '٢' | '۲' => '2',
            '٣' | '۳' => '3',
            '٤' | '۴' => '4',
            '٥' | '۵' => '5',
            '٦' | '۶' => '6',
            '٧' | '۷' => '7',
            '٨' | '۸' => '8',
            '٩' | '۹' => '9',
            '٫' => '.',
            '٬' => ',',
            '−' | '–' | '—' => '-',
            '٪' => '%',
            other => other,
        })
        .collect()
}

fn trim_numeric_affixes(raw: &str) -> String {
    raw.trim_matches(|character: char| {
        !(character.is_ascii_digit() || matches!(character, '.' | ',' | '+' | '-' | 'e' | 'E'))
    })
    .to_string()
}

fn normalize_separators(raw: &str) -> Option<(String, u8)> {
    let dot_positions: Vec<usize> = raw.match_indices('.').map(|(index, _)| index).collect();
    let comma_positions: Vec<usize> = raw.match_indices(',').map(|(index, _)| index).collect();

    if !dot_positions.is_empty() && !comma_positions.is_empty() {
        let decimal = if dot_positions.last()? > comma_positions.last()? {
            '.'
        } else {
            ','
        };
        return Some((rewrite_with_decimal(raw, decimal), 95));
    }
    let separator = if !dot_positions.is_empty() {
        Some('.')
    } else if !comma_positions.is_empty() {
        Some(',')
    } else {
        None
    };
    let Some(separator) = separator else {
        return Some((raw.to_string(), 100));
    };
    let positions: Vec<usize> = raw
        .match_indices(separator)
        .map(|(index, _)| index)
        .collect();
    if positions.len() > 1 {
        let groups: Vec<&str> = raw.split(separator).collect();
        let thousands = groups
            .iter()
            .skip(1)
            .all(|group| group.len() == 3 && group.bytes().all(|byte| byte.is_ascii_digit()));
        if thousands {
            return Some((raw.replace(separator, ""), 90));
        }
        return Some((rewrite_with_decimal(raw, separator), 70));
    }
    let position = *positions.first()?;
    let trailing_digits = raw.len().saturating_sub(position + 1);
    let leading_digits = raw[..position]
        .trim_start_matches(['+', '-'])
        .bytes()
        .filter(|byte| byte.is_ascii_digit())
        .count();
    if trailing_digits == 3 && (1..=3).contains(&leading_digits) {
        Some((raw.replace(separator, ""), 70))
    } else {
        Some((rewrite_with_decimal(raw, separator), 90))
    }
}

fn rewrite_with_decimal(raw: &str, decimal_separator: char) -> String {
    let last_decimal = raw.rfind(decimal_separator);
    let mut output = String::with_capacity(raw.len());
    for (index, character) in raw.char_indices() {
        if character == '.' || character == ',' {
            if Some(index) == last_decimal {
                output.push('.');
            }
        } else {
            output.push(character);
        }
    }
    output
}

/// Normalize Arabic/English text for matching while preserving source text
/// elsewhere.
pub fn normalize_text(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut previous_space = true;
    for character in raw.to_lowercase().chars() {
        let character = match character {
            '\u{064b}'..='\u{065f}' | '\u{0670}' | '\u{0640}' => continue,
            'أ' | 'إ' | 'آ' => 'ا',
            'ى' => 'ي',
            'ة' => 'ه',
            '²' => '2',
            '³' => '3',
            other => other,
        };
        if character.is_alphanumeric() {
            output.push(character);
            previous_space = false;
        } else if !previous_space {
            output.push(' ');
            previous_space = true;
        }
    }
    output.trim().to_string()
}

/// Recognize common metric/imperial and BOQ unit aliases.
pub fn normalize_unit(raw: &str) -> Option<NormalizedUnit> {
    let compact = normalize_text(raw).replace(' ', "");
    let (canonical, dimension) = match compact.as_str() {
        "m" | "meter" | "meters" | "metre" | "metres" | "م" | "متر" => ("m", "length"),
        "m2" | "sqm" | "sqmeter" | "sqmeters" | "squaremeter" | "squaremetre" | "م2"
        | "مترمربع" => ("m2", "area"),
        "m3" | "cum" | "cubicmeter" | "cubicmetre" | "م3" | "مترمكعب" => ("m3", "volume"),
        "mm" | "millimeter" | "millimetre" | "مم" => ("mm", "length"),
        "cm" | "centimeter" | "centimetre" | "سم" => ("cm", "length"),
        "kg" | "kilogram" | "kilograms" | "كجم" | "كيلوجرام" => ("kg", "mass"),
        "t" | "ton" | "tons" | "tonne" | "tonnes" | "طن" => ("t", "mass"),
        "l" | "liter" | "litre" | "liters" | "litres" | "لتر" => ("l", "volume"),
        "ft" | "foot" | "feet" | "قدم" => ("ft", "length"),
        "ft2" | "sqft" | "squarefoot" | "قدممربع" => ("ft2", "area"),
        "ft3" | "cuft" | "cubicfoot" | "قدممكعب" => ("ft3", "volume"),
        "no" | "nos" | "nr" | "number" | "each" | "ea" | "عدد" | "قطعه" => ("no", "count"),
        "ls" | "lumpsum" | "مقطوعيه" | "مقطوع" => ("ls", "lump_sum"),
        "%" | "percent" | "percentage" | "نسبه" => ("%", "ratio"),
        _ => return None,
    };
    Some(NormalizedUnit {
        canonical: canonical.to_string(),
        dimension: dimension.to_string(),
    })
}

/// Detect an explicit currency code or symbol without assuming a workbook
/// base currency.
pub fn detect_currency(raw: &str) -> Option<DetectedCurrency> {
    let normalized = normalize_text(raw);
    let upper = raw.to_ascii_uppercase();
    let known = [
        ("EGP", ["EGP", "ج م", "جنيه", "جنيه مصري"].as_slice()),
        ("USD", ["USD", "US DOLLAR", "دولار امريكي"].as_slice()),
        ("EUR", ["EUR", "EURO", "يورو"].as_slice()),
        ("GBP", ["GBP", "STERLING", "جنيه استرليني"].as_slice()),
        ("SAR", ["SAR", "ر س", "ريال سعودي"].as_slice()),
        ("AED", ["AED", "د ا", "درهم اماراتي"].as_slice()),
        ("QAR", ["QAR", "ريال قطري"].as_slice()),
        ("KWD", ["KWD", "دينار كويتي"].as_slice()),
    ];
    for (code, aliases) in known {
        if aliases.iter().any(|alias| {
            upper.contains(&alias.to_ascii_uppercase())
                || normalized.contains(&normalize_text(alias))
        }) {
            return Some(DetectedCurrency {
                code: code.to_string(),
                confidence_percent: 100,
            });
        }
    }
    let (code, confidence_percent) = if raw.contains('€') {
        ("EUR", 95)
    } else if raw.contains('£') {
        ("GBP", 80)
    } else if raw.contains('$') {
        ("USD", 70)
    } else {
        return None;
    };
    Some(DetectedCurrency {
        code: code.to_string(),
        confidence_percent,
    })
}

/// Deterministic token-set similarity in `0.0..=1.0`.
pub fn text_similarity(left: &str, right: &str) -> f64 {
    let left: BTreeSet<String> = normalize_text(left)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let right: BTreeSet<String> = normalize_text(right)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(&right).count();
    let union = left.union(&right).count();
    intersection as f64 / union as f64
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn parses_western_arabic_and_negative_forms() {
        assert_eq!(
            parse_number("1,250.50").expect("number").value,
            Decimal::new(125_050, 2)
        );
        assert_eq!(
            parse_number("١٬٢٥٠٫٥٠").expect("number").value,
            Decimal::new(125_050, 2)
        );
        assert_eq!(
            parse_number("(25.5)").expect("number").value,
            Decimal::new(-255, 1)
        );
        assert_eq!(
            parse_number("12,5-").expect("number").value,
            Decimal::new(-125, 1)
        );
    }

    #[test]
    fn percent_is_explicit_and_scaled() {
        let parsed = parse_number("12.5%").expect("percent");
        assert!(parsed.is_percent);
        assert_eq!(parsed.value, Decimal::new(125, 3));
    }

    #[test]
    fn normalizes_bilingual_units_without_converting() {
        assert_eq!(normalize_unit("متر مربع").expect("unit").canonical, "m2");
        assert_eq!(normalize_unit("sq. ft").expect("unit").canonical, "ft2");
        assert_eq!(normalize_unit("كجم").expect("unit").dimension, "mass");
        assert!(normalize_unit("mystery").is_none());
    }

    #[test]
    fn detects_only_explicit_currency_evidence() {
        assert_eq!(detect_currency("EGP").expect("currency").code, "EGP");
        assert_eq!(detect_currency("ريال سعودي").expect("currency").code, "SAR");
        assert_eq!(
            detect_currency("$").expect("currency").confidence_percent,
            70
        );
        assert!(detect_currency("100").is_none());
    }

    #[test]
    fn text_normalization_and_similarity_are_deterministic() {
        assert_eq!(normalize_text("أعمالُ الخرسانة"), "اعمال الخرسانه");
        assert_eq!(
            text_similarity("reinforced concrete wall", "concrete wall reinforced"),
            1.0
        );
        assert!(text_similarity("concrete wall", "steel door") < 0.2);
    }
}
