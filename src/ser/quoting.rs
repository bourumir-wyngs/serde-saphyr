use crate::parse_scalars::parse_yaml11_bool;


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
    fn consume_digits(bytes: &[u8], mut p: usize) -> usize {
        while p < bytes.len() && bytes[p].is_ascii_digit() {
            p += 1;
        }
        p
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

        let start = p;
        p = consume_digits(bytes, p);

        (p > start).then_some(p)
    }

    // YAML 1.2 core: lowercase-only 0o / 0x, no sign on radix-prefixed integers.
    if len >= 3 && bytes[0] == b'0' {
        match bytes[1] {
            b'o' => {
                let mut p = 2;
                if p == len {
                    return false;
                }

                while p < len {
                    if !(b'0'..=b'7').contains(&bytes[p]) {
                        return false;
                    }
                    p += 1;
                }

                return true;
            }
            b'x' => {
                let mut p = 2;
                if p == len {
                    return false;
                }

                while p < len {
                    if !bytes[p].is_ascii_hexdigit() {
                        return false;
                    }
                    p += 1;
                }

                return true;
            }
            _ => {}
        }
    }

    let mut p = 0;
    if bytes[0] == b'+' || bytes[0] == b'-' {
        p = 1;
        if p == len {
            return false;
        }
    }

    // Dot-leading float: .5, +.5, -.5
    if bytes[p] == b'.' {
        p += 1;
        let start = p;
        p = consume_digits(bytes, p);

        if p == start {
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

    p = consume_digits(bytes, p);

    if p == len {
        return true;
    }

    if bytes[p] == b'.' {
        p += 1;
        p = consume_digits(bytes, p);

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
    let bytes = s.as_bytes();
    if bytes[0].is_ascii_whitespace() {
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
/// This is slightly more permissive than `is_plain_safe` for keys: it allows ':' inside values.
/// Additionally, we make this stricter for strings that appear inside flow-style sequences/maps
/// where certain characters would break parsing (e.g., commas and brackets) or where the token
/// could be misinterpreted as a number or boolean.
#[inline]
pub(crate) fn is_plain_value_safe(s: &str, yaml_12: bool, in_flow: bool) -> bool {
    if is_ambiguous_value(s, yaml_12) {
        return false;
    }

    let bytes = s.as_bytes();
    if bytes[0].is_ascii_whitespace() {
        return false;
    }

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

fn contains_any_or_is_control(string: &str, values: &[char]) -> bool {
    string
        .chars()
        .any(|x| values.iter().any(|v| &x == v || x.is_control()))
}

#[cfg(test)]
mod tests {
    use super::is_numeric_looking;

    #[test]
    fn numeric_looking_yaml12_core() {
        for s in [
            "0",
            "-19",
            "+12",
            "01",
            "0o7",
            "0x3A",
            ".5",
            "+.5",
            "-.5",
            "0.",
            "-0.0",
            "12e03",
            "-2E+05",
            "12.34e-5",
        ] {
            assert!(is_numeric_looking(s), "{s:?} should match");
        }

        for s in [
            "",
            "+",
            "-",
            ".",
            "_1",
            "1_0",
            "1e_2",
            "_.5",
            "._5",
            "0X3A",
            "+0x3A",
            "0b101",
            "0o",
            "0x",
            "12e",
            ".e5",
            ".inf", // handled by the separate special-float helper
            ".nan", // handled by the separate special-float helper
        ] {
            assert!(!is_numeric_looking(s), "{s:?} should not match");
        }
    }
}