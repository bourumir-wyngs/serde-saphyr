use crate::Location;
use crate::de_error::{Error, MessageFormatter, UserMessageFormatter};
use crate::localizer::{ExternalMessage, Localizer};

use std::borrow::Cow;

#[cfg(any(feature = "garde", feature = "validator"))]
use crate::localizer::ExternalMessageSource;

#[cfg(any(feature = "garde", feature = "validator"))]
use crate::path_map::format_path_with_resolved_leaf;

#[cfg(any(feature = "garde", feature = "validator"))]
use crate::Locations;

#[cfg(feature = "garde")]
use crate::de_error::collect_garde_issues;

#[cfg(feature = "validator")]
use crate::de_error::collect_validator_issues;

/// Default developer-oriented message formatter.
///
/// This formatter at places produces recommendations on how to adjust settings and API
/// calls for the parsing to work, so normally should not be user-facing. Use UserMessageFormatter
/// for user-facing content, or implement custom MessageFormatter for full control over output.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultMessageFormatter;

/// Alias for the default developer-oriented formatter.
pub type DeveloperMessageFormatter = DefaultMessageFormatter;

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
        Error::Budget { breach, .. } => Cow::Owned(format!("budget breached: {breach:?}")),
        Error::QuotingRequired { value, .. } => {
            Cow::Owned(format!("The string value [{value}] must be quoted"))
        }
        Error::CannotBorrowTransformedString { reason, .. } => Cow::Owned(format!(
            "input does not contain value verbatim so cannot deserialize into &str ({reason}); use String or Cow<str> instead",
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
        Error::ValidationError { report, locations } => {
            let l10n = formatter.localizer();

            let issues = collect_garde_issues(report);
            let mut lines = Vec::with_capacity(issues.len());
            for issue in issues {
                let entry = issue.display_entry_overridden(l10n, ExternalMessageSource::Garde);
                let path_key = issue.path;
                let original_leaf = path_key
                    .leaf_string()
                    .unwrap_or_else(|| l10n.root_path_label().into_owned());

                let (locs, resolved_leaf) = locations
                    .search(&path_key)
                    .unwrap_or((Locations::UNKNOWN, original_leaf));

                let loc = if locs.reference_location != Location::UNKNOWN {
                    locs.reference_location
                } else {
                    locs.defined_location
                };

                let resolved_path = format_path_with_resolved_leaf(&path_key, &resolved_leaf);

                lines.push(l10n.validation_issue_line(
                    &resolved_path,
                    &entry,
                    (loc != Location::UNKNOWN).then_some(loc),
                ));
            }
            Cow::Owned(l10n.join_validation_issues(&lines))
        }
        #[cfg(feature = "garde")]
        Error::ValidationErrors { errors } => Cow::Owned(format!(
            "validation failed for {} document(s)",
            errors.len()
        )),
        #[cfg(feature = "validator")]
        Error::ValidatorError { errors, locations } => {
            let l10n = formatter.localizer();

            let issues = collect_validator_issues(errors);
            let mut lines = Vec::with_capacity(issues.len());
            for issue in issues {
                let entry = issue.display_entry_overridden(l10n, ExternalMessageSource::Validator);
                let path_key = issue.path;
                let original_leaf = path_key
                    .leaf_string()
                    .unwrap_or_else(|| l10n.root_path_label().into_owned());

                let (locs, resolved_leaf) = locations
                    .search(&path_key)
                    .unwrap_or((Locations::UNKNOWN, original_leaf));

                let loc = if locs.reference_location != Location::UNKNOWN {
                    locs.reference_location
                } else {
                    locs.defined_location
                };

                let resolved_path = format_path_with_resolved_leaf(&path_key, &resolved_leaf);

                lines.push(l10n.validation_issue_line(
                    &resolved_path,
                    &entry,
                    (loc != Location::UNKNOWN).then_some(loc),
                ));
            }
            Cow::Owned(l10n.join_validation_issues(&lines))
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

    #[test]
    fn default_with_snippet_delegates() {
        let formatter = DefaultMessageFormatter;
        let inner = Error::Eof { location: loc() };
        let err = Error::WithSnippet {
            regions: vec![],
            crop_radius: 3,
            error: Box::new(inner),
        };
        assert_eq!(formatter.format_message(&err), "unexpected end of input");
    }

    #[test]
    fn default_hook_error() {
        let formatter = DefaultMessageFormatter;
        let err = Error::HookError {
            msg: "hook msg".to_owned(),
            location: loc(),
        };
        assert_eq!(formatter.format_message(&err), "hook msg");
    }

    #[test]
    fn default_serde_variant_id() {
        let formatter = DefaultMessageFormatter;
        let err = Error::SerdeVariantId {
            msg: "variant id msg".to_owned(),
            location: loc(),
        };
        assert_eq!(formatter.format_message(&err), "variant id msg");
    }

    #[test]
    fn default_invalid_binary_base64() {
        let formatter = DefaultMessageFormatter;
        let err = Error::InvalidBinaryBase64 { location: loc() };
        assert_eq!(formatter.format_message(&err), "invalid !!binary base64");
    }

    #[test]
    fn default_unexpected_sequence_end() {
        let formatter = DefaultMessageFormatter;
        let err = Error::UnexpectedSequenceEnd { location: loc() };
        assert_eq!(formatter.format_message(&err), "unexpected sequence end");
    }

    #[test]
    fn default_unexpected_mapping_end() {
        let formatter = DefaultMessageFormatter;
        let err = Error::UnexpectedMappingEnd { location: loc() };
        assert_eq!(formatter.format_message(&err), "unexpected mapping end");
    }

    #[test]
    fn default_unexpected_container_end_while_skipping() {
        let formatter = DefaultMessageFormatter;
        let err = Error::UnexpectedContainerEndWhileSkippingNode { location: loc() };
        assert_eq!(
            formatter.format_message(&err),
            "unexpected container end while skipping node"
        );
    }

    #[test]
    fn default_internal_seed_reused() {
        let formatter = DefaultMessageFormatter;
        let err = Error::InternalSeedReusedForMapKey { location: loc() };
        assert_eq!(
            formatter.format_message(&err),
            "internal error: seed reused for map key"
        );
    }

    #[test]
    fn default_value_requested_before_key() {
        let formatter = DefaultMessageFormatter;
        let err = Error::ValueRequestedBeforeKey { location: loc() };
        assert_eq!(formatter.format_message(&err), "value requested before key");
    }

    #[test]
    fn default_alias_replay_counter_overflow() {
        let formatter = DefaultMessageFormatter;
        let err = Error::AliasReplayCounterOverflow { location: loc() };
        assert_eq!(
            formatter.format_message(&err),
            "alias replay counter overflow"
        );
    }

    #[test]
    fn default_folded_block_scalar() {
        let formatter = DefaultMessageFormatter;
        let err = Error::FoldedBlockScalarMustIndentContent { location: loc() };
        assert_eq!(
            formatter.format_message(&err),
            "folded block scalars must indent their content"
        );
    }

    #[test]
    fn default_internal_depth_underflow() {
        let formatter = DefaultMessageFormatter;
        let err = Error::InternalDepthUnderflow { location: loc() };
        assert_eq!(formatter.format_message(&err), "internal depth underflow");
    }

    #[test]
    fn default_internal_recursion_stack_empty() {
        let formatter = DefaultMessageFormatter;
        let err = Error::InternalRecursionStackEmpty { location: loc() };
        assert_eq!(
            formatter.format_message(&err),
            "internal recursion stack empty"
        );
    }

    #[test]
    fn default_recursive_references_require_weak_types() {
        let formatter = DefaultMessageFormatter;
        let err = Error::RecursiveReferencesRequireWeakTypes { location: loc() };
        assert_eq!(
            formatter.format_message(&err),
            "recursive references require weak recursion types"
        );
    }

    #[test]
    fn default_serde_invalid_value() {
        let formatter = DefaultMessageFormatter;
        let err = Error::SerdeInvalidValue {
            unexpected: "null".to_owned(),
            expected: "string".to_owned(),
            location: loc(),
        };
        let msg = formatter.format_message(&err);
        assert!(msg.contains("invalid value"), "got: {msg}");
        assert!(msg.contains("null"), "got: {msg}");
        assert!(msg.contains("string"), "got: {msg}");
    }

    #[test]
    fn default_serde_unknown_variant() {
        let formatter = DefaultMessageFormatter;
        let err = Error::SerdeUnknownVariant {
            variant: "foo".to_owned(),
            expected: vec!["bar", "baz"],
            location: loc(),
        };
        let msg = formatter.format_message(&err);
        assert!(msg.contains("unknown variant"), "got: {msg}");
        assert!(msg.contains("foo"), "got: {msg}");
    }

    #[test]
    fn default_serde_unknown_field() {
        let formatter = DefaultMessageFormatter;
        let err = Error::SerdeUnknownField {
            field: "xyz".to_owned(),
            expected: vec!["a", "b"],
            location: loc(),
        };
        let msg = formatter.format_message(&err);
        assert!(msg.contains("unknown field"), "got: {msg}");
        assert!(msg.contains("xyz"), "got: {msg}");
    }

    #[test]
    fn default_unexpected_container_end_while_reading_key() {
        let formatter = DefaultMessageFormatter;
        let err = Error::UnexpectedContainerEndWhileReadingKeyNode { location: loc() };
        assert_eq!(
            formatter.format_message(&err),
            "unexpected container end while reading key"
        );
    }

    #[test]
    fn default_expected_mapping_end_after_enum_variant() {
        let formatter = DefaultMessageFormatter;
        let err = Error::ExpectedMappingEndAfterEnumVariantValue { location: loc() };
        assert_eq!(
            formatter.format_message(&err),
            "expected end of mapping after enum variant value"
        );
    }

    #[test]
    fn default_container_end_mismatch() {
        let formatter = DefaultMessageFormatter;
        let err = Error::ContainerEndMismatch { location: loc() };
        assert_eq!(
            formatter.format_message(&err),
            "list or mapping end with no start"
        );
    }

    #[test]
    fn default_io_error() {
        let formatter = DefaultMessageFormatter;
        let err = Error::IOError {
            cause: std::io::Error::other("disk full"),
        };
        let msg = formatter.format_message(&err);
        assert!(msg.contains("IO error"), "got: {msg}");
        assert!(msg.contains("disk full"), "got: {msg}");
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

    #[test]
    fn user_with_snippet_delegates() {
        let formatter = UserMessageFormatter;
        let inner = Error::Eof { location: loc() };
        let err = Error::WithSnippet {
            regions: vec![],
            crop_radius: 3,
            error: Box::new(inner),
        };
        assert_eq!(formatter.format_message(&err), "unexpected end of file");
    }

    #[test]
    fn user_eof() {
        let formatter = UserMessageFormatter;
        let err = Error::Eof { location: loc() };
        assert_eq!(formatter.format_message(&err), "unexpected end of file");
    }

    #[test]
    fn user_multiple_documents() {
        let formatter = UserMessageFormatter;
        let err = Error::MultipleDocuments {
            hint: "use from_str_multidoc",
            location: loc(),
        };
        assert_eq!(
            formatter.format_message(&err),
            "only single YAML document expected but multiple found"
        );
    }

    #[test]
    fn user_invalid_utf8_input() {
        let formatter = UserMessageFormatter;
        let err = Error::InvalidUtf8Input;
        assert_eq!(
            formatter.format_message(&err),
            "YAML parser input is not valid UTF-8"
        );
    }

    #[test]
    fn user_binary_not_utf8() {
        let formatter = UserMessageFormatter;
        let err = Error::BinaryNotUtf8 { location: loc() };
        assert!(formatter.format_message(&err).contains("!!binary"));
    }

    #[test]
    fn user_invalid_boolean_strict() {
        let formatter = UserMessageFormatter;
        let err = Error::InvalidBooleanStrict { location: loc() };
        assert_eq!(
            formatter.format_message(&err),
            "invalid boolean (true or false expected)"
        );
    }

    #[test]
    fn user_null_into_string() {
        let formatter = UserMessageFormatter;
        let err = Error::NullIntoString { location: loc() };
        assert_eq!(formatter.format_message(&err), "null is not allowed here");
    }

    #[test]
    fn user_invalid_char_null() {
        let formatter = UserMessageFormatter;
        let err = Error::InvalidCharNull { location: loc() };
        assert_eq!(formatter.format_message(&err), "null is not allowed here");
    }

    #[test]
    fn user_invalid_char_not_single_scalar() {
        let formatter = UserMessageFormatter;
        let err = Error::InvalidCharNotSingleScalar { location: loc() };
        assert_eq!(
            formatter.format_message(&err),
            "only single character allowed here"
        );
    }

    #[test]
    fn user_bytes_not_supported_missing_binary_tag() {
        let formatter = UserMessageFormatter;
        let err = Error::BytesNotSupportedMissingBinaryTag { location: loc() };
        assert_eq!(formatter.format_message(&err), "missing !!binary tag");
    }

    #[test]
    fn user_expected_empty_mapping_for_unit_struct() {
        let formatter = UserMessageFormatter;
        let err = Error::ExpectedEmptyMappingForUnitStruct { location: loc() };
        assert_eq!(
            formatter.format_message(&err),
            "expected empty mapping here"
        );
    }

    #[test]
    fn user_unexpected_container_end_while_skipping() {
        let formatter = UserMessageFormatter;
        let err = Error::UnexpectedContainerEndWhileSkippingNode { location: loc() };
        assert_eq!(formatter.format_message(&err), "unexpected container end");
    }

    #[test]
    fn user_alias_replay_counter_overflow() {
        let formatter = UserMessageFormatter;
        let err = Error::AliasReplayCounterOverflow { location: loc() };
        assert_eq!(
            formatter.format_message(&err),
            "YAML document too large or too complex"
        );
    }

    #[test]
    fn user_alias_replay_limit_exceeded() {
        let formatter = UserMessageFormatter;
        let err = Error::AliasReplayLimitExceeded {
            total_replayed_events: 1000,
            max_total_replayed_events: 500,
            location: loc(),
        };
        let msg = formatter.format_message(&err);
        assert!(msg.contains("too large or too complex"), "got: {msg}");
        assert!(msg.contains("1000"), "got: {msg}");
    }

    #[test]
    fn user_alias_expansion_limit_exceeded() {
        let formatter = UserMessageFormatter;
        let err = Error::AliasExpansionLimitExceeded {
            anchor_id: 7,
            expansions: 200,
            max_expansions_per_anchor: 100,
            location: loc(),
        };
        let msg = formatter.format_message(&err);
        assert!(msg.contains("too large or too complex"), "got: {msg}");
        assert!(msg.contains("7"), "got: {msg}");
    }

    #[test]
    fn user_alias_replay_stack_depth_exceeded() {
        let formatter = UserMessageFormatter;
        let err = Error::AliasReplayStackDepthExceeded {
            depth: 50,
            max_depth: 20,
            location: loc(),
        };
        let msg = formatter.format_message(&err);
        assert!(msg.contains("too large or too complex"), "got: {msg}");
        assert!(msg.contains("50"), "got: {msg}");
    }

    #[test]
    fn user_unknown_anchor() {
        let formatter = UserMessageFormatter;
        let err = Error::UnknownAnchor { location: loc() };
        assert_eq!(formatter.format_message(&err), "reference to unknown value");
    }

    #[test]
    fn user_recursive_references_require_weak_types() {
        let formatter = UserMessageFormatter;
        let err = Error::RecursiveReferencesRequireWeakTypes { location: loc() };
        assert_eq!(
            formatter.format_message(&err),
            "Recursive reference not allowed here"
        );
    }

    #[test]
    fn user_duplicate_mapping_key_with_key() {
        let formatter = UserMessageFormatter;
        let err = Error::DuplicateMappingKey {
            key: Some("mykey".to_owned()),
            location: loc(),
        };
        let msg = formatter.format_message(&err);
        assert!(msg.contains("mykey"), "got: {msg}");
        assert!(msg.contains("duplicate"), "got: {msg}");
    }

    #[test]
    fn user_duplicate_mapping_key_without_key() {
        let formatter = UserMessageFormatter;
        let err = Error::DuplicateMappingKey {
            key: None,
            location: loc(),
        };
        let msg = formatter.format_message(&err);
        assert!(msg.contains("duplicate"), "got: {msg}");
    }

    #[test]
    fn user_quoting_required() {
        let formatter = UserMessageFormatter;
        let err = Error::QuotingRequired {
            value: "yes".to_owned(),
            location: loc(),
        };
        assert_eq!(formatter.format_message(&err), "value requires quoting");
    }

    #[test]
    fn user_budget() {
        use crate::budget::BudgetBreach;
        let formatter = UserMessageFormatter;
        let err = Error::Budget {
            breach: BudgetBreach::Events { events: 9999 },
            location: loc(),
        };
        let msg = formatter.format_message(&err);
        assert!(msg.contains("too large or too complex"), "got: {msg}");
    }

    #[test]
    fn user_cannot_borrow_transformed_string() {
        let formatter = UserMessageFormatter;
        let err = Error::CannotBorrowTransformedString {
            reason: TransformReason::EscapeSequence,
            location: loc(),
        };
        assert_eq!(
            formatter.format_message(&err),
            "Only single string with no escape sequences is allowed here"
        );
    }

    #[test]
    fn user_falls_through_to_default_for_unhandled() {
        // SerdeInvalidType is not explicitly handled by user_format_message → falls through to default
        let formatter = UserMessageFormatter;
        let err = Error::SerdeInvalidType {
            unexpected: "seq".to_owned(),
            expected: "map".to_owned(),
            location: loc(),
        };
        let msg = formatter.format_message(&err);
        assert!(msg.contains("invalid type"), "got: {msg}");
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
