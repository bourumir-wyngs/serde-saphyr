use std::borrow::Cow;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PropertyError {
    /// `${NAME}` had no value in the property map and no default was supplied.
    Unresolved(String),
    /// A `${...}` candidate was present but did not parse as `${NAME}` or
    /// `${NAME:-default}`. The string is the full candidate including braces.
    InvalidName(String),
}

/// Checks whether a character is valid as the first character of a variable name.
fn is_var_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

/// Checks whether a character is valid as a continuing character of a variable name.
fn is_var_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

/// Parses a valid variable name from the beginning of the input string.
/// Returns the parsed name and the remaining unparsed input.
fn parse_name(input: &str) -> Option<(&str, &str)> {
    let mut chars = input.char_indices();
    let (_, first) = chars.next()?;
    if !is_var_start(first) {
        return None;
    }

    let mut end = first.len_utf8();
    for (i, ch) in chars {
        if !is_var_continue(ch) {
            return Some((&input[..end], &input[i..]));
        }
        end = i + ch.len_utf8();
    }

    Some((&input[..end], &input[end..]))
}

struct BraceRef<'a> {
    name: &'a str,
    default: Option<&'a str>,
}

/// Returns `Err` when the `${...}` candidate is malformed, `Ok(None)` when the brace
/// isn't closed (treat the `$` as literal), or `Ok(Some(...))` with the parsed reference
/// and the byte index just past the closing `}`.
fn parse_braced_reference(
    input: &str,
    start: usize,
) -> Result<Option<(BraceRef<'_>, usize)>, String> {
    let body_start = start + 2;
    let Some(close_rel) = input[body_start..].find('}') else {
        return Ok(None);
    };
    let close = close_rel + body_start;
    let body = &input[body_start..close];
    let Some((name, rest)) = parse_name(body) else {
        return Err(input[start..close + 1].to_owned());
    };

    if rest.is_empty() {
        return Ok(Some((
            BraceRef {
                name,
                default: None,
            },
            close + 1,
        )));
    }

    let default_text = rest
        .strip_prefix(":-")
        .ok_or_else(|| input[start..close + 1].to_owned())?;
    Ok(Some((
        BraceRef {
            name,
            default: Some(default_text),
        },
        close + 1,
    )))
}

fn resolve_brace<'a>(
    brace: &'a BraceRef<'a>,
    vars: &'a HashMap<String, String>,
) -> Result<&'a str, &'a str> {
    match (vars.get(brace.name).map(String::as_str), brace.default) {
        (Some(value), _) => Ok(value),
        (None | Some(""), Some(default)) => Ok(default),
        (None, None) => Err(brace.name),
    }
}

/// Expands `${NAME}` and `${NAME:-default}` references in `input` against `vars`.
/// Values in `vars` are taken as final, so placeholders inside map entries are not re-expanded.
/// Returns `Cow::Borrowed` when nothing changed so the common no-`$` path stays allocation-free.
pub(crate) fn interpolate_compose_style<'s>(
    input: Cow<'s, str>,
    vars: &HashMap<String, String>,
) -> Result<Cow<'s, str>, PropertyError> {
    if !input.contains('$') {
        return Ok(input);
    }

    let input_str = input.as_ref();
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut changed = false;
    let mut last = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] != b'$' {
            i += 1;
            continue;
        }

        let next = i + 1;
        if next >= bytes.len() {
            i += 1;
            continue;
        }

        if bytes[next] == b'$' {
            if !changed {
                out.push_str(&input_str[..i]);
                changed = true;
            } else {
                out.push_str(&input_str[last..i]);
            }
            out.push('$');
            i += 2;
            last = i;
            continue;
        }

        if bytes[next] != b'{' {
            i += 1;
            continue;
        }

        let Some((brace, end)) =
            parse_braced_reference(input_str, i).map_err(PropertyError::InvalidName)?
        else {
            i += 1;
            continue;
        };

        let value =
            resolve_brace(&brace, vars).map_err(|n| PropertyError::Unresolved(n.to_owned()))?;

        if !changed {
            out.push_str(&input_str[..i]);
            changed = true;
        } else {
            out.push_str(&input_str[last..i]);
        }
        out.push_str(value);

        i = end;
        last = i;
    }

    if !changed {
        return Ok(input);
    }

    out.push_str(&input_str[last..]);
    Ok(Cow::Owned(out))
}

#[cfg(test)]
mod tests {
    use super::{PropertyError, interpolate_compose_style};
    use std::borrow::Cow;
    use std::collections::HashMap;

    #[test]
    fn keeps_input_without_dollar_borrowed() {
        let vars = HashMap::from([(String::from("NAME"), String::from("value"))]);
        let input = Cow::Borrowed("plain text");

        let output = interpolate_compose_style(input, &vars).unwrap();

        assert_eq!(output, Cow::Borrowed("plain text"));
    }

    #[test]
    fn replaces_braced_property_reference() {
        let vars = HashMap::from([(String::from("NAME"), String::from("world"))]);

        let output = interpolate_compose_style(Cow::Borrowed("hello ${NAME}"), &vars).unwrap();

        assert_eq!(output.as_ref(), "hello world");
    }

    #[test]
    fn replaces_reference_after_non_ascii_text() {
        let vars = HashMap::from([(String::from("NAME"), String::from("world"))]);

        let output = interpolate_compose_style(Cow::Borrowed("h\u{e9} ${NAME}"), &vars).unwrap();

        assert_eq!(output.as_ref(), "h\u{e9} world");
    }

    #[test]
    fn reports_invalid_property_name() {
        let vars = HashMap::from([(String::from("NAME"), String::from("world"))]);

        let error =
            interpolate_compose_style(Cow::Borrowed("${NAME:?fallback}"), &vars).unwrap_err();

        assert_eq!(
            error,
            PropertyError::InvalidName("${NAME:?fallback}".to_string())
        );
    }

    #[test]
    fn returns_unresolved_property_name() {
        let vars = HashMap::new();

        let error = interpolate_compose_style(Cow::Borrowed("${NAME}"), &vars).unwrap_err();

        assert_eq!(error, PropertyError::Unresolved("NAME".to_string()));
    }

    #[test]
    fn treats_double_dollar_as_escape() {
        let vars = HashMap::from([(String::from("NAME"), String::from("world"))]);

        let output = interpolate_compose_style(Cow::Borrowed("$${NAME}"), &vars).unwrap();

        assert_eq!(output.as_ref(), "${NAME}");
    }
}
