use crate::parse_scalars::parse_yaml11_bool;
use std::fmt::{self, Write};

/// Check numeric syntax without constructing a value: overflow and resolver errors
/// must not turn a serialized string into a number or an unreadable document.
fn is_numeric(s: &str, yaml_12: bool) -> bool {
    if !matches!(
        s.as_bytes().first(),
        Some(b'0'..=b'9' | b'+' | b'-' | b'.' | b'_')
    ) {
        return false;
    }
    let unsigned = s.strip_prefix(['+', '-']).unwrap_or(s);
    if unsigned.eq_ignore_ascii_case(".inf") || unsigned.eq_ignore_ascii_case(".nan") {
        return true;
    }
    if !yaml_12 {
        if is_yaml11_sexagesimal(s) {
            return true;
        }
        // YAML 1.1 radix productions accept [digits_]+, even an all-underscore
        // suffix that a resolver tags as numeric but cannot construct.
        if let Some((radix, digits)) = radix_digits(unsigned)
            && is_digit_run(digits, radix, false)
        {
            return true;
        }
    }

    // Go YAML removes underscores before numeric resolution, including around
    // radix prefixes and exponents. YAML 1.2 output keeps the stricter spelling.
    let normalized;
    let s = if !yaml_12 && s.contains('_') {
        normalized = s.replace('_', "");
        normalized.as_str()
    } else {
        s
    };
    let unsigned = s.strip_prefix(['+', '-']).unwrap_or(s);
    if let Some((radix, digits)) = radix_digits(unsigned) {
        // Go YAML's binary fallback accepts a sign after an unsigned 0b prefix.
        let digits = if !yaml_12 && s.starts_with("0b") {
            digits.strip_prefix(['+', '-']).unwrap_or(digits)
        } else {
            digits
        };
        return is_digit_run(digits, radix, true);
    }
    let mantissa = match unsigned.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => {
            let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
            if !is_digit_run(exponent, 10, true) {
                return false;
            }
            mantissa
        }
        None => unsigned,
    };
    match mantissa.split_once('.') {
        Some((whole, fraction)) => {
            if !whole.is_empty() && !is_digit_run(whole, 10, true) {
                return false;
            }
            if !yaml_12 {
                // The YAML 1.1 float production allows an empty integer part
                // and [0-9.]* after the dot, including unconstructible spellings.
                fraction.bytes().all(|b| b.is_ascii_digit() || b == b'.')
            } else if fraction.is_empty() {
                !whole.is_empty()
            } else {
                is_digit_run(fraction, 10, true)
            }
        }
        None => is_digit_run(mantissa, 10, true),
    }
}

fn radix_digits(s: &str) -> Option<(u32, &str)> {
    match s.as_bytes() {
        [b'0', b'b' | b'B', ..] => Some((2, &s[2..])),
        [b'0', b'o' | b'O', ..] => Some((8, &s[2..])),
        [b'0', b'x' | b'X', ..] => Some((16, &s[2..])),
        _ => None,
    }
}

fn is_digit_run(s: &str, radix: u32, strict_underscores: bool) -> bool {
    let is_digit = |b: u8| b.is_ascii() && char::from(b).is_digit(radix);
    let bytes = s.as_bytes();
    !bytes.is_empty()
        && bytes.iter().enumerate().all(|(i, &b)| {
            is_digit(b)
                || (b == b'_'
                    && (!strict_underscores
                        || (i > 0
                            && is_digit(bytes[i - 1])
                            && bytes.get(i + 1).is_some_and(|&next| is_digit(next)))))
        })
}

