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
    /// `message` may be empty; hints containing property values preserve their source text.
    RequiredButUnset { name: String, message: String },
    /// `${NAME:?text}` referenced a variable that was present but empty.
    /// `message` may be empty; hints containing property values preserve their source text.
    RequiredButEmpty { name: String, message: String },
    /// A selected operator branch exceeded the configured nesting limit.
    ExpansionDepthLimitExceeded { depth: usize, max_depth: usize },
    /// Property scanning exceeded the configured aggregate work limit.
    ExpansionWorkLimitExceeded { work: usize, max_work: usize },
}

/// Checks whether a character is valid as the first character of a variable name.
fn is_var_start(ch: char, syntax: PropertySyntax) -> bool {
    ch == '_'
        || ch.is_ascii_alphabetic()
        // Compose's case-insensitive [a-z] regex also includes these Unicode folds.
        || (syntax == PropertySyntax::DockerCompose && matches!(ch, '\u{212a}' | '\u{017f}'))
}

/// Checks whether a character is valid as a continuing character of a variable name.
fn is_var_continue(ch: char, syntax: PropertySyntax) -> bool {
    is_var_start(ch, syntax) || ch.is_ascii_digit()
}

/// Parses a valid variable name from the beginning of the input string.
/// Returns the parsed name and the remaining unparsed input.
fn parse_name<'a>(
    input: &'a str,
    syntax: PropertySyntax,
    work: &mut WorkBudget<'_>,
) -> Result<Option<(&'a str, &'a str)>, PropertyError> {
    let mut chars = input.char_indices();
    let Some((_, first)) = chars.next() else {
        return Ok(None);
    };
    work.charge(first.len_utf8())?;
    if !is_var_start(first, syntax) {
        return Ok(None);
    }

    let mut end = first.len_utf8();
    for (i, ch) in chars {
        work.charge(ch.len_utf8())?;
        if !is_var_continue(ch, syntax) {
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
    /// Errors when `VAR` is unset, except in Compose mode (empty string).
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

/// Returns `Err` when the `${...}` candidate is malformed, or `Ok(Some(...))` with
/// the parsed reference and the byte index just past the closing `}`. Unclosed
/// references return `Ok(None)` in the legacy modes and an error in Compose mode.
fn parse_braced_reference<'a>(
    input: &'a str,
    start: usize,
    syntax: PropertySyntax,
    work: &mut WorkBudget<'_>,
) -> Result<Option<(BraceRef<'a>, usize)>, PropertyError> {
    let body_start = start + 2;
    let Some(close) = find_braced_reference_close(input, body_start, syntax, work)? else {
        if syntax == PropertySyntax::DockerCompose {
            return Err(PropertyError::InvalidName(input[start..].to_owned()));
        }
        return Ok(None);
    };
    let body = &input[body_start..close];
    let Some((name, rest)) = parse_name(body, syntax, work)? else {
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
    syntax: PropertySyntax,
    work: &mut WorkBudget<'_>,
) -> Result<Option<usize>, PropertyError> {
    if syntax == PropertySyntax::DockerCompose {
        return find_compose_braced_reference_close(input, body_start, work);
    }
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

/// Reproduce compose-go's candidate matching and brace-prefix trimming without a regex.
/// See https://github.com/compose-spec/compose-go/blob/main/template/template.go:
/// `DefaultPattern` greedily matches operator text through the last `}` on its line,
/// then `getFirstBraceClosingIndex` trims that match when it finds a balanced prefix.
/// Return balanced prefixes immediately, retaining the last `}` only as a fallback.
fn find_compose_braced_reference_close(
    input: &str,
    body_start: usize,
    work: &mut WorkBudget<'_>,
) -> Result<Option<usize>, PropertyError> {
    let Some((name, rest)) = parse_name(&input[body_start..], PropertySyntax::DockerCompose, work)?
    else {
        return Ok(None);
    };
    let operator_start = body_start + name.len();
    if rest.starts_with('}') {
        return Ok(Some(operator_start));
    }
    let operator_len = if rest.starts_with(":-") || rest.starts_with(":+") || rest.starts_with(":?")
    {
        2
    } else if rest.starts_with(['-', '+', '?']) {
        1
    } else {
        return Ok(None);
    };

    // The validated name and operator contain no braces; only the outer `{` is open.
    let mut depth = 1usize;
    let mut last_close = None;
    let mut skip_next = false;
    let text_start = operator_start + operator_len;
    for (offset, &byte) in input.as_bytes()[text_start..].iter().enumerate() {
        work.charge(1)?;
        if byte == b'\n' {
            break;
        }
        let cursor = text_start + offset;
        if byte == b'}' {
            last_close = Some(cursor);
        }
        if skip_next {
            skip_next = false;
            continue;
        }
        match byte {
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(Some(cursor));
                }
            }
            b'{' => {
                depth = depth.saturating_add(1);
                // compose-go skips the byte after every opening brace. Preserve this
                // detail: adjacent literal braces can change where the prefix ends.
                // Skipped bytes still count toward the last-`}` fallback and newline boundary.
                skip_next = true;
            }
            _ => {}
        }
    }
    // Compose accepts an unmatched literal opening brace when its original greedy
    // candidate still ends in `}`; only the selected operator text is expanded.
    Ok(last_close)
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
    /// A non-empty property-map value contributed to this frame's output.
    contains_property_value: bool,
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
            contains_property_value: false,
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

    fn into_error_message(self, source: &str, contains_property_value: bool) -> String {
        if contains_property_value {
            // Required hints are public diagnostics. Keep their source text rather than
            // copying property values into errors, snippets, or alias-error messages.
            return source.to_owned();
        }
        match self {
            Self::Borrowed(value) => value.to_owned(),
            Self::Owned(value) => value,
        }
    }
}

/// Action produced by resolving one braced property reference against the property map.
enum BraceAction<'a> {
    /// Append a literal replacement directly.
    Append(&'a str),
    /// Append a final property-map value, tracking its provenance for error hints.
    AppendProperty(&'a str),
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
    syntax: PropertySyntax,
) -> BraceAction<'a> {
    let name = brace.name;
    let value = vars.get(name).map(String::as_str);
    match (brace.op, value) {
        (BraceOp::Required, Some(value)) => BraceAction::AppendProperty(value),
        (BraceOp::Required, None) if syntax == PropertySyntax::DockerCompose => {
            BraceAction::Append("")
        }
        (BraceOp::Required, None) => BraceAction::Error(PropertyError::Unresolved(name.to_owned())),
        (BraceOp::DefaultIfUnset(text), None)
        | (BraceOp::DefaultIfUnsetOrEmpty(text), None | Some("")) => BraceAction::Interpolate {
            text,
            completion: FrameCompletion::Append,
        },
        (BraceOp::DefaultIfUnset(_), Some(value))
        | (BraceOp::DefaultIfUnsetOrEmpty(_), Some(value)) => BraceAction::AppendProperty(value),
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
        (BraceOp::ErrorIfUnset(_), Some(value)) => BraceAction::AppendProperty(value),
        (BraceOp::ErrorIfUnset(message), None) => BraceAction::Interpolate {
            text: message,
            completion: FrameCompletion::RequiredButUnset(name.to_owned()),
        },
        (BraceOp::ErrorIfUnsetOrEmpty(_), Some(value)) if !value.is_empty() => {
            BraceAction::AppendProperty(value)
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
/// (which uses Required semantics), or [`PropertySyntax::DockerCompose`] for
/// Compose-compatible missing references and nested expansion.
///
/// Values in `vars` are taken as final.
/// Placeholders inside map entries are not re-expanded. Braced placeholders inside
/// default, alternate, and error text from the input are expanded recursively.
/// Compose mode also expands bare references and dollar escapes in selected text.
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
                FrameCompletion::Append => {
                    let parent = frames
                        .last_mut()
                        .expect("nested interpolation has a parent");
                    parent.out.push_str(value.as_str());
                    parent.contains_property_value |= frame.contains_property_value;
                }
                FrameCompletion::RequiredButUnset(name) => {
                    return Err(PropertyError::RequiredButUnset {
                        name,
                        message: value
                            .into_error_message(frame.input, frame.contains_property_value),
                    });
                }
                FrameCompletion::RequiredButEmpty(name) => {
                    return Err(PropertyError::RequiredButEmpty {
                        name,
                        message: value
                            .into_error_message(frame.input, frame.contains_property_value),
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
            let syntax = frame.syntax;
            let Some((brace, end)) = parse_braced_reference(frame_input, i, syntax, &mut work)?
            else {
                frames
                    .last_mut()
                    .expect("interpolation stack is non-empty")
                    .cursor += 1;
                continue;
            };

            let action = resolve_brace_action(brace, vars, syntax);
            let contains_property_value =
                matches!(&action, BraceAction::AppendProperty(value) if !value.is_empty());
            match action {
                BraceAction::Append(value) | BraceAction::AppendProperty(value) => {
                    let frame = frames.last_mut().expect("interpolation stack is non-empty");
                    frame.append_replacement(i, end, value);
                    frame.contains_property_value |= contains_property_value;
                }
                BraceAction::Error(error) => return Err(error),
                BraceAction::Interpolate { text, completion } => {
                    // Preserve lazy operators: only selected text is inspected or depth-limited.
                    // The scan itself is charged because repeating it at each selected level is
                    // the source of the historical quadratic behavior.
                    work.charge(text.len())?;
                    let nested_syntax = if syntax == PropertySyntax::DockerCompose {
                        PropertySyntax::DockerCompose
                    } else {
                        PropertySyntax::Braced
                    };
                    let needs_expansion = if syntax == PropertySyntax::DockerCompose {
                        text.match_indices('$').any(|(index, _)| {
                            text[index + 1..].chars().next().is_some_and(|next| {
                                next == '$' || next == '{' || is_var_start(next, syntax)
                            })
                        })
                    } else {
                        text.contains("${")
                    };
                    if !needs_expansion {
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
                    frames.push(ExpansionFrame::new(text, nested_syntax, completion));
                }
            }
        } else if frame.syntax == PropertySyntax::Braced {
            frame.cursor += 1;
            continue;
        } else {
            let body = &frame.input[next..];
            let Some((name, _rest)) = parse_name(body, frame.syntax, &mut work)? else {
                frame.cursor += 1;
                continue;
            };
            let value = match vars.get(name) {
                Some(value) => value.as_str(),
                None if frame.syntax == PropertySyntax::DockerCompose => "",
                None => return Err(PropertyError::Unresolved(name.to_owned())),
            };
            frame.append_replacement(i, next + name.len(), value);
            frame.contains_property_value |= !value.is_empty();
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
    fn brace_op_resolves(
        #[case] input: &str,
        #[case] expected: &str,
        #[values(PropertySyntax::Braced, PropertySyntax::DockerCompose)] syntax: PropertySyntax,
    ) {
        let output = interpolate_compose_style(Cow::Borrowed(input), &vars(), syntax).unwrap();
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
                message: "${SET}".into()
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
                message: "${SET}".into()
            }
        );
    }

    #[rstest]
    fn deeply_nested_defaults_are_rejected(
        #[values(PropertySyntax::Braced, PropertySyntax::DockerCompose)] syntax: PropertySyntax,
    ) {
        let depth = 10_000;
        let input = format!("{}value{}", "${MISSING:-".repeat(depth), "}".repeat(depth));

        let result = interpolate_compose_style(Cow::Borrowed(&input), &vars(), syntax);

        assert_eq!(
            result.unwrap_err(),
            PropertyError::ExpansionDepthLimitExceeded {
                depth: 65,
                max_depth: 64,
            }
        );
    }

    #[rstest]
    fn nesting_at_configured_limit_is_preserved(
        #[values(PropertySyntax::Braced, PropertySyntax::DockerCompose)] syntax: PropertySyntax,
    ) {
        let input = "${MISSING:-${MISSING:-${MISSING:-value}}}";
        let budget = Budget::default();
        let total_work = Cell::new(0);

        let output = interpolate_compose_style_with_limits(
            Cow::Borrowed(input),
            &vars(),
            syntax,
            2,
            budget.max_total_property_interpolation_work,
            &total_work,
        )
        .unwrap();

        assert_eq!(output.as_ref(), "value");
    }

    #[rstest]
    fn nesting_above_configured_limit_is_rejected(
        #[values(PropertySyntax::Braced, PropertySyntax::DockerCompose)] syntax: PropertySyntax,
    ) {
        let input = "${MISSING:-${MISSING:-${MISSING:-${MISSING:-value}}}}";
        let budget = Budget::default();
        let total_work = Cell::new(0);

        let error = interpolate_compose_style_with_limits(
            Cow::Borrowed(input),
            &vars(),
            syntax,
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

    #[rstest]
    fn interpolation_work_is_cumulative_and_checked_at_the_boundary(
        #[values(PropertySyntax::Braced, PropertySyntax::DockerCompose)] syntax: PropertySyntax,
    ) {
        let input = Cow::Borrowed("${SET}");
        let budget = Budget::default();
        let total_work = Cell::new(0);

        for _ in 0..2 {
            let output = interpolate_compose_style_with_limits(
                input.clone(),
                &vars(),
                syntax,
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
            syntax,
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

    #[rstest]
    fn deeply_nested_unselected_default_remains_lazy(
        #[values(PropertySyntax::Braced, PropertySyntax::DockerCompose)] syntax: PropertySyntax,
    ) {
        let nested = format!("{}value{}", "${MISSING:-".repeat(1_000), "}".repeat(1_000));
        let input = format!("${{SET:-{nested}}}");

        let output = interpolate_compose_style(Cow::Borrowed(&input), &vars(), syntax).unwrap();

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

    #[rstest]
    #[case::bare("$SET", "value")]
    #[case::missing_bare("before $MISSING after", "before  after")]
    #[case::missing_braced("before ${MISSING} after", "before  after")]
    #[case::nested_bare_default("${MISSING:-$SET}", "value")]
    #[case::nested_bare_hard_default("${MISSING-$SET}", "value")]
    #[case::nested_bare_replacement("${SET:+$SET}", "value")]
    #[case::nested_bare_set_replacement("${EMPTY+$SET}", "value")]
    #[case::nested_escaped_dollar("${MISSING:-$$SET}", "$SET")]
    #[case::nested_escaped_braces("${MISSING:-$${SET}}", "${SET}")]
    #[case::nested_escaped_replacement("${SET:+$$SET}", "$SET")]
    #[case::mixed_nested(
        "${MISSING:-$SET ${EMPTY:+unused} ${EMPTY:-$SET} $$SET}",
        "value  value $SET"
    )]
    #[case::nested_missing("${MISSING:-$ALSO_MISSING}", "")]
    #[case::skip_default("${SET:-$MISSING}", "value")]
    #[case::skip_replacement("${EMPTY:+${MISSING:?error}}", "")]
    #[case::skip_required_message("${SET?${MISSING:?error}}", "value")]
    #[case::skip_invalid_default("${SET:-${INVALID!}}", "value")]
    #[case::literal_braces_default("${MISSING:-{json}}", "{json}")]
    #[case::literal_braces_unused("${SET:-{json}}", "value")]
    #[case::multiple_literal_braces("${MISSING:-{{a}{b}}}", "{{a}{b}}")]
    #[case::unbalanced_literal_default("${MISSING:-{json}", "{json")]
    #[case::unbalanced_literal_unused("${SET:-{json}", "value")]
    #[case::adjacent_literal_unused("${SET:-{{}}}", "value}")]
    #[case::unclosed_nested_unused("${SET:-${}", "value")]
    #[case::newline_after_reference("${MISSING:-default}\n$SET", "default\nvalue")]
    #[case::carriage_return_default("${MISSING:-a\rb}", "a\rb")]
    #[case::literal_braces_replacement("${SET:+{json}}", "{json}")]
    #[case::literal_braces_and_expansion("${MISSING:-{${SET}}}", "{value}")]
    #[case::trailing_text("${SET:-{json}}-${MISSING:-$SET}", "value-value")]
    #[case::all_escapes("$$SET $${SET} $$$SET $$$$", "$SET ${SET} $value $$")]
    #[case::escaped_unclosed("$${", "${")]
    #[case::unicode_prefix("h\u{e9} ${SET}", "h\u{e9} value")]
    #[case::greedy_bare("$SETfoo/$SET-tail", "/value-tail")]
    #[case::literal_dollars("$ $1 $/ $\u{03a9} $}", "$ $1 $/ $\u{03a9} $}")]
    fn docker_compose_interpolation(#[case] input: &str, #[case] expected: &str) {
        let output =
            interpolate_compose_style(Cow::Borrowed(input), &vars(), PropertySyntax::DockerCompose)
                .unwrap();
        assert_eq!(output.as_ref(), expected);
    }

    #[rstest]
    #[case("${")]
    #[case("${SET")]
    #[case("${}")]
    #[case("${ }")]
    #[case("${ SET}")]
    #[case("${SET }")]
    #[case("${SET!}")]
    #[case("${1SET}")]
    #[case("${SET:=fallback}")]
    #[case("${SET/foo/bar}")]
    #[case("${MISSING:-a\nb}")]
    #[case("${SET:-a\nb}")]
    #[case("${SET:+a\nb}")]
    #[case("${MISSING?error\nmessage}")]
    #[case("${MISSING:-${INVALID!}}")]
    fn docker_compose_rejects_invalid_references(#[case] input: &str) {
        let error =
            interpolate_compose_style(Cow::Borrowed(input), &vars(), PropertySyntax::DockerCompose)
                .unwrap_err();
        assert!(matches!(error, PropertyError::InvalidName(_)), "{error:?}");
    }

    #[rstest]
    #[case::missing("${MISSING?$SET $$SET}", PropertyError::RequiredButUnset {
        name: "MISSING".into(), message: "$SET $$SET".into()
    })]
    #[case::empty("${EMPTY:?$SET $$SET}", PropertyError::RequiredButEmpty {
        name: "EMPTY".into(), message: "$SET $$SET".into()
    })]
    #[case::empty_message("${MISSING?}", PropertyError::RequiredButUnset {
        name: "MISSING".into(), message: String::new()
    })]
    #[case::nested_message("${MISSING:?${EMPTY:-$SET}}", PropertyError::RequiredButUnset {
        name: "MISSING".into(), message: "${EMPTY:-$SET}".into()
    })]
    fn docker_compose_expands_required_messages(
        #[case] input: &str,
        #[case] expected: PropertyError,
    ) {
        let error =
            interpolate_compose_style(Cow::Borrowed(input), &vars(), PropertySyntax::DockerCompose)
                .unwrap_err();
        assert_eq!(error, expected);
    }

    #[test]
    fn docker_compose_expands_missing_references_and_skips_unselected_branches() {
        let output = interpolate_compose_style(
            Cow::Borrowed("${MISSING} $BARE ${EMPTY} ${MISSING:-$NESTED} ${SET:-${SKIPPED?required}} ${MISSING:+${SKIPPED?required}} ${SET?${SKIPPED?required}}"),
            &vars(),
            PropertySyntax::DockerCompose,
        )
        .unwrap();

        assert_eq!(output.as_ref(), "    value  value");
    }

    #[test]
    fn docker_compose_expands_missing_variables_in_required_messages() {
        let error = interpolate_compose_style(
            Cow::Borrowed("${MISSING:?message $NESTED}"),
            &vars(),
            PropertySyntax::DockerCompose,
        )
        .unwrap_err();

        assert_eq!(
            error,
            PropertyError::RequiredButUnset {
                name: "MISSING".into(),
                message: "message ".into()
            }
        );
    }

    #[test]
    fn docker_compose_property_map_values_are_final_and_case_sensitive() {
        let vars = HashMap::from([
            ("SET".into(), "${INVALID!} $$SET $set".into()),
            ("set".into(), "lowercase".into()),
            ("_1".into(), "underscore".into()),
        ]);
        let output = interpolate_compose_style(
            Cow::Borrowed("$SET ${SET} $set ${_1} ${MISSING:-$SET}"),
            &vars,
            PropertySyntax::DockerCompose,
        )
        .unwrap();

        assert_eq!(
            output.as_ref(),
            "${INVALID!} $$SET $set ${INVALID!} $$SET $set lowercase underscore ${INVALID!} $$SET $set"
        );
    }
}
