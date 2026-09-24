//! Conservative scalar resolution for string emission across YAML readers.
//!
//! The selected public schema handles standard spellings. Compatibility checks
//! also protect strings that other readers resolve differently or cannot construct.

use super::{ScalarKind, ScalarStyle, Schema, legacy};

pub(crate) fn is_ambiguous(text: &str, schema: Schema) -> bool {
    if !super::resolve(text, ScalarStyle::Plain, None, schema)
        .is_ok_and(|scalar| scalar.kind() == ScalarKind::String)
    {
        return true;
    }
    // Other schemas follow their selected vocabulary exactly. In particular,
    // Strings does not reinterpret boolean/numeric-looking strings, and JSON
    // rejects unmatched plain strings, which the error branch above quotes.
    if !matches!(schema, Schema::Yaml11 | Schema::Yaml12) {
        return false;
    }
    let yaml_12 = schema == Schema::Yaml12;
    text.eq_ignore_ascii_case("null")
        || text.eq_ignore_ascii_case("true")
        || text.eq_ignore_ascii_case("false")
        || is_numeric(text, yaml_12)
        // Preserve compatibility with readers accepting undotted float tokens.
        || text.eq_ignore_ascii_case("nan")
        || text.eq_ignore_ascii_case("inf")
        || text.eq_ignore_ascii_case("+inf")
        || text.eq_ignore_ascii_case("-inf")
        || (!yaml_12
            && (super::bool_value(
                text.trim(),
                Schema::Specific {
                    strict_booleans: false,
                    legacy_octal_numbers: false,
                    quote_all: false,
                    yaml_12_quoting: false,
                },
            )
            .is_some()
                || legacy::is_quoting_timestamp(text)))
}

/// Check syntax without constructing a value: overflow and resolver errors must
/// not turn a serialized string into a number or an unreadable document.
fn is_numeric(text: &str, yaml_12: bool) -> bool {
    if !matches!(
        text.as_bytes().first(),
        Some(b'0'..=b'9' | b'+' | b'-' | b'.' | b'_')
    ) {
        return false;
    }
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    if unsigned.eq_ignore_ascii_case(".inf") || unsigned.eq_ignore_ascii_case(".nan") {
        return true;
    }
    if !yaml_12 {
        if legacy::is_sexagesimal(text) {
            return true;
        }
        // The YAML 1.1 draft tags even an all-underscore radix suffix as numeric.
        if let Some((radix, digits)) = radix_digits(unsigned)
            && is_digit_run(digits, radix, false)
        {
            return true;
        }
    }

    // Go YAML removes underscores before resolution, including around prefixes
    // and exponents. YAML 1.2 output only accepts them between digits.
    let normalized;
    let text = if !yaml_12 && text.contains('_') {
        normalized = text.replace('_', "");
        normalized.as_str()
    } else {
        text
    };
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    if let Some((radix, digits)) = radix_digits(unsigned) {
        // Go's unsigned lowercase binary/octal fallback accepts an inner sign.
        let digits = if !yaml_12 && (text.starts_with("0b") || text.starts_with("0o")) {
            digits.strip_prefix(['+', '-']).unwrap_or(digits)
        } else {
            digits
        };
        return is_digit_run(digits, radix, true);
    }

    let normalized;
    let text = if yaml_12 && text.contains('_') {
        if !text.as_bytes().iter().enumerate().all(|(i, &byte)| {
            byte != b'_'
                || (i > 0
                    && text.as_bytes()[i - 1].is_ascii_digit()
                    && text.as_bytes().get(i + 1).is_some_and(u8::is_ascii_digit))
        }) {
            return false;
        }
        normalized = text.replace('_', "");
        normalized.as_str()
    } else {
        text
    };
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    // Removing underscores must not invent a special float token (e.g. .i_nf).
    if unsigned.eq_ignore_ascii_case(".inf") || unsigned.eq_ignore_ascii_case(".nan") {
        return false;
    }
    if super::is_float(text, Schema::Yaml12) {
        return true;
    }
    if yaml_12 {
        return false;
    }
    // YAML 1.1's draft float production also tags unconstructible spellings:
    // an empty integer part and multiple dots are permitted in the mantissa.
    let mantissa = match unsigned.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => {
            if !super::digits(exponent.strip_prefix(['+', '-']).unwrap_or(exponent), 10) {
                return false;
            }
            mantissa
        }
        None => unsigned,
    };
    mantissa.split_once('.').is_some_and(|(whole, fraction)| {
        (whole.is_empty() || super::digits(whole, 10))
            && fraction
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
    })
}