/// Recognize YAML 1.1 base-60 integers and floats without parsing their magnitude.
fn is_yaml11_sexagesimal(s: &str) -> bool {
    let unsigned = s.strip_prefix(['+', '-']).unwrap_or(s);
    let (integer, fraction) = match unsigned.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (unsigned, None),
    };
    let Some((first, rest)) = integer.split_once(':') else {
        return false;
    };
    let Some(first_digit) = first.as_bytes().first() else {
        return false;
    };
    if !first_digit.is_ascii_digit()
        || (fraction.is_none() && *first_digit == b'0')
        || !first.bytes().all(|b| b.is_ascii_digit() || b == b'_')
    {
        return false;
    }
    // Every subsequent base-60 component is one or two digits in 0..=59.
    if !rest.split(':').all(|part| match part.as_bytes() {
        [digit] => digit.is_ascii_digit(),
        [tens, units] => matches!(tens, b'0'..=b'5') && units.is_ascii_digit(),
        _ => false,
    }) {
        return false;
    }
    fraction.is_none_or(|part| part.bytes().all(|b| b.is_ascii_digit() || b == b'_'))
}

/// Match timestamp syntax without calendar validation: a reader can assign a
/// timestamp tag even when constructing the date would fail.
fn is_yaml11_timestamp(mut s: &str) -> bool {
    fn digits(s: &mut &str, min: usize, max: usize) -> bool {
        let len = s.bytes().take_while(u8::is_ascii_digit).count();
        if !(min..=max).contains(&len) {
            return false;
        }
        *s = &s[len..];
        true
    }
    fn prefix(s: &mut &str, ch: char) -> bool {
        if let Some(rest) = s.strip_prefix(ch) {
            *s = rest;
            true
        } else {
            false
        }
    }
    if !digits(&mut s, 4, 4)
        || !prefix(&mut s, '-')
        || !digits(&mut s, 1, 2)
        || !prefix(&mut s, '-')
        || !digits(&mut s, 1, 2)
    {
        return false;
    }
    // Go YAML also accepts single-digit date and time components.
    if s.is_empty() {
        return true;
    }
    if let Some(rest) = s.strip_prefix(['T', 't']) {
        s = rest;
    } else {
        let rest = s.trim_start_matches([' ', '\t']);
        if rest.len() == s.len() {
            return false;
        }
        s = rest;
    }
    if !digits(&mut s, 1, 2)
        || !prefix(&mut s, ':')
        || !digits(&mut s, 1, 2)
        || !prefix(&mut s, ':')
        || !digits(&mut s, 1, 2)
    {
        return false;
    }
    // Go's time parser also accepts a comma as the fractional separator.
    if prefix(&mut s, '.') || prefix(&mut s, ',') {
        digits(&mut s, 0, usize::MAX);
    }
    s = s.trim_start_matches([' ', '\t']);
    if s.is_empty() || s == "Z" {
        return true;
    }
    if !(prefix(&mut s, '+') || prefix(&mut s, '-')) || !digits(&mut s, 1, 2) {
        return false;
    }
    if prefix(&mut s, ':') && !digits(&mut s, 2, 2) {
        return false;
    }
    s.is_empty()
}

/// Whether implicit scalar resolution can change a string's type or reject it.
fn is_ambiguous(s: &str, yaml_12: bool) -> bool {
    s.is_empty()
        || s == "~"
        || s.eq_ignore_ascii_case("null")
        || s.eq_ignore_ascii_case("true")
        || s.eq_ignore_ascii_case("false")
        || is_numeric(s, yaml_12)
        // Preserve compatibility with readers accepting undotted float tokens.
        || s.eq_ignore_ascii_case("nan")
        || s.eq_ignore_ascii_case("inf")
        || s.eq_ignore_ascii_case("+inf")
        || s.eq_ignore_ascii_case("-inf")
        || (!yaml_12
            && (parse_yaml11_bool(s).is_ok()
                || is_yaml11_timestamp(s)
                || matches!(s, "<<" | "=")))
}

#[inline]
fn starts_with_document_marker(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("---").or_else(|| s.strip_prefix("...")) else {
        return false;
    };

    rest.is_empty() || rest.as_bytes().first().is_some_and(u8::is_ascii_whitespace)
}

