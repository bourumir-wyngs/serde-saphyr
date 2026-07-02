use crate::Location;
use crate::de_error::{Error, MessageFormatter, UserMessageFormatter};
use crate::localizer::{ExternalMessage, Localizer};

use std::borrow::Cow;

#[cfg(any(feature = "garde", feature = "validator"))]
use crate::{
    Locations,
    de_error::ValidationIssue,
    localizer::ExternalMessageSource,
    path_map::{PathMap, format_path_with_resolved_leaf},
};

/// Default developer-oriented message formatter.
///
/// This formatter at places produces recommendations on how to adjust settings and API
/// calls for the parsing to work, so normally should not be user-facing. Use UserMessageFormatter
/// for user-facing content, or implement custom MessageFormatter for full control over output.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultMessageFormatter;

/// Alias for the default developer-oriented formatter.
pub type DeveloperMessageFormatter = DefaultMessageFormatter;

#[cfg(any(feature = "garde", feature = "validator"))]
fn format_validation_issues(
    l10n: &dyn Localizer,
    source: ExternalMessageSource,
    issues: &[ValidationIssue],
    locations: &PathMap,
) -> String {
    let mut lines = Vec::with_capacity(issues.len());
    for issue in issues {
        let entry = issue.display_entry_overridden(l10n, source);
        let path_key = &issue.path;
        let original_leaf = path_key
            .leaf_string()
            .unwrap_or_else(|| l10n.root_path_label().into_owned());

        let (locs, resolved_leaf) = locations
            .search(path_key)
            .unwrap_or((Locations::UNKNOWN, original_leaf));

        let loc = if locs.reference_location != Location::UNKNOWN {
            locs.reference_location
        } else {
            locs.defined_location
        };

        let resolved_path = format_path_with_resolved_leaf(path_key, &resolved_leaf);

        lines.push(l10n.validation_issue_line(
            &resolved_path,
            &entry,
            (loc != Location::UNKNOWN).then_some(loc),
        ));
    }
    l10n.join_validation_issues(&lines)
}