fn radix_digits(text: &str) -> Option<(u32, &str)> {
    match text.as_bytes() {
        [b'0', b'b' | b'B', ..] => Some((2, &text[2..])),
        [b'0', b'o' | b'O', ..] => Some((8, &text[2..])),
        [b'0', b'x' | b'X', ..] => Some((16, &text[2..])),
        _ => None,
    }
}

fn is_digit_run(text: &str, radix: u32, strict_underscores: bool) -> bool {
    let is_digit = |byte: u8| byte.is_ascii() && char::from(byte).is_digit(radix);
    let bytes = text.as_bytes();
    !bytes.is_empty()
        && bytes.iter().enumerate().all(|(i, &byte)| {
            is_digit(byte)
                || (byte == b'_'
                    && (!strict_underscores
                        || (i > 0
                            && is_digit(bytes[i - 1])
                            && bytes.get(i + 1).is_some_and(|&next| is_digit(next)))))
        })
}

#[cfg(test)]
mod tests {
    use super::{is_ambiguous, is_numeric};
    use crate::scalar::Schema;
    use rstest::rstest;

    #[rstest]
    #[case::inf(".inf")]
    #[case::nan(".nan")]
    #[case::zero("0")]
    #[case::neg_int("-19")]
    #[case::pos_int("+12")]
    #[case::leading_zero("01")]
    #[case::underscore_sep("1_0")]
    #[case::multi_underscore("1000_1000_1000")]
    #[case::binary("0b10")]
    #[case::pos_binary("+0b10")]
    #[case::neg_binary_upper("-0B10")]
    #[case::binary_underscore("0b1010_1010")]
    #[case::octal("0o7")]
    #[case::pos_octal_upper("+0O7")]
    #[case::octal_underscore("0o7_1")]
    #[case::hex("0x3A")]
    #[case::pos_hex_upper("+0X3A")]
    #[case::hex_underscore("0x3_A")]
    #[case::leading_dot(".5")]
    #[case::pos_leading_dot("+.5")]
    #[case::neg_leading_dot("-.5")]
    #[case::trailing_dot("0.")]
    #[case::pos_zero_float("+0.0")]
    #[case::neg_zero_float("-0.0")]
    #[case::exponent("12e03")]
    #[case::exponent_underscore("12e0_3")]
    #[case::neg_exponent_upper("-2E+05")]
    #[case::float_neg_exponent("12.34e-5")]
    #[case::leading_dot_exponent(".5e+1")]
    #[case::neg_leading_dot_exponent("-.5E-2")]
    fn numeric_looking_matches(#[case] input: &str) {
        assert!(is_numeric(input, true), "{input:?} should match");
    }

    #[rstest]
    #[case::empty("")]
    #[case::lone_plus("+")]
    #[case::lone_minus("-")]
    #[case::lone_dot(".")]
    #[case::leading_underscore("_1000")]
    #[case::trailing_underscore("1000_")]
    #[case::double_underscore("1__0")]
    #[case::exponent_leading_underscore("1e_2")]
    #[case::underscore_before_dot("_.5")]
    #[case::underscore_after_dot("._5")]
    #[case::empty_binary("0b")]
    #[case::binary_trailing_underscore("0b10_")]
    #[case::octal_leading_underscore("0o_7")]
    #[case::hex_trailing_underscore("0x3A_")]
    #[case::empty_octal("0o")]
    #[case::empty_hex("0x")]
    #[case::hex_with_sign("0x+1")]
    #[case::hex_inner_sign("-0x-1")]
    #[case::exponent_no_digits("12e")]
    #[case::dot_exponent_no_mantissa(".e5")]
    #[case::fractional_inner_underscore("1._0")]
    fn numeric_looking_non_matches(#[case] input: &str) {
        assert!(!is_numeric(input, true), "{input:?} should not match");
    }

    #[test]
    fn reader_extensions_preserve_quoting_policy() {
        for text in [
            ".",
            ".e2",
            "1.2.3",
            "0x_",
            "0b+1",
            "0o-7",
            "0_x_F",
            "_1000",
            "oN",
            "1:20",
            "0:20.5",
            "2001-1-2",
            "2001-12-14 1:2:3,4",
        ] {
            assert!(is_ambiguous(text, Schema::Yaml11), "{text:?}");
            assert!(!is_ambiguous(text, Schema::Yaml12), "{text:?}");
        }
        for text in [".i_nf", ".n_an", "0:20", "1:60", "1:20.0e+1", "word"] {
            assert!(!is_ambiguous(text, Schema::Yaml11), "{text:?}");
        }
        for text in [
            "nUlL", "tRuE", ".iNf", "+.NaN", "nan", "-INF", "1e999", "0B10", "1_0",
        ] {
            assert!(is_ambiguous(text, Schema::Yaml12), "{text:?}");
            assert!(is_ambiguous(text, Schema::Yaml11), "{text:?}");
        }
    }
}