/// Controls quoting behavior of the serializer.
///
/// Returns true if `s` can be emitted as a plain scalar without quoting.
/// Internal heuristic used by `write_plain_or_quoted`.
#[inline]
pub(crate) fn is_plain_safe(s: &str) -> bool {
    if is_ambiguous(s, true) {
        return false;
    }
    // A plain, untagged "<<" key would be a YAML merge key, not the literal string "<<".
    if s == "<<" {
        return false;
    }
    if starts_with_document_marker(s) {
        return false;
    }
    let bytes = s.as_bytes();
    // Keys with leading or trailing whitespace must be quoted:
    // a key's surrounding whitespace is not preserved across a round trip
    // (e.g. `foo : x` and `foo: x` parse to the same key), so a plain scalar would silently collapse distinct keys.
    if bytes.first().is_some_and(u8::is_ascii_whitespace)
        || bytes.last().is_some_and(u8::is_ascii_whitespace)
    {
        return false;
    }

    // YAML indicators are only special in certain forms.
    // For example, "-a" and "?query" are valid plain scalars, while "-" / "?"
    // or "- " / "? " should be quoted.
    match bytes[0] {
        b'-' | b'?' => {
            if bytes.len() == 1 {
                return false;
            }
            if bytes[1].is_ascii_whitespace() {
                return false;
            }
        }
        // ',' is a flow indicator and cannot start a plain scalar.
        b',' | b':' | b'[' | b']' | b'{' | b'}' | b'#' | b'&' | b'*' | b'!' | b'|' | b'>'
        | b'\'' | b'"' | b'%' | b'@' | b'`' => return false,
        _ => {}
    }

    // In block style, commas are just characters (only flow style treats them as structural).
    // `#` inside a plain scalar is allowed as long as it is not preceded by whitespace,
    // where it would begin a comment.
    !contains_any_or_is_control(s, &[':']) && !has_comment_start(s)
}

/// Returns true if `s` can be emitted as a plain scalar in VALUE position without quoting.
///
/// This is slightly more permissive than `is_plain_safe` for keys:
/// - it allows ':' inside values
///
/// Additionally, we make this stricter for strings that appear inside flow-style sequences/maps
/// where certain characters would break parsing (e.g., commas and brackets) or where the token
/// could be misinterpreted as a number or boolean.
#[inline]
pub(crate) fn is_plain_value_safe(s: &str, yaml_12: bool, in_flow: bool) -> bool {
    if is_ambiguous(s, yaml_12) {
        return false;
    }
    if starts_with_document_marker(s) {
        return false;
    }

    let bytes = s.as_bytes();
    // Plain scalar edge whitespace is parsed as separation/indentation, not
    // scalar content, so it would be lost on round-trip.
    if bytes.first().is_some_and(u8::is_ascii_whitespace)
        || bytes.last().is_some_and(u8::is_ascii_whitespace)
    {
        return false;
    }

    match bytes {
        [b'-' | b'?', b1, ..] if b1.is_ascii_whitespace() => return false,
        // ',' is a flow indicator and cannot start a plain scalar.
        [b'-' | b'?']
        | [
            b',' | b':' | b'[' | b']' | b'{' | b'}' | b'#' | b'&' | b'*' | b'!' | b'|' | b'>'
            | b'\'' | b'"' | b'%' | b'@' | b'`',
            ..,
        ] => return false,
        _ => {}
    }

    // Yet while colon is ok, colon after whitespace is not.
    if s.contains(": ") || s.trim().ends_with(':') {
        // We only need to check for space as CR, LF and TAB are control characters and will
        // trigger escape on their own anyway.
        return false;
    }

    if in_flow {
        // In flow style, commas and brackets/braces are structural.
        // In values, ':' is allowed. `#` is only problematic when preceded by whitespace.
        !contains_any_or_is_control(s, &[',', '[', ']', '{', '}']) && !has_comment_start(s)
    } else {
        // In block style, commas/brackets/braces are ordinary characters.
        !contains_any_or_is_control(s, &[]) && !has_comment_start(s)
    }
}

