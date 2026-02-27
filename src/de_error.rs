use crate::Location;
use crate::budget::BudgetBreach;
use crate::de_snipped::fmt_snippet_window_offset_or_fallback;
use crate::localizer::{DEFAULT_ENGLISH_LOCALIZER, ExternalMessageSource, Localizer};
use crate::location::Locations;
use crate::parse_scalars::{
    parse_int_signed, parse_yaml11_bool, parse_yaml12_float, scalar_is_nullish,
};
#[cfg(feature = "garde")]
use crate::path_map::path_key_from_garde;
#[cfg(any(feature = "garde", feature = "validator"))]
use crate::path_map::{PathKey, PathMap, format_path_with_resolved_leaf};
use crate::tags::SfTag;
use annotate_snippets::Level;
use saphyr_parser::{ScalarStyle, ScanError};
use serde::de::{self};
use std::borrow::Cow;
use std::cell::Cell;
use std::fmt;
#[cfg(feature = "validator")]
use validator::{ValidationErrors, ValidationErrorsKind};

#[cfg(any(feature = "garde", feature = "validator"))]
use crate::localizer::ExternalMessage;

/// Formats error *messages* (not including locations/snippets).
///
/// This is the core customization hook for deferred rendering. The error value remains
/// structured data; the formatter decides what message text to show (developer-oriented,
/// user-oriented, localized, etc.).
///
/// Important: implementations must NOT call `err.to_string()` / `Display` for `Error` to
/// avoid recursion once `Display` delegates to `Error::render()`.
///
/// # Example
///
/// Override a couple of messages, returning `Cow::Borrowed` for a fixed string and
/// `Cow::Owned` for a formatted message, while delegating all other cases to
/// `UserMessageFormatter`.
///
/// ```rust
/// use serde_saphyr::{Error, Location, MessageFormatter, UserMessageFormatter};
/// use std::borrow::Cow;
///
/// struct PoliteFormatter;
///
/// impl MessageFormatter for PoliteFormatter {
///     fn format_message<'a>(&self, err: &'a Error) -> Cow<'a, str> {
///         // `UserMessageFormatter` is a zero-sized type, so it is cheap to instantiate.
///         let fallback = UserMessageFormatter;
///
///         match err {
///             // Fixed string => `Cow::Borrowed`
///             Error::Eof { .. } => Cow::Borrowed("could you please provide a YAML document?"),
///
///             // Formatted string => `Cow::Owned`
///             Error::UnknownAnchor { .. } => {
///                 Cow::Borrowed("sorry but unknown reference")
///             }
///
///             // Everything else => delegate
///             _ => fallback.format_message(err),
///         }
///     }
/// }
///
/// let err = serde_saphyr::from_str::<String>("").unwrap_err();
/// assert!(err.render_with_formatter(&PoliteFormatter).contains("please provide"));
///
/// let err = Error::UnknownAnchor {
///     location: Location::UNKNOWN,
/// };
/// assert!(err
///     .render_with_formatter(&PoliteFormatter)
///     .contains("unknown reference"));
/// ```
pub trait MessageFormatter {
    /// Return the [`Localizer`] used by the renderer.
    ///
    /// This controls wording that is produced outside of [`MessageFormatter::format_message`],
    /// such as location suffixes and snippet/validation labels.
    fn localizer(&self) -> &dyn Localizer {
        &DEFAULT_ENGLISH_LOCALIZER
    }

    /// Return the message text for `err`.
    ///
    /// The returned string should NOT include location suffixes like
    /// `"at line X, column Y"`; those are added by the renderer.
    fn format_message<'a>(&self, err: &'a Error) -> Cow<'a, str>;
}

/// User-facing message formatter.
///
/// This formatter simplifies technical errors and removes internal details.
/// ```
/// use serde_saphyr::UserMessageFormatter;
///
/// let err = serde_saphyr::from_str::<String>("").unwrap_err();
/// let msg = err.render_with_formatter(&UserMessageFormatter);
///
/// assert_eq!(msg, "unexpected end of file at line 1, column 1");
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct UserMessageFormatter;

/// Controls whether snippet output is included when available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetMode {
    /// Render snippets when the error is wrapped in `Error::WithSnippet`.
    Auto,
    /// Never render snippets; render a plain (location-suffixed) message instead.
    Off,
}

/// Options for deferred error rendering.
///
/// Prefer constructing this via the [`render_options!`](crate::render_options!) macro
/// instead of a struct literal. This keeps call sites stable even if new fields are added
/// in the future (this type is `#[non_exhaustive]`).
///
/// # Example (using the `render_options!` macro)
///
/// ```rust
/// use serde_saphyr::{DefaultMessageFormatter, SnippetMode};
///
/// let dev = DefaultMessageFormatter;
/// // Customize how an error is rendered later (formatter + snippet mode).
/// let render_opts = serde_saphyr::render_options! {
///     formatter: &dev,
///     snippets: SnippetMode::Off,
/// };
///
/// let err = serde_saphyr::from_str::<String>("").unwrap_err();
/// let rendered = err.render_with_options(render_opts);
/// assert!(rendered.contains("unexpected"));
/// ```
#[non_exhaustive]
#[derive(Clone, Copy)]
pub struct RenderOptions<'a> {
    /// Message formatter used to produce the core error message text.
    pub formatter: &'a dyn MessageFormatter,
    /// Snippet rendering mode.
    pub snippets: SnippetMode,
}

impl<'a> Default for RenderOptions<'a> {
    #[inline]
    fn default() -> Self {
        // Keep the default formatter reference valid even if `RenderOptions` is stored.
        static DEFAULT_FMT: crate::message_formatters::DefaultMessageFormatter =
            crate::message_formatters::DefaultMessageFormatter;

        Self::new(&DEFAULT_FMT)
    }
}

impl<'a> RenderOptions<'a> {
    /// Construct render options with the given message `formatter` and default values
    /// for all other fields.
    ///
    /// Defaults:
    /// - `snippets`: [`SnippetMode::Auto`]
    #[inline]
    pub fn new(formatter: &'a dyn MessageFormatter) -> Self {
        Self {
            formatter,
            snippets: SnippetMode::Auto,
        }
    }
}

/// Cropped YAML source window stored inside [`Error::WithSnippet`].
///
/// The window is described in terms of the original (absolute) 1-based line numbers.
/// This allows selecting the best-matching region for a particular error location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CroppedRegion {
    /// Cropped source text used for snippet rendering.
    pub text: String,
    /// The 1-based line number in the *original* input where `text` starts.
    pub start_line: usize,
    /// The 1-based line number in the *original* input where `text` ends (inclusive).
    pub end_line: usize,
}

impl CroppedRegion {
    fn covers(&self, location: &Location) -> bool {
        if location == &Location::UNKNOWN {
            return false;
        }
        let line = location.line as usize;
        self.start_line <= line && line <= self.end_line
    }
}

fn line_count_including_trailing_empty_line(text: &str) -> usize {
    let mut lines = text.split_terminator('\n').count().max(1);
    if text.ends_with('\n') {
        lines = lines.saturating_add(1);
    }
    lines
}

#[cfg(any(feature = "garde", feature = "validator"))]
#[derive(Debug, Clone)]
pub(crate) struct ValidationIssue {
    pub(crate) path: PathKey,
    pub(crate) code: String,
    pub(crate) message: Option<String>,
    pub(crate) params: Vec<(String, String)>,
}

#[cfg(any(feature = "garde", feature = "validator"))]
impl ValidationIssue {
    pub(crate) fn display_entry(&self) -> String {
        if let Some(msg) = &self.message {
            return msg.clone();
        }

        if self.params.is_empty() {
            return self.code.clone();
        }

        let mut params = String::new();
        for (i, (k, v)) in self.params.iter().enumerate() {
            if i > 0 {
                params.push_str(", ");
            }
            params.push_str(k);
            params.push('=');
            params.push_str(v);
        }
        format!("{} ({params})", self.code)
    }

    pub(crate) fn display_entry_overridden(
        &self,
        l10n: &dyn Localizer,
        source: ExternalMessageSource,
    ) -> String {
        let raw = self.display_entry();
        let overridden = l10n
            .override_external_message(ExternalMessage {
                source,
                original: raw.as_str(),
                code: Some(self.code.as_str()),
                params: &self.params,
            })
            .unwrap_or(Cow::Borrowed(raw.as_str()));
        overridden.into_owned()
    }
}

// Fallback location for Serde's static error constructors (`unknown_field`, `missing_field`,
// etc.) which have no `&self` and cannot access deserializer state. Thread-local because
// that is the only side-channel available. `Cell` suffices since `Location` is `Copy`.
//
// Set to the current key's location before each key deserialization via
// [`MissingFieldLocationGuard`]; read by [`maybe_attach_fallback_location`].
// The guard saves/restores the previous value on drop for correct nesting.
thread_local! {
    static MISSING_FIELD_FALLBACK: Cell<Option<Location>> = const { Cell::new(None) };
}

/// RAII guard for [`MISSING_FIELD_FALLBACK`]. Saves the previous value on creation,
/// restores it on drop.
pub(crate) struct MissingFieldLocationGuard {
    prev: Option<Location>,
}

impl MissingFieldLocationGuard {
    pub(crate) fn new(location: Location) -> Self {
        let prev = MISSING_FIELD_FALLBACK.with(|c| c.replace(Some(location)));
        Self { prev }
    }

