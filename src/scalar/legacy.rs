//! YAML 1.1 scalar grammars from <https://yaml.org/type/>.
//!
//! These predicates recognize complete, constructible spellings independently of
//! native numeric range. Where the drafts' regular expressions contradict their
//! examples, decimal fractions allow underscores and a single decimal point, and
//! timestamp offsets allow preceding spaces or tabs. Numeric mantissas and radix
//! suffixes must contain at least one digit; `.` and `0x_` are not numbers.

pub(super) fn parse_bool(text: &str) -> Option<bool> {
    match text {
        "y" | "Y" | "yes" | "Yes" | "YES" | "true" | "True" | "TRUE" | "on" | "On" | "ON" => {
            Some(true)
        }
        "n" | "N" | "no" | "No" | "NO" | "false" | "False" | "FALSE" | "off" | "Off" | "OFF" => {
            Some(false)
        }
        _ => None,
    }
}

pub(super) fn is_integer(text: &str) -> bool {
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    if unsigned == "0" {
        return true;
    }
    if let Some(digits) = unsigned.strip_prefix("0b") {
        return digit_run(digits, 2);
    }
    if let Some(digits) = unsigned.strip_prefix("0x") {
        return digit_run(digits, 16);
    }
    if let Some(digits) = unsigned.strip_prefix('0') {
        return digit_run(digits, 8);
    }
    if unsigned.contains(':') {
        return sexagesimal_integer_part(unsigned, false);
    }
    unsigned
        .as_bytes()
        .first()
        .is_some_and(|b| matches!(b, b'1'..=b'9'))
        && digit_run(unsigned, 10)
}

/// Match YAML 1.1 floats with the draft's decimal-fraction inconsistency resolved
/// in favor of its underscore-bearing example and constructible numeric values.
pub(super) fn is_float(text: &str) -> bool {
    if matches!(text, ".nan" | ".NaN" | ".NAN") {
        return true;
    }
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    if matches!(unsigned, ".inf" | ".Inf" | ".INF") {
        return true;
    }
    if unsigned.contains(':') {
        return unsigned.contains('.') && is_sexagesimal(text);
    }
    let mantissa = match unsigned.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => {
            let Some(exponent) = exponent.strip_prefix(['+', '-']) else {
                return false;
            };
            if exponent.is_empty() || !exponent.bytes().all(|b| b.is_ascii_digit()) {
                return false;
            }
            mantissa
        }
        None => unsigned,
    };
    let Some((whole, fraction)) = mantissa.split_once('.') else {
        return false;
    };
    (whole.is_empty()
        || (whole.as_bytes().first().is_some_and(u8::is_ascii_digit) && digit_run(whole, 10)))
        && fraction.bytes().all(|b| b.is_ascii_digit() || b == b'_')
        && (whole.bytes().any(|b| b.is_ascii_digit())
            || fraction.bytes().any(|b| b.is_ascii_digit()))
}

/// Recognize timestamps lexically, without checking calendar or clock ranges.
pub(super) fn is_timestamp(text: &str) -> bool {
    timestamp(text, false)
}

/// Quoting also protects timestamp spellings accepted by Go YAML readers.
#[cfg(feature = "serialize")]
pub(super) fn is_quoting_timestamp(text: &str) -> bool {
    timestamp(text, true)
}