/// Returns true when `s` can be emitted literally inside a block scalar without
/// relying on double-quoted escape sequences.
///
/// This is character-level safety only: it says whether the codepoints can be
/// represented inside `|`/`>` blocks.
///
/// - `\r` is a YAML 1.2 line break (parsers normalize it to `\n`) and must be
///   escaped to preserve the exact Rust string on round-trip.
/// - BOM (U+FEFF) is excluded from the `nb-char` production and must be escaped.
/// - NEL, LS, and PS are non-break characters in YAML 1.2, but many tools and
///   editors mishandle them; we reject them in block scalars as a conservative
///   interoperability/readability policy. The double-quoted path preserves them
///   exactly via `\N`/`\L`/`\P` escapes.
#[inline]
pub(crate) fn is_block_scalar_content_safe(s: &str) -> bool {
    s.chars().all(|ch| match ch {
        '\n' | '\t' => true,
        '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}' | '\u{FEFF}' => false,
        c => matches!(
            c as u32,
            0x20..=0x7E | 0xA0..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x0010_FFFF
        ),
    })
}

/// Readability policy for auto-selected block scalars.
///
/// Distinct from [`is_block_scalar_content_safe`]: that asks "can this be a
/// block scalar at all?", whereas this asks "should we pick block style for
/// this content automatically?".
///
/// Intentionally permissive. Trailing spaces/tabs are preserved by YAML block
/// scalars and are a tooling/presentation concern, not a scalar safety concern.
/// A future option can opt into a stricter policy for users who prefer quoted
/// output when line-end whitespace is present.
#[inline]
pub(crate) fn is_auto_block_scalar_readable(s: &str) -> bool {
    !s.is_empty()
}

/// Characters that cannot survive a round-trip inside a plain or single-quoted
/// scalar and therefore force double-quoted emission.
/// `char::is_control` misses BOM (U+FEFF), the LS/PS separators (U+2028/U+2029),
/// and the non-printable U+FFFE/U+FFFF codepoints, which also need escaping.
#[inline]
pub(crate) fn is_controll_which_needs_escaping(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{FEFF}' | '\u{2028}' | '\u{2029}' | '\u{FFFE}' | '\u{FFFF}'
        )
}

/// Write the contents of a YAML double-quoted scalar, without surrounding quotes.
pub(crate) fn escape_double_quoted(s: &str, out: &mut impl Write) -> fmt::Result {
    for ch in s.chars() {
        match ch {
            '\\' => out.write_str("\\\\")?,
            '"' => out.write_str("\\\"")?,
            // YAML named escapes for common control characters.
            '\0' => out.write_str("\\0")?,
            '\u{7}' => out.write_str("\\a")?,
            '\u{8}' => out.write_str("\\b")?,
            '\t' => out.write_str("\\t")?,
            '\n' => out.write_str("\\n")?,
            '\u{b}' => out.write_str("\\v")?,
            '\u{c}' => out.write_str("\\f")?,
            '\r' => out.write_str("\\r")?,
            '\u{1b}' => out.write_str("\\e")?,
            // Unicode BOM should use the standard \u escape rather than Rust's \u{...}.
            '\u{FEFF}' => out.write_str("\\uFEFF")?,
            // YAML named escapes for Unicode separators.
            '\u{0085}' => out.write_str("\\N")?,
            '\u{2028}' => out.write_str("\\L")?,
            '\u{2029}' => out.write_str("\\P")?,
            c if (c as u32) <= 0xFF && (c.is_control() || (0x7F..=0x9F).contains(&(c as u32))) => {
                write!(out, "\\x{:02X}", c as u32)?;
            }
            c if (c as u32) <= 0xFFFF && is_controll_which_needs_escaping(c) => {
                write!(out, "\\u{:04X}", c as u32)?;
            }
            c => out.write_char(c)?,
        }
    }

    Ok(())
}

