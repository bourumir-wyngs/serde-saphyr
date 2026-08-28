use super::options::PropertySyntax;
#[cfg(test)]
use crate::Budget;
use std::borrow::Cow;
use std::cell::Cell;
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
    /// A selected operator branch exceeded the configured nesting limit.
    ExpansionDepthLimitExceeded { depth: usize, max_depth: usize },
    /// Property scanning exceeded the configured aggregate work limit.
    ExpansionWorkLimitExceeded { work: usize, max_work: usize },
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
fn parse_name<'a>(
    input: &'a str,
    work: &mut WorkBudget<'_>,
) -> Result<Option<(&'a str, &'a str)>, PropertyError> {
    let mut chars = input.char_indices();
    let Some((_, first)) = chars.next() else {
        return Ok(None);
    };
    work.charge(first.len_utf8())?;
    if !is_var_start(first) {
        return Ok(None);
    }

    let mut end = first.len_utf8();
    for (i, ch) in chars {
        work.charge(ch.len_utf8())?;
        if !is_var_continue(ch) {
            return Ok(Some((&input[..end], &input[i..])));
        }
        end = i + ch.len_utf8();
    }

    Ok(Some((&input[..end], &input[end..])))
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

/// Charges property-interpolation scanning work against the stream-wide cumulative limit.
struct WorkBudget<'a> {
    total: &'a Cell<usize>,
    max: usize,
}

impl WorkBudget<'_> {
    #[inline]
    fn charge(&mut self, additional: usize) -> Result<(), PropertyError> {
        let current = self.total.get();
        let work = current.saturating_add(additional);
        if work > self.max {
            self.total.set(work);
            return Err(PropertyError::ExpansionWorkLimitExceeded {
                work,
                max_work: self.max,
            });
        }
        self.total.set(work);
        Ok(())
    }
}

/// Returns `Err` when the `${...}` candidate is malformed, `Ok(None)` when the brace
/// isn't closed (treat the `$` as literal), or `Ok(Some(...))` with the parsed reference
/// and the byte index just past the closing `}`.
fn parse_braced_reference<'a>(
    input: &'a str,
    start: usize,
    work: &mut WorkBudget<'_>,
) -> Result<Option<(BraceRef<'a>, usize)>, PropertyError> {
    let body_start = start + 2;
    let Some(close) = find_braced_reference_close(input, body_start, work)? else {
        return Ok(None);
    };
    let body = &input[body_start..close];
    let Some((name, rest)) = parse_name(body, work)? else {
        return Err(PropertyError::InvalidName(input[start..=close].to_owned()));
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
        return Err(PropertyError::InvalidName(input[start..=close].to_owned()));
    };

    Ok(Some((BraceRef { name, op }, close + 1)))
}

fn find_braced_reference_close(
    input: &str,
    body_start: usize,
    work: &mut WorkBudget<'_>,
) -> Result<Option<usize>, PropertyError> {
    let bytes = input.as_bytes();
    let mut depth = 0usize;
    let mut i = body_start;

    while i < bytes.len() {
        work.charge(1)?;
        if bytes[i] == b'$' && bytes.get(i + 1) == Some(&b'{') {
            work.charge(1)?;
            depth = depth.saturating_add(1);
            i += 2;
            continue;
        }

        if bytes[i] == b'}' {
            if depth == 0 {
                return Ok(Some(i));
            }
            depth -= 1;
        }

        i += 1;
    }

    Ok(None)
}

/// Describes how a completed interpolation frame is applied to its parent or returned as an error.
enum FrameCompletion {
    /// Return the completed value as the result of the top-level interpolation.
    Root,
    /// Append the completed nested operator text to its parent frame.
    Append,
    /// Report an unset required property after interpolating its error text.
    RequiredButUnset(String),
    /// Report an empty required property after interpolating its error text.
    RequiredButEmpty(String),
}

/// Explicit-stack state for expanding one root scalar or selected operator-text branch.
struct ExpansionFrame<'a> {
    input: &'a str,
    syntax: PropertySyntax,
    out: String,
    changed: bool,
    last: usize,
    cursor: usize,
    completion: FrameCompletion,
}