fn default_format_message<'a>(formatter: &dyn MessageFormatter, err: &'a Error) -> Cow<'a, str> {
    match err {
        Error::WithSnippet { error, .. } => default_format_message(formatter, error),
        Error::ExternalMessage {
            source,
            msg,
            code,
            params,
            ..
        } => {
            let l10n = formatter.localizer();
            l10n.override_external_message(ExternalMessage {
                source: *source,
                original: msg.as_str(),
                code: code.as_deref(),
                params,
            })
            .unwrap_or(Cow::Borrowed(msg.as_str()))
        }
        Error::Message { msg, .. }
        | Error::HookError { msg, .. }
        | Error::SerdeVariantId { msg, .. } => Cow::Borrowed(msg.as_str()),
        Error::UnresolvedProperty { name, .. } => Cow::Owned(format!("missing property `{name}`")),
        Error::InvalidPropertyName { name, .. } => Cow::Owned(format!("Invalid name: '{name}'")),
        Error::PropertyRequiredButUnset { name, message, .. } if message.is_empty() => {
            Cow::Owned(format!("missing property `{name}`"))
        }
        Error::PropertyRequiredButUnset { name, message, .. } => {
            Cow::Owned(format!("missing property `{name}`: {message}"))
        }
        Error::PropertyRequiredButEmpty { name, message, .. } if message.is_empty() => {
            Cow::Owned(format!("empty property `{name}`"))
        }
        Error::PropertyRequiredButEmpty { name, message, .. } => {
            Cow::Owned(format!("empty property `{name}`: {message}"))
        }
        Error::Eof { .. } => Cow::Borrowed("unexpected end of input"),
        Error::MultipleDocuments { hint, .. } => {
            Cow::Owned(format!("multiple YAML documents detected; {hint}"))
        }
        Error::Unexpected { expected, .. } => {
            Cow::Owned(format!("unexpected event: expected {expected}"))
        }
        Error::MergeValueNotMapOrSeqOfMaps { .. } => {
            Cow::Borrowed("YAML merge value must be mapping or sequence of mappings")
        }
        Error::MergeKeyNotAllowed { .. } => {
            Cow::Borrowed("YAML merge keys are not allowed by configured policy")
        }
        Error::InvalidBinaryBase64 { .. } => Cow::Borrowed("invalid !!binary base64"),
        Error::InvalidUtf8Input => Cow::Borrowed("input is not valid UTF-8"),
        Error::BinaryNotUtf8 { .. } => Cow::Borrowed(
            "!!binary scalar is not valid UTF-8 so cannot be stored into string. \
                 If you just use !!binary for documentation/annotation, set ignore_binary_tag_for_string in Options",
        ),
        Error::TaggedScalarCannotDeserializeIntoString { .. } => {
            Cow::Borrowed("cannot deserialize tagged scalar into string")
        }
        Error::UnexpectedSequenceEnd { .. } => Cow::Borrowed("unexpected sequence end"),
        Error::UnexpectedMappingEnd { .. } => Cow::Borrowed("unexpected mapping end"),
        Error::InvalidBooleanStrict { .. } => {
            Cow::Borrowed("invalid boolean (strict mode expects true/false)")
        }
        Error::InvalidCharNull { .. } => {
            Cow::Borrowed("invalid char: cannot deserialize null; use Option<char>")
        }
        Error::InvalidCharNotSingleScalar { .. } => {
            Cow::Borrowed("invalid char: expected a single Unicode scalar value")
        }
        Error::NullIntoString { .. } => {
            Cow::Borrowed("cannot deserialize null into string; use Option<String>")
        }
        Error::BytesNotSupportedMissingBinaryTag { .. } => {
            Cow::Borrowed("bytes not supported (missing !!binary tag)")
        }
        Error::UnexpectedValueForUnit { .. } => Cow::Borrowed("unexpected value for unit"),
        Error::ExpectedEmptyMappingForUnitStruct { .. } => {
            Cow::Borrowed("expected empty mapping for unit struct")
        }
        Error::UnexpectedContainerEndWhileSkippingNode { .. } => {
            Cow::Borrowed("unexpected container end while skipping node")
        }
        Error::InternalSeedReusedForMapKey { .. } => {
            Cow::Borrowed("internal error: seed reused for map key")
        }
        Error::ValueRequestedBeforeKey { .. } => Cow::Borrowed("value requested before key"),
        Error::ExpectedStringKeyForExternallyTaggedEnum { .. } => {
            Cow::Borrowed("expected string key for externally tagged enum")
        }
        Error::ExternallyTaggedEnumExpectedScalarOrMapping { .. } => {
            Cow::Borrowed("externally tagged enum expected scalar or mapping")
        }
        Error::UnexpectedValueForUnitEnumVariant { .. } => {
            Cow::Borrowed("unexpected value for unit enum variant")
        }
        Error::AliasReplayCounterOverflow { .. } => Cow::Borrowed("alias replay counter overflow"),
        Error::AliasReplayLimitExceeded {
            total_replayed_events,
            max_total_replayed_events,
            ..
        } => Cow::Owned(format!(
            "alias replay limit exceeded: total_replayed_events={total_replayed_events} > {max_total_replayed_events}"
        )),
        Error::AliasExpansionLimitExceeded {
            anchor_id,
            expansions,
            max_expansions_per_anchor,
            ..
        } => Cow::Owned(format!(
            "alias expansion limit exceeded for anchor id {anchor_id}: {expansions} > {max_expansions_per_anchor}"
        )),
        Error::AliasReplayStackDepthExceeded {
            depth, max_depth, ..
        } => Cow::Owned(format!(
            "alias replay stack depth exceeded: depth={depth} > {max_depth}"
        )),
        Error::FoldedBlockScalarMustIndentContent { .. } => {
            Cow::Borrowed("folded block scalars must indent their content")
        }
        Error::InternalDepthUnderflow { .. } => Cow::Borrowed("internal depth underflow"),
        Error::InternalRecursionStackEmpty { .. } => {
            Cow::Borrowed("internal recursion stack empty")
        }
        Error::RecursiveReferencesRequireWeakTypes { .. } => {
            Cow::Borrowed("recursive references require weak recursion types")
        }
        Error::InvalidScalar { ty, .. } => Cow::Owned(format!("invalid {ty}")),
        Error::SerdeInvalidType {
            unexpected,
            expected,
            ..
        } => Cow::Owned(format!("invalid type: {unexpected}, expected {expected}")),
        Error::SerdeInvalidValue {
            unexpected,
            expected,
            ..
        } => Cow::Owned(format!("invalid value: {unexpected}, expected {expected}")),
        Error::SerdeUnknownVariant {
            variant, expected, ..
        } => Cow::Owned(format!(
            "unknown variant `{variant}`, expected one of {}",
            expected.join(", ")
        )),
        Error::SerdeUnknownField {
            field, expected, ..
        } => Cow::Owned(format!(
            "unknown field `{field}`, expected one of {}",
            expected.join(", ")
        )),
        Error::SerdeMissingField { field, .. } => Cow::Owned(format!("missing field `{field}`")),
        Error::UnexpectedContainerEndWhileReadingKeyNode { .. } => {
            Cow::Borrowed("unexpected container end while reading key")
        }
        Error::DuplicateMappingKey { key, .. } => match key {
            Some(k) => Cow::Owned(format!(
                "duplicate mapping key: {k}, set DuplicateKeyPolicy in Options if acceptable"
            )),
            None => Cow::Borrowed(
                "duplicate mapping key, set DuplicateKeyPolicy in Options if acceptable",
            ),
        },
        Error::TaggedEnumMismatch { tagged, target, .. } => Cow::Owned(format!(
            "tagged enum `{tagged}` does not match target enum `{target}`",
        )),
        Error::ExpectedMappingEndAfterEnumVariantValue { .. } => {
            Cow::Borrowed("expected end of mapping after enum variant value")
        }
        Error::ContainerEndMismatch { .. } => Cow::Borrowed("list or mapping end with no start"),
        Error::UnknownAnchor { .. } => Cow::Borrowed("alias references unknown anchor"),
        Error::CyclicInclude { id, stack, .. } => {
            let mut full_msg = format!("cyclic include detected: {id}");
            if !stack.is_empty() {
                full_msg.push_str("\nwhile processing include from ");
                full_msg.push_str(&stack.join(" -> "));
            }
            Cow::Owned(full_msg)
        }
        Error::UnsupportedIncludeForm { .. } => {
            Cow::Borrowed("!include currently only supports the scalar form: !include <path>")
        }
        Error::ResolverError {
            target,
            error,
            stack,
            ..
        } => {
            let mut full_msg = format!("failed to resolve include {target:?}");
            if !stack.is_empty() {
                full_msg.push_str("\nwhile processing include from ");
                full_msg.push_str(&stack.join(" -> "));
            }
            full_msg.push('\n');
            let msg = match error {
                crate::input_source::IncludeResolveError::Io(e) => e.to_string(),
                crate::input_source::IncludeResolveError::Message(m) => m.clone(),
                crate::input_source::IncludeResolveError::SizeLimitExceeded(size, limit) => {
                    format!("include size {size} bytes exceeds remaining size limit {limit} bytes")
                }
                crate::input_source::IncludeResolveError::FileInclude(problem) => {
                    match &**problem {
                        crate::input_source::ResolveProblem::ResolveFailed {
                            spec,
                            base_dir,
                            err,
                        } => {
                            format!(
                                "failed to resolve include '{}' from '{}': {}",
                                spec, base_dir, err
                            )
                        }
                        crate::input_source::ResolveProblem::TargetNotRegularFile { target } => {
                            format!("include target '{}' is not a regular file", target)
                        }
                        crate::input_source::ResolveProblem::TargetIsRootFile { spec } => {
                            format!(
                                "include target '{}' resolves to the configured root file itself",
                                spec
                            )
                        }
                        crate::input_source::ResolveProblem::ParentIdNotAbsoluteCanonical {
                            parent_id,
                        } => {
                            format!(
                                "SafeFileResolver expected parent include id to be an absolute canonical path, got '{}'",
                                parent_id
                            )
                        }
                        crate::input_source::ResolveProblem::ParentResolveFailed {
                            parent_id,
                            from_name,
                            err,
                        } => {
                            format!(
                                "failed to resolve parent include source '{}' (from '{}'): {}",
                                parent_id, from_name, err
                            )
                        }
                        crate::input_source::ResolveProblem::ParentNotRegularFile { parent } => {
                            format!("include parent '{}' is not a regular file", parent)
                        }
                        crate::input_source::ResolveProblem::ParentHasNoDirectory { parent } => {
                            format!(
                                "include parent '{}' does not have a parent directory",
                                parent
                            )
                        }
                        crate::input_source::ResolveProblem::ResolvesOutsideRoot { spec, root } => {
                            format!(
                                "include '{}' resolves outside the configured root '{}'",
                                spec, root
                            )
                        }
                        crate::input_source::ResolveProblem::TraversesSymlink { spec } => {
                            format!(
                                "include '{}' traverses a symlink, which is disabled by policy",
                                spec
                            )
                        }
                        crate::input_source::ResolveProblem::AbsolutePathNotAllowed { spec } => {
                            format!("absolute include paths are not allowed: {}", spec)
                        }
                        crate::input_source::ResolveProblem::EmptyPath => {
                            "include path must not be empty".to_string()
                        }
                        crate::input_source::ResolveProblem::InvalidExtension { spec } => {
                            format!(
                                "include target '{}' does not have a valid YAML extension (.yml or .yaml)",
                                spec
                            )
                        }
                        crate::input_source::ResolveProblem::HiddenFile { spec } => {
                            format!(
                                "include target '{}' is a hidden file, which is not allowed",
                                spec
                            )
                        }
                        crate::input_source::ResolveProblem::EmptyFragment => {
                            "include fragment must not be empty".to_string()
                        }
                        crate::input_source::ResolveProblem::FragmentContainsHash { spec } => {
                            format!("include fragment must not contain '#': {}", spec)
                        }
                    }
                }
            };
            full_msg.push_str(&msg);
            Cow::Owned(full_msg)
        }
        Error::Budget { breach, .. } => Cow::Owned(format!("budget breached: {breach:?}")),
        Error::QuotingRequired { value, .. } => {
            Cow::Owned(format!("The string value [{value}] must be quoted"))
        }
        Error::CannotBorrowTransformedString { reason, .. } => Cow::Owned(format!(
            "input does not contain value verbatim so cannot deserialize into &str ({reason}); use String or Cow<str> instead",
        )),
        Error::IndentationError {
            required, actual, ..
        } => Cow::Owned(format!(
            "indentation error: expected {required}, found {actual} spaces"
        )),
        Error::IOError { cause } => Cow::Owned(format!("IO error: {cause}")),
        Error::AliasError { msg, locations } => {
            let l10n = formatter.localizer();
            let ref_loc = locations.reference_location;
            let def_loc = locations.defined_location;
            match (ref_loc, def_loc) {
                (Location::UNKNOWN, Location::UNKNOWN) => Cow::Borrowed(msg.as_str()),
                (r, d) if r != Location::UNKNOWN && (d == Location::UNKNOWN || d == r) => {
                    Cow::Borrowed(msg.as_str())
                }
                (_r, d) => Cow::Owned(format!("{msg}{}", l10n.alias_defined_at(d))),
            }
        }

        #[cfg(feature = "garde")]
        Error::ValidationError { issues, locations } => {
            let l10n = formatter.localizer();
            Cow::Owned(format_validation_issues(
                l10n,
                ExternalMessageSource::Garde,
                issues,
                locations,
            ))
        }
        #[cfg(feature = "garde")]
        Error::ValidationErrors { errors } => Cow::Owned(format!(
            "validation failed for {} document(s)",
            errors.len()
        )),
        #[cfg(feature = "validator")]
        Error::ValidatorError { issues, locations } => {
            let l10n = formatter.localizer();
            Cow::Owned(format_validation_issues(
                l10n,
                ExternalMessageSource::Validator,
                issues,
                locations,
            ))
        }
        #[cfg(feature = "validator")]
        Error::ValidatorErrors { errors } => Cow::Owned(format!(
            "validation failed for {} document(s)",
            errors.len()
        )),
    }
}