    /// Update the fallback location in place, reusing the existing guard's restore point.
    pub(crate) fn replace_location(&mut self, location: Location) {
        MISSING_FIELD_FALLBACK.with(|c| c.set(Some(location)));
    }
}

impl Drop for MissingFieldLocationGuard {
    fn drop(&mut self) {
        MISSING_FIELD_FALLBACK.with(|c| c.set(self.prev));
    }
}

/// The reason why a string value was transformed during parsing and cannot be borrowed.
///
/// When deserializing to `&str`, the value must exist verbatim in the input. However,
/// certain YAML constructs require string transformation, making borrowing impossible.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformReason {
    /// Escape sequences were processed (e.g., `\n`, `\t`, `\uXXXX` in double-quoted strings).
    EscapeSequence,
    /// Line folding was applied (folded block scalar `>`).
    LineFolding,
    /// Multi-line plain or quoted scalar with whitespace normalization.
    MultiLineNormalization,
    /// Block scalar processing (literal `|` or folded `>` with chomping/indentation).
    BlockScalarProcessing,
    /// Single-quoted string with `''` escape processing.
    SingleQuoteEscape,
    /// Borrowing is not supported because the deserializer does not have access to the full input
    /// buffer (for example, when deserializing from a `Read`er), or because the parser did not
    /// provide a slice that is a subslice of the original input.
    InputNotBorrowable,

    /// The parser returned an owned string for this scalar.
    ///
    /// In newer `saphyr-parser` versions, zero-copy is represented directly as `Cow::Borrowed`.
    /// If a scalar comes through as `Cow::Owned`, the deserializer cannot safely fabricate a
    /// borrow, because it would not refer to the original input buffer.
    ParserReturnedOwned,
}

impl fmt::Display for TransformReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransformReason::EscapeSequence => write!(f, "escape sequence processing"),
            TransformReason::LineFolding => write!(f, "line folding"),
            TransformReason::MultiLineNormalization => {
                write!(f, "multi-line whitespace normalization")
            }
            TransformReason::BlockScalarProcessing => write!(f, "block scalar processing"),
            TransformReason::SingleQuoteEscape => write!(f, "single-quote escape processing"),
            TransformReason::InputNotBorrowable => {
                write!(f, "input is not available for borrowing")
            }
            TransformReason::ParserReturnedOwned => write!(f, "parser returned an owned string"),
        }
    }
}

/// Error type compatible with `serde::de::Error`.
#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    /// Free-form error with optional source location.
    Message {
        msg: String,
        location: Location,
    },

    /// Text primarily produced by a dependency (parser / validators).
    ///
    /// Renderers should call [`Localizer::override_external_message`] to allow callers
    /// to replace or translate this text.
    ExternalMessage {
        source: ExternalMessageSource,
        msg: String,
        /// Stable-ish identifier when available (e.g. validator error code).
        code: Option<String>,
        /// Optional structured parameters when available.
        params: Vec<(String, String)>,
        location: Location,
    },
    /// Unexpected end of input.
    Eof {
        location: Location,
    },
    /// More than one YAML document was found when a single document was expected.
    ///
    /// This is typically returned by single-document entrypoints like `from_str*` / `from_slice*`
    /// / `read_to_end*` when the input stream contains multiple `---`-delimited documents.
    MultipleDocuments {
        /// Developer-facing hint (may mention specific APIs).
        hint: &'static str,
        location: Location,
    },
    /// Structural/type mismatch — something else than the expected token/value was seen.
    Unexpected {
        expected: &'static str,
        location: Location,
    },

    /// YAML merge (`<<`) value was not a mapping or a sequence of mappings.
    MergeValueNotMapOrSeqOfMaps {
        location: Location,
    },

    /// `!!binary` scalar could not be decoded as base64.
    InvalidBinaryBase64 {
        location: Location,
    },

    /// `!!binary` scalar decoded successfully but was not valid UTF-8 when a string was expected.
    BinaryNotUtf8 {
        location: Location,
    },

    /// A scalar was explicitly tagged but could not be deserialized into a string.
    TaggedScalarCannotDeserializeIntoString {
        location: Location,
    },

    /// Encountered a sequence end where it was not expected.
    UnexpectedSequenceEnd {
        location: Location,
    },

    /// Encountered a mapping end where it was not expected.
    UnexpectedMappingEnd {
        location: Location,
    },

    /// Invalid boolean literal in strict mode.
    InvalidBooleanStrict {
        location: Location,
    },

    /// Invalid char: null cannot be deserialized into `char`.
    InvalidCharNull {
        location: Location,
    },

    /// Invalid char: expected a single Unicode scalar value.
    InvalidCharNotSingleScalar {
        location: Location,
    },

    /// Cannot deserialize null into string.
    NullIntoString {
        location: Location,
    },

    /// Bytes (`&[u8]` / `Vec<u8>`) are not supported unless the scalar is tagged as `!!binary`.
    BytesNotSupportedMissingBinaryTag {
        location: Location,
    },

    /// Unexpected value for unit (`()`).
    UnexpectedValueForUnit {
        location: Location,
    },

    /// Unit struct expected an empty mapping.
    ExpectedEmptyMappingForUnitStruct {
        location: Location,
    },

    /// While skipping a node, a container end event was encountered unexpectedly.
    UnexpectedContainerEndWhileSkippingNode {
        location: Location,
    },

    /// Internal error: a seed was reused for a map key.
    InternalSeedReusedForMapKey {
        location: Location,
    },

    /// Internal error: value requested before key.
    ValueRequestedBeforeKey {
        location: Location,
    },

    /// Externally tagged enum: expected a string key.
    ExpectedStringKeyForExternallyTaggedEnum {
        location: Location,
    },

    /// Externally tagged enum: expected either a scalar or a mapping.
    ExternallyTaggedEnumExpectedScalarOrMapping {
        location: Location,
    },

    /// Unexpected value for unit enum variant.
    UnexpectedValueForUnitEnumVariant {
        location: Location,
    },

    /// Input was not valid UTF-8.
    InvalidUtf8Input,

    /// Alias replay counter overflow.
    AliasReplayCounterOverflow {
        location: Location,
    },

    /// Alias replay total event limit exceeded.
    AliasReplayLimitExceeded {
        total_replayed_events: usize,
        max_total_replayed_events: usize,
        location: Location,
    },

    /// Alias expansion limit exceeded for a single anchor.
    AliasExpansionLimitExceeded {
        anchor_id: usize,
        expansions: usize,
        max_expansions_per_anchor: usize,
        location: Location,
    },

    /// Alias replay stack depth limit exceeded.
    AliasReplayStackDepthExceeded {
        depth: usize,
        max_depth: usize,
        location: Location,
    },

    /// Folded block scalars must indent their content.
    FoldedBlockScalarMustIndentContent {
        location: Location,
    },

    /// Internal: depth counter underflow.
    InternalDepthUnderflow {
        location: Location,
    },

    /// Internal: recursion stack empty.
    InternalRecursionStackEmpty {
        location: Location,
    },

    /// recursive references require weak recursion types.
    RecursiveReferencesRequireWeakTypes {
        location: Location,
    },

    /// Scalar parsing failed for the requested target type.
    InvalidScalar {
        ty: &'static str,
        location: Location,
    },

    /// Serde-generated: invalid type.
    SerdeInvalidType {
        unexpected: String,
        expected: String,
        location: Location,
    },

    /// Serde-generated: invalid value.
    SerdeInvalidValue {
        unexpected: String,
        expected: String,
        location: Location,
    },

    /// Serde-generated: unknown enum variant.
    SerdeUnknownVariant {
        variant: String,
        expected: Vec<&'static str>,
        location: Location,
    },

    /// Serde-generated: unknown field.
    SerdeUnknownField {
        field: String,
        expected: Vec<&'static str>,
        location: Location,
    },

    /// Serde-generated: missing required field.
    SerdeMissingField {
        field: &'static str,
        location: Location,
    },

    /// Encountered the end of a sequence or mapping while reading a key node.
    ///
    /// This indicates a structural mismatch in the input.
    UnexpectedContainerEndWhileReadingKeyNode {
        location: Location,
    },

    /// Duplicate key in a mapping.
    ///
    /// When the duplicate key can be rendered as a string-like scalar, `key` is provided.
    DuplicateMappingKey {
        key: Option<String>,
        location: Location,
    },

    /// Tagged enum name does not match the target enum.
    TaggedEnumMismatch {
        tagged: String,
        target: &'static str,
        location: Location,
    },

    /// Serde-generated error while deserializing an enum variant identifier.
    SerdeVariantId {
        msg: String,
        location: Location,
    },

    /// Expected the end of a mapping after an externally tagged enum variant value.
    ExpectedMappingEndAfterEnumVariantValue {
        location: Location,
    },
    ContainerEndMismatch {
        location: Location,
    },
    /// Alias references a non-existent anchor.
    UnknownAnchor {
        location: Location,
    },
    /// Error related to an alias, with both reference (use-site) and defined (anchor) locations.
    ///
    /// This variant allows reporting both where an alias is used and where the anchor is defined,
    /// which is useful for errors that occur when deserializing aliased values.
    AliasError {
        msg: String,
        locations: Locations,
    },
    /// Error when parsing robotic and other extensions beyond standard YAML.
    /// (error in extension hook).
    HookError {
        msg: String,
        location: Location,
    },
    /// A YAML budget limit was exceeded.
    Budget {
        breach: BudgetBreach,
        location: Location,
    },
    /// Unexpected I/O error. This may happen only when deserializing from a reader.
    IOError {
        cause: std::io::Error,
    },
    /// The value is targeted to the string field but can be interpreted as a number or boolean.
    /// This error can only happen if no_schema set true.
    QuotingRequired {
        value: String, // sanitized (checked) value that must be quoted
        location: Location,
    },

    /// The target type requires a borrowed string (`&str`), but the value was transformed
    /// during parsing (e.g., through escape processing, line folding, or multi-line normalization)
    /// and cannot be borrowed from the input.
    ///
    /// Use `String` or `Cow<str>` instead of `&str` to handle transformed values.
    CannotBorrowTransformedString {
        /// The reason why the string had to be transformed and cannot be borrowed.
        reason: TransformReason,
        location: Location,
    },

    /// Wrap an error with the full input text, enabling rustc-like snippet rendering.
    WithSnippet {
        /// Cropped source windows used for snippet rendering.
        ///
        /// This intentionally does NOT store the full input text, to avoid retaining
        /// large YAML inputs inside errors.
        regions: Vec<CroppedRegion>,
        crop_radius: usize,
        error: Box<Error>,
    },

    /// Garde validation failure.
    #[cfg(feature = "garde")]
    ValidationError {
        report: garde::Report,
        locations: PathMap,
    },

    /// Garde validation failures (multiple, if multiple validations fail)
    #[cfg(feature = "garde")]
    ValidationErrors {
        errors: Vec<Error>,
    },

    /// Validator validation failure.
    #[cfg(feature = "validator")]
    ValidatorError {
        errors: ValidationErrors,
        locations: PathMap,
    },

    /// Validator validation failures (multiple, if multiple validations fail)
    #[cfg(feature = "validator")]
    ValidatorErrors {
        errors: Vec<Error>,
    },
}