fn contains_any_or_is_control(string: &str, values: &[char]) -> bool {
    string
        .chars()
        .any(|x| is_controll_which_needs_escaping(x) || values.iter().any(|v| &x == v))
}

fn has_comment_start(string: &str) -> bool {
    // In plain style, `#` starts a comment only when separated by whitespace.
    string
        .chars()
        .zip(string.chars().skip(1))
        .any(|(prev, curr)| prev.is_whitespace() && curr == '#')
}

#[cfg(test)]
mod tests {
    use super::{is_controll_which_needs_escaping, is_numeric, is_plain_safe, is_plain_value_safe};
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

    #[rstest]
    #[case::dash_no_space("-value")]
    #[case::question_no_space("?query")]
    #[case::document_start_prefix_no_separation("---value")]
    #[case::document_end_prefix_no_separation("...value")]
    #[case::interior_space("a b")]
    fn plain_keys_allow_safe_inputs(#[case] input: &str) {
        assert!(is_plain_safe(input), "{input:?}");
    }

    #[rstest]
    #[case::dash_indicator_space("- value")]
    #[case::question_indicator_tab("?\tvalue")]
    #[case::trailing_space("foo ")]
    #[case::leading_space(" foo")]
    #[case::trailing_tab("foo\t")]
    #[case::merge_key("<<")]
    #[case::document_start_marker("---")]
    #[case::document_end_marker("...")]
    #[case::document_start_marker_with_value("--- value")]
    #[case::document_end_marker_with_value("... value")]
    fn plain_keys_reject_unsafe_inputs(#[case] input: &str) {
        assert!(!is_plain_safe(input), "{input:?}");
    }

    #[rstest]
    #[case::leading_space(" foo")]
    #[case::trailing_space("foo ")]
    #[case::leading_tab("\tfoo")]
    #[case::trailing_tab("foo\t")]
    #[case::document_start_marker("---")]
    #[case::document_end_marker("...")]
    #[case::document_start_marker_with_value("--- value")]
    #[case::document_end_marker_with_value("... value")]
    fn plain_values_reject_lossy_surrounding_whitespace(#[case] input: &str) {
        assert!(!is_plain_value_safe(input, false, false), "{input:?}");
        assert!(
            !is_plain_value_safe(input, true, true),
            "flow value {input:?}"
        );
    }

    #[rstest]
    #[case::nul('\0')]
    #[case::tab('\t')]
    #[case::newline('\n')]
    #[case::carriage_return('\r')]
    #[case::nel('\u{0085}')]
    #[case::bom('\u{FEFF}')]
    #[case::line_sep('\u{2028}')]
    #[case::para_sep('\u{2029}')]
    fn chars_needing_escaping(#[case] ch: char) {
        assert!(is_controll_which_needs_escaping(ch), "{ch:?} should escape");
    }

    #[rstest]
    #[case::ascii('a')]
    #[case::space(' ')]
    #[case::unicode('é')]
    #[case::cjk('字')]
    fn chars_not_needing_escaping(#[case] ch: char) {
        assert!(
            !is_controll_which_needs_escaping(ch),
            "{ch:?} should stay plain"
        );
    }

    #[rstest]
    #[case::bom("\u{FEFF}")]
    #[case::bom_prefixed("\u{FEFF}key")]
    #[case::line_sep("a\u{2028}b")]
    #[case::para_sep("a\u{2029}b")]
    fn format_chars_are_not_plain_safe(#[case] input: &str) {
        assert!(!is_plain_safe(input), "key {input:?}");
        assert!(!is_plain_value_safe(input, false, false), "value {input:?}");
        assert!(
            !is_plain_value_safe(input, true, true),
            "flow value {input:?}"
        );
    }
}
