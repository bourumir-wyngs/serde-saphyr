use super::options::PropertySyntax;
use std::borrow::Cow;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PropertyError {
    /// `${NAME}` had no value in the property map and no default was supplied.
    Unresolved(String),
    /// A `${...}` candidate was present but did not parse as a supported form.
    /// The string is the full candidate including braces.
    InvalidName(String),
    /// `${NAME?text}` or `${NAME:?text}` referenced a variable that was unset.
    /// `message` may be empty.
    RequiredButUnset { name: String, message: String },
    /// `${NAME:?text}` referenced a variable that was present but empty.
    /// `message` may be empty.
    RequiredButEmpty { name: String, message: String },
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

/// The docker-compose `${...}` substitution forms.
/// The `&str` payload is the default, replacement, or error text.
/// It may be empty.
enum BraceOp<'a> {
    /// `${VAR}`.
    /// Errors when `VAR` is unset.
    Required,
    /// `${VAR-text}`.
    /// An empty `VAR` still passes through.
    DefaultIfUnset(&'a str),
    /// `${VAR:-text}`.
    DefaultIfUnsetOrEmpty(&'a str),
    /// `${VAR+text}`.
    /// An empty `VAR` counts as set.
    AlternateIfSet(&'a str),
    /// `${VAR:+text}`.
    AlternateIfSetAndNonEmpty(&'a str),
    /// `${VAR?text}`.
    /// Errors when `VAR` is unset; an empty `VAR` still passes through.
    ErrorIfUnset(&'a str),
    /// `${VAR:?text}`.
    /// Errors when `VAR` is unset or empty.
    ErrorIfUnsetOrEmpty(&'a str),
}

struct BraceRef<'a> {
    name: &'a str,
    op: BraceOp<'a>,
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

    let op = if rest.is_empty() {
        BraceOp::Required
    } else if let Some(text) = rest.strip_prefix(":-") {
        BraceOp::DefaultIfUnsetOrEmpty(text)
    } else if let Some(text) = rest.strip_prefix(":+") {
        BraceOp::AlternateIfSetAndNonEmpty(text)
    } else if let Some(text) = rest.strip_prefix('-') {
        BraceOp::DefaultIfUnset(text)
    } else if let Some(text) = rest.strip_prefix('+') {
        BraceOp::AlternateIfSet(text)
    } else if let Some(text) = rest.strip_prefix(":?") {
        BraceOp::ErrorIfUnsetOrEmpty(text)
    } else if let Some(text) = rest.strip_prefix('?') {
        BraceOp::ErrorIfUnset(text)
    } else {
        return Err(input[start..close + 1].to_owned());
    };

    Ok(Some((BraceRef { name, op }, close + 1)))
}

fn resolve_brace<'a>(
    brace: &'a BraceRef<'a>,
    vars: &'a HashMap<String, String>,
) -> Result<&'a str, PropertyError> {
    let name = brace.name;
    let value = vars.get(name).map(String::as_str);
    match (&brace.op, value) {
        (BraceOp::Required, Some(v)) => Ok(v),
        (BraceOp::Required, None) => Err(PropertyError::Unresolved(name.to_owned())),
        (BraceOp::DefaultIfUnset(text), None) => Ok(text),
        (BraceOp::DefaultIfUnset(_), Some(v)) => Ok(v),
        (BraceOp::DefaultIfUnsetOrEmpty(text), None | Some("")) => Ok(text),
        (BraceOp::DefaultIfUnsetOrEmpty(_), Some(v)) => Ok(v),
        (BraceOp::AlternateIfSet(text), Some(_)) => Ok(text),
        (BraceOp::AlternateIfSet(_), None) => Ok(""),
        (BraceOp::AlternateIfSetAndNonEmpty(_), None | Some("")) => Ok(""),
        (BraceOp::AlternateIfSetAndNonEmpty(text), Some(_)) => Ok(text),
        (BraceOp::ErrorIfUnset(_), Some(v)) => Ok(v),
        (BraceOp::ErrorIfUnset(msg), None) => Err(PropertyError::RequiredButUnset {
            name: name.to_owned(),
            message: (*msg).to_owned(),
        }),
        (BraceOp::ErrorIfUnsetOrEmpty(_), Some(v)) if !v.is_empty() => Ok(v),
        (BraceOp::ErrorIfUnsetOrEmpty(msg), Some(_)) => Err(PropertyError::RequiredButEmpty {
            name: name.to_owned(),
            message: (*msg).to_owned(),
        }),
        (BraceOp::ErrorIfUnsetOrEmpty(msg), None) => Err(PropertyError::RequiredButUnset {
            name: name.to_owned(),
            message: (*msg).to_owned(),
        }),
    }
}

/// Expands docker-compose-style `${...}` references in `input` against `vars`.
/// See [`BraceOp`] for the supported forms.
/// Pass [`PropertySyntax::BracedOrBare`] to also recognize the bare `$NAME` form
/// (which uses Required semantics).
///
/// Values in `vars` are taken as final.
/// Placeholders inside map entries are not re-expanded.
/// Returns `Cow::Borrowed` when nothing changed so the common no-`$` path stays allocation-free.
pub(crate) fn interpolate_compose_style<'s>(
    input: Cow<'s, str>,
    vars: &HashMap<String, String>,
    syntax: PropertySyntax,
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
            // $$ = escaped, so skip and treat as literal
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

        if bytes[next] == b'{' {
            // a ${.. reference, so parse as braced
            let Some((brace, end)) =
                parse_braced_reference(input_str, i).map_err(PropertyError::InvalidName)?
            else {
                i += 1;
                continue;
            };

            let value = resolve_brace(&brace, vars)?;

            if !changed {
                out.push_str(&input_str[..i]);
                changed = true;
            } else {
                out.push_str(&input_str[last..i]);
            }
            out.push_str(value);

            i = end;
            last = i;
        } else if syntax == PropertySyntax::Braced {
            i += 1; // not a ${.. reference, so skip and treat as literal
            continue;
        } else {
            // not a ${.. reference but we are PropertySyntax::BracedOrBare, so parse as bare
            let body = &input_str[next..];
            let Some((name, _rest)) = parse_name(body) else {
                // $ not followed by a valid name start -> treat $ as literal
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

            i = next + name.len();
            last = i;
        }
    }

    if !changed {
        return Ok(input);
    }

    out.push_str(&input_str[last..]);
    Ok(Cow::Owned(out))
}

#[cfg(test)]
mod tests {
    use super::{PropertyError, PropertySyntax, interpolate_compose_style};
    use rstest::rstest;
    use std::borrow::Cow;
    use std::collections::HashMap;

    fn vars() -> HashMap<String, String> {
        HashMap::from([
            (String::from("SET"), String::from("value")),
            (String::from("EMPTY"), String::new()),
        ])
    }

    #[rstest]
    #[case::required_set("${SET}", "value")]
    #[case::required_empty("${EMPTY}", "")]
    #[case::default_if_unset_set("${SET-fallback}", "value")]
    #[case::default_if_unset_empty("${EMPTY-fallback}", "")]
    #[case::default_if_unset_missing("${MISSING-fallback}", "fallback")]
    #[case::default_if_unset_or_empty_set("${SET:-fallback}", "value")]
    #[case::default_if_unset_or_empty_empty("${EMPTY:-fallback}", "fallback")]
    #[case::default_if_unset_or_empty_missing("${MISSING:-fallback}", "fallback")]
    #[case::alternate_if_set_set("${SET+yes}", "yes")]
    #[case::alternate_if_set_empty("${EMPTY+yes}", "yes")]
    #[case::alternate_if_set_missing("${MISSING+yes}", "")]
    #[case::alternate_if_set_and_nonempty_set("${SET:+yes}", "yes")]
    #[case::alternate_if_set_and_nonempty_empty("${EMPTY:+yes}", "")]
    #[case::alternate_if_set_and_nonempty_missing("${MISSING:+yes}", "")]
    #[case::error_if_unset_set("${SET?msg}", "value")]
    #[case::error_if_unset_empty("${EMPTY?msg}", "")]
    #[case::error_if_unset_or_empty_set("${SET:?msg}", "value")]
    fn brace_op_resolves(#[case] input: &str, #[case] expected: &str) {
        let output =
            interpolate_compose_style(Cow::Borrowed(input), &vars(), PropertySyntax::Braced)
                .unwrap();
        assert_eq!(output.as_ref(), expected);
    }

    #[rstest]
    #[case("${MISSING-}")]
    #[case("${MISSING:-}")]
    #[case("${SET+}")]
    #[case("${SET:+}")]
    fn empty_default_or_replacement_text_resolves_to_empty(#[case] input: &str) {
        let output =
            interpolate_compose_style(Cow::Borrowed(input), &vars(), PropertySyntax::Braced)
                .unwrap();
        assert_eq!(output.as_ref(), "");
    }

    #[test]
    fn keeps_input_without_dollar_borrowed() {
        let input = Cow::Borrowed("plain text");

        let output = interpolate_compose_style(input, &vars(), PropertySyntax::Braced).unwrap();

        assert_eq!(output, Cow::Borrowed("plain text"));
    }

    #[test]
    fn replaces_reference_after_non_ascii_text() {
        let output = interpolate_compose_style(
            Cow::Borrowed("h\u{e9} ${SET}"),
            &vars(),
            PropertySyntax::Braced,
        )
        .unwrap();

        assert_eq!(output.as_ref(), "h\u{e9} value");
    }

    #[test]
    fn reports_invalid_property_name() {
        let error = interpolate_compose_style(
            Cow::Borrowed("${NAME:=fallback}"),
            &vars(),
            PropertySyntax::Braced,
        )
        .unwrap_err();

        assert_eq!(
            error,
            PropertyError::InvalidName("${NAME:=fallback}".to_string())
        );
    }

    #[rstest]
    #[case::required_missing("${MISSING}", PropertyError::Unresolved("MISSING".into()))]
    #[case::error_if_unset_missing(
        "${MISSING?nope}",
        PropertyError::RequiredButUnset { name: "MISSING".into(), message: "nope".into() }
    )]
    #[case::error_if_unset_missing_empty_msg(
        "${MISSING?}",
        PropertyError::RequiredButUnset { name: "MISSING".into(), message: "".into() }
    )]
    #[case::error_if_unset_or_empty_missing(
        "${MISSING:?nope}",
        PropertyError::RequiredButUnset { name: "MISSING".into(), message: "nope".into() }
    )]
    #[case::error_if_unset_or_empty_missing_empty_msg(
        "${MISSING:?}",
        PropertyError::RequiredButUnset { name: "MISSING".into(), message: "".into() }
    )]
    #[case::error_if_unset_or_empty_empty(
        "${EMPTY:?nope}",
        PropertyError::RequiredButEmpty { name: "EMPTY".into(), message: "nope".into() }
    )]
    #[case::error_if_unset_or_empty_empty_empty_msg(
        "${EMPTY:?}",
        PropertyError::RequiredButEmpty { name: "EMPTY".into(), message: "".into() }
    )]
    fn brace_op_errors(#[case] input: &str, #[case] expected: PropertyError) {
        let error =
            interpolate_compose_style(Cow::Borrowed(input), &vars(), PropertySyntax::Braced)
                .unwrap_err();
        assert_eq!(error, expected);
    }

    #[rstest]
    #[case::two_braced("${SET}-${SET}", "value-value", PropertySyntax::Braced)]
    #[case::two_braced_bare("${SET}-${SET}", "value-value", PropertySyntax::BracedOrBare)]
    #[case::two_escapes("$$a$$b", "$a$b", PropertySyntax::Braced)]
    #[case::two_escapes_bare("$$a$$b", "$a$b", PropertySyntax::BracedOrBare)]
    #[case::escape_then_braced("$$x${SET}", "$xvalue", PropertySyntax::Braced)]
    #[case::braced_then_escape("${SET}$$x", "value$x", PropertySyntax::Braced)]
    #[case::var_escape_then_bare("$$x$SET", "$xvalue", PropertySyntax::BracedOrBare)]
    #[case::no_var_escape_then_bare("$$$SET", "$value", PropertySyntax::BracedOrBare)]
    #[case::bare_then_var_escape("$SET$$x", "value$x", PropertySyntax::BracedOrBare)]
    fn multiple_substitutions_use_last_cursor(
        #[case] input: &str,
        #[case] expected: &str,
        #[case] syntax: PropertySyntax,
    ) {
        let output = interpolate_compose_style(Cow::Borrowed(input), &vars(), syntax).unwrap();
        assert_eq!(output.as_ref(), expected);
    }

    #[rstest]
    #[case::braced("$${SET}", "${SET}", PropertySyntax::Braced)]
    #[case::braced("$${SET}", "${SET}", PropertySyntax::BracedOrBare)]
    #[case::bare("$$SET", "$SET", PropertySyntax::Braced)]
    #[case::bare("$$SET", "$SET", PropertySyntax::BracedOrBare)]
    fn treats_double_dollar_as_escape(
        #[case] input: &str,
        #[case] expected: &str,
        #[case] syntax: PropertySyntax,
    ) {
        let output = interpolate_compose_style(Cow::Borrowed(input), &vars(), syntax).unwrap();
        assert_eq!(output.as_ref(), expected);
    }

    #[rstest]
    #[case::bare_set("$SET", "value")]
    #[case::bare_empty("$EMPTY", "")]
    #[case::with_prefix("hello $SET", "hello value")]
    #[case::with_suffix("$SET world", "value world")]
    #[case::two_adjacent("$SET$EMPTY", "value")]
    #[case::dot_terminator("$SET.tail", "value.tail")]
    #[case::slash_terminator("$SET/tail", "value/tail")]
    #[case::dash_is_literal_unbraced("$SET-default", "value-default")]
    #[case::underscore("_$SET", "_value")]
    fn unbraced_resolves(#[case] input: &str, #[case] expected: &str) {
        let output =
            interpolate_compose_style(Cow::Borrowed(input), &vars(), PropertySyntax::BracedOrBare)
                .unwrap();
        assert_eq!(output.as_ref(), expected);
    }

    #[rstest]
    #[case::set("$SET")]
    #[case::empty("$EMPTY")]
    #[case::unset("$MISSING")]
    fn braced_ignores_unbraced(#[case] input: &str) {
        let output =
            interpolate_compose_style(Cow::Borrowed(input), &vars(), PropertySyntax::Braced)
                .unwrap();
        assert_eq!(output.as_ref(), input);
    }

    #[rstest]
    #[case::digit("$1.99")]
    #[case::slash("$/path")]
    #[case::space("price: $ 100")]
    #[case::end_of_input("trailing $")]
    #[case::unicode_letter("$\u{03a9}")]
    #[case::unclosed_brace("${SET")]
    #[case::unclosed_empty_brace("${")]
    #[case::unclosed_brace_with_prefix("prefix ${SET and more")]
    fn does_not_change_literal(
        #[case] input: &str,
        #[values(PropertySyntax::Braced, PropertySyntax::BracedOrBare)] syntax: PropertySyntax,
    ) {
        let output = interpolate_compose_style(Cow::Borrowed(input), &vars(), syntax).unwrap();
        assert_eq!(output.as_ref(), input);
    }

    #[rstest]
    #[case::like_braced("$MISSING", "MISSING")]
    #[case::like_braced("$SET_", "SET_")]
    #[case::greedy_name_boundary("$SETfoo", "SETfoo")]
    fn unbraced_unresolved_errors(#[case] input: &str, #[case] expected_name: &str) {
        let error =
            interpolate_compose_style(Cow::Borrowed(input), &vars(), PropertySyntax::BracedOrBare)
                .unwrap_err();
        assert_eq!(error, PropertyError::Unresolved(expected_name.into()));
    }

    #[test]
    fn unbraced_does_not_change_default_as_literal() {
        let output = interpolate_compose_style(
            Cow::Borrowed("${MISSING-$SET}"),
            &vars(),
            PropertySyntax::BracedOrBare,
        )
        .unwrap();
        assert_eq!(output.as_ref(), "$SET");
    }
}
