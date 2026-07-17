use crate::parse_scalars::parse_yaml11_bool;
use std::fmt::{self, Write};

#[inline]
// Match a broad set of YAML numeric tokens (integer / float) even if they would overflow
// when parsed into Rust numeric types.
fn is_numeric_looking(s: &str) -> bool {
    let bytes = s.as_bytes();
    let len = bytes.len();

    if len == 0 {
        return false;
    }

    #[inline]
    fn consume_digit_run<F>(bytes: &[u8], mut p: usize, is_digit: F) -> Result<(usize, bool), ()>
    where
        F: Fn(u8) -> bool,
    {
        let start = p;
        let mut saw_digit = false;

        while p < bytes.len() {
            if is_digit(bytes[p]) {
                saw_digit = true;
                p += 1;
                continue;
            }

            if bytes[p] == b'_' {
                let prev_is_digit = p > start && is_digit(bytes[p - 1]);
                let next_is_digit = match bytes.get(p + 1) {
                    Some(&next) => is_digit(next),
                    None => false,
                };

                if !prev_is_digit || !next_is_digit {
                    return Err(());
                }

                p += 1;
                continue;
            }

            break;
        }

        Ok((p, saw_digit))
    }

    #[inline]
    fn consume_exponent(bytes: &[u8], mut p: usize) -> Option<usize> {
        if p >= bytes.len() || (bytes[p] != b'e' && bytes[p] != b'E') {
            return None;
        }

        p += 1;

        if p < bytes.len() && (bytes[p] == b'+' || bytes[p] == b'-') {
            p += 1;
        }

        let (p, saw_digit) = consume_digit_run(bytes, p, |b| b.is_ascii_digit()).ok()?;
        saw_digit.then_some(p)
    }

    let mut p = 0;
    if bytes[0] == b'+' || bytes[0] == b'-' {
        p = 1;
        if p == len {
            return false;
        }
    }

    // Match the integer spellings accepted by the deserializer, including signed and
    // uppercase radix prefixes, so string round-tripping stays stable.
    if len.saturating_sub(p) >= 3 && bytes[p] == b'0' {
        match bytes[p + 1] {
            b'b' | b'B' => {
                let (end, saw_digit) =
                    match consume_digit_run(bytes, p + 2, |b| matches!(b, b'0' | b'1')) {
                        Ok(run) => run,
                        Err(()) => return false,
                    };
                return saw_digit && end == len;
            }
            b'o' | b'O' => {
                let (end, saw_digit) =
                    match consume_digit_run(bytes, p + 2, |b| (b'0'..=b'7').contains(&b)) {
                        Ok(run) => run,
                        Err(()) => return false,
                    };
                return saw_digit && end == len;
            }
            b'x' | b'X' => {
                let (end, saw_digit) =
                    match consume_digit_run(bytes, p + 2, |b| b.is_ascii_hexdigit()) {
                        Ok(run) => run,
                        Err(()) => return false,
                    };
                return saw_digit && end == len;
            }
            _ => {}
        }
    }

    // Dot-leading float: .5, +.5, -.5
    if bytes[p] == b'.' {
        p += 1;
        let (end, saw_digit) = match consume_digit_run(bytes, p, |b| b.is_ascii_digit()) {
            Ok(run) => run,
            Err(()) => return false,
        };
        p = end;

        if !saw_digit {
            return false;
        }

        return match consume_exponent(bytes, p) {
            Some(end) => end == len,
            None => p == len,
        };
    }

    // Decimal integer / float / scientific notation.
    if !bytes[p].is_ascii_digit() {
        return false;
    }

    let (end, saw_digit) = match consume_digit_run(bytes, p, |b| b.is_ascii_digit()) {
        Ok(run) => run,
        Err(()) => return false,
    };
    p = end;

    debug_assert!(saw_digit, "decimal branch always starts on a digit");

    if p == len {
        return true;
    }

    if bytes[p] == b'.' {
        p += 1;
        let (end, _) = match consume_digit_run(bytes, p, |b| b.is_ascii_digit()) {
            Ok(run) => run,
            Err(()) => return false,
        };
        p = end;

        return match consume_exponent(bytes, p) {
            Some(end) => end == len,
            None => p == len,
        };
    }

    if bytes[p] == b'e' || bytes[p] == b'E' {
        return matches!(consume_exponent(bytes, p), Some(end) if end == len);
    }

    false
}

