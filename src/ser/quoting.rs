use crate::scalar::{Schema, resolve_for_quoting};
use std::fmt::{self, Write};

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
pub(crate) fn is_plain_safe(s: &str, schema: Schema) -> bool {
    if s.is_empty() || resolve_for_quoting(s, schema) {
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
pub(crate) fn is_plain_value_safe(s: &str, schema: Schema, in_flow: bool) -> bool {
    if s.is_empty()
        || resolve_for_quoting(s, schema)
        || (schema == Schema::Yaml11 && matches!(s, "<<" | "="))
    {
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
    use super::{is_controll_which_needs_escaping, is_plain_safe, is_plain_value_safe};
    use crate::scalar::Schema;
    use rstest::rstest;

    #[rstest]
    #[case::dash_no_space("-value")]
    #[case::question_no_space("?query")]
    #[case::document_start_prefix_no_separation("---value")]
    #[case::document_end_prefix_no_separation("...value")]
    #[case::interior_space("a b")]
    fn plain_keys_allow_safe_inputs(#[case] input: &str) {
        assert!(is_plain_safe(input, Schema::Yaml12), "{input:?}");
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
        assert!(!is_plain_safe(input, Schema::Yaml12), "{input:?}");
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
        assert!(
            !is_plain_value_safe(input, Schema::Yaml11, false),
            "{input:?}"
        );
        assert!(
            !is_plain_value_safe(input, Schema::Yaml12, true),
            "flow value {input:?}"
        );
    }

    #[test]
    fn legacy_value_indicators_remain_quoted() {
        for text in ["<<", "="] {
            assert!(
                !is_plain_value_safe(text, Schema::Yaml11, false),
                "{text:?}"
            );
            assert!(is_plain_value_safe(text, Schema::Yaml12, false), "{text:?}");
        }
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
        assert!(!is_plain_safe(input, Schema::Yaml12), "key {input:?}");
        assert!(
            !is_plain_value_safe(input, Schema::Yaml11, false),
            "value {input:?}"
        );
        assert!(
            !is_plain_value_safe(input, Schema::Yaml12, true),
            "flow value {input:?}"
        );
    }
}