impl<'a> ExpansionFrame<'a> {
    fn new(input: &'a str, syntax: PropertySyntax, completion: FrameCompletion) -> Self {
        Self {
            input,
            syntax,
            // Allocate only for materialized output. Reserving every nested suffix here would
            // recreate the quadratic retained-memory behavior of the recursive implementation.
            out: String::new(),
            changed: false,
            last: 0,
            cursor: 0,
            completion,
        }
    }

    fn begin_replacement(&mut self, start: usize, end: usize) {
        if self.changed {
            self.out.push_str(&self.input[self.last..start]);
        } else {
            self.out.push_str(&self.input[..start]);
            self.changed = true;
        }
        self.cursor = end;
        self.last = end;
    }

    fn append_replacement(&mut self, start: usize, end: usize, value: &str) {
        self.begin_replacement(start, end);
        self.out.push_str(value);
    }
}

/// Completed frame output that preserves borrowing when interpolation made no replacement.
enum FrameValue<'a> {
    /// The frame completed without changing its input.
    Borrowed(&'a str),
    /// The frame materialized an interpolated value.
    Owned(String),
}

impl FrameValue<'_> {
    fn as_str(&self) -> &str {
        match self {
            Self::Borrowed(value) => value,
            Self::Owned(value) => value,
        }
    }

    fn into_owned(self) -> String {
        match self {
            Self::Borrowed(value) => value.to_owned(),
            Self::Owned(value) => value,
        }
    }
}

/// Action produced by resolving one braced property reference against the property map.
enum BraceAction<'a> {
    /// Append a property value or an empty replacement directly.
    Append(&'a str),
    /// Evaluate selected operator text in a nested interpolation frame.
    Interpolate {
        text: &'a str,
        completion: FrameCompletion,
    },
    /// Stop interpolation and return the property error.
    Error(PropertyError),
}

fn resolve_brace_action<'a>(
    brace: BraceRef<'a>,
    vars: &'a HashMap<String, String>,
) -> BraceAction<'a> {
    let name = brace.name;
    let value = vars.get(name).map(String::as_str);
    match (brace.op, value) {
        (BraceOp::Required, Some(value)) => BraceAction::Append(value),
        (BraceOp::Required, None) => BraceAction::Error(PropertyError::Unresolved(name.to_owned())),
        (BraceOp::DefaultIfUnset(text), None)
        | (BraceOp::DefaultIfUnsetOrEmpty(text), None | Some("")) => BraceAction::Interpolate {
            text,
            completion: FrameCompletion::Append,
        },
        (BraceOp::DefaultIfUnset(_), Some(value))
        | (BraceOp::DefaultIfUnsetOrEmpty(_), Some(value)) => BraceAction::Append(value),
        (BraceOp::AlternateIfSet(text), Some(_)) => BraceAction::Interpolate {
            text,
            completion: FrameCompletion::Append,
        },
        (BraceOp::AlternateIfSet(_), None) => BraceAction::Append(""),
        (BraceOp::AlternateIfSetAndNonEmpty(text), Some(value)) => {
            if value.is_empty() {
                BraceAction::Append("")
            } else {
                BraceAction::Interpolate {
                    text,
                    completion: FrameCompletion::Append,
                }
            }
        }
        (BraceOp::AlternateIfSetAndNonEmpty(_), None) => BraceAction::Append(""),
        (BraceOp::ErrorIfUnset(_), Some(value)) => BraceAction::Append(value),
        (BraceOp::ErrorIfUnset(message), None) => BraceAction::Interpolate {
            text: message,
            completion: FrameCompletion::RequiredButUnset(name.to_owned()),
        },
        (BraceOp::ErrorIfUnsetOrEmpty(_), Some(value)) if !value.is_empty() => {
            BraceAction::Append(value)
        }
        (BraceOp::ErrorIfUnsetOrEmpty(message), Some(_)) => BraceAction::Interpolate {
            text: message,
            completion: FrameCompletion::RequiredButEmpty(name.to_owned()),
        },
        (BraceOp::ErrorIfUnsetOrEmpty(message), None) => BraceAction::Interpolate {
            text: message,
            completion: FrameCompletion::RequiredButUnset(name.to_owned()),
        },
    }
}