impl MessageFormatter for DefaultMessageFormatter {
    fn format_message<'a>(&self, err: &'a Error) -> Cow<'a, str> {
        default_format_message(self, err)
    }
}

pub struct DefaultMessageFormatterWithLocalizer<'a> {
    localizer: &'a dyn Localizer,
}

impl MessageFormatter for DefaultMessageFormatterWithLocalizer<'_> {
    fn localizer(&self) -> &dyn Localizer {
        self.localizer
    }

    fn format_message<'a>(&self, err: &'a Error) -> Cow<'a, str> {
        default_format_message(self, err)
    }
}

impl DefaultMessageFormatter {
    /// Return a formatter that uses a custom [`Localizer`].
    ///
    /// This allows reusing the built-in developer-oriented messages while customizing
    /// wording that is produced outside `format_message` (location suffixes, validation
    /// issue composition, snippet labels, etc.).
    pub fn with_localizer<'a>(
        &self,
        localizer: &'a dyn Localizer,
    ) -> DefaultMessageFormatterWithLocalizer<'a> {
        DefaultMessageFormatterWithLocalizer { localizer }
    }
}

fn user_format_message<'a>(formatter: &dyn MessageFormatter, err: &'a Error) -> Cow<'a, str> {
    if let Error::WithSnippet { error, .. } = err {
        return user_format_message(formatter, error);
    }

    match err {
        // handled by early return above
        Error::WithSnippet { .. } => unreachable!(),

        Error::Eof { .. } => Cow::Borrowed("unexpected end of file"),
        Error::MultipleDocuments { .. } => {
            Cow::Borrowed("only single YAML document expected but multiple found")
        }
        Error::InvalidUtf8Input => Cow::Borrowed("YAML parser input is not valid UTF-8"),
        Error::BinaryNotUtf8 { .. } => {
            Cow::Borrowed("!!binary scalar is not valid UTF-8 so cannot be stored into string.")
        }
        Error::InvalidBooleanStrict { .. } => {
            Cow::Borrowed("invalid boolean (true or false expected)")
        }
        Error::NullIntoString { .. } | Error::InvalidCharNull { .. } => {
            Cow::Borrowed("null is not allowed here")
        }
        Error::InvalidCharNotSingleScalar { .. } => {
            Cow::Borrowed("only single character allowed here")
        }
        Error::BytesNotSupportedMissingBinaryTag { .. } => Cow::Borrowed("missing !!binary tag"),
        Error::ExpectedEmptyMappingForUnitStruct { .. } => {
            Cow::Borrowed("expected empty mapping here")
        }
        Error::UnexpectedContainerEndWhileSkippingNode { .. } => {
            Cow::Borrowed("unexpected container end")
        }
        Error::AliasReplayCounterOverflow { .. } => {
            Cow::Borrowed("YAML document too large or too complex")
        }
        Error::AliasReplayLimitExceeded {
            total_replayed_events,
            max_total_replayed_events,
            ..
        } => Cow::Owned(format!(
            "YAML document too large or too complex: total_replayed_events={total_replayed_events} > {max_total_replayed_events}"
        )),
        Error::AliasExpansionLimitExceeded {
            anchor_id,
            expansions,
            max_expansions_per_anchor,
            ..
        } => Cow::Owned(format!(
            "YAML document too large or too complex: anchor id {anchor_id}: {expansions} > {max_expansions_per_anchor}"
        )),
        Error::AliasReplayStackDepthExceeded {
            depth, max_depth, ..
        } => Cow::Owned(format!(
            "YAML document too large or too complex: depth={depth} > {max_depth}"
        )),
        Error::UnknownAnchor { .. } => Cow::Borrowed("reference to unknown value"),
        Error::MergeKeyNotAllowed { .. } => Cow::Borrowed("merge key not allowed here"),
        Error::CyclicInclude { .. } => Cow::Borrowed("cyclic include detected"),
        Error::UnsupportedIncludeForm { .. } => {
            Cow::Borrowed("!include currently only supports the scalar form: !include <path>")
        }
        Error::ResolverError { .. } => Cow::Borrowed("failed to resolve include"),
        Error::RecursiveReferencesRequireWeakTypes { .. } => {
            Cow::Borrowed("Recursive reference not allowed here")
        }
        Error::DuplicateMappingKey { key, .. } => match key {
            Some(k) => Cow::Owned(format!("duplicate mapping key: {k} not allowed here")),
            None => Cow::Borrowed("duplicate mapping key not allowed here"),
        },
        Error::QuotingRequired { .. } => Cow::Borrowed("value requires quoting"),
        Error::Budget { breach, .. } => Cow::Owned(format!(
            "YAML document too large or too complex: limits breached: {breach:?}"
        )),
        Error::CannotBorrowTransformedString { .. } => {
            Cow::Borrowed("Only single string with no escape sequences is allowed here")
        }
        Error::IndentationError {
            required, actual, ..
        } => Cow::Owned(format!(
            "incorrect indentation: expected {required}, found {actual} spaces"
        )),

        // All cases when the standard message is good enough.
        _ => default_format_message(formatter, err),
    }
}

