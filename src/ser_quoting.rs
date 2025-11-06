use crate::parse_scalars::{parse_int_signed, parse_yaml11_bool, parse_yaml12_float};
use crate::tags::SfTag;
use crate::Location;

/// Controls quoting behavior of the serializer.

/// Returns true if `s` can be emitted as a plain scalar without quoting.
/// Internal heuristic used by `write_plain_or_quoted`.
#[inline]
pub(crate) fn is_plain_safe(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    if s == "~"
        || s.eq_ignore_ascii_case("null")
        || s.eq_ignore_ascii_case("true")
        || s.eq_ignore_ascii_case("false")
    {
        return false;
    }
    let bytes = s.as_bytes();
    if bytes[0].is_ascii_whitespace()
        || matches!(
            bytes[0],
            b'-' | b'?'
                | b':'
                | b'['
                | b']'
                | b'{'
                | b'}'
                | b'#'
                | b'&'
                | b'*'
                | b'!'
                | b'|'
                | b'>'
                | b'\''
                | b'"'
                | b'%'
                | b'@'
                | b'`'
        )
    {
        return false;
    }

    !contains_any_or_is_control(s, &[':' , '#' , ','])
}

/// Returns true if `s` can be emitted as a plain scalar in VALUE position without quoting.
/// This is slightly more permissive than `is_plain_safe` for keys: it allows ':' inside values.
/// Additionally, we make this stricter for strings that appear inside flow-style sequences/maps
/// where certain characters would break parsing (e.g., commas and brackets) or where the token
/// could be misinterpreted as a number or boolean.
#[inline]
pub(crate) fn is_plain_value_safe(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    // Nulls and YAML 1.2 booleans
    if s == "~"
        || s.eq_ignore_ascii_case("null")
    {
        return false;
    }
    // Numeric-looking tokens: quote them to preserve strings
    // Use parsing as a heuristic; if it parses as a number, don't allow plain style
    if parse_int_signed::<i64>(s, "i64", Location::UNKNOWN, true).is_ok() {
        return false;
    }
    if parse_yaml12_float::<f64>(s, Location::UNKNOWN, SfTag::Float, false).is_ok() {
        return false;
    }
    if parse_yaml11_bool(s).is_ok() {
        return false;
    }

    // Special float tokens per YAML
    let bytes = s.as_bytes();
    if bytes[0].is_ascii_whitespace()
        || matches!(
            bytes[0],
            b'-' | b'?'
                | b':'
                | b'['
                | b']'
                | b'{'
                | b'}'
                | b'#'
                | b'&'
                | b'*'
                | b'!'
                | b'|'
                | b'>'
                | b'\''
                | b'"'
                | b'%'
                | b'@'
                | b'`'
        )
    {
        return false;
    }

    // Yet while colon is ok, colon after whitespace is not.
    if s.contains(": ") {
        // We only need to check for space as CR, LF and TAB are control characters and will
        // trigger escape on their own anyway.
        return false;
    }

    // In flow style, commas and brackets/braces are structural; quote strings containing them.
    // In values, ':' is allowed, but '#' would start a comment so still disallow '#'.
    !contains_any_or_is_control(s, &[',', '[', ']', '{', '}', '#'])
}

fn contains_any_or_is_control(string: &str, values: &[char]) -> bool {
    string
        .chars()
        .any(|x| values.iter().any(|v| &x == v || x.is_control()))
}