/// Returns true if `s` is a special YAML token or looks like a number/boolean,
/// which means it should be quoted to be treated as a string.
fn is_ambiguous(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    if s == "~"
        || s.eq_ignore_ascii_case("null")
        || s.eq_ignore_ascii_case("true")
        || s.eq_ignore_ascii_case("false")
    {
        return true;
    }

    // Special float tokens (ASCII case-insensitive) should not be plain, to avoid
    // being interpreted as floats during parse. Quote these as strings.
    // Accept common forms with optional leading sign and optional leading dot.
    // Examples: "NaN", ".nan", ".inf", "-.inf", "+inf". No allocation.
    #[inline]
    fn is_ascii_lower(b: u8) -> u8 {
        b | 0x20
    }
    #[inline]
    fn is_special_inf_nan_ascii(s: &str) -> bool {
        let bytes = s.as_bytes();
        let mut i = 0usize;
        if let Some(&c) = bytes.first()
            && (c == b'+' || c == b'-')
        {
            i = 1;
        }
        if let Some(&c) = bytes.get(i)
            && c == b'.'
        {
            i += 1;
        } else {
            return false;
        }
        if bytes.len() == i + 3 {
            let a = is_ascii_lower(bytes[i]);
            let b = is_ascii_lower(bytes[i + 1]);
            let c = is_ascii_lower(bytes[i + 2]);
            return (a == b'n' && b == b'a' && c == b'n') || (a == b'i' && b == b'n' && c == b'f');
        }
        false
    }
    if is_special_inf_nan_ascii(s) {
        return true;
    }

    // Numeric-looking tokens: quote them to preserve strings even if they would overflow
    // our numeric parsers.
    if is_numeric_looking(s) {
        return true;
    }

    false
}

#[inline]
fn starts_with_document_marker(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("---").or_else(|| s.strip_prefix("...")) else {
        return false;
    };

    rest.is_empty() || rest.as_bytes().first().is_some_and(u8::is_ascii_whitespace)
}

/// Like `is_ambiguous`, but used for VALUE position.
///
/// For values we are more conservative: quote additional spellings that many YAML
/// parsers accept as floats even if YAML 1.2 requires the leading-dot form.
#[inline]
fn is_ambiguous_value(s: &str, yaml_12: bool) -> bool {
    if is_ambiguous(s) {
        return true;
    }

    // YAML 1.1 boolean spellings: quote them as strings for compatibility and
    // round-tripping (e.g. "YES", "no", "On", "off", "y", "n").
    if !yaml_12 && parse_yaml11_bool(s).is_ok() {
        return true;
    }

    // Quote non-YAML-1.2 float spellings too (e.g. "nan", "inf").
    // This preserves round-tripping of strings and matches tests.
    s.eq_ignore_ascii_case("nan")
        || s.eq_ignore_ascii_case("inf")
        || s.eq_ignore_ascii_case("+inf")
        || s.eq_ignore_ascii_case("-inf")
}

/// Controls quoting behavior of the serializer.
///
/// Returns true if `s` can be emitted as a plain scalar without quoting.
/// Internal heuristic used by `write_plain_or_quoted`.
#[inline]
pub(crate) fn is_plain_safe(s: &str) -> bool {
    if is_ambiguous(s) {
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
        b',' => return false,
        b':' | b'[' | b']' | b'{' | b'}' | b'#' | b'&' | b'*' | b'!' | b'|' | b'>' | b'\''
        | b'"' | b'%' | b'@' | b'`' => return false,
        _ => {}
    }

    // In block style, commas are just characters (only flow style treats them as structural).
    !contains_any_or_is_control(s, &[':', '#'])
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
    if is_ambiguous_value(s, yaml_12) {
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
        [b'-' | b'?'] => return false,
        [b'-' | b'?', b1, ..] if b1.is_ascii_whitespace() => return false,
        // ',' is a flow indicator and cannot start a plain scalar.
        [
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
        // In values, ':' is allowed, but '#' would start a comment so still disallow '#'.
        !contains_any_or_is_control(s, &[',', '[', ']', '{', '}', '#'])
    } else {
        // In block style, commas/brackets/braces are ordinary characters.
        !contains_any_or_is_control(s, &['#'])
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
/// `char::is_control` misses BOM (U+FEFF) and the LS/PS separators (U+2028/U+2029), which are not controls.
#[inline]
pub(crate) fn is_controll_which_needs_escaping(ch: char) -> bool {
    ch.is_control() || matches!(ch, '\u{FEFF}' | '\u{2028}' | '\u{2029}')
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
            c if (c as u32) <= 0xFFFF
                && (c.is_control() || (0x7F..=0x9F).contains(&(c as u32))) =>
            {
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

#[cfg(test)]
mod tests {
    use super::{
        is_controll_which_needs_escaping, is_numeric_looking, is_plain_safe, is_plain_value_safe,
    };
    use rstest::rstest;

    #[rstest]
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
        assert!(is_numeric_looking(input), "{input:?} should match");
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
    #[case::inf(".inf")]
    #[case::nan(".nan")]
    #[case::fractional_inner_underscore("1._0")]
    fn numeric_looking_non_matches(#[case] input: &str) {
        assert!(!is_numeric_looking(input), "{input:?} should not match");
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