impl Error {
    #[cold]
    #[inline(never)]
    pub(crate) fn with_snippet(self, text: &str, crop_radius: usize) -> Self {
        // Avoid nesting snippet wrappers: keep the innermost error and rebuild the
        // wrapper with freshly cropped source window.
        let inner = match self {
            Error::WithSnippet { error, .. } => *error,
            other => other,
        };

        // Keep snippet coordinates aligned with parsers that ignore a leading UTF-8 BOM.
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);

        fn push_region_for_location(
            regions: &mut Vec<CroppedRegion>,
            text: &str,
            location: &Location,
            mapping: crate::de_snipped::LineMapping,
            crop_radius: usize,
        ) {
            if crop_radius == 0 || *location == Location::UNKNOWN {
                return;
            }
            let (cropped, start_line) =
                crate::de_snipped::crop_source_window(text, location, mapping, crop_radius);
            if cropped.is_empty() {
                return;
            }
            let lines = line_count_including_trailing_empty_line(cropped.as_str());
            let end_line = start_line.saturating_add(lines.saturating_sub(1));
            regions.push(CroppedRegion {
                text: cropped,
                start_line,
                end_line,
            });
        }

        let mut regions: Vec<CroppedRegion> = Vec::new();
        let mapping = crate::de_snipped::LineMapping::Identity;

        // Validation errors may contain multiple independent issue locations; pre-crop
        // one region per issue so we can later pick the region that covers the issue.
        #[cfg(feature = "garde")]
        if let Error::ValidationError { report, locations } = &inner {
            for (path, _entry) in report.iter() {
                let key = path_key_from_garde(path);
                let (locs, _) = locations
                    .search(&key)
                    .unwrap_or((Locations::UNKNOWN, String::new()));
                push_region_for_location(
                    &mut regions,
                    text,
                    &locs.reference_location,
                    mapping,
                    crop_radius,
                );
                if locs.defined_location != locs.reference_location {
                    push_region_for_location(
                        &mut regions,
                        text,
                        &locs.defined_location,
                        mapping,
                        crop_radius,
                    );
                }
            }
        }
        #[cfg(feature = "validator")]
        if let Error::ValidatorError { errors, locations } = &inner {
            for issue in collect_validator_issues(errors) {
                let (locs, _) = locations
                    .search(&issue.path)
                    .unwrap_or((Locations::UNKNOWN, String::new()));
                push_region_for_location(
                    &mut regions,
                    text,
                    &locs.reference_location,
                    mapping,
                    crop_radius,
                );
                if locs.defined_location != locs.reference_location {
                    push_region_for_location(
                        &mut regions,
                        text,
                        &locs.defined_location,
                        mapping,
                        crop_radius,
                    );
                }
            }
        }

        // Fallback: crop around the top-level error locations (including dual-location
        // errors such as AliasError).
        if regions.is_empty() {
            if let Some(locs) = inner.locations() {
                push_region_for_location(
                    &mut regions,
                    text,
                    &locs.reference_location,
                    mapping,
                    crop_radius,
                );
                if locs.defined_location != locs.reference_location {
                    push_region_for_location(
                        &mut regions,
                        text,
                        &locs.defined_location,
                        mapping,
                        crop_radius,
                    );
                }
            } else if let Some(loc) = inner.location() {
                push_region_for_location(&mut regions, text, &loc, mapping, crop_radius);
            }
        }

