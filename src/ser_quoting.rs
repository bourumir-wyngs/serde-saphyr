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
    if s.chars().any(|c| c.is_control()) {
        return false;
    }
    if s.contains(':') || s.contains('#') {
        return false;
    }
    true
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
        || s.eq_ignore_ascii_case("true")
        || s.eq_ignore_ascii_case("false")
    {
        return false;
    }
    // YAML 1.1 boolean aliases that some parsers accept
    if s.eq_ignore_ascii_case("y")
        || s.eq_ignore_ascii_case("yes")
        || s.eq_ignore_ascii_case("n")
        || s.eq_ignore_ascii_case("no")
        || s.eq_ignore_ascii_case("on")
        || s.eq_ignore_ascii_case("off")
    {
        return false;
    }
    // Numeric-looking tokens: quote them to preserve strings
    // Use parsing as a heuristic; if it parses as a number, don't allow plain style
    if s.parse::<i64>().is_ok() || s.parse::<u64>().is_ok() || s.parse::<f64>().is_ok() {
        return false;
    }
    // Special float tokens per YAML
    let sl = s.to_ascii_lowercase();
    if sl == ".nan" || sl == ".inf" || sl == "-.inf" {
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

    // Yet while colon is ok, colon after space is not.
    if s.contains(": ") {
        return false;
    }

    // In flow style, commas and brackets/braces are structural; quote strings containing them.
    // In values, ':' is allowed, but '#' would start a comment so still disallow '#'.
    if contains_any_or_is_control(s, &[',', '[', ']', '{', '}', '#']) {
        return false;
    }
    true
}

fn contains_any_or_is_control(string: &str, values: &[char]) -> bool {
    string
        .chars()
        .any(|x| values.iter().any(|v| &x == v || x.is_control()))
}
