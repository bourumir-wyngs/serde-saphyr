use crate::Location;
use crate::parse_scalars::{parse_int_signed, parse_yaml11_bool, parse_yaml12_float};
use crate::tags::SfTag;

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
            b':' | b'['
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

    // Handle '-' and '?' specially: only quote if they're alone or followed by whitespace
    // This matches Go's yaml.v3 behavior where "-foo" is a safe plain scalar
    // but "- foo" would be a sequence indicator
    if bytes[0] == b'-' || bytes[0] == b'?' {
        // Single character '-' or '?' needs quoting
        if bytes.len() == 1 {
            return false;
        }
        // Followed by whitespace needs quoting (sequence/mapping indicator)
        if bytes[1].is_ascii_whitespace() {
            return false;
        }
        // "-foo" or "?bar" is safe - continue to other checks
    }

    !contains_any_or_is_control(s, &[':', '#', ','])
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
    if s == "~" || s.eq_ignore_ascii_case("null") {
        return false;
    }
    // ISO8601 timestamps need quoting to avoid being parsed as dates
    if looks_like_timestamp(s) {
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
            b':' | b'['
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

    // Handle '-' and '?' specially: only quote if they're alone or followed by whitespace
    // This matches Go's yaml.v3 behavior where "-foo" is a safe plain scalar
    // but "- foo" would be a sequence indicator
    if bytes[0] == b'-' || bytes[0] == b'?' {
        // Single character '-' or '?' needs quoting
        if bytes.len() == 1 {
            return false;
        }
        // Followed by whitespace needs quoting (sequence/mapping indicator)
        if bytes[1].is_ascii_whitespace() {
            return false;
        }
        // "-foo" or "?bar" is safe - continue to other checks
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

/// Returns true if `s` can be emitted as a plain scalar in block-style VALUE position.
/// This is more permissive than `is_plain_value_safe` because in block style, commas
/// and brackets WITHIN values are allowed (they're only structural in flow style).
/// However, standalone structural characters (just ",", "[", etc.) still need quoting.
/// This matches Go's yaml.v3 behavior.
#[inline]
pub(crate) fn is_plain_block_value_safe(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    // Nulls and YAML 1.2 booleans
    if s == "~" || s.eq_ignore_ascii_case("null") {
        return false;
    }
    // ISO8601 timestamps need quoting to avoid being parsed as dates
    // Matches patterns like: 2025-07-03T15:30:00Z, 2025-07-03T15:30:00+00:00
    if looks_like_timestamp(s) {
        return false;
    }
    // Numeric-looking tokens: quote them to preserve strings
    if parse_int_signed::<i64>(s, "i64", Location::UNKNOWN, true).is_ok() {
        return false;
    }
    if parse_yaml12_float::<f64>(s, Location::UNKNOWN, SfTag::Float, false).is_ok() {
        return false;
    }
    if parse_yaml11_bool(s).is_ok() {
        return false;
    }

    let bytes = s.as_bytes();

    // Standalone structural characters need quoting even in block style
    // (they can confuse the parser at value position)
    if bytes.len() == 1 && matches!(bytes[0], b',' | b'[' | b']' | b'{' | b'}') {
        return false;
    }

    if bytes[0].is_ascii_whitespace()
        || matches!(
            bytes[0],
            b':' | b'['
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

    // Handle '-' and '?' specially: only quote if they're alone or followed by whitespace
    if bytes[0] == b'-' || bytes[0] == b'?' {
        if bytes.len() == 1 {
            return false;
        }
        if bytes[1].is_ascii_whitespace() {
            return false;
        }
    }

    // Colon after whitespace would start a mapping value
    if s.contains(": ") {
        return false;
    }

    // In block style, commas and brackets WITHIN values are allowed.
    // Only disallow '#' (comment indicator) and control characters.
    !contains_any_or_is_control(s, &['#'])
}

fn contains_any_or_is_control(string: &str, values: &[char]) -> bool {
    string
        .chars()
        .any(|x| values.iter().any(|v| &x == v || x.is_control()))
}

/// Returns true if `s` looks like an ISO8601 timestamp that YAML parsers might
/// interpret as a date/time. These need to be quoted to preserve them as strings.
/// Matches patterns like: 2025-07-03T15:30:00Z, 2025-07-03T15:30:00+00:00
#[inline]
fn looks_like_timestamp(s: &str) -> bool {
    let bytes = s.as_bytes();
    // Must be at least "YYYY-MM-DDTHH:MM:SS" (19 chars)
    if bytes.len() < 19 {
        return false;
    }
    // Check for YYYY-MM-DDTHH:MM:SS pattern
    // Position: 0123456789...
    //           YYYY-MM-DDTHH:MM:SS
    bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[0..4].iter().all(|b| b.is_ascii_digit())
        && bytes[5..7].iter().all(|b| b.is_ascii_digit())
        && bytes[8..10].iter().all(|b| b.is_ascii_digit())
        && bytes[11..13].iter().all(|b| b.is_ascii_digit())
        && bytes[14..16].iter().all(|b| b.is_ascii_digit())
        && bytes[17..19].iter().all(|b| b.is_ascii_digit())
}
