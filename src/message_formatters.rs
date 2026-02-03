use crate::de_error::{Error, MessageFormatter, UserMessageFormatter};
use crate::location::Locations;
use crate::Location;

use std::borrow::Cow;

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
        match err {
            Error::WithSnippet { error, .. } => return self.format_message(error),

            _ => {}
        }

        let msg: Cow<'_, str> = match err {
            // handled by early return above
            Error::WithSnippet { .. } => unreachable!(),

            Error::Message { msg, .. }
            | Error::HookError { msg, .. }
            | Error::SerdeVariantId { msg, .. } => Cow::Borrowed(msg),
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
            Error::InvalidUtf8Input | Error::BinaryNotUtf8 { .. } => Cow::Borrowed(
                "!!binary scalar is not valid UTF-8 so cannot be stored into string. If you just use !!binary for documentation/annotation, set ignore_binary_tag_for_string in Options",
            ),
            Error::TaggedScalarCannotDeserializeIntoString { .. } => {
                Cow::Borrowed("cannot deserialize scalar tagged into string")
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
            Error::AliasReplayCounterOverflow { .. } => {
                Cow::Borrowed("alias replay counter overflow")
            }
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
                Cow::Borrowed("Recursive references require weak recursion types")
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
            Error::SerdeMissingField { field, .. } => {
                Cow::Owned(format!("missing field `{field}`"))
            }
            Error::UnexpectedContainerEndWhileReadingKeyNode { .. } => {
                Cow::Borrowed("unexpected container end while reading key")
            }
            Error::DuplicateMappingKey { key, .. } => match key {
                Some(k) => Cow::Owned(format!("duplicate mapping key: {k}, set DuplicateKeyPolicy in Options if acceptable")),
                None => Cow::Borrowed("duplicate mapping key, set DuplicateKeyPolicy in Options if acceptable"),
            },
            Error::TaggedEnumMismatch { tagged, target, .. } => Cow::Owned(format!(
                "tagged enum `{tagged}` does not match target enum `{target}`",
            )),
            Error::ExpectedMappingEndAfterEnumVariantValue { .. } => {
                Cow::Borrowed("expected end of mapping after enum variant value")
            }
            Error::ContainerEndMismatch { .. } => {
                Cow::Borrowed("list or mapping end with no start")
            }
            Error::UnknownAnchor { id, .. } => {
                Cow::Owned(format!("alias references unknown anchor id {id}"))
            }
            Error::Budget { breach, .. } => {
                Cow::Owned(format!("budget breached: {breach:?}"))
            }
            Error::QuotingRequired { value, .. } => {
                Cow::Owned(format!("The string value [{value}] must be quoted"))
            }
            Error::CannotBorrowTransformedString { reason, .. } => Cow::Owned(format!(
                "cannot deserialize into &str ({reason}); use String or Cow<str> instead",
            )),
            Error::IOError { cause } => Cow::Owned(format!("IO error: {cause}")),
            Error::AliasError { msg, locations } => {
                let ref_loc = locations.reference_location;
                let def_loc = locations.defined_location;
                match (ref_loc, def_loc) {
                    (Location::UNKNOWN, Location::UNKNOWN) => Cow::Borrowed(msg),
                    (r, d)
                        if r != Location::UNKNOWN && (d == Location::UNKNOWN || d == r) =>
                    {
                        Cow::Borrowed(msg)
                    }
                    (_r, d) => Cow::Owned(format!(
                        "{msg} (defined at line {}, column {})",
                        d.line, d.column
                    )),
                }
            }

            #[cfg(feature = "garde")]
            Error::ValidationError { report, locations } => {
                use std::fmt::Write;

                let issues = collect_garde_issues(report);
                let mut buf = String::new();
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

                    let resolved_path =
                        format_path_with_resolved_leaf(&path_key, &resolved_leaf);
                    let _ = write!(buf, "validation error at {resolved_path}: {entry}");
                    if loc != Location::UNKNOWN {
                        let _ = write!(buf, " at line {}, column {}", loc.line, loc.column);
                    }
                }
                Cow::Owned(buf)
            }
            #[cfg(feature = "garde")]
            Error::ValidationErrors { errors } => Cow::Owned(format!(
                "validation failed for {} document(s)",
                errors.len()
            )),
            #[cfg(feature = "validator")]
            Error::ValidatorError { errors, locations } => {
                use std::fmt::Write;

                let issues = collect_validator_issues(errors);
                let mut buf = String::new();
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

                    let resolved_path =
                        format_path_with_resolved_leaf(&path_key, &resolved_leaf);
                    let _ = write!(buf, "validation error at {resolved_path}: {entry}");
                    if loc != Location::UNKNOWN {
                        let _ = write!(buf, " at line {}, column {}", loc.line, loc.column);
                    }
                }
                Cow::Owned(buf)
            }
            #[cfg(feature = "validator")]
            Error::ValidatorErrors { errors } => Cow::Owned(format!(
                "validation failed for {} document(s)",
                errors.len()
            )),
        };

        msg.into_owned()
    }
}

