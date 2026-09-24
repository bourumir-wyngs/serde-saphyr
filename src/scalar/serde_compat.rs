//! Scalar spelling rules retained by the Serde APIs for compatibility.
//!
//! These are deliberately distinct from the standard YAML schemas: boolean case
//! folding and integer syntax have independent options. Serde callers apply
//! their own whitespace trimming before using these helpers.

pub(super) fn bool_value(text: &str, strict: bool) -> Option<bool> {
    if text.eq_ignore_ascii_case("true")
        || (!strict
            && (text.eq_ignore_ascii_case("yes")
                || text.eq_ignore_ascii_case("y")
                || text.eq_ignore_ascii_case("on")))
    {
        Some(true)
    } else if text.eq_ignore_ascii_case("false")
        || (!strict
            && (text.eq_ignore_ascii_case("no")
                || text.eq_ignore_ascii_case("n")
                || text.eq_ignore_ascii_case("off")))
    {
        Some(false)
    } else {
        None
    }
}

pub(super) fn is_null(text: &str) -> bool {
    text.is_empty() || text == "~" || text.eq_ignore_ascii_case("null")
}

/// Validate integer syntax without limiting the magnitude to a machine type.
/// The sign is returned separately so unsigned conversions can reject `-0`.
pub(super) fn integer_parts(text: &str, legacy_octal: bool) -> Option<(bool, u32, &str)> {
    let (negative, unsigned) = if let Some(unsigned) = text.strip_prefix('-') {
        (true, unsigned)
    } else {
        (false, text.strip_prefix('+').unwrap_or(text))
    };
    let (radix, digits) = if let Some(digits) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
    {
        (16, digits)
    } else if let Some(digits) = unsigned
        .strip_prefix("0o")
        .or_else(|| unsigned.strip_prefix("0O"))
    {
        (8, digits)
    } else if let Some(digits) = unsigned
        .strip_prefix("0b")
        .or_else(|| unsigned.strip_prefix("0B"))
    {
        (2, digits)
    } else if legacy_octal && unsigned.starts_with('0') {
        (8, if unsigned == "0" { "0" } else { &unsigned[1..] })
    } else {
        (10, unsigned)
    };
    let digits = if legacy_octal && radix != 10 {
        digits.strip_prefix('_').unwrap_or(digits)
    } else {
        digits
    };
    if digits.is_empty() || (radix == 10 && digits.starts_with('0') && digits != "0") {
        return None;
    }
    let mut previous_digit = false;
    for byte in digits.bytes() {
        if byte == b'_' && previous_digit {
            previous_digit = false;
        } else if char::from(byte).is_digit(radix) {
            previous_digit = true;
        } else {
            return None;
        }
    }
    previous_digit.then_some((negative, radix, digits))
}

/// Recognize Rust's decimal float syntax and the supported dotted specials.
/// Unlike primitive parsing, lexical classification is independent of range.
pub(super) fn is_float(text: &str) -> bool {
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    if unsigned.eq_ignore_ascii_case(".nan") || unsigned.eq_ignore_ascii_case(".inf") {
        return true;
    }
    super::is_float(text, super::Schema::Yaml12)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scalar::{ScalarError, ScalarKind, ScalarStyle, Schema, resolve};

    #[test]
    fn boolean_and_null_rules_preserve_case_folding_without_trimming() {
        assert_eq!(bool_value("TrUe", true), Some(true));
        assert_eq!(bool_value("oFf", true), None);
        assert_eq!(bool_value("oFf", false), Some(false));
        assert_eq!(bool_value("yes", false), Some(true));
        assert_eq!(bool_value(" true ", true), None);
        assert!(is_null("nUlL"));
        assert!(!is_null(" null "));
        assert!(!is_null(" "));
    }

    #[test]
    fn integer_compatibility_preserves_independent_octal_policy() {
        assert_eq!(integer_parts("-0Xf_F", false), Some((true, 16, "f_F")));
        assert_eq!(integer_parts("+0B1_0", false), Some((false, 2, "1_0")));
        assert_eq!(integer_parts("-0", false), Some((true, 10, "0")));
        assert_eq!(integer_parts("0o_7", false), None);
        assert_eq!(integer_parts("0o_7", true), Some((false, 8, "7")));
        assert_eq!(integer_parts("0_7", true), Some((false, 8, "7")));
        assert_eq!(integer_parts("007", true), Some((false, 8, "07")));
        assert_eq!(integer_parts("007", false), None);
        for text in [
            "0x__f", "0b2", "08", "1__2", "1_", "_1", "--1", "+-1", " 1 ",
        ] {
            assert_eq!(integer_parts(text, true), None, "{text}");
        }
        let oversized = "340282366920938463463374607431768211456";
        assert_eq!(
            integer_parts(oversized, false),
            Some((false, 10, oversized))
        );
    }

    #[test]
    fn float_syntax_and_conversion_keep_distinct_range_checks() {
        for text in ["1e999", "+.5e999", "-01.", "42", "-.nAn", "+.InF"] {
            assert!(is_float(text), "{text}");
        }
        for text in [
            "inf", "NaN", "infinity", "1_0.0", ".", "1e", "1e+-2", "--1", " 1 ",
        ] {
            assert!(!is_float(text), "{text}");
        }
        let float = |text| {
            resolve(
                text,
                ScalarStyle::Plain,
                Some("tag:yaml.org,2002:float"),
                Schema::Specific {
                    strict_booleans: false,
                    legacy_octal_numbers: false,
                    quote_all: false,
                },
            )
        };
        assert_eq!(
            float("1e999").unwrap().to_f64(),
            Err(ScalarError::OutOfRange)
        );
        assert_eq!(
            float("1e39").unwrap().to_f32(),
            Err(ScalarError::OutOfRange)
        );
        assert!(float("1e39").unwrap().to_f64().unwrap().is_finite());
        assert!(float("-.NaN").unwrap().to_f64().unwrap().is_nan());
        assert_eq!(float("-.InF").unwrap().to_f64(), Ok(f64::NEG_INFINITY));
        assert_eq!(
            float("inf"),
            Err(ScalarError::InvalidValue {
                kind: ScalarKind::Float
            })
        );
    }
}