        Error::WithSnippet {
            regions,
            crop_radius,
            error: Box::new(inner),
        }
    }

    /// Attach a snippet from a partial YAML fragment (e.g., from `RingReader`).
    ///
    /// This is similar to `with_snippet`, but the `text` is a fragment that starts
    /// at `start_line` (1-based) rather than at line 1. The renderer will adjust
    /// line numbers accordingly.
    #[cold]
    #[inline(never)]
    pub(crate) fn with_snippet_offset(
        self,
        text: &str,
        start_line: usize,
        crop_radius: usize,
    ) -> Self {
        let inner = match self {
            Error::WithSnippet { error, .. } => *error,
            other => other,
        };

        // Keep snippet coordinates aligned with parsers that ignore a leading UTF-8 BOM.
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);

        fn push_region_for_location(
            regions: &mut Vec<CroppedRegion>,
            text: &str,
            location: &Location,
            mapping: crate::de_snipped::LineMapping,
            crop_radius: usize,
        ) {
            if crop_radius == 0 || *location == Location::UNKNOWN {
                return;
            }
            let (cropped, region_start_line) =
                crate::de_snipped::crop_source_window(text, location, mapping, crop_radius);
            if cropped.is_empty() {
                return;
            }
            let lines = line_count_including_trailing_empty_line(cropped.as_str());
            let end_line = region_start_line.saturating_add(lines.saturating_sub(1));
            regions.push(CroppedRegion {
                text: cropped,
                start_line: region_start_line,
                end_line,
            });
        }

        let mut regions: Vec<CroppedRegion> = Vec::new();
        let mapping = crate::de_snipped::LineMapping::Offset { start_line };

        #[cfg(feature = "garde")]
        if let Error::ValidationError { report, locations } = &inner {
            for (path, _entry) in report.iter() {
                let key = path_key_from_garde(path);
                let (locs, _) = locations
                    .search(&key)
                    .unwrap_or((Locations::UNKNOWN, String::new()));
                push_region_for_location(
                    &mut regions,
                    text,
                    &locs.reference_location,
                    mapping,
                    crop_radius,
                );
                if locs.defined_location != locs.reference_location {
                    push_region_for_location(
                        &mut regions,
                        text,
                        &locs.defined_location,
                        mapping,
                        crop_radius,
                    );
                }
            }
        }
        #[cfg(feature = "validator")]
        if let Error::ValidatorError { errors, locations } = &inner {
            for issue in collect_validator_issues(errors) {
                let (locs, _) = locations
                    .search(&issue.path)
                    .unwrap_or((Locations::UNKNOWN, String::new()));
                push_region_for_location(
                    &mut regions,
                    text,
                    &locs.reference_location,
                    mapping,
                    crop_radius,
                );
                if locs.defined_location != locs.reference_location {
                    push_region_for_location(
                        &mut regions,
                        text,
                        &locs.defined_location,
                        mapping,
                        crop_radius,
                    );
                }
            }
        }

        if regions.is_empty() {
            if let Some(locs) = inner.locations() {
                push_region_for_location(
                    &mut regions,
                    text,
                    &locs.reference_location,
                    mapping,
                    crop_radius,
                );
                if locs.defined_location != locs.reference_location {
                    push_region_for_location(
                        &mut regions,
                        text,
                        &locs.defined_location,
                        mapping,
                        crop_radius,
                    );
                }
            } else if let Some(loc) = inner.location() {
                push_region_for_location(&mut regions, text, &loc, mapping, crop_radius);
            }
        }

        Error::WithSnippet {
            regions,
            crop_radius,
            error: Box::new(inner),
        }
    }

    /// Provide "no snippet" version for cases when snippet rendering is not  desired.
    pub fn without_snippet(&self) -> &Self {
        match self {
            Error::WithSnippet { error, .. } => error,
            other => other,
        }
    }

    /// Render this error using the built-in developer formatter.
    ///
    /// This is the deferred-rendering entrypoint. It is equivalent to `Display`/`to_string()`
    /// output, but also allows callers to choose a custom [`MessageFormatter`] via
    /// [`Error::render_with_options`].
    pub fn render(&self) -> String {
        self.render_with_options(RenderOptions::default())
    }

    /// Render this error using a custom message formatter.
    pub fn render_with_formatter(&self, formatter: &dyn MessageFormatter) -> String {
        self.render_with_options(RenderOptions {
            formatter,
            snippets: SnippetMode::Auto,
        })
    }

    /// Render this error using the provided options.
    pub fn render_with_options(&self, options: RenderOptions<'_>) -> String {
        struct RenderDisplay<'a> {
            err: &'a Error,
            options: RenderOptions<'a>,
        }

        impl fmt::Display for RenderDisplay<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt_error_rendered(f, self.err, self.options)
            }
        }

        RenderDisplay { err: self, options }.to_string()
    }

    /// Construct a `Message` error with no known location.
    ///
    /// Arguments:
    /// - `s`: human-readable message.
    ///
    /// Returns:
    /// - `Error::Message` pointing at [`Location::UNKNOWN`].
    ///
    /// Called by:
    /// - Scalar parsers and helpers throughout this module.
    #[cold]
    #[inline(never)]
    pub(crate) fn msg<S: Into<String>>(s: S) -> Self {
        Error::Message {
            msg: s.into(),
            location: Location::UNKNOWN,
        }
    }

    /// Construct a `QuotingRequired` error with no known location.
    /// Called by:
    /// - Deserializer, when deserializing into string if no_schema set to true.
    #[cold]
    #[inline(never)]
    pub(crate) fn quoting_required(value: &str) -> Self {
        // Ensure the value really is like number or boolean (do not reflect back content
        // that may be used for attack)
        let location = Location::UNKNOWN;
        let value = if parse_yaml12_float::<f64>(value, location, SfTag::None, false).is_ok()
            || parse_int_signed::<i128>(value, "i128", location, false).is_ok()
            || parse_yaml11_bool(value).is_ok()
            || scalar_is_nullish(value, &ScalarStyle::Plain)
        {
            value.to_string()
        } else {
            String::new()
        };
        Error::QuotingRequired { value, location }
    }

    /// Convenience for an `Unexpected` error pre-filled with a human phrase.
    ///
    /// Arguments:
    /// - `what`: short description like "sequence start".
    ///
    /// Returns:
    /// - `Error::Unexpected` at unknown location.
    ///
    /// Called by:
    /// - Deserializer methods that validate the next event kind.
    #[cold]
    #[inline(never)]
    pub(crate) fn unexpected(what: &'static str) -> Self {
        Error::Unexpected {
            expected: what,
            location: Location::UNKNOWN,
        }
    }

    /// Construct an unexpected end-of-input error with unknown location.
    ///
    /// Used by:
    /// - Lookahead and pull methods when `None` appears prematurely.
    #[cold]
    #[inline(never)]
    pub(crate) fn eof() -> Self {
        Error::Eof {
            location: Location::UNKNOWN,
        }
    }

    #[cold]
    #[inline(never)]
    pub(crate) fn multiple_documents(hint: &'static str) -> Self {
        Error::MultipleDocuments {
            hint,
            location: Location::UNKNOWN,
        }
    }

    /// Construct an `UnknownAnchor` error (unknown location).
    ///
    /// Called by:
    /// - Alias replay logic in the live event source.
    #[cold]
    #[inline(never)]
    pub(crate) fn unknown_anchor() -> Self {
        Error::UnknownAnchor {
            location: Location::UNKNOWN,
        }
    }

    /// Construct a `CannotBorrowTransformedString` error for the given reason.
    ///
    /// This error is returned when deserializing to `&str` but the string value
    /// was transformed during parsing and cannot be borrowed from the input.
    #[cold]
    #[inline(never)]
    pub fn cannot_borrow_transformed(reason: TransformReason) -> Self {
        Error::CannotBorrowTransformedString {
            reason,
            location: Location::UNKNOWN,
        }
    }

    /// Attach/override a concrete location to this error and return it.
    ///
    /// Arguments:
    /// - `set_location`: location to store in the error.
    ///
    /// Returns:
    /// - The same `Error` with location updated.
    ///
    /// Called by:
    /// - Most error paths once the event position becomes known.
    #[cold]
    #[inline(never)]
    pub(crate) fn with_location(mut self, set_location: Location) -> Self {
        match &mut self {
            Error::Message { location, .. }
            | Error::ExternalMessage { location, .. }
            | Error::Eof { location }
            | Error::MultipleDocuments { location, .. }
            | Error::Unexpected { location, .. }
            | Error::MergeValueNotMapOrSeqOfMaps { location }
            | Error::InvalidBinaryBase64 { location }
            | Error::BinaryNotUtf8 { location }
            | Error::TaggedScalarCannotDeserializeIntoString { location }
            | Error::UnexpectedSequenceEnd { location }
            | Error::UnexpectedMappingEnd { location }
            | Error::InvalidBooleanStrict { location }
            | Error::InvalidCharNull { location }
            | Error::InvalidCharNotSingleScalar { location }
            | Error::NullIntoString { location }
            | Error::BytesNotSupportedMissingBinaryTag { location }
            | Error::UnexpectedValueForUnit { location }
            | Error::ExpectedEmptyMappingForUnitStruct { location }
            | Error::UnexpectedContainerEndWhileSkippingNode { location }
            | Error::InternalSeedReusedForMapKey { location }
            | Error::ValueRequestedBeforeKey { location }
            | Error::ExpectedStringKeyForExternallyTaggedEnum { location }
            | Error::ExternallyTaggedEnumExpectedScalarOrMapping { location }
            | Error::UnexpectedValueForUnitEnumVariant { location }
            | Error::AliasReplayCounterOverflow { location }
            | Error::AliasReplayLimitExceeded { location, .. }
            | Error::AliasExpansionLimitExceeded { location, .. }
            | Error::AliasReplayStackDepthExceeded { location, .. }
            | Error::FoldedBlockScalarMustIndentContent { location }
            | Error::InternalDepthUnderflow { location }
            | Error::InternalRecursionStackEmpty { location }
            | Error::RecursiveReferencesRequireWeakTypes { location }
            | Error::InvalidScalar { location, .. }
            | Error::SerdeInvalidType { location, .. }
            | Error::SerdeInvalidValue { location, .. }
            | Error::SerdeUnknownVariant { location, .. }
            | Error::SerdeUnknownField { location, .. }
            | Error::SerdeMissingField { location, .. }
            | Error::UnexpectedContainerEndWhileReadingKeyNode { location }
            | Error::DuplicateMappingKey { location, .. }
            | Error::TaggedEnumMismatch { location, .. }
            | Error::SerdeVariantId { location, .. }
            | Error::ExpectedMappingEndAfterEnumVariantValue { location }
            | Error::HookError { location, .. }
            | Error::ContainerEndMismatch { location, .. }
            | Error::UnknownAnchor { location, .. }
            | Error::QuotingRequired { location, .. }
            | Error::Budget { location, .. }
            | Error::CannotBorrowTransformedString { location, .. } => {
                *location = set_location;
            }
            Error::InvalidUtf8Input => {}
            Error::IOError { .. } => {} // this error does not support location
            Error::AliasError { .. } => {
                // AliasError carries its own Locations; don't override with a single location.
            }
            Error::WithSnippet { error, .. } => {
                let inner = *std::mem::replace(error, Box::new(Error::eof()));
                **error = inner.with_location(set_location);
            }
            #[cfg(feature = "garde")]
            Error::ValidationError { .. } => {
                // Validation errors carry their own per-path locations.
            }
            #[cfg(feature = "garde")]
            Error::ValidationErrors { .. } => {
                // Aggregate validation errors carry their own per-entry locations.
            }
            #[cfg(feature = "validator")]
            Error::ValidatorError { .. } => {
                // Validation errors carry their own per-path locations.
            }
            #[cfg(feature = "validator")]
            Error::ValidatorErrors { .. } => {
                // Aggregate validation errors carry their own per-entry locations.
            }
        }
        self
    }

    /// If the error has a known location, return it.
    ///
    /// Returns:
    /// - `Some(Location)` when coordinates are known; `None` otherwise.
    ///
    /// Used by:
    /// - Callers that want to surface precise positions to users.
    pub fn location(&self) -> Option<Location> {
        match self {
            Error::Message { location, .. }
            | Error::ExternalMessage { location, .. }
            | Error::Eof { location }
            | Error::MultipleDocuments { location, .. }
            | Error::Unexpected { location, .. }
            | Error::MergeValueNotMapOrSeqOfMaps { location }
            | Error::InvalidBinaryBase64 { location }
            | Error::BinaryNotUtf8 { location }
            | Error::TaggedScalarCannotDeserializeIntoString { location }
            | Error::UnexpectedSequenceEnd { location }
            | Error::UnexpectedMappingEnd { location }
            | Error::InvalidBooleanStrict { location }
            | Error::InvalidCharNull { location }
            | Error::InvalidCharNotSingleScalar { location }
            | Error::NullIntoString { location }
            | Error::BytesNotSupportedMissingBinaryTag { location }
            | Error::UnexpectedValueForUnit { location }
            | Error::ExpectedEmptyMappingForUnitStruct { location }
            | Error::UnexpectedContainerEndWhileSkippingNode { location }
            | Error::InternalSeedReusedForMapKey { location }
            | Error::ValueRequestedBeforeKey { location }
            | Error::ExpectedStringKeyForExternallyTaggedEnum { location }
            | Error::ExternallyTaggedEnumExpectedScalarOrMapping { location }
            | Error::UnexpectedValueForUnitEnumVariant { location }
            | Error::AliasReplayCounterOverflow { location }
            | Error::AliasReplayLimitExceeded { location, .. }
            | Error::AliasExpansionLimitExceeded { location, .. }
            | Error::AliasReplayStackDepthExceeded { location, .. }
            | Error::FoldedBlockScalarMustIndentContent { location }
            | Error::InternalDepthUnderflow { location }
            | Error::InternalRecursionStackEmpty { location }
            | Error::RecursiveReferencesRequireWeakTypes { location }
            | Error::InvalidScalar { location, .. }
            | Error::SerdeInvalidType { location, .. }
            | Error::SerdeInvalidValue { location, .. }
            | Error::SerdeUnknownVariant { location, .. }
            | Error::SerdeUnknownField { location, .. }
            | Error::SerdeMissingField { location, .. }
            | Error::UnexpectedContainerEndWhileReadingKeyNode { location }
            | Error::DuplicateMappingKey { location, .. }
            | Error::TaggedEnumMismatch { location, .. }
            | Error::SerdeVariantId { location, .. }
            | Error::ExpectedMappingEndAfterEnumVariantValue { location }
            | Error::HookError { location, .. }
            | Error::ContainerEndMismatch { location, .. }
            | Error::UnknownAnchor { location, .. }
            | Error::QuotingRequired { location, .. }
            | Error::Budget { location, .. }
            | Error::CannotBorrowTransformedString { location, .. } => {
                if location != &Location::UNKNOWN {
                    Some(*location)
                } else {
                    None
                }
            }
            Error::InvalidUtf8Input => None,
            Error::IOError { cause: _ } => None,
            Error::AliasError { locations, .. } => Locations::primary_location(*locations),
            Error::WithSnippet { error, .. } => error.location(),
            #[cfg(feature = "garde")]
            Error::ValidationError { report, locations } => {
                report.iter().next().and_then(|(path, _)| {
                    let key = path_key_from_garde(path);
                    let (locs, _) = locations.search(&key)?;
                    let loc = if locs.reference_location != Location::UNKNOWN {
                        locs.reference_location
                    } else {
                        locs.defined_location
                    };
                    if loc != Location::UNKNOWN {
                        Some(loc)
                    } else {
                        None
                    }
                })
            }
            #[cfg(feature = "garde")]
            Error::ValidationErrors { errors } => errors.iter().find_map(|e| e.location()),
            #[cfg(feature = "validator")]
            Error::ValidatorError { errors, locations } => {
                collect_validator_issues(errors).first().and_then(|issue| {
                    let (locs, _) = locations.search(&issue.path)?;
                    let loc = if locs.reference_location != Location::UNKNOWN {
                        locs.reference_location
                    } else {
                        locs.defined_location
                    };
                    if loc != Location::UNKNOWN {
                        Some(loc)
                    } else {
                        None
                    }
                })
            }
            #[cfg(feature = "validator")]
            Error::ValidatorErrors { errors } => errors.iter().find_map(|e| e.location()),
        }
    }

    /// Return a pair of locations associated with this error.
    ///
    /// - For syntax and other errors that carry a single [`Location`], this returns two
    ///   identical locations.
    /// - For validation errors (when the `garde` / `validator` feature is enabled), this returns
    ///   the `(reference_location, defined_location)` pair for the *first* validation entry.
    ///
    ///   These two locations may differ when YAML anchors/aliases are involved.
    /// - Returns `None` when no meaningful location information is available.
    pub fn locations(&self) -> Option<Locations> {
        match self {
            Error::Message { location, .. }
            | Error::ExternalMessage { location, .. }
            | Error::Eof { location }
            | Error::MultipleDocuments { location, .. }
            | Error::Unexpected { location, .. }
            | Error::MergeValueNotMapOrSeqOfMaps { location }
            | Error::InvalidBinaryBase64 { location }
            | Error::BinaryNotUtf8 { location }
            | Error::TaggedScalarCannotDeserializeIntoString { location }
            | Error::UnexpectedSequenceEnd { location }
            | Error::UnexpectedMappingEnd { location }
            | Error::InvalidBooleanStrict { location }
            | Error::InvalidCharNull { location }
            | Error::InvalidCharNotSingleScalar { location }
            | Error::NullIntoString { location }
            | Error::BytesNotSupportedMissingBinaryTag { location }
            | Error::UnexpectedValueForUnit { location }
            | Error::ExpectedEmptyMappingForUnitStruct { location }
            | Error::UnexpectedContainerEndWhileSkippingNode { location }
            | Error::InternalSeedReusedForMapKey { location }
            | Error::ValueRequestedBeforeKey { location }
            | Error::ExpectedStringKeyForExternallyTaggedEnum { location }
            | Error::ExternallyTaggedEnumExpectedScalarOrMapping { location }
            | Error::UnexpectedValueForUnitEnumVariant { location }
            | Error::AliasReplayCounterOverflow { location }
            | Error::AliasReplayLimitExceeded { location, .. }
            | Error::AliasExpansionLimitExceeded { location, .. }
            | Error::AliasReplayStackDepthExceeded { location, .. }
            | Error::FoldedBlockScalarMustIndentContent { location }
            | Error::InternalDepthUnderflow { location }
            | Error::InternalRecursionStackEmpty { location }
            | Error::RecursiveReferencesRequireWeakTypes { location }
            | Error::InvalidScalar { location, .. }
            | Error::SerdeInvalidType { location, .. }
            | Error::SerdeInvalidValue { location, .. }
            | Error::SerdeUnknownVariant { location, .. }
            | Error::SerdeUnknownField { location, .. }
            | Error::SerdeMissingField { location, .. }
            | Error::UnexpectedContainerEndWhileReadingKeyNode { location }
            | Error::DuplicateMappingKey { location, .. }
            | Error::TaggedEnumMismatch { location, .. }
            | Error::SerdeVariantId { location, .. }
            | Error::ExpectedMappingEndAfterEnumVariantValue { location }
            | Error::HookError { location, .. }
            | Error::ContainerEndMismatch { location, .. }
            | Error::UnknownAnchor { location, .. }
            | Error::QuotingRequired { location, .. }
            | Error::Budget { location, .. }
            | Error::CannotBorrowTransformedString { location, .. } => Locations::same(location),
            Error::InvalidUtf8Input => None,
            Error::IOError { .. } => None,
            Error::AliasError { locations, .. } => Some(*locations),
            Error::WithSnippet { error, .. } => error.locations(),
            #[cfg(feature = "garde")]
            Error::ValidationError { report, locations } => {
                report.iter().next().and_then(|(path, _)| {
                    let key = path_key_from_garde(path);
                    search_locations_with_ancestor_fallback(locations, &key)
                })
            }
            #[cfg(feature = "garde")]
            Error::ValidationErrors { errors } => errors.first().and_then(Error::locations),
            #[cfg(feature = "validator")]
            Error::ValidatorError { errors, locations } => collect_validator_issues(errors)
                .first()
                .and_then(|issue| locations.search(&issue.path).map(|(locs, _)| locs)),
            #[cfg(feature = "validator")]
            Error::ValidatorErrors { errors } => errors.first().and_then(Error::locations),
        }
    }

    /// Map a `saphyr_parser::ScanError` into our error type with location.
    ///
    /// Called by:
    /// - The live events adapter when the underlying parser fails.
    #[cold]
    #[inline(never)]
    pub(crate) fn from_scan_error(err: ScanError) -> Self {
        use crate::location::SpanIndex;
        let mark = err.marker();
        let location =
            Location::new(mark.line(), mark.col() + 1).with_span(crate::location::Span {
                offset: mark.index() as SpanIndex,
                len: 1,
                byte_info: (0, 0),
            });

        // `saphyr_parser` reports missing aliases/anchors as a `ScanError` with a textual
        // message (e.g. "unknown anchor"). To keep our formatter overrides working for the
        // common real-world case (`*missing`), detect this and convert to our structured
        // `Error::UnknownAnchor`.
        //
        // Note: the parser message usually contains an anchor *name*. We intentionally do not
        // attempt to parse or store it, to keep this variant free of best-effort identifiers.
        let info = err.info();
        if info.to_ascii_lowercase().contains("unknown anchor") {
            return Error::UnknownAnchor { location };
        }

        Error::ExternalMessage {
            source: ExternalMessageSource::SaphyrParser,
            msg: info.to_owned(),
            code: None,
            params: Vec::new(),
            location,
        }
    }
}

