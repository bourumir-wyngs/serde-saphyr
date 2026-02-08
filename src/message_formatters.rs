use crate::de_error::{Error, MessageFormatter, UserMessageFormatter};
use crate::localizer::{ExternalMessage, Localizer};
use crate::Location;

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
            depth,
            max_depth,
            ..
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
            variant,
            expected,
            ..
        } => Cow::Owned(format!(
            "unknown variant `{variant}`, expected one of {}",
            expected.join(", ")
        )),
        Error::SerdeUnknownField {
            field,
            expected,
            ..
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
        Error::ValidationErrors { errors } => {
            Cow::Owned(format!("validation failed for {} document(s)", errors.len()))
        }
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
        Error::ValidatorErrors { errors } => {
            Cow::Owned(format!("validation failed for {} document(s)", errors.len()))
        }
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
        Error::InvalidBooleanStrict { .. } => Cow::Borrowed("invalid boolean (true or false expected)"),
        Error::NullIntoString { .. } | Error::InvalidCharNull { .. } => Cow::Borrowed("null is not allowed here"),
        Error::InvalidCharNotSingleScalar { .. } => Cow::Borrowed("only single character allowed here"),
        Error::BytesNotSupportedMissingBinaryTag { .. } => Cow::Borrowed("missing !!binary tag"),
        Error::ExpectedEmptyMappingForUnitStruct { .. } => Cow::Borrowed("expected empty mapping here"),
        Error::UnexpectedContainerEndWhileSkippingNode { .. } => Cow::Borrowed("unexpected container end"),
        Error::AliasReplayCounterOverflow { .. } => Cow::Borrowed("YAML document too large or too complex"),
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
        Error::AliasReplayStackDepthExceeded { depth, max_depth, .. } => Cow::Owned(format!(
            "YAML document too large or too complex: depth={depth} > {max_depth}"
        )),
        Error::UnknownAnchor { .. } => Cow::Borrowed("reference to unknown value"),
        Error::RecursiveReferencesRequireWeakTypes { .. } => Cow::Borrowed("Recursive reference not allowed here"),
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