impl MessageFormatter for UserMessageFormatter {
    fn format_message<'a>(&self, err: &'a Error) -> Cow<'a, str> {
        user_format_message(self, err)
    }
}

pub struct UserMessageFormatterWithLocalizer<'a> {
    localizer: &'a dyn Localizer,
}

impl MessageFormatter for UserMessageFormatterWithLocalizer<'_> {
    fn localizer(&self) -> &dyn Localizer {
        self.localizer
    }

    fn format_message<'a>(&self, err: &'a Error) -> Cow<'a, str> {
        user_format_message(self, err)
    }
}

impl UserMessageFormatter {
    /// Return a formatter that uses a custom [`Localizer`].
    ///
    /// This allows reusing the built-in user-facing messages while customizing wording
    /// that is produced outside `format_message` (location suffixes, validation issue
    /// composition, snippet labels, etc.).
    pub fn with_localizer<'a>(
        &self,
        localizer: &'a dyn Localizer,
    ) -> UserMessageFormatterWithLocalizer<'a> {
        UserMessageFormatterWithLocalizer { localizer }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Location;
    use crate::de_error::{Error, MessageFormatter, TransformReason};
    use crate::location::Locations;

    fn loc() -> Location {
        Location::UNKNOWN
    }

    // -----------------------------------------------------------------------
    // DefaultMessageFormatter – uncovered arms
    // -----------------------------------------------------------------------

    #[rstest::rstest]
    #[case::with_snippet_delegates(
        Error::WithSnippet {
            regions: vec![],
            crop_radius: 3,
            error: Box::new(Error::Eof { location: loc() }),
        },
        "unexpected end of input"
    )]
    #[case::hook_error(
        Error::HookError { msg: "hook msg".to_owned(), location: loc() },
        "hook msg"
    )]
    #[case::serde_variant_id(
        Error::SerdeVariantId { msg: "variant id msg".to_owned(), location: loc() },
        "variant id msg"
    )]
    #[case::invalid_binary_base64(
        Error::InvalidBinaryBase64 { location: loc() },
        "invalid !!binary base64"
    )]
    #[case::merge_key_not_allowed(
        Error::MergeKeyNotAllowed { location: loc() },
        "YAML merge keys are not allowed by configured policy"
    )]
    #[case::unexpected_sequence_end(
        Error::UnexpectedSequenceEnd { location: loc() },
        "unexpected sequence end"
    )]
    #[case::unexpected_mapping_end(
        Error::UnexpectedMappingEnd { location: loc() },
        "unexpected mapping end"
    )]
    #[case::unexpected_container_end_while_skipping(
        Error::UnexpectedContainerEndWhileSkippingNode { location: loc() },
        "unexpected container end while skipping node"
    )]
    #[case::internal_seed_reused(
        Error::InternalSeedReusedForMapKey { location: loc() },
        "internal error: seed reused for map key"
    )]
    #[case::value_requested_before_key(
        Error::ValueRequestedBeforeKey { location: loc() },
        "value requested before key"
    )]
    #[case::alias_replay_counter_overflow(
        Error::AliasReplayCounterOverflow { location: loc() },
        "alias replay counter overflow"
    )]
    #[case::folded_block_scalar(
        Error::FoldedBlockScalarMustIndentContent { location: loc() },
        "folded block scalars must indent their content"
    )]
    #[case::internal_depth_underflow(
        Error::InternalDepthUnderflow { location: loc() },
        "internal depth underflow"
    )]
    #[case::internal_recursion_stack_empty(
        Error::InternalRecursionStackEmpty { location: loc() },
        "internal recursion stack empty"
    )]
    #[case::recursive_references_require_weak_types(
        Error::RecursiveReferencesRequireWeakTypes { location: loc() },
        "recursive references require weak recursion types"
    )]
    #[case::unexpected_container_end_while_reading_key(
        Error::UnexpectedContainerEndWhileReadingKeyNode { location: loc() },
        "unexpected container end while reading key"
    )]
    #[case::expected_mapping_end_after_enum_variant(
        Error::ExpectedMappingEndAfterEnumVariantValue { location: loc() },
        "expected end of mapping after enum variant value"
    )]
    #[case::container_end_mismatch(
        Error::ContainerEndMismatch { location: loc() },
        "list or mapping end with no start"
    )]
    #[case::unresolved_property(
        Error::UnresolvedProperty { name: "MISSING".to_owned(), location: loc() },
        "missing property `MISSING`"
    )]
    #[case::invalid_property_name(
        Error::InvalidPropertyName { name: "${ab-cd}".to_owned(), location: loc() },
        "Invalid name: '${ab-cd}'"
    )]
    fn default_exact_messages(#[case] err: Error, #[case] expected: &str) {
        let formatter = DefaultMessageFormatter;
        assert_eq!(formatter.format_message(&err), expected);
    }

    #[rstest::rstest]
    #[case::serde_invalid_value(
        Error::SerdeInvalidValue {
            unexpected: "null".to_owned(),
            expected: "string".to_owned(),
            location: loc(),
        },
        &["invalid value", "null", "string"]
    )]
    #[case::serde_unknown_variant(
        Error::SerdeUnknownVariant {
            variant: "foo".to_owned(),
            expected: vec!["bar", "baz"],
            location: loc(),
        },
        &["unknown variant", "foo"]
    )]
    #[case::serde_unknown_field(
        Error::SerdeUnknownField {
            field: "xyz".to_owned(),
            expected: vec!["a", "b"],
            location: loc(),
        },
        &["unknown field", "xyz"]
    )]
    #[case::io_error(
        Error::IOError { cause: std::io::Error::other("disk full") },
        &["IO error", "disk full"]
    )]
    fn default_contains_messages(#[case] err: Error, #[case] needles: &[&str]) {
        let formatter = DefaultMessageFormatter;
        let msg = formatter.format_message(&err);
        for needle in needles {
            assert!(msg.contains(needle), "got: {msg}, missing: {needle}");
        }
    }

    #[rstest::rstest]
    #[case::unset_with_message(
        Error::PropertyRequiredButUnset {
            name: "DB_HOST".to_owned(),
            message: "set DB_HOST in .env".to_owned(),
            location: loc(),
        },
        "missing property `DB_HOST`: set DB_HOST in .env",
    )]
    #[case::unset_empty_message(
        Error::PropertyRequiredButUnset {
            name: "DB_HOST".to_owned(),
            message: String::new(),
            location: loc(),
        },
        "missing property `DB_HOST`",
    )]
    #[case::empty_with_message(
        Error::PropertyRequiredButEmpty {
            name: "DB_HOST".to_owned(),
            message: "must not be blank".to_owned(),
            location: loc(),
        },
        "empty property `DB_HOST`: must not be blank",
    )]
    #[case::empty_empty_message(
        Error::PropertyRequiredButEmpty {
            name: "DB_HOST".to_owned(),
            message: String::new(),
            location: loc(),
        },
        "empty property `DB_HOST`",
    )]
    fn default_property_required_messages(#[case] err: Error, #[case] expected: &str) {
        let formatter = DefaultMessageFormatter;
        assert_eq!(formatter.format_message(&err), expected);
    }

    #[test]
    fn default_alias_error_both_unknown() {
        let formatter = DefaultMessageFormatter;
        let err = Error::AliasError {
            msg: "alias msg".to_owned(),
            locations: Locations::UNKNOWN,
        };
        assert_eq!(formatter.format_message(&err), "alias msg");
    }

    #[test]
    fn default_alias_error_ref_known_def_unknown() {
        let formatter = DefaultMessageFormatter;
        let ref_loc = Location::new(1, 0);
        let err = Error::AliasError {
            msg: "alias msg".to_owned(),
            locations: Locations {
                reference_location: ref_loc,
                defined_location: Location::UNKNOWN,
            },
        };
        // r != UNKNOWN and d == UNKNOWN → returns msg as-is
        assert_eq!(formatter.format_message(&err), "alias msg");
    }

    #[test]
    fn default_alias_error_both_known_different() {
        let formatter = DefaultMessageFormatter;
        let ref_loc = Location::new(1, 0);
        let def_loc = Location::new(5, 0);
        let err = Error::AliasError {
            msg: "alias msg".to_owned(),
            locations: Locations {
                reference_location: ref_loc,
                defined_location: def_loc,
            },
        };
        // _r != UNKNOWN, d != UNKNOWN, d != r → appends defined-at suffix
        let msg = formatter.format_message(&err);
        assert!(msg.starts_with("alias msg"), "got: {msg}");
    }

    // -----------------------------------------------------------------------
    // UserMessageFormatter – all arms
    // -----------------------------------------------------------------------

    #[rstest::rstest]
    #[case::with_snippet_delegates(
        Error::WithSnippet {
            regions: vec![],
            crop_radius: 3,
            error: Box::new(Error::Eof { location: loc() }),
        },
        "unexpected end of file"
    )]
    #[case::eof(Error::Eof { location: loc() }, "unexpected end of file")]
    #[case::multiple_documents(
        Error::MultipleDocuments { hint: "use from_str_multidoc", location: loc() },
        "only single YAML document expected but multiple found"
    )]
    #[case::invalid_utf8_input(Error::InvalidUtf8Input, "YAML parser input is not valid UTF-8")]
    #[case::invalid_boolean_strict(
        Error::InvalidBooleanStrict { location: loc() },
        "invalid boolean (true or false expected)"
    )]
    #[case::null_into_string(
        Error::NullIntoString { location: loc() },
        "null is not allowed here"
    )]
    #[case::invalid_char_null(
        Error::InvalidCharNull { location: loc() },
        "null is not allowed here"
    )]
    #[case::invalid_char_not_single_scalar(
        Error::InvalidCharNotSingleScalar { location: loc() },
        "only single character allowed here"
    )]
    #[case::bytes_not_supported_missing_binary_tag(
        Error::BytesNotSupportedMissingBinaryTag { location: loc() },
        "missing !!binary tag"
    )]
    #[case::expected_empty_mapping_for_unit_struct(
        Error::ExpectedEmptyMappingForUnitStruct { location: loc() },
        "expected empty mapping here"
    )]
    #[case::unexpected_container_end_while_skipping(
        Error::UnexpectedContainerEndWhileSkippingNode { location: loc() },
        "unexpected container end"
    )]
    #[case::alias_replay_counter_overflow(
        Error::AliasReplayCounterOverflow { location: loc() },
        "YAML document too large or too complex"
    )]
    #[case::unknown_anchor(
        Error::UnknownAnchor { location: loc() },
        "reference to unknown value"
    )]
    #[case::merge_key_not_allowed(
        Error::MergeKeyNotAllowed { location: loc() },
        "merge key not allowed here"
    )]
    #[case::recursive_references_require_weak_types(
        Error::RecursiveReferencesRequireWeakTypes { location: loc() },
        "Recursive reference not allowed here"
    )]
    #[case::quoting_required(
        Error::QuotingRequired { value: "yes".to_owned(), location: loc() },
        "value requires quoting"
    )]
    #[case::cannot_borrow_transformed_string(
        Error::CannotBorrowTransformedString {
            reason: TransformReason::EscapeSequence,
            location: loc(),
        },
        "Only single string with no escape sequences is allowed here"
    )]
    fn user_exact_messages(#[case] err: Error, #[case] expected: &str) {
        let formatter = UserMessageFormatter;
        assert_eq!(formatter.format_message(&err), expected);
    }

    #[rstest::rstest]
    #[case::binary_not_utf8(Error::BinaryNotUtf8 { location: loc() }, &["!!binary"])]
    #[case::alias_replay_limit_exceeded(
        Error::AliasReplayLimitExceeded {
            total_replayed_events: 1000,
            max_total_replayed_events: 500,
            location: loc(),
        },
        &["too large or too complex", "1000"]
    )]
    #[case::alias_expansion_limit_exceeded(
        Error::AliasExpansionLimitExceeded {
            anchor_id: 7,
            expansions: 200,
            max_expansions_per_anchor: 100,
            location: loc(),
        },
        &["too large or too complex", "7"]
    )]
    #[case::alias_replay_stack_depth_exceeded(
        Error::AliasReplayStackDepthExceeded {
            depth: 50,
            max_depth: 20,
            location: loc(),
        },
        &["too large or too complex", "50"]
    )]
    #[case::duplicate_mapping_key_with_key(
        Error::DuplicateMappingKey { key: Some("mykey".to_owned()), location: loc() },
        &["mykey", "duplicate"]
    )]
    #[case::duplicate_mapping_key_without_key(
        Error::DuplicateMappingKey { key: None, location: loc() },
        &["duplicate"]
    )]
    #[case::budget(
        Error::Budget {
            breach: crate::budget::BudgetBreach::Events { events: 9999 },
            location: loc(),
        },
        &["too large or too complex"]
    )]
    #[case::falls_through_to_default_for_unhandled(
        Error::SerdeInvalidType {
            unexpected: "seq".to_owned(),
            expected: "map".to_owned(),
            location: loc(),
        },
        &["invalid type"]
    )]
    fn user_contains_messages(#[case] err: Error, #[case] needles: &[&str]) {
        let formatter = UserMessageFormatter;
        let msg = formatter.format_message(&err);
        for needle in needles {
            assert!(msg.contains(needle), "got: {msg}, missing: {needle}");
        }
    }

    // -----------------------------------------------------------------------
    // UserMessageFormatterWithLocalizer
    // -----------------------------------------------------------------------

    #[test]
    fn user_with_localizer_delegates() {
        use crate::localizer::DefaultEnglishLocalizer;
        let localizer = DefaultEnglishLocalizer;
        let formatter = UserMessageFormatter.with_localizer(&localizer);
        let err = Error::Eof { location: loc() };
        assert_eq!(formatter.format_message(&err), "unexpected end of file");
    }

    // -----------------------------------------------------------------------
    // DefaultMessageFormatterWithLocalizer
    // -----------------------------------------------------------------------

    #[test]
    fn default_with_localizer_delegates() {
        use crate::localizer::DefaultEnglishLocalizer;
        let localizer = DefaultEnglishLocalizer;
        let formatter = DefaultMessageFormatter.with_localizer(&localizer);
        let err = Error::Eof { location: loc() };
        assert_eq!(formatter.format_message(&err), "unexpected end of input");
    }
}