fn fmt_error_plain_with_formatter(
    f: &mut fmt::Formatter<'_>,
    err: &Error,
    formatter: &dyn MessageFormatter,
) -> fmt::Result {
    let err = err.without_snippet();

    let msg = formatter.format_message(err);

    // Validation errors embed per-issue locations in their formatted message (potentially
    // multiple distinct locations). Do not attach a single top-level location suffix here,
    // or we'd duplicate location wording.
    #[cfg(feature = "garde")]
    if matches!(err, Error::ValidationError { .. }) {
        return write!(f, "{msg}");
    }
    #[cfg(feature = "validator")]
    if matches!(err, Error::ValidatorError { .. }) {
        return write!(f, "{msg}");
    }

    if let Some(loc) = err.location() {
        fmt_with_location(f, formatter.localizer(), msg.as_ref(), &loc)?;
    } else {
        write!(f, "{msg}")?;
    }

    #[cfg(feature = "garde")]
    if let Error::ValidationErrors { errors } = err {
        for err in errors {
            writeln!(f)?;
            writeln!(f)?;
            fmt_error_plain_with_formatter(f, err, formatter)?;
        }
    }

    #[cfg(feature = "validator")]
    if let Error::ValidatorErrors { errors } = err {
        for err in errors {
            writeln!(f)?;
            writeln!(f)?;
            fmt_error_plain_with_formatter(f, err, formatter)?;
        }
    }

    Ok(())
}