fn timestamp(mut text: &str, conservative: bool) -> bool {
    if take_digits(&mut text, 4, 4).is_none() || !take_prefix(&mut text, '-') {
        return false;
    }
    let Some(month_width) = take_digits(&mut text, 1, 2) else {
        return false;
    };
    if !take_prefix(&mut text, '-') {
        return false;
    }
    let Some(day_width) = take_digits(&mut text, 1, 2) else {
        return false;
    };
    if text.is_empty() {
        // The date-only alternative requires two-digit month and day fields.
        return conservative || (month_width == 2 && day_width == 2);
    }
    if let Some(rest) = text.strip_prefix(['T', 't']) {
        text = rest;
    } else {
        let rest = text.trim_start_matches([' ', '\t']);
        if rest.len() == text.len() {
            return false;
        }
        text = rest;
    }
    if take_digits(&mut text, 1, 2).is_none()
        || !take_prefix(&mut text, ':')
        || take_digits(&mut text, if conservative { 1 } else { 2 }, 2).is_none()
        || !take_prefix(&mut text, ':')
        || take_digits(&mut text, if conservative { 1 } else { 2 }, 2).is_none()
    {
        return false;
    }
    if take_prefix(&mut text, '.') || (conservative && take_prefix(&mut text, ',')) {
        take_digits(&mut text, 0, usize::MAX);
    }
    if text.is_empty() {
        return true;
    }
    // Follow the space-separated offset in the draft's timestamp examples.
    text = text.trim_start_matches([' ', '\t']);
    if text == "Z" || (conservative && text.is_empty()) {
        return true;
    }
    if !(take_prefix(&mut text, '+') || take_prefix(&mut text, '-'))
        || take_digits(&mut text, 1, 2).is_none()
    {
        return false;
    }
    if take_prefix(&mut text, ':') && take_digits(&mut text, 2, 2).is_none() {
        return false;
    }
    text.is_empty()
}

fn digit_run(text: &str, radix: u32) -> bool {
    text.bytes().any(|b| b != b'_')
        && text
            .bytes()
            .all(|b| b == b'_' || char::from(b).is_digit(radix))
}

/// Recognize base-60 integers and floats without constructing their magnitude.
pub(super) fn is_sexagesimal(text: &str) -> bool {
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    let (whole, fraction) = unsigned
        .split_once('.')
        .map_or((unsigned, None), |(whole, fraction)| {
            (whole, Some(fraction))
        });
    sexagesimal_integer_part(whole, fraction.is_some())
        && fraction.is_none_or(|part| part.bytes().all(|b| b.is_ascii_digit() || b == b'_'))
}

fn sexagesimal_integer_part(text: &str, allow_leading_zero: bool) -> bool {
    let Some((first, rest)) = text.split_once(':') else {
        return false;
    };
    if !first
        .as_bytes()
        .first()
        .is_some_and(|b| b.is_ascii_digit() && (allow_leading_zero || *b != b'0'))
        || !digit_run(first, 10)
    {
        return false;
    }
    rest.split(':').all(|part| match part.as_bytes() {
        [digit] => digit.is_ascii_digit(),
        [tens, units] => matches!(tens, b'0'..=b'5') && units.is_ascii_digit(),
        _ => false,
    })
}

fn take_digits(text: &mut &str, min: usize, max: usize) -> Option<usize> {
    let width = text.bytes().take_while(u8::is_ascii_digit).count();
    if !(min..=max).contains(&width) {
        return None;
    }
    *text = &text[width..];
    Some(width)
}