impl MessageFormatter for UserMessageFormatter {
    fn format_message(&self, err: &Error) -> String {
        match err {
            Error::Message { msg, .. } => msg.clone(),
            Error::HookError { msg, .. } => msg.clone(),
            Error::Eof { .. } => "unexpected end of file".to_owned(),
            Error::MultipleDocuments { .. } => {
                "multiple YAML documents found where only one was expected".to_owned()
            }
            Error::Unexpected { expected, .. } => format!("unexpected content, expected {expected}"),
            Error::MergeValueNotMapOrSeqOfMaps { .. } => {
                "merge value must be a mapping (or a list of mappings)".to_owned()
            }
            Error::InvalidBinaryBase64 { .. } => "invalid binary value".to_owned(),
            Error::BinaryNotUtf8 { .. } => "invalid binary value".to_owned(),
            Error::TaggedScalarCannotDeserializeIntoString { .. } => "invalid value".to_owned(),
            Error::UnexpectedSequenceEnd { .. } => "structure mismatch".to_owned(),
            Error::UnexpectedMappingEnd { .. } => "structure mismatch".to_owned(),
            Error::InvalidBooleanStrict { .. } => "invalid boolean value".to_owned(),
            Error::InvalidCharNull { .. } => "invalid character value".to_owned(),
            Error::InvalidCharNotSingleScalar { .. } => "invalid character value".to_owned(),
            Error::NullIntoString { .. } => "invalid value".to_owned(),
            Error::BytesNotSupportedMissingBinaryTag { .. } => "invalid value".to_owned(),
            Error::UnexpectedValueForUnit { .. } => "invalid value".to_owned(),
            Error::ExpectedEmptyMappingForUnitStruct { .. } => "invalid value".to_owned(),
            Error::UnexpectedContainerEndWhileSkippingNode { .. } => "processing failed".to_owned(),
            Error::InternalSeedReusedForMapKey { .. } => "processing failed".to_owned(),
            Error::ValueRequestedBeforeKey { .. } => "processing failed".to_owned(),
            Error::ExpectedStringKeyForExternallyTaggedEnum { .. } => "invalid value".to_owned(),
            Error::ExternallyTaggedEnumExpectedScalarOrMapping { .. } => "invalid value".to_owned(),
            Error::UnexpectedValueForUnitEnumVariant { .. } => "invalid value".to_owned(),
            Error::InvalidUtf8Input => "input is not valid UTF-8".to_owned(),
            Error::AliasReplayCounterOverflow { .. } => "processing failed".to_owned(),
            Error::AliasReplayLimitExceeded { .. } => "processing failed".to_owned(),
            Error::AliasExpansionLimitExceeded { .. } => "processing failed".to_owned(),
            Error::AliasReplayStackDepthExceeded { .. } => "processing failed".to_owned(),
            Error::FoldedBlockScalarMustIndentContent { .. } => "invalid value".to_owned(),
            Error::InternalDepthUnderflow { .. } => "processing failed".to_owned(),
            Error::InternalRecursionStackEmpty { .. } => "processing failed".to_owned(),
            Error::RecursiveReferencesRequireWeakTypes { .. } => {
                "recursive references require a compatible target type".to_owned()
            }
            Error::InvalidScalar { .. } => "invalid value".to_owned(),
            Error::SerdeInvalidType {
                unexpected,
                expected,
                ..
            } => format!("invalid type: {unexpected}, expected {expected}"),
            Error::SerdeInvalidValue {
                unexpected,
                expected,
                ..
            } => format!("invalid value: {unexpected}, expected {expected}"),
            Error::SerdeUnknownVariant {
                variant,
                expected,
                ..
            } => format!(
                "unknown variant `{variant}`, expected one of {}",
                expected.join(", ")
            ),
            Error::SerdeUnknownField { field, expected, .. } => format!(
                "unknown field `{field}`, expected one of {}",
                expected.join(", ")
            ),
            Error::SerdeMissingField { field, .. } => format!("missing field `{field}`"),

            // Structural / mapping errors
            Error::UnexpectedContainerEndWhileReadingKeyNode { .. } => "invalid value".to_owned(),
            Error::DuplicateMappingKey { key, .. } => match key {
                Some(k) => format!("duplicate mapping key: {k}"),
                None => "duplicate mapping key".to_owned(),
            },
            Error::TaggedEnumMismatch { .. } => "invalid value".to_owned(),
            Error::SerdeVariantId { .. } => "invalid value".to_owned(),
            Error::ExpectedMappingEndAfterEnumVariantValue { .. } => "invalid value".to_owned(),

            Error::ContainerEndMismatch { .. } => "structure mismatch".to_owned(),
            Error::UnknownAnchor { .. } => "reference to unknown value".to_owned(),
            Error::Budget { .. } => "document is too complex, resource limit exceeded".to_owned(),
            Error::QuotingRequired { .. } => "value requires quoting".to_owned(),
            Error::CannotBorrowTransformedString { .. } => "processing failed".to_owned(),
            Error::IOError { .. } => "processing failed (IO error)".to_owned(),

            Error::AliasError { msg, locations } => {
                let msg = if msg.contains("alias references unknown anchor") {
                    "reference to unknown value"
                } else {
                    msg.as_str()
                };

                let ref_loc = locations.reference_location;
                let def_loc = locations.defined_location;
                match (ref_loc, def_loc) {
                    (Location::UNKNOWN, Location::UNKNOWN) => msg.to_owned(),
                    (r, d) if r != Location::UNKNOWN && (d == Location::UNKNOWN || d == r) => {
                        msg.to_owned()
                    }
                    (_r, d) => format!("{msg} (defined at line {}, column {})", d.line, d.column),
                }
            }

            Error::WithSnippet { error, .. } => self.format_message(error),

            #[cfg(feature = "garde")]
            Error::ValidationError { report, locations } => {
                use std::fmt::Write;

                let mut buf = String::new();
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

                    let (_locs, resolved_leaf) = locations
                        .search(&path_key)
                        .unwrap_or((Locations::UNKNOWN, original_leaf));

                    let resolved_path = format_path_with_resolved_leaf(&path_key, &resolved_leaf);
                    let _ = write!(buf, "invalid value at {resolved_path}: {entry}");
                }
                buf
            }
            #[cfg(feature = "garde")]
            Error::ValidationErrors { errors } => {
                format!("validation failed for {} document(s)", errors.len())
            }

            #[cfg(feature = "validator")]
            Error::ValidatorError { errors, locations } => {
                use std::fmt::Write;

                let mut buf = String::new();
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

                    let (_locs, resolved_leaf) = locations
                        .search(&path_key)
                        .unwrap_or((Locations::UNKNOWN, original_leaf));

                    let resolved_path = format_path_with_resolved_leaf(&path_key, &resolved_leaf);
                    let _ = write!(buf, "invalid value at {resolved_path}: {entry}");
                }
                buf
            }
            #[cfg(feature = "validator")]
            Error::ValidatorErrors { errors } => {
                format!("validation failed for {} document(s)", errors.len())
            }
        }
    }
}