fn pick_cropped_region<'a>(
    regions: &'a [CroppedRegion],
    location: &Location,
) -> Option<&'a CroppedRegion> {
    regions
        .iter()
        .find(|r| r.covers(location))
        .or_else(|| regions.first())
}

fn fmt_error_rendered(
    f: &mut fmt::Formatter<'_>,
    err: &Error,
    options: RenderOptions<'_>,
) -> fmt::Result {
    if options.snippets == SnippetMode::Off {
        return fmt_error_plain_with_formatter(f, err, options.formatter);
    }

    match err {
        #[cfg(feature = "garde")]
        Error::ValidationErrors { errors } => {
            let msg = options.formatter.format_message(err);
            if !msg.is_empty() {
                writeln!(f, "{}", msg)?;
            }
            let mut first = true;
            for err in errors {
                if !first {
                    writeln!(f)?;
                    writeln!(f)?;
                }
                first = false;
                fmt_error_rendered(f, err, options)?;
            }
            Ok(())
        }

        #[cfg(feature = "validator")]
        Error::ValidatorErrors { errors } => {
            let msg = options.formatter.format_message(err);
            if !msg.is_empty() {
                writeln!(f, "{}", msg)?;
            }
            let mut first = true;
            for err in errors {
                if !first {
                    writeln!(f)?;
                    writeln!(f)?;
                }
                first = false;
                fmt_error_rendered(f, err, options)?;
            }
            Ok(())
        }

        Error::WithSnippet {
            regions,
            crop_radius,
            error,
        } => {
            if *crop_radius == 0 {
                // Treat as "snippet disabled".
                return fmt_error_plain_with_formatter(f, error, options.formatter);
            }

            if regions.is_empty() {
                return fmt_error_plain_with_formatter(f, error, options.formatter);
            }

            // Validation errors have custom snippet formatting (paths, alias context, and
            // messages without location duplication).
            #[cfg(feature = "garde")]
            if let Error::ValidationError { report, locations } = error.as_ref() {
                return fmt_validation_error_with_snippets_offset(
                    f,
                    options.formatter.localizer(),
                    report,
                    locations,
                    regions,
                    *crop_radius,
                );
            }
            #[cfg(feature = "garde")]
            if let Error::ValidationErrors { errors } = error.as_ref() {
                let msg = options.formatter.format_message(error);
                if !msg.is_empty() {
                    writeln!(f, "{}", msg)?;
                }
                let mut first = true;
                for err in errors {
                    if !first {
                        writeln!(f)?;
                        writeln!(f)?;
                    }
                    first = false;
                    fmt_error_with_snippets_offset(
                        f,
                        err,
                        regions,
                        *crop_radius,
                        options.formatter,
                    )?;
                }
                return Ok(());
            }

            #[cfg(feature = "validator")]
            if let Error::ValidatorError { errors, locations } = error.as_ref() {
                return fmt_validator_error_with_snippets_offset(
                    f,
                    options.formatter.localizer(),
                    errors,
                    locations,
                    regions,
                    *crop_radius,
                );
            }
            #[cfg(feature = "validator")]
            if let Error::ValidatorErrors { errors } = error.as_ref() {
                let msg = options.formatter.format_message(error);
                if !msg.is_empty() {
                    writeln!(f, "{}", msg)?;
                }
                let mut first = true;
                for err in errors {
                    if !first {
                        writeln!(f)?;
                        writeln!(f)?;
                    }
                    first = false;
                    fmt_error_with_snippets_offset(
                        f,
                        err,
                        regions,
                        *crop_radius,
                        options.formatter,
                    )?;
                }
                return Ok(());
            }

            // Render a snippet from the cropped source window. If anything is missing,
            // fall back to the plain nested error.
            let Some(location) = error.location() else {
                return fmt_error_plain_with_formatter(f, error, options.formatter);
            };
            if location == Location::UNKNOWN {
                return fmt_error_plain_with_formatter(f, error, options.formatter);
            }

            let l10n = options.formatter.localizer();

            let region = match pick_cropped_region(regions, &location) {
                Some(r) => r,
                None => return fmt_error_plain_with_formatter(f, error, options.formatter),
            };

            // Dual-location rendering: show both the reference and the definition window.
            let dual_locations = error.locations().filter(|locs| {
                locs.reference_location != Location::UNKNOWN
                    && locs.defined_location != Location::UNKNOWN
                    && locs.reference_location != locs.defined_location
            });

            let mut msg = options.formatter.format_message(error);

            // Renderer-level de-duplication for AliasError:
            // when we are about to show a secondary “defined here” window, drop the
            // default message suffix " (defined at …)" if present.
            if dual_locations.is_some()
                && let Error::AliasError { locations, .. } = error.as_ref()
            {
                let suffix = l10n.alias_defined_at(locations.defined_location);
                if let Some(stripped) = msg.as_ref().strip_suffix(&suffix) {
                    msg = Cow::Owned(stripped.to_string());
                }
            }

            if let Some(locs) = dual_locations {
                let ref_loc = locs.reference_location;
                let def_loc = locs.defined_location;

                let used_region = pick_cropped_region(regions, &ref_loc).unwrap_or(region);
                let label = l10n.value_used_here();
                let ctx = crate::de_snipped::Snippet::new(
                    used_region.text.as_str(),
                    label.as_ref(),
                    *crop_radius,
                )
                .with_offset(used_region.start_line);
                ctx.fmt_or_fallback(f, Level::ERROR, l10n, msg.as_ref(), &ref_loc)?;

                let def_region = pick_cropped_region(regions, &def_loc).unwrap_or(region);
                writeln!(f)?;
                writeln!(f, "{}", l10n.value_comes_from_the_anchor(def_loc))?;
                fmt_snippet_window_offset_or_fallback(
                    f,
                    l10n,
                    &def_loc,
                    def_region.text.as_str(),
                    def_region.start_line,
                    l10n.defined_window().as_ref(),
                    *crop_radius,
                )?;
                Ok(())
            } else {
                // Single location rendering.
                let ctx =
                    crate::de_snipped::Snippet::new(region.text.as_str(), "<input>", *crop_radius)
                        .with_offset(region.start_line);
                ctx.fmt_or_fallback(f, Level::ERROR, l10n, msg.as_ref(), &location)
            }
        }
        _ => fmt_error_plain_with_formatter(f, err, options.formatter),
    }
}

#[cfg(any(feature = "garde", feature = "validator"))]
fn search_locations_with_ancestor_fallback(
    locations: &PathMap,
    path: &PathKey,
) -> Option<Locations> {
    if let Some((locs, _)) = locations.search(path) {
        return Some(locs);
    }

    let mut p = path.parent();
    while let Some(cur) = p {
        if let Some((locs, _)) = locations.search(&cur) {
            return Some(locs);
        }
        p = cur.parent();
    }

    None
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_error_rendered(f, self, RenderOptions::default())
    }
}