fn take_prefix(text: &mut &str, prefix: char) -> bool {
    if let Some(rest) = text.strip_prefix(prefix) {
        *text = rest;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn booleans_match_only_published_spellings() {
        for text in [
            "y", "Y", "yes", "Yes", "YES", "true", "True", "TRUE", "on", "On", "ON",
        ] {
            assert_eq!(parse_bool(text), Some(true), "{text:?}");
        }
        for text in [
            "n", "N", "no", "No", "NO", "false", "False", "FALSE", "off", "Off", "OFF",
        ] {
            assert_eq!(parse_bool(text), Some(false), "{text:?}");
        }
        for text in ["", " yes", "yes\n", "tRuE", "oN", "yesman", "0"] {
            assert_eq!(parse_bool(text), None, "{text:?}");
        }
    }

    #[test]
    fn integer_grammar_preserves_radix_and_underscore_rules() {
        for text in [
            "0",
            "+0",
            "-0",
            "685_230",
            "+685_230",
            "02472256",
            "0_7",
            "0b1010_0111",
            "0b_1_",
            "0x_0A_74_AE",
            "0x__F",
            "-0xF",
            "1__0_",
            "190:20:30",
            "1_:00:9",
            "123456789012345678901234567890123456789012345678901234567890",
        ] {
            assert!(is_integer(text), "{text:?}");
        }
        for text in [
            "",
            "+",
            " 1",
            "1 ",
            "1\n",
            "0b",
            "0x",
            "0b2",
            "08",
            "0o7",
            "0B1",
            "0Xf",
            "_1",
            "1.0",
            "1e2",
            "0:20",
            "01:20",
            "1:60",
            "1:000",
            "1:2_0",
            "1::20",
            "1:20:",
            "1:20suffix",
            "1:20.0",
            "١",
            "0xé",
            "0_",
            "0b_",
            "0x__",
        ] {
            assert!(!is_integer(text), "{text:?}");
        }
    }

    #[test]
    fn float_grammar_requires_whole_scalar_matches() {
        for text in [
            "6.8523015e+5",
            "685_230.15",
            "190:20:30.15",
            "1.0E-2",
            ".5",
            "-.5",
            "+1.",
            "0:20.5",
            "01:20.5",
            "1:59._",
            "1__0_:20.5_0",
            "1.0_5",
            "1._",
            "._5",
            "685.230_15e+03",
            ".inf",
            "+.Inf",
            "-.INF",
            ".nan",
            ".NaN",
            ".NAN",
        ] {
            assert!(is_float(text), "{text:?}");
        }
        for text in [
            "",
            "1",
            "1e+2",
            "1.0e2",
            "1.0e+",
            "1.0e+2e+3",
            "1.0e+1_0",
            ".",
            "1.2.3",
            ".e+2",
            "._",
            "_.1",
            "1:60.0",
            "1:20.0e+1",
            "1:20.0.1",
            "1:2_0.0",
            "+.nan",
            "-.NaN",
            ".Nan",
            ".iNf",
            "inf",
            "nan",
            "a.infra",
            "a.nanotube",
            ".infinitude",
            " .inf",
            ".inf\n",
            "1.5suffix",
            "１２.３",
        ] {
            assert!(!is_float(text), "{text:?}");
        }
    }

    #[test]
    fn timestamp_grammar_is_lexical_and_not_calendar_validation() {
        for text in [
            "2002-12-14",
            "2001-12-15T02:59:43.1Z",
            "2001-12-14t21:59:43.10-05:00",
            "2001-12-15 2:59:43.10",
            "2001-1-5t2:59:43",
            "2001-12-14\t21:59:43.10\tZ",
            "2001-12-14 21:59:43.-5",
            "2001-12-14 21:59:43+2:30",
            "2001-12-14 21:59:43.",
            "2001-99-99",
            "2001-99-99T99:99:99+99:99",
            "2001-12-14T2:59:43 -5",
            "2001-12-14T2:59:43\t+05:30",
        ] {
            assert!(is_timestamp(text), "{text:?}");
        }
        for text in [
            "",
            "2001-1-5",
            "2001-01-5",
            "2001-1-05",
            "20001-01-01",
            "001-01-01",
            "2001-12-14T2:9:43Z",
            "2001-12-14T2:59:3Z",
            "2001-12-14T2:59:43,1Z",
            "2001-12-14T2:59:43z",
            "2001-12-14T2:59:43+1:2",
            "2001-12-14T2:59:43-0500",
            "2001-12-14T2:59:43 ",
            "2001-12-14T2:59:43Zsuffix",
            "2001-12-14\n2:59:43",
            " 2001-12-14",
            "2001-12-14\n",
            "2001-12-14T",
        ] {
            assert!(!is_timestamp(text), "{text:?}");
        }
    }
}
