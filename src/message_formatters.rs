use crate::de_error::{Error, MessageFormatter};
use crate::location::Locations;
use crate::Location;

#[cfg(any(feature = "garde", feature = "validator"))]
use crate::path_map::format_path_with_resolved_leaf;

#[cfg(feature = "garde")]
use crate::de_error::collect_garde_issues;

#[cfg(feature = "validator")]
use crate::de_error::collect_validator_issues;

/// Default developer-oriented message formatter.
///
/// This reproduces the current built-in English messages.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultMessageFormatter;

/// Alias for the default developer-oriented formatter.
pub type DeveloperMessageFormatter = DefaultMessageFormatter;

impl MessageFormatter for DefaultMessageFormatter {
    fn format_message(&self, err: &Error) -> String {
        let mut buf = String::new();

        let msg = match err {
            Error::WithSnippet { error, .. } => &self.format_message(error),

            Error::Message { msg, .. } | Error::HookError { msg, .. } => msg,
            Error::Eof { .. } => "unexpected end of input",
            Error::MultipleDocuments { hint, .. } => {
                &format!("multiple YAML documents detected; {hint}")
            }
            Error::Unexpected { expected, .. } => &format!("unexpected event: expected {expected}"),
            Error::MergeValueNotMapOrSeqOfMaps { .. } => {
                "YAML merge value must be mapping or sequence of mappings"
            }
            Error::InvalidBinaryBase64 { .. } => "invalid !!binary base64",
            Error::BinaryNotUtf8 { .. } => {
                "!!binary scalar is not valid UTF-8 so cannot be stored into string"
            }
            Error::TaggedScalarCannotDeserializeIntoString { .. } => {
                "cannot deserialize scalar tagged into string"
            }
            Error::UnexpectedSequenceEnd { .. } => "unexpected sequence end",
            Error::UnexpectedMappingEnd { .. } => "unexpected mapping end",
            Error::InvalidBooleanStrict { .. } => {
                "invalid boolean (strict mode expects true/false)"
            }
            Error::InvalidCharNull { .. } => {
                "invalid char: cannot deserialize null; use Option<char>"
            }
            Error::InvalidCharNotSingleScalar { .. } => {
                "invalid char: expected a single Unicode scalar value"
            }
            Error::NullIntoString { .. } => {
                "cannot deserialize null into string; use Option<String>"
            }
            Error::BytesNotSupportedMissingBinaryTag { .. } => {
                "bytes not supported (missing !!binary tag)"
            }
            Error::UnexpectedValueForUnit { .. } => "unexpected value for unit",
            Error::ExpectedEmptyMappingForUnitStruct { .. } => {
                "expected empty mapping for unit struct"
            }
            Error::UnexpectedContainerEndWhileSkippingNode { .. } => {
                "unexpected container end while skipping node"
            }
            Error::InternalSeedReusedForMapKey { .. } => {
                "internal error: seed reused for map key"
            }
            Error::ValueRequestedBeforeKey { .. } => "value requested before key",
            Error::ExpectedStringKeyForExternallyTaggedEnum { .. } => {
                "expected string key for externally tagged enum"
            }
            Error::ExternallyTaggedEnumExpectedScalarOrMapping { .. } => {
                "externally tagged enum expected scalar or mapping"
            }
            Error::UnexpectedValueForUnitEnumVariant { .. } => {
                "unexpected value for unit enum variant"
            }
            Error::InvalidUtf8Input => "input is not valid UTF-8",
            Error::AliasReplayCounterOverflow { .. } => "alias replay counter overflow",
            Error::AliasReplayLimitExceeded {
                total_replayed_events,
                max_total_replayed_events,
                ..
            } => &format!(
                "alias replay limit exceeded: total_replayed_events={total_replayed_events} > {max_total_replayed_events}"
            ),
            Error::AliasExpansionLimitExceeded {
                anchor_id,
                expansions,
                max_expansions_per_anchor,
                ..
            } => &format!(
                "alias expansion limit exceeded for anchor id {anchor_id}: {expansions} > {max_expansions_per_anchor}"
            ),
            Error::AliasReplayStackDepthExceeded {
                depth,
                max_depth,
                ..
            } => &format!("alias replay stack depth exceeded: depth={depth} > {max_depth}"),
            Error::FoldedBlockScalarMustIndentContent { .. } => {
                "folded block scalars must indent their content"
            }
            Error::InternalDepthUnderflow { .. } => "internal depth underflow",
            Error::InternalRecursionStackEmpty { .. } => {
                "internal recursion stack empty"
            }
            Error::RecursiveReferencesRequireWeakTypes { .. } => {
                "Recursive references require weak recursion types"
            }
            Error::InvalidScalar { ty, .. } => &format!("invalid {ty}"),
            Error::SerdeInvalidType {
                unexpected,
                expected,
                ..
            } => &format!("invalid type: {unexpected}, expected {expected}"),
            Error::SerdeInvalidValue {
                unexpected,
                expected,
                ..
            } => &format!("invalid value: {unexpected}, expected {expected}"),
            Error::SerdeUnknownVariant {
                variant,
                expected,
                ..
            } => &format!(
                "unknown variant `{variant}`, expected one of {}",
                expected.join(", ")
            ),
            Error::SerdeUnknownField {
                field,
                expected,
                ..
            } => &format!(
                "unknown field `{field}`, expected one of {}",
                expected.join(", ")
            ),
            Error::SerdeMissingField { field, .. } => &format!("missing field `{field}`"),
            Error::UnexpectedContainerEndWhileReadingKeyNode { .. } => {
                "unexpected container end while reading key"
            }
            Error::DuplicateMappingKey { key, .. } => match key {
                Some(k) => &format!("duplicate mapping key: {k}"),
                None => "duplicate mapping key",
            },
            Error::TaggedEnumMismatch { tagged, target, .. } => {
                &format!("tagged enum `{tagged}` does not match target enum `{target}`")
            }
            Error::SerdeVariantId { msg, .. } => msg,
            Error::ExpectedMappingEndAfterEnumVariantValue { .. } =>
                "expected end of mapping after enum variant value",
            Error::ContainerEndMismatch { .. } => "list or mapping end with no start",
            Error::UnknownAnchor { id, .. } => &format!("alias references unknown anchor id {id}"),
            Error::Budget { breach, .. } => &format!("YAML document is too long or complex: {breach:?}"),
            Error::QuotingRequired { value, .. } => {
                &format!("The string value [{value}] must be quoted")
            }
            Error::CannotBorrowTransformedString { reason, .. } =>
                "Only single string without escape characters is accepted here",
            Error::IOError { cause } => &format!("IO error: {cause}"),
            Error::AliasError { msg, locations } => {
                let ref_loc = locations.reference_location;
                let def_loc = locations.defined_location;
                &match (ref_loc, def_loc) {
                    (Location::UNKNOWN, Location::UNKNOWN) => msg,
                    (r, d) if r != Location::UNKNOWN && (d == Location::UNKNOWN || d == r) => {
                        msg
                    }
                    (_r, d) => &format!("{msg} (defined at line {}, column {})", d.line, d.column),
                }
            }

            #[cfg(feature = "garde")]
            Error::ValidationError { report, locations } => {
                use std::fmt::Write;
                let issues = collect_garde_issues(report);
                let mut first = true;
                for issue in issues {
                    if !first {
                        let _ = writeln!(buf);
                    }
                    first = false;

                    let entry = issue.display_entry();
                    let path_key = issue.path;
                    let original_leaf = path_key
                        .leaf_string()
                        .unwrap_or_else(|| "<root>".to_string());

                    let (locs, resolved_leaf) = locations
                        .search(&path_key)
                        .unwrap_or((Locations::UNKNOWN, original_leaf));

                    let loc = if locs.reference_location != Location::UNKNOWN {
                        locs.reference_location
                    } else {
                        locs.defined_location
                    };

                    let resolved_path = format_path_with_resolved_leaf(&path_key, &resolved_leaf);
                    let _ = write!(buf, "validation error at {resolved_path}: {entry}");
                    if loc != Location::UNKNOWN {
                        let _ = write!(buf, " at line {}, column {}", loc.line, loc.column);
                    }
                }
                &buf
            }
            #[cfg(feature = "garde")]
            Error::ValidationErrors { errors } => {
                &format!("validation failed for {} document(s)", errors.len())
            }
            #[cfg(feature = "validator")]
            Error::ValidatorError { errors, locations } => {
                use std::fmt::Write;
                let issues = collect_validator_issues(errors);
                let mut first = true;
                for issue in issues {
                    if !first {
                        let _ = writeln!(buf);
                    }
                    first = false;

                    let entry = issue.display_entry();
                    let path_key = issue.path;
                    let original_leaf = path_key
                        .leaf_string()
                        .unwrap_or_else(|| "<root>".to_string());

                    let (locs, resolved_leaf) = locations
                        .search(&path_key)
                        .unwrap_or((Locations::UNKNOWN, original_leaf));

                    let loc = if locs.reference_location != Location::UNKNOWN {
                        locs.reference_location
                    } else {
                        locs.defined_location
                    };

                    let resolved_path = format_path_with_resolved_leaf(&path_key, &resolved_leaf);
                    let _ = write!(buf, "validation error at {resolved_path}: {entry}");
                    if loc != Location::UNKNOWN {
                        let _ = write!(buf, " at line {}, column {}", loc.line, loc.column);
                    }
                }
                &buf
            }
            #[cfg(feature = "validator")]
            Error::ValidatorErrors { errors } => {
                &format!("validation failed for {} document(s)", errors.len())
            }
        };
        msg.to_owned()
    }
}
