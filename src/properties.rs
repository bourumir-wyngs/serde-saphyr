use std::borrow::Cow;
use std::collections::HashMap;

/// Property interpolation failure reported while scanning a scalar value.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PropertyError {
    /// A valid `${NAME}` reference could not be resolved from the property map.
    Unresolved(String),
    /// A `${...}` candidate was present but did not use a valid property name.
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

/// Parses `${name}` starting at `start`, returning the referenced property name and the end index.
///
/// Returns `Err(candidate)` when a braced `${...}` sequence is present but its name is invalid.
/// Returns `Ok(None)` when the input does not contain a complete braced candidate at `start`.
fn parse_braced_reference(input: &str, start: usize) -> Result<Option<(&str, usize)>, String> {
    let body_start = start + 2;
    let Some(close_rel) = input[body_start..].find('}') else {
        return Ok(None);
    };
    let close = close_rel + body_start;
    let body = &input[body_start..close];
    let candidate = input[start..close + 1].to_owned();
    let Some((name, rest)) = parse_name(body) else {
        return Err(candidate);
    };

    if rest.is_empty() {
        Ok(Some((name, close + 1)))
    } else {
        Err(candidate)
    }
}

/// Parameters:
/// - `input`: scalar text after YAML parsing, before Serde type conversion.
/// - `vars`: final caller-supplied property map. Values are treated as final values,
///   so this function does not recursively expand placeholders inside map entries.
///
/// Returns:
/// - `Cow::Borrowed` when the scalar is unchanged, or `Cow::Owned` with the expanded value.
pub (crate) fn interpolate_compose_style<'s>(
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
        let ch = input_str[i..].chars().next().expect("valid UTF-8 boundary");
        if ch != '$' {
            i += ch.len_utf8();
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

        let Some((name, end)) = parse_braced_reference(input_str, i)
            .map_err(PropertyError::InvalidName)?
        else {
            i += 1;
            continue;
        };

        let value = vars
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| PropertyError::Unresolved(name.to_owned()))?;

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
    use super::{interpolate_compose_style, PropertyError};
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
    fn reports_invalid_property_name() {
        let vars = HashMap::from([(String::from("NAME"), String::from("world"))]);

        let error = interpolate_compose_style(Cow::Borrowed("${NAME:-fallback}"), &vars).unwrap_err();

        assert_eq!(
            error,
            PropertyError::InvalidName("${NAME:-fallback}".to_string())
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