#[cfg(feature = "garde")]
fn fmt_validation_error_with_snippets_offset(
    f: &mut fmt::Formatter<'_>,
    l10n: &dyn Localizer,
    report: &garde::Report,
    locations: &PathMap,
    regions: &[CroppedRegion],
    crop_radius: usize,
) -> fmt::Result {
    let mut first = true;
    for (path, entry) in report.iter() {
        if !first {
            writeln!(f)?;
        }
        first = false;

        let path_key = path_key_from_garde(path);
        let original_leaf = path_key
            .leaf_string()
            .unwrap_or_else(|| l10n.root_path_label().into_owned());

        let (locs, resolved_leaf) = locations
            .search(&path_key)
            .unwrap_or((Locations::UNKNOWN, original_leaf));

        let ref_loc = locs.reference_location;
        let def_loc = locs.defined_location;

        let resolved_path = format_path_with_resolved_leaf(&path_key, &resolved_leaf);
        let entry_raw = entry.to_string();
        let entry = l10n
            .override_external_message(ExternalMessage {
                source: ExternalMessageSource::Garde,
                original: entry_raw.as_str(),
                code: None,
                params: &[],
            })
            .unwrap_or(Cow::Borrowed(entry_raw.as_str()));
        let base_msg = l10n.validation_base_message(entry.as_ref(), &resolved_path);

        match (ref_loc, def_loc) {
            (Location::UNKNOWN, Location::UNKNOWN) => {
                write!(f, "{base_msg}")?;
            }
            (r, d) if r != Location::UNKNOWN && (d == Location::UNKNOWN || d == r) => {
                let label = l10n.defined();
                if let Some(region) = pick_cropped_region(regions, &r) {
                    let ctx = crate::de_snipped::Snippet::new(
                        region.text.as_str(),
                        label.as_ref(),
                        crop_radius,
                    )
                    .with_offset(region.start_line);
                    ctx.fmt_or_fallback(f, Level::ERROR, l10n, &base_msg, &r)?;
                } else {
                    fmt_with_location(f, l10n, &base_msg, &r)?;
                }
            }
            (r, d) if r == Location::UNKNOWN && d != Location::UNKNOWN => {
                let label = l10n.defined_here();
                if let Some(region) = pick_cropped_region(regions, &d) {
                    let ctx = crate::de_snipped::Snippet::new(
                        region.text.as_str(),
                        label.as_ref(),
                        crop_radius,
                    )
                    .with_offset(region.start_line);
                    ctx.fmt_or_fallback(f, Level::ERROR, l10n, &base_msg, &d)?;
                } else {
                    fmt_with_location(f, l10n, &base_msg, &d)?;
                }
            }
            (r, d) => {
                let label = l10n.value_used_here();
                let invalid_here = l10n.invalid_here(&base_msg);
                if let Some(region) = pick_cropped_region(regions, &r) {
                    let ctx = crate::de_snipped::Snippet::new(
                        region.text.as_str(),
                        label.as_ref(),
                        crop_radius,
                    )
                    .with_offset(region.start_line);
                    ctx.fmt_or_fallback(f, Level::ERROR, l10n, &invalid_here, &r)?;
                } else {
                    fmt_with_location(f, l10n, &invalid_here, &r)?;
                }
                writeln!(f)?;
                writeln!(f, "{}", l10n.value_comes_from_the_anchor(d))?;
                if let Some(region) = pick_cropped_region(regions, &d) {
                    crate::de_snipped::fmt_snippet_window_offset_or_fallback(
                        f,
                        l10n,
                        &d,
                        region.text.as_str(),
                        region.start_line,
                        l10n.defined_window().as_ref(),
                        crop_radius,
                    )?;
                } else {
                    fmt_with_location(f, l10n, l10n.defined_window().as_ref(), &d)?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(feature = "validator")]
fn fmt_validator_error_with_snippets_offset(
    f: &mut fmt::Formatter<'_>,
    l10n: &dyn Localizer,
    errors: &ValidationErrors,
    locations: &PathMap,
    regions: &[CroppedRegion],
    crop_radius: usize,
) -> fmt::Result {
    let entries = collect_validator_issues(errors);
    let mut first = true;

    for issue in entries {
        if !first {
            writeln!(f)?;
        }
        first = false;

        let original_leaf = issue
            .path
            .leaf_string()
            .unwrap_or_else(|| l10n.root_path_label().into_owned());
        let (locs, resolved_leaf) = locations
            .search(&issue.path)
            .unwrap_or((Locations::UNKNOWN, original_leaf));

        let resolved_path = format_path_with_resolved_leaf(&issue.path, &resolved_leaf);
        let entry = issue.display_entry_overridden(l10n, ExternalMessageSource::Validator);
        let base_msg = l10n.validation_base_message(&entry, &resolved_path);

        match (locs.reference_location, locs.defined_location) {
            (Location::UNKNOWN, Location::UNKNOWN) => {
                write!(f, "{base_msg}")?;
            }
            (r, d) if r != Location::UNKNOWN && (d == Location::UNKNOWN || d == r) => {
                let label = l10n.defined();
                if let Some(region) = pick_cropped_region(regions, &r) {
                    let ctx = crate::de_snipped::Snippet::new(
                        region.text.as_str(),
                        label.as_ref(),
                        crop_radius,
                    )
                    .with_offset(region.start_line);
                    ctx.fmt_or_fallback(f, Level::ERROR, l10n, &base_msg, &r)?;
                } else {
                    fmt_with_location(f, l10n, &base_msg, &r)?;
                }
            }
            (r, d) if r == Location::UNKNOWN && d != Location::UNKNOWN => {
                let label = l10n.defined_here();
                if let Some(region) = pick_cropped_region(regions, &d) {
                    let ctx = crate::de_snipped::Snippet::new(
                        region.text.as_str(),
                        label.as_ref(),
                        crop_radius,
                    )
                    .with_offset(region.start_line);
                    ctx.fmt_or_fallback(f, Level::ERROR, l10n, &base_msg, &d)?;
                } else {
                    fmt_with_location(f, l10n, &base_msg, &d)?;
                }
            }
            (r, d) => {
                let label = l10n.value_used_here();
                let invalid_here = l10n.invalid_here(&base_msg);
                if let Some(region) = pick_cropped_region(regions, &r) {
                    let ctx = crate::de_snipped::Snippet::new(
                        region.text.as_str(),
                        label.as_ref(),
                        crop_radius,
                    )
                    .with_offset(region.start_line);
                    ctx.fmt_or_fallback(f, Level::ERROR, l10n, &invalid_here, &r)?;
                } else {
                    fmt_with_location(f, l10n, &invalid_here, &r)?;
                }
                writeln!(f)?;
                writeln!(f, "{}", l10n.value_comes_from_the_anchor(d))?;
                if let Some(region) = pick_cropped_region(regions, &d) {
                    crate::de_snipped::fmt_snippet_window_offset_or_fallback(
                        f,
                        l10n,
                        &d,
                        region.text.as_str(),
                        region.start_line,
                        l10n.defined_window().as_ref(),
                        crop_radius,
                    )?;
                } else {
                    fmt_with_location(f, l10n, l10n.defined_window().as_ref(), &d)?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(any(feature = "garde", feature = "validator"))]
fn fmt_error_with_snippets_offset(
    f: &mut fmt::Formatter<'_>,
    err: &Error,
    regions: &[CroppedRegion],
    crop_radius: usize,
    formatter: &dyn MessageFormatter,
) -> fmt::Result {
    if crop_radius == 0 {
        return fmt_error_plain_with_formatter(f, err, formatter);
    }

    // Keep existing snippet output if the nested error is already wrapped.
    if let Error::WithSnippet { .. } = err {
        return fmt_error_rendered(f, err, RenderOptions::new(formatter));
    }

    #[cfg(feature = "garde")]
    if let Error::ValidationError { report, locations } = err {
        return fmt_validation_error_with_snippets_offset(
            f,
            formatter.localizer(),
            report,
            locations,
            regions,
            crop_radius,
        );
    }

    #[cfg(feature = "validator")]
    if let Error::ValidatorError { errors, locations } = err {
        return fmt_validator_error_with_snippets_offset(
            f,
            formatter.localizer(),
            errors,
            locations,
            regions,
            crop_radius,
        );
    }

    let msg = formatter.format_message(err);
    let Some(location) = err.location() else {
        return write!(f, "{msg}");
    };
    if location == Location::UNKNOWN {
        return write!(f, "{msg}");
    }

    let Some(region) = pick_cropped_region(regions, &location) else {
        return fmt_with_location(f, formatter.localizer(), msg.as_ref(), &location);
    };
    let ctx = crate::de_snipped::Snippet::new(region.text.as_str(), "<input>", crop_radius)
        .with_offset(region.start_line);
    ctx.fmt_or_fallback(
        f,
        Level::ERROR,
        formatter.localizer(),
        msg.as_ref(),
        &location,
    )
}

#[cfg(feature = "validator")]
pub(crate) fn collect_validator_issues(errors: &ValidationErrors) -> Vec<ValidationIssue> {
    let mut out = Vec::new();
    let root = PathKey::empty();
    collect_validator_issues_inner(errors, &root, &mut out);
    out
}

#[cfg(feature = "validator")]
fn collect_validator_issues_inner(
    errors: &ValidationErrors,
    path: &PathKey,
    out: &mut Vec<ValidationIssue>,
) {
    for (field, kind) in errors.errors() {
        let field_path = path.clone().join(field.as_ref());
        match kind {
            ValidationErrorsKind::Field(entries) => {
                for entry in entries {
                    let mut params = Vec::new();
                    for (k, v) in &entry.params {
                        params.push((k.to_string(), v.to_string()));
                    }

                    out.push(ValidationIssue {
                        path: field_path.clone(),
                        code: entry.code.to_string(),
                        message: entry.message.as_ref().map(|m| m.to_string()),
                        params,
                    });
                }
            }
            ValidationErrorsKind::Struct(inner) => {
                collect_validator_issues_inner(inner, &field_path, out);
            }
            ValidationErrorsKind::List(list) => {
                for (idx, inner) in list {
                    let index_path = field_path.clone().join(*idx);
                    collect_validator_issues_inner(inner, &index_path, out);
                }
            }
        }
    }
}

#[cfg(feature = "garde")]
pub(crate) fn collect_garde_issues(report: &garde::Report) -> Vec<ValidationIssue> {
    let mut out = Vec::new();
    for (path, entry) in report.iter() {
        out.push(ValidationIssue {
            path: path_key_from_garde(path),
            code: "garde".to_string(),
            message: Some(entry.message().to_string()),
            params: Vec::new(),
        });
    }
    out
}
impl std::error::Error for Error {}

/// Attach the current [`MISSING_FIELD_FALLBACK`] location to `err`, if available.
fn maybe_attach_fallback_location(mut err: Error) -> Error {
    let loc = MISSING_FIELD_FALLBACK.with(|c| c.get());
    if let Some(loc) = loc
        && loc != Location::UNKNOWN
    {
        err = err.with_location(loc);
    }
    err
}

impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        // Keep custom errors locationless by default; the deserializer should attach an explicit
        // location when it can. For Serde-generated errors, we override the relevant hooks below
        // and attach a best-effort fallback location.
        Error::msg(msg.to_string())
    }

    fn invalid_type(unexp: de::Unexpected, exp: &dyn de::Expected) -> Self {
        // Mirror serde’s default formatting, but add a best-effort location.
        maybe_attach_fallback_location(Error::SerdeInvalidType {
            unexpected: unexp.to_string(),
            expected: exp.to_string(),
            location: Location::UNKNOWN,
        })
    }

    fn invalid_value(unexp: de::Unexpected, exp: &dyn de::Expected) -> Self {
        maybe_attach_fallback_location(Error::SerdeInvalidValue {
            unexpected: unexp.to_string(),
            expected: exp.to_string(),
            location: Location::UNKNOWN,
        })
    }

    fn unknown_variant(variant: &str, expected: &'static [&'static str]) -> Self {
        maybe_attach_fallback_location(Error::SerdeUnknownVariant {
            variant: variant.to_owned(),
            expected: expected.to_vec(),
            location: Location::UNKNOWN,
        })
    }

    fn unknown_field(field: &str, expected: &'static [&'static str]) -> Self {
        maybe_attach_fallback_location(Error::SerdeUnknownField {
            field: field.to_owned(),
            expected: expected.to_vec(),
            location: Location::UNKNOWN,
        })
    }

    fn missing_field(field: &'static str) -> Self {
        maybe_attach_fallback_location(Error::SerdeMissingField {
            field,
            location: Location::UNKNOWN,
        })
    }
}

/// Print a message optionally suffixed with "at line X, column Y".
///
/// Arguments:
/// - `f`: destination formatter.
/// - `msg`: main text.
/// - `location`: position to attach if known.
///
/// Returns:
/// - `fmt::Result` as required by `Display`.
#[cold]
#[inline(never)]
fn fmt_with_location(
    f: &mut fmt::Formatter<'_>,
    l10n: &dyn Localizer,
    msg: &str,
    location: &Location,
) -> fmt::Result {
    let out = l10n.attach_location(Cow::Borrowed(msg), *location);
    write!(f, "{out}")
}

/// Convert a budget breach report into a user-facing error.
///
/// Arguments:
/// - `breach`: which limit was exceeded (from the streaming budget checker).
///
/// Returns:
/// - `Error::Message` with a formatted description.
///
/// Called by:
/// - The live events layer when enforcing budgets during/after parsing.
#[cold]
#[inline(never)]
pub(crate) fn budget_error(breach: BudgetBreach) -> Error {
    Error::Budget {
        breach,
        location: Location::UNKNOWN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_for_basic_error_duplicates_location() {
        let l = Location::new(3, 7);
        let err = Error::Message {
            msg: "x".to_owned(),
            location: l,
        };
        assert_eq!(
            err.locations(),
            Some(Locations {
                reference_location: l,
                defined_location: l,
            })
        );
    }

    #[test]
    fn locations_for_io_error_is_unknown() {
        let err = Error::IOError {
            cause: std::io::Error::other("x"),
        };
        assert_eq!(err.locations(), None);
    }

    #[test]
    fn alias_error_returns_both_locations() {
        let ref_loc = Location::new(5, 10);
        let def_loc = Location::new(2, 3);
        let err = Error::AliasError {
            msg: "test error".to_owned(),
            locations: Locations {
                reference_location: ref_loc,
                defined_location: def_loc,
            },
        };

        // location() should return the primary (reference) location
        assert_eq!(err.location(), Some(ref_loc));

        // locations() should return both
        assert_eq!(
            err.locations(),
            Some(Locations {
                reference_location: ref_loc,
                defined_location: def_loc,
            })
        );
    }

    #[test]
    fn alias_error_display_shows_both_locations() {
        let ref_loc = Location::new(5, 10);
        let def_loc = Location::new(2, 3);
        let err = Error::AliasError {
            msg: "invalid value".to_owned(),
            locations: Locations {
                reference_location: ref_loc,
                defined_location: def_loc,
            },
        };

        let display = err.to_string();
        assert!(display.contains("invalid value"));
        assert!(display.contains("line 5"));
        assert!(display.contains("column 10"));
        assert!(display.contains("line 2"));
        assert!(display.contains("column 3"));
    }

    #[test]
    fn alias_error_display_with_same_locations() {
        let loc = Location::new(3, 7);
        let err = Error::AliasError {
            msg: "test".to_owned(),
            locations: Locations {
                reference_location: loc,
                defined_location: loc,
            },
        };

        let display = err.to_string();
        // When both locations are the same, should only show one
        assert!(display.contains("line 3"));
        assert!(display.contains("column 7"));
        // Should not contain "defined at" since locations are the same
        assert!(!display.contains("defined at"));
    }

    #[test]
    fn with_snippet_counts_trailing_empty_line_for_end_line() {
        // `"a\n"` has two logical lines: "a" and a trailing empty line.
        let text = "a\n";
        let err = Error::Message {
            msg: "x".to_owned(),
            location: Location::new(2, 1),
        };

        let wrapped = err.with_snippet(text, 50);
        let Error::WithSnippet { regions, .. } = wrapped else {
            panic!("expected WithSnippet wrapper");
        };
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].start_line, 1);
        assert_eq!(regions[0].end_line, 2);
    }

    #[test]
    fn with_snippet_offset_counts_trailing_empty_line_for_end_line() {
        // Fragment starts at line 10, and ends with a newline -> includes empty line 11.
        let text = "a\n";
        let err = Error::Message {
            msg: "x".to_owned(),
            location: Location::new(11, 1),
        };

        let wrapped = err.with_snippet_offset(text, 10, 50);
        let Error::WithSnippet { regions, .. } = wrapped else {
            panic!("expected WithSnippet wrapper");
        };
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].start_line, 10);
        assert_eq!(regions[0].end_line, 11);
    }

    #[cfg(feature = "validator")]
    #[test]
    fn locations_for_validator_error_uses_first_entry() {
        use validator::Validate;

        #[derive(Debug, Validate)]
        struct Cfg {
            #[validate(length(min = 2))]
            second_string: String,
        }

        let cfg = Cfg {
            second_string: "x".to_owned(),
        };
        let errors = cfg.validate().expect_err("validation error expected");

        let referenced_loc = Location::new(3, 15);
        let defined_loc = Location::new(2, 18);

        let mut locations = PathMap::new();
        locations.insert(
            PathKey::empty().join("secondString"),
            Locations {
                reference_location: referenced_loc,
                defined_location: defined_loc,
            },
        );

        let err = Error::ValidatorError { errors, locations };
        assert_eq!(
            err.locations(),
            Some(Locations {
                reference_location: referenced_loc,
                defined_location: defined_loc,
            })
        );
    }

    #[test]
    fn nested_snippet_preserves_custom_formatter() {
        struct Custom;
        impl MessageFormatter for Custom {
            fn localizer(&self) -> &dyn Localizer {
                &DEFAULT_ENGLISH_LOCALIZER
            }
            fn format_message<'a>(&self, err: &'a Error) -> Cow<'a, str> {
                match err {
                    Error::Message { msg, .. } => Cow::Owned(format!("CUSTOM: {}", msg.as_str())),
                    _ => Cow::Borrowed(""),
                }
            }
        }
        let loc = Location::new(1, 1);
        let base = Error::Message {
            msg: "original".to_string(),
            location: loc,
        };
        let text = "input";
        let start_line = 1;
        let radius = 1;
        let inner = base.with_snippet_offset(text, start_line, radius);
        let outer = inner.with_snippet_offset(text, start_line, radius);
        let rendered = outer.render_with_options(RenderOptions::new(&Custom));
        assert!(rendered.contains("CUSTOM: original"));
    }

    #[test]
    fn alias_error_dual_snippet_rendering() {
        // YAML with anchor on line 2 and alias usage on line 5
        let yaml = r#"config:
  anchor: &myval 42
  other: stuff
  more: data
  use_it: *myval
"#;
        // Reference location: line 5, column 11 (where *myval is used)
        let ref_loc = Location::new(5, 11);
        // Defined location: line 2, column 11 (where &myval is defined)
        let def_loc = Location::new(2, 11);

        let err = Error::AliasError {
            msg: "invalid value type".to_owned(),
            locations: Locations {
                reference_location: ref_loc,
                defined_location: def_loc,
            },
        };

        // Wrap with snippet
        let wrapped = err.with_snippet(yaml, 5);
        let rendered = wrapped.render();

        // Should contain the error message
        assert!(
            rendered.contains("invalid value type"),
            "rendered: {}",
            rendered
        );

        // When a secondary snippet window is shown, avoid duplicating the alias
        // "defined at …" suffix in the main message.
        assert!(
            !rendered.contains("(defined at line"),
            "did not expect alias defined-at suffix when secondary window is present: {}",
            rendered
        );
        // Should show "the value is used here" for the reference location
        assert!(
            rendered.contains("the value is used here") || rendered.contains("use_it"),
            "rendered should show reference location context: {}",
            rendered
        );
        // Should show "defined here" for the anchor location
        assert!(
            rendered.contains("defined here") || rendered.contains("anchor"),
            "rendered should show defined location context: {}",
            rendered
        );
        // Should mention both line numbers in some form
        assert!(
            rendered.contains("5") || rendered.contains("use_it"),
            "rendered should reference line 5: {}",
            rendered
        );
        assert!(
            rendered.contains("2") || rendered.contains("anchor"),
            "rendered should reference line 2: {}",
            rendered
        );
    }

    #[test]
    fn alias_error_same_location_single_snippet() {
        let yaml = "value: &anchor 42\n";
        let loc = Location::new(1, 8);

        let err = Error::AliasError {
            msg: "test error".to_owned(),
            locations: Locations {
                reference_location: loc,
                defined_location: loc,
            },
        };

        let wrapped = err.with_snippet(yaml, 5);
        let rendered = wrapped.render();

        // Should contain the error message
        assert!(rendered.contains("test error"), "rendered: {}", rendered);
        // Should NOT show dual-snippet labels when locations are the same
        assert!(
            !rendered.contains("defined here"),
            "should not show 'defined here' when locations are same: {}",
            rendered
        );
        assert!(
            !rendered.contains("the value is used here"),
            "should not show 'value used here' when locations are same: {}",
            rendered
        );
    }
}