/// Expands docker-compose-style `${...}` references in `input` against `vars`.
/// See [`BraceOp`] for the supported forms.
/// Pass [`PropertySyntax::BracedOrBare`] to also recognize the bare `$NAME` form
/// (which uses Required semantics).
///
/// Values in `vars` are taken as final.
/// Placeholders inside map entries are not re-expanded. Braced placeholders inside
/// default, alternate, and error text from the input are expanded recursively.
/// Returns `Cow::Borrowed` when nothing changed so the common no-`$` path stays allocation-free.
#[cfg(test)]
pub(crate) fn interpolate_compose_style<'s>(
    input: Cow<'s, str>,
    vars: &HashMap<String, String>,
    syntax: PropertySyntax,
) -> Result<Cow<'s, str>, PropertyError> {
    let total_work = Cell::new(0);
    let budget = Budget::default();
    interpolate_compose_style_with_limits(
        input,
        vars,
        syntax,
        budget.max_property_expansion_depth,
        budget.max_total_property_interpolation_work,
        &total_work,
    )
}

pub(crate) fn interpolate_compose_style_with_limits<'s>(
    input: Cow<'s, str>,
    vars: &HashMap<String, String>,
    syntax: PropertySyntax,
    max_nested_expansions: usize,
    max_total_interpolation_work: usize,
    total_work: &Cell<usize>,
) -> Result<Cow<'s, str>, PropertyError> {
    let mut work = WorkBudget {
        total: total_work,
        max: max_total_interpolation_work,
    };
    let Some(first_dollar) = input.find('$') else {
        work.charge(input.len())?;
        return Ok(input);
    };
    work.charge(first_dollar.saturating_add(1))?;

    let input_str = input.as_ref();
    let mut frames = vec![ExpansionFrame::new(
        input_str,
        syntax,
        FrameCompletion::Root,
    )];

    loop {
        if let Some(mut frame) = frames.pop_if(|frame| frame.cursor >= frame.input.len()) {
            let value = if frame.changed {
                frame.out.push_str(&frame.input[frame.last..]);
                FrameValue::Owned(frame.out)
            } else {
                FrameValue::Borrowed(frame.input)
            };

            match frame.completion {
                FrameCompletion::Root => {
                    return match value {
                        FrameValue::Borrowed(_) => Ok(input),
                        FrameValue::Owned(value) => Ok(Cow::Owned(value)),
                    };
                }
                FrameCompletion::Append => frames
                    .last_mut()
                    .expect("nested interpolation has a parent")
                    .out
                    .push_str(value.as_str()),
                FrameCompletion::RequiredButUnset(name) => {
                    return Err(PropertyError::RequiredButUnset {
                        name,
                        message: value.into_owned(),
                    });
                }
                FrameCompletion::RequiredButEmpty(name) => {
                    return Err(PropertyError::RequiredButEmpty {
                        name,
                        message: value.into_owned(),
                    });
                }
            }
            continue;
        }

        let frame = frames.last_mut().expect("interpolation stack is non-empty");
        work.charge(1)?;
        let bytes = frame.input.as_bytes();
        let i = frame.cursor;
        if bytes[i] != b'$' {
            frame.cursor += 1;
            continue;
        }
        let next = i + 1;
        if next >= bytes.len() {
            frame.cursor += 1;
            continue;
        }
        work.charge(1)?;

        if bytes[next] == b'$' {
            frame.append_replacement(i, i + 2, "$");
            continue;
        }

        if bytes[next] == b'{' {
            let frame_input = frame.input;
            let Some((brace, end)) = parse_braced_reference(frame_input, i, &mut work)? else {
                frames
                    .last_mut()
                    .expect("interpolation stack is non-empty")
                    .cursor += 1;
                continue;
            };

            match resolve_brace_action(brace, vars) {
                BraceAction::Append(value) => frames
                    .last_mut()
                    .expect("interpolation stack is non-empty")
                    .append_replacement(i, end, value),
                BraceAction::Error(error) => return Err(error),
                BraceAction::Interpolate { text, completion } => {
                    // Preserve lazy operators: only selected text is inspected or depth-limited.
                    // The scan itself is charged because repeating it at each selected level is
                    // the source of the historical quadratic behavior.
                    work.charge(text.len())?;
                    if !text.contains("${") {
                        match completion {
                            FrameCompletion::Append => frames
                                .last_mut()
                                .expect("interpolation stack is non-empty")
                                .append_replacement(i, end, text),
                            FrameCompletion::RequiredButUnset(name) => {
                                return Err(PropertyError::RequiredButUnset {
                                    name,
                                    message: text.to_owned(),
                                });
                            }
                            FrameCompletion::RequiredButEmpty(name) => {
                                return Err(PropertyError::RequiredButEmpty {
                                    name,
                                    message: text.to_owned(),
                                });
                            }
                            FrameCompletion::Root => {
                                unreachable!("operator text cannot complete the root frame")
                            }
                        }
                        continue;
                    }

                    let depth = frames.len();
                    if depth > max_nested_expansions {
                        return Err(PropertyError::ExpansionDepthLimitExceeded {
                            depth,
                            max_depth: max_nested_expansions,
                        });
                    }
                    frames
                        .last_mut()
                        .expect("interpolation stack is non-empty")
                        .begin_replacement(i, end);
                    frames.push(ExpansionFrame::new(
                        text,
                        PropertySyntax::Braced,
                        completion,
                    ));
                }
            }
        } else if frame.syntax == PropertySyntax::Braced {
            frame.cursor += 1;
            continue;
        } else {
            let body = &frame.input[next..];
            let Some((name, _rest)) = parse_name(body, &mut work)? else {
                frame.cursor += 1;
                continue;
            };
            let value = vars
                .get(name)
                .map(String::as_str)
                .ok_or_else(|| PropertyError::Unresolved(name.to_owned()))?;
            frame.append_replacement(i, next + name.len(), value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PropertyError, PropertySyntax, interpolate_compose_style,
        interpolate_compose_style_with_limits,
    };
    use crate::Budget;
    use rstest::rstest;
    use std::borrow::Cow;
    use std::cell::Cell;
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

    #[rstest]
    #[case::outer_set_skips_nested_default("${SET:-${MISSING}}", "value")]
    #[case::default_if_unset("${MISSING-${SET}}", "value")]
    #[case::outer_missing_resolves_nested_default("${MISSING:-${SET}}", "value")]
    #[case::outer_empty_resolves_nested_default("${EMPTY:-${SET}}", "value")]
    #[case::alternate_if_set("${EMPTY+${SET}}", "value")]
    #[case::alternate_if_set_and_nonempty("${SET:+${SET}}", "value")]
    #[case::multiple_levels("${MISSING:-${ALSO_MISSING:-${SET}}}", "value")]
    #[case::with_prefix_and_suffix("prefix-${MISSING:-${SET}}-suffix", "prefix-value-suffix")]
    fn nested_braced_references_in_operator_text_resolve(
        #[case] input: &str,
        #[case] expected: &str,
    ) {
        let output =
            interpolate_compose_style(Cow::Borrowed(input), &vars(), PropertySyntax::Braced)
                .unwrap();
        assert_eq!(output.as_ref(), expected);
    }

    #[test]
    fn nested_braced_reference_can_be_escaped_in_operator_text() {
        let output = interpolate_compose_style(
            Cow::Borrowed("${MISSING:-$${SET}}"),
            &vars(),
            PropertySyntax::Braced,
        )
        .unwrap();

        assert_eq!(output.as_ref(), "${SET}");
    }

    #[test]
    fn nested_braced_reference_in_error_message_resolves_before_error() {
        let error = interpolate_compose_style(
            Cow::Borrowed("${MISSING?${SET}}"),
            &vars(),
            PropertySyntax::Braced,
        )
        .unwrap_err();

        assert_eq!(
            error,
            PropertyError::RequiredButUnset {
                name: "MISSING".into(),
                message: "value".into()
            }
        );
    }

    #[test]
    fn nested_braced_reference_in_empty_error_message_resolves_before_error() {
        let error = interpolate_compose_style(
            Cow::Borrowed("${EMPTY:?${SET}}"),
            &vars(),
            PropertySyntax::Braced,
        )
        .unwrap_err();

        assert_eq!(
            error,
            PropertyError::RequiredButEmpty {
                name: "EMPTY".into(),
                message: "value".into()
            }
        );
    }

    #[test]
    fn deeply_nested_defaults_are_rejected() {
        let depth = 10_000;
        let input = format!("{}value{}", "${MISSING:-".repeat(depth), "}".repeat(depth));

        let result =
            interpolate_compose_style(Cow::Borrowed(&input), &vars(), PropertySyntax::Braced);

        assert_eq!(
            result.unwrap_err(),
            PropertyError::ExpansionDepthLimitExceeded {
                depth: 65,
                max_depth: 64,
            }
        );
    }

    #[test]
    fn nesting_at_configured_limit_is_preserved() {
        let input = "${MISSING:-${MISSING:-${MISSING:-value}}}";
        let budget = Budget::default();
        let total_work = Cell::new(0);

        let output = interpolate_compose_style_with_limits(
            Cow::Borrowed(input),
            &vars(),
            PropertySyntax::Braced,
            2,
            budget.max_total_property_interpolation_work,
            &total_work,
        )
        .unwrap();

        assert_eq!(output.as_ref(), "value");
    }

    #[test]
    fn nesting_above_configured_limit_is_rejected() {
        let input = "${MISSING:-${MISSING:-${MISSING:-${MISSING:-value}}}}";
        let budget = Budget::default();
        let total_work = Cell::new(0);

        let error = interpolate_compose_style_with_limits(
            Cow::Borrowed(input),
            &vars(),
            PropertySyntax::Braced,
            2,
            budget.max_total_property_interpolation_work,
            &total_work,
        )
        .unwrap_err();

        assert_eq!(
            error,
            PropertyError::ExpansionDepthLimitExceeded {
                depth: 3,
                max_depth: 2,
            }
        );
    }

    #[test]
    fn interpolation_work_is_cumulative_and_checked_at_the_boundary() {
        let input = Cow::Borrowed("${SET}");
        let budget = Budget::default();
        let total_work = Cell::new(0);

        for _ in 0..2 {
            let output = interpolate_compose_style_with_limits(
                input.clone(),
                &vars(),
                PropertySyntax::Braced,
                budget.max_property_expansion_depth,
                20,
                &total_work,
            )
            .unwrap();
            assert_eq!(output.as_ref(), "value");
        }

        let error = interpolate_compose_style_with_limits(
            input,
            &vars(),
            PropertySyntax::Braced,
            budget.max_property_expansion_depth,
            20,
            &total_work,
        )
        .unwrap_err();
        assert_eq!(
            error,
            PropertyError::ExpansionWorkLimitExceeded {
                work: 21,
                max_work: 20,
            }
        );
    }

    #[test]
    fn deeply_nested_unselected_default_remains_lazy() {
        let nested = format!("{}value{}", "${MISSING:-".repeat(1_000), "}".repeat(1_000));
        let input = format!("${{SET:-{nested}}}");

        let output =
            interpolate_compose_style(Cow::Borrowed(&input), &vars(), PropertySyntax::Braced)
                .unwrap();

        assert_eq!(output.as_ref(), "value");
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
        PropertyError::RequiredButUnset { name: "MISSING".into(), message: String::new() }
    )]
    #[case::error_if_unset_or_empty_missing(
        "${MISSING:?nope}",
        PropertyError::RequiredButUnset { name: "MISSING".into(), message: "nope".into() }
    )]
    #[case::error_if_unset_or_empty_missing_empty_msg(
        "${MISSING:?}",
        PropertyError::RequiredButUnset { name: "MISSING".into(), message: String::new() }
    )]
    #[case::error_if_unset_or_empty_empty(
        "${EMPTY:?nope}",
        PropertyError::RequiredButEmpty { name: "EMPTY".into(), message: "nope".into() }
    )]
    #[case::error_if_unset_or_empty_empty_empty_msg(
        "${EMPTY:?}",
        PropertyError::RequiredButEmpty { name: "EMPTY".into(), message: String::new() }
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
