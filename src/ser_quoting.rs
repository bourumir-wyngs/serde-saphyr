use crate::parse_scalars::parse_yaml11_bool;
use regex::Regex;
use std::sync::OnceLock;

fn is_numeric_looking(s: &str) -> bool {
    // Match a broad set of YAML numeric tokens (integer / float) even if they would overflow
    // when parsed into Rust numeric types.
    //
    // Notes:
    // - Underscores are allowed between digits.
    // - Supports optional sign.
    // - Supports `0x`/`0o`/`0b` prefixes.
    // - Supports decimal scientific notation with or without a dot (e.g. `1e9`, `1.0e9`, `.5`).
    //
    // Compiled once to avoid regex compile cost on hot path.
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"(?x)
            ^[+-]?(?:
                # Explicit radices
                0x[0-9A-Fa-f_]+ |
                0o[0-7_]+       |
                0b[01_]+        |

                # Decimal floats / integers
                (?:
                    # Float with dot: 1. , 1.0 , .5
                    (?:[0-9][0-9_]*\.[0-9_]*|\.[0-9][0-9_]*)
                    (?:[eE][+-]?[0-9][0-9_]*)?
                |
                    # Scientific without dot: 1e9
                    [0-9][0-9_]*[eE][+-]?[0-9][0-9_]*
                |
                    # Plain integer: 123
                    [0-9][0-9_]*
                )
            )$
            ",
        )
            .expect("valid numeric-looking regex")
    });

    re.is_match(s)
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
        if let Some(&c) = bytes.first() && (c == b'+' || c == b'-') {
            i = 1;
        }
        if let Some(&c) = bytes.get(i) && c == b'.' {
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

/// Like `is_ambiguous`, but used for VALUE position.
///
/// For values we are more conservative: quote additional spellings that many YAML
/// parsers accept as floats even if YAML 1.2 requires the leading-dot form.
#[inline]
fn is_ambiguous_value(s: &str) -> bool {
    if is_ambiguous(s) {
        return true;
    }

    // YAML 1.1 boolean spellings: quote them as strings for compatibility and
    // round-tripping (e.g. "YES", "no", "On", "off", "y", "n").
    if parse_yaml11_bool(s).is_ok() {
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

    !contains_any_or_is_control(s, &[':', '#', ','])
}

/// Returns true if `s` can be emitted as a plain scalar in VALUE position without quoting.
/// This is slightly more permissive than `is_plain_safe` for keys: it allows ':' inside values.
/// Additionally, we make this stricter for strings that appear inside flow-style sequences/maps
/// where certain characters would break parsing (e.g., commas and brackets) or where the token
/// could be misinterpreted as a number or boolean.
#[inline]
pub(crate) fn is_plain_value_safe(s: &str) -> bool {
    if is_ambiguous_value(s) {
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
    if s.contains(": ") || s.trim().ends_with(':') {
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
