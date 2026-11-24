use crate::budget::Budget;
use crate::indentation::RequireIndent;
#[cfg(feature = "properties")]
use std::collections::HashMap;
#[cfg(feature = "include_fs")]
use std::io;
#[cfg(feature = "include_fs")]
use std::path::Path;
use std::rc::Rc;

#[cfg(feature = "serde_derived_types")]
const fn default_allow_numeric_underscores() -> bool {
    true
}

#[cfg(all(feature = "serde_derived_types", feature = "comments"))]
const fn default_emit_comments() -> bool {
    true
}

#[cfg(feature = "serde_derived_types")]
const fn default_reject_tagged_field_names() -> bool {
    true
}

/// Duplicate key handling policy for mappings.
///
/// Except with [`Self::PassThrough`], YAML integer keys are compared by their
/// parsed integer value, so `0xB` and `11` are duplicates even when the target
/// key type is `String`. The retained key preserves its original spelling when
/// deserialized into a string. Merge expansion always uses YAML key identity.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(
    feature = "serde_derived_types",
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "serde_derived_types", serde(rename_all = "snake_case"))]
pub enum DuplicateKeyPolicy {
    /// Error out on encountering a duplicate key.
    Error,
    /// First key wins: later duplicate pairs are skipped (key+value are consumed and ignored).
    FirstWins,
    /// Last key wins: retain the later value. Struct fields are buffered and
    /// collapsed before Serde sees them. Maps stream until an integer-containing
    /// key requires numeric comparison, then buffer the remaining entries to
    /// retain the last key's original spelling and value.
    LastWins,
    /// Forward repeated explicit mapping entries to the receiving Serde visitor.
    ///
    /// This matches `serde_yaml`'s visitor-driven handling of explicit entries:
    /// derived structs reject repeated fields, standard maps keep the last value,
    /// and custom visitors decide whether to reject, replace, or retain duplicates.
    /// Integer-equivalent keys such as `0xB` and `11` are also forwarded separately.
    ///
    /// Merge expansion still follows [`MergeKeyPolicy`] and suppresses overridden
    /// entries. Repeated keys within a merge source retain the first entry. Use
    /// [`MergeKeyPolicy::AsOrdinary`] to forward literal `<<` entries as well.
    ///
    /// This is a permissive extension: YAML requires unique mapping keys.
    /// [`Self::Error`] remains the default.
    PassThrough,
}

/// Handling of non-finite floats in `deserialize_any`.
///
/// By default, non-finite floats are rejected. This policy applies to inferred values,
/// including Serde's buffering for untagged enums and flattened fields. Direct `f32`/`f64`
/// deserialization is unaffected.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde_derived_types",
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "serde_derived_types", serde(rename_all = "snake_case"))]
pub enum NonFiniteFloatPolicy {
    // Compatibility sentinel: defer to the deprecated boolean until it is removed,
    // then use the unconfigured policy (Reject).
    #[doc(hidden)]
    #[default]
    Default,
    /// Pass NaN and positive/negative infinity to the visitor as `f64` values.
    ///
    /// The receiving visitor determines how to represent them. For example,
    /// `serde_json::Value` converts them to `Null` without returning an error.
    PassThrough,
    /// Return [`crate::Error::NonFiniteFloat`] before calling the visitor.
    Reject,
    /// Pass canonical strings (`.nan`, `.inf`, `-.inf`) to the visitor.
    AsString,
}

/// Syntax and expansion rules for `${NAME}` / `$NAME` property interpolation.
#[cfg(feature = "properties")]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde_derived_types",
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "serde_derived_types", serde(rename_all = "snake_case"))]
pub enum PropertySyntax {
    /// Only the braced `${NAME}` form is interpolated. Bare `$NAME` stays literal.
    #[default]
    Braced,

    /// Both `${NAME}` and the unbraced shorthand `$NAME` are interpolated.
    /// The unbraced form uses Required semantics (missing values error).
    BracedOrBare,

    /// Property interpolation compatible with [Docker Compose]'s syntax and value expansion.
    /// Expanded values remain strings for generic targets. Numeric and boolean targets can
    /// parse expansions from plain scalars; quoted and block scalars, and scalars tagged
    /// with `!!str` or standalone `!`, are rejected for these targets.
    ///
    /// [Docker Compose]: https://docs.docker.com/reference/compose-file/interpolation/
    DockerCompose,

    /// [`DockerCompose`](Self::DockerCompose) interpolation, except that an expanded value
    /// can become YAML null:
    ///
    /// - A value consisting solely of `$NAME` or `${NAME}` is null when `NAME` is unset.
    /// - A value whose expansion is exactly `null`, `Null`, `NULL`, or `~` is null.
    ///
    /// Everything else keeps the Compose behavior: an explicitly empty property and an empty
    /// selected default (`${A:-${B}}` with both unset) are empty strings, surrounding text is
    /// kept (`prefix-$MISSING` is `prefix-`), and numeric and boolean targets can parse
    /// expansions from plain scalars without string-forcing tags.
    ///
    /// The rules apply to the scalar's text whatever its style, so a quoted `"$NAME"` is null
    /// as well, while a `|` block keeps its trailing newline and is therefore not a sole
    /// reference. String-forcing tags (`!!str`, `!`) and other explicit tags keep their
    /// meaning, and scalars without an expansion are resolved exactly as in other modes.
    DockerComposeWithNulls,
}

#[cfg(feature = "properties")]
impl PropertySyntax {
    /// Whether this syntax uses Docker Compose interpolation rules.
    pub(crate) const fn is_docker_compose(self) -> bool {
        matches!(self, Self::DockerCompose | Self::DockerComposeWithNulls)
    }
}

/// Merge key handling policy for YAML mappings.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde_derived_types",
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "serde_derived_types", serde(rename_all = "snake_case"))]
pub enum MergeKeyPolicy {
    /// Expand YAML merge keys (`<<` and explicit `!!merge <<`) into the surrounding mapping,
    /// as per the YAML 1.1 specification.
    #[default]
    Merge,

    /// Treat YAML merge keys (`<<` and explicit `!!merge <<`) as ordinary mapping keys.
    AsOrdinary,

    /// Error out on encountering a YAML merge key (`<<` or explicit `!!merge <<`), reporting
    /// the location.
    Error,
}

/// Limits applied to alias replay to harden against alias bombs.
///
/// These limits also apply while aliases are expanded from anchored `!include` fragments.
///
/// Prefer constructing this via the [`alias_limits!`](crate::alias_limits!) macro instead of a
/// struct literal. This keeps call sites stable if new fields are added in the future.
///
/// ```rust
/// # #[cfg(feature = "deserialize")]
/// # {
/// let limits = serde_saphyr::alias_limits! {
///     max_replay_stack_depth: 32,
/// };
///
/// assert_eq!(limits.max_replay_stack_depth, 32);
/// # }
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(
    feature = "serde_derived_types",
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct AliasLimits {
    /// Maximum total number of **replayed** events injected from aliases across the entire parse.
    /// When exceeded, deserialization errors (alias replay limit exceeded).
    pub max_total_replayed_events: usize,
    /// Maximum depth of the alias replay stack (nested alias → injected buffer → alias, etc.).
    pub max_replay_stack_depth: usize,
    /// Maximum number of times a **single anchor id** may be expanded via alias.
    /// Use `usize::MAX` for "unlimited".
    pub max_alias_expansions_per_anchor: usize,
}

impl Default for AliasLimits {
    fn default() -> Self {
        Self {
            max_total_replayed_events: 1_000_000,
            max_replay_stack_depth: 64,
            max_alias_expansions_per_anchor: usize::MAX,
        }
    }
}

/// Parser configuration options.
///
/// Use this to configure comment emission, duplicate-key policy, alias-replay limits, and an
/// optional pre-parse YAML [`Budget`].
///
/// Example: parse a small `Config` using custom `Options`.
///
/// ```rust
/// use serde::Deserialize;
///
/// use serde_saphyr::options::DuplicateKeyPolicy;
/// use serde_saphyr::{from_str_with_options, Budget, Options};
///
/// #[derive(Deserialize)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: My Application
/// enabled: true
/// retries: 5
/// "#;
///
/// let options = serde_saphyr::options! {
///     budget: serde_saphyr::budget! {
///         max_documents: 2,
///     },
///     duplicate_keys: DuplicateKeyPolicy::LastWins,
/// };
///
/// let cfg: Config = from_str_with_options(yaml, options).unwrap();
/// assert_eq!(cfg.name, "My Application");
/// ```
#[non_exhaustive]
#[derive(Clone)]
#[cfg_attr(
    feature = "serde_derived_types",
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct Options {
    /// Optional YAML budget enforced during parsing and deserialization, including raw events,
    /// retained anchor copies, and property interpolation. `None` disables budget enforcement.
    pub budget: Option<Budget>,
    /// Optional callback invoked with the final budget report after parsing.
    /// It is invoked both when parsing is successful and when budget was breached.
    #[cfg_attr(feature = "serde_derived_types", serde(skip))]
    pub budget_report: Option<fn(&crate::budget::BudgetReport)>,

    /// Invoked both when parsing is successful and when budget was breached.
    #[cfg_attr(feature = "serde_derived_types", serde(skip))]
    pub budget_report_cb: Option<BudgetReportCallback>,

    /// Whether the parser emits comment events for deserialization.
    ///
    /// The default is `true`. When this is `false`, comments are still recognized and validated
    /// as YAML syntax, but their text is not captured and [`crate::Commented`] values receive an
    /// empty comment string.
    /// Comment bytes are still consumed and validated, so disabling emission is not an input-size
    /// or processing-time limit.
    ///
    /// Comment-related budget accounting follows emitted, retained parser data. Setting this to
    /// `false` also:
    ///
    /// - disables enforcement of [`Budget::max_total_comment_bytes`];
    /// - makes [`Budget::max_buffered_comment_events`] ineffective;
    /// - excludes comments from [`Budget::max_events`]; and
    /// - keeps [`crate::budget::BudgetReport::total_comment_bytes`] at zero.
    #[cfg_attr(
        feature = "serde_derived_types",
        serde(default = "default_emit_comments")
    )]
    #[cfg(feature = "comments")]
    pub emit_comments: bool,

    /// Enforce YAML indentation rules for flow collections (`[...]` and `{...}`)
    /// and scalar continuations.
    ///
    /// When enabled, flow entries, delimiters, and scalar continuation lines must be
    /// indented beyond the enclosing block collection. Quoted scalar continuations
    /// in block context must also be indented and cannot use tabs for indentation.
    /// Applies to included YAML as well. Default: false (accept under-indented flow
    /// collections and relaxed quoted scalar continuations for compatibility).
    ///
    /// Accepted with `strict_indentation: true`: entries and closing delimiters are
    /// indented beyond the enclosing block mapping.
    ///
    /// ```yaml
    /// sequence: [
    ///   first,
    ///   second
    ///  ]
    /// mapping: {
    ///   first: one,
    ///   second: two
    ///  }
    /// ```
    ///
    /// Accepted with `strict_indentation: false`, but rejected with `true` because
    /// the closing delimiters are at the same indentation as the enclosing block mapping:
    ///
    /// ```yaml
    /// sequence: [
    ///   first,
    ///   second
    /// ]
    /// mapping: {
    ///   first: one,
    ///   second: two
    /// }
    /// ```
    ///
    /// Relaxed mode also accepts under-indented entries, not just closing delimiters.
    /// This is accepted with `strict_indentation: false`, but rejected with `true`
    /// at the unindented `"value"` entry:
    ///
    /// ```yaml
    /// key: [
    /// "value",
    /// ]
    /// ```
    #[cfg_attr(feature = "serde_derived_types", serde(default))]
    pub strict_indentation: bool,

    /// Treat a byte-order mark (U+FEFF) at the start of a token inside a document
    /// as the start of a plain scalar, preserving it in the scalar's text.
    ///
    /// Default: false (report a parser error for such a BOM). When enabled, this
    /// matches libyaml's behavior and applies to included YAML as well. A BOM in
    /// a document prefix is still skipped. BOMs within an existing plain or quoted
    /// scalar remain content in either mode.
    ///
    /// A preserved BOM is part of a mapping key, so `\u{FEFF}key` does not match
    /// a struct field named `key`.
    #[cfg_attr(feature = "serde_derived_types", serde(default))]
    pub bom_inside_document_as_content: bool,

    /// Policy for duplicate keys.
    pub duplicate_keys: DuplicateKeyPolicy,
    /// Policy for YAML merge keys (`<<` and explicit `!!merge <<`).
    ///
    /// [`MergeKeyPolicy::Merge`] expands merge keys and counts them against
    /// [`Budget::max_merge_keys`]. [`MergeKeyPolicy::AsOrdinary`] accepts `<<`
    /// as a regular key. [`MergeKeyPolicy::Error`] rejects merge keys.
    ///
    /// Default: [`MergeKeyPolicy::Merge`].
    #[cfg_attr(feature = "serde_derived_types", serde(default))]
    pub merge_keys: MergeKeyPolicy,
    /// Limits for alias replay to harden against alias bombs.
    pub alias_limits: AliasLimits,
    /// Enable legacy octal parsing where values starting with `0` are treated as base-8.
    /// They are deprecated in YAML 1.2. Default: false.
    /// Applies to integer spellings converted to floats as well as integer targets
    /// and mapping-key comparison. Invalid or overflowing octal spellings do not
    /// fall back to decimal float parsing.
    pub legacy_octal_numbers: bool,
    /// Preserve plain decimal integer spellings with redundant leading zeros as
    /// strings and reject them in numeric parsing.
    ///
    /// Examples include `0755`, `-007`, and `+00`. Floating-point spellings such as
    /// `0.5`, `00.5`, and `01e2`, and explicit radix prefixes, are unaffected.
    /// This supports compatibility with `serde_yaml` when reading untyped values.
    /// Default: false.
    ///
    /// Applies to untyped values and Serde's buffering for untagged enums and
    /// flattened fields. Integer and float targets reject these spellings,
    /// including explicit `!!int`/`!!float` tags and quoted scalars. Takes
    /// precedence over [`Self::legacy_octal_numbers`] in every numeric parser.
    /// Mapping-key comparison, including composite keys and merge expansion,
    /// keeps these strings distinct from integers. In [`Self::no_schema`] mode,
    /// these spellings can be read as strings without quoting.
    #[cfg_attr(feature = "serde_derived_types", serde(default))]
    pub leading_zero_numbers_as_strings: bool,
    /// Allow supported underscore digit separators in numbers. Default: true.
    /// Integer spellings use the same syntax when converted to floats.
    /// Underscores in fractional or exponential spellings still require
    /// robotics angle conversions.
    ///
    /// When false, plain scalars containing underscores remain strings during
    /// implicit type inference (`deserialize_any`), including untagged enums and
    /// flattened fields. Examples include `1_000`, `0xFF_FF`, `0b10_01`, and `1_000.5`.
    /// This supports compatibility with `serde_yaml` when reading untyped values.
    /// Integer and float targets reject these scalars, for both mapping keys and
    /// values. Explicit `!!int` and `!!float` tags and [`Self::legacy_octal_numbers`]
    /// do not override this restriction.
    /// Otherwise valid numeric scalars return
    /// [`Error::NumericUnderscoresDisabled`](crate::Error::NumericUnderscoresDisabled).
    /// Invalid syntax and target-range failures retain the parser's original error.
    ///
    /// Mapping-key comparison follows the same rule: plain `1_000` and `1000`
    /// are distinct keys when this option is false, including inside composite
    /// keys and during merge expansion.
    #[cfg_attr(
        feature = "serde_derived_types",
        serde(default = "default_allow_numeric_underscores")
    )]
    pub allow_numeric_underscores: bool,
    /// Accept only lowercase integer radix prefixes (`0x`, `0o`, `0b`).
    /// Default: false (also accept `0X`, `0O`, and `0B`).
    /// Uppercase hexadecimal digits remain accepted, for example `0xFF`.
    ///
    /// When true, uppercase-prefixed plain scalars remain strings during implicit
    /// inference, including untagged enums and flattened fields. Integer and float
    /// targets reject these spellings for both mapping keys and values; explicit
    /// `!!int`/`!!float` tags and [`Self::legacy_octal_numbers`] do not override it.
    /// Otherwise valid integers return
    /// [`Error::UppercaseRadixPrefixDisabled`](crate::Error::UppercaseRadixPrefixDisabled).
    /// Invalid syntax and target-range failures retain the parser's original error.
    ///
    /// Mapping-key comparison uses the same integer parser: plain `0XFF` and `255`
    /// are distinct keys when this option is true, including inside composite
    /// keys and during merge expansion. With the default false they compare equal.
    #[cfg_attr(feature = "serde_derived_types", serde(default))]
    pub lowercase_radix_prefixes_only: bool,
    /// If true, accept only YAML 1.2 Core boolean spellings:
    /// `true`, `True`, `TRUE`, `false`, `False`, and `FALSE`.
    /// Other spellings, including YAML 1.1 forms such as `yes`/`no`/`on`/`off`,
    /// are rejected for boolean targets and are not inferred as booleans.
    /// Default: false (accept YAML 1.1 boolean forms).
    pub strict_booleans: bool,
    /// Preserve plain null-like scalars (`null`, `Null`, `NULL`, `~`, and empty)
    /// as their text when deserializing directly into a string target.
    ///
    /// Applies to both mapping keys and values, including `String`, `&str`, and
    /// `Cow<str>`. Borrowed targets retain the usual input borrowing requirements.
    /// Default: false (reject null-like scalars for string targets).
    ///
    /// Explicit `!!null` tags remain errors for string targets. `Option<T>`, unit,
    /// and implicit type inference (`deserialize_any`, including untyped values,
    /// untagged enums, and flattened fields) retain their existing null semantics.
    /// [`Self::no_schema`] still requires ambiguous plain strings to be quoted.
    #[cfg_attr(feature = "serde_derived_types", serde(default))]
    pub null_as_string: bool,
    /// When a field marked with the `!!binary` tag is deserialized into a `String`,
    /// `serde-saphyr` normally expects the value to be base64-encoded UTF-8.
    /// If you want to treat the value as a plain string and ignore the `!!binary` tag,
    /// set this to `true` (the default is `false`).
    pub ignore_binary_tag_for_string: bool,
    /// Activates YAML conventions common in robotics community. These extensions support
    /// conversion functions (deg, rad) and simple mathematical expressions such as deg(180),
    /// rad(pi), 1 + 2*(3 - 4/5), or rad(pi/2). `robotics` feature must also be enabled.
    /// The `!degrees`, `!radians`, and `!time` tags select floating-point conversions.
    /// `!time` represents a duration in seconds; the standard `!!timestamp` tag does not.
    pub angle_conversions: bool,
    /// If true, values that can be parsed as booleans or numbers are rejected as
    /// unquoted strings. This flag is intended for teams that want to enforce
    /// compatibility with YAML parsers that infer types from unquoted values,
    /// requiring such strings to be explicitly quoted.
    /// Signed NaN spellings such as `+.nan` and `-.nan` remain strings, matching
    /// implicit inference, even though direct float targets accept them as NaN.
    /// The default is false (a number or boolean will be stored in the string
    /// field exactly as provided, without quoting).
    pub no_schema: bool,

    /// Reject explicitly specified YAML tags that `serde-saphyr` does not recognize.
    ///
    /// This includes application-specific local and global tags. YAML 1.1 `!!merge`
    /// and `!!value` tags are accepted only on their exact scalar mapping keys, `<<`
    /// and `=` respectively. Known scalar, sequence, and mapping tags are accepted
    /// only on matching node kinds. These shape checks are independent of this option.
    /// Local robotics tags may instead select typed enum variants; their deferred
    /// collection-shape checks do not run on discarded values, which have no target type.
    /// Robotics-only `!degrees`, `!radians`, and `!time` tags are
    /// supported only when the `robotics` crate feature and
    /// [`Self::angle_conversions`] are both enabled. The `!include` tag is supported
    /// only when the `include` crate feature is enabled and an include resolver is
    /// configured.
    ///
    /// Default: false (preserve unsupported tags for compatibility with tagged enums
    /// and otherwise ignore them where possible).
    #[cfg_attr(feature = "serde_derived_types", serde(default))]
    pub reject_unsupported_tags: bool,

    /// Reject explicit tags on struct field names, except standard string tags.
    ///
    /// A derived struct normally matches each mapping key by its text alone. A
    /// custom tag on that key can therefore be discarded without the struct ever
    /// seeing it. Such a tag has no effect on field selection and may be a value
    /// annotation accidentally placed on the field name. By default, this reports
    /// [`crate::Error::TaggedStructFieldName`] instead of silently accepting it.
    /// Set this option to `false` to accept such tags without checking them.
    ///
    /// For example, an application can inspect a value's `!secret` tag by reading
    /// that value as [`crate::Tagged<T>`]:
    ///
    /// ```yaml
    /// password: !secret API_TOKEN
    /// ```
    ///
    /// This misplaced annotation is rejected when the option is enabled:
    ///
    /// ```yaml
    /// !secret password: API_TOKEN
    /// ```
    ///
    /// Untagged and quoted field names remain accepted. Explicit standard string tags
    /// (`!!str`, including equivalent verbatim or directive-expanded forms) and
    /// the non-specific `!` tag are also accepted. Field names are the external
    /// strings used by Serde: `!!str 0.0: Alice` can populate a Rust field marked
    /// `#[serde(rename = "0.0")]`. Here `!!str` explicitly gives `0.0` string
    /// semantics, and Serde's rename handles the external field name.
    /// The exception uses the resolved tag identity; a local `!str` tag is still
    /// rejected, including when a directive rebinds `!!` to an application namespace.
    ///
    /// This option applies only to struct mappings entered through Serde's
    /// `deserialize_struct`, including externally tagged struct enum variants.
    /// It checks every key in such a mapping, including unknown keys that Serde
    /// would otherwise ignore without `#[serde(deny_unknown_fields)]`.
    /// It also checks field names replayed through aliases and expanded merges.
    /// Keys are checked before duplicate-key or merge precedence filtering, even
    /// when their values would otherwise be discarded.
    /// Ordinary map keys, including string, numeric, and enum keys, and tags on
    /// values retain their existing behavior. Unlike
    /// [`Self::reject_unsupported_tags`], it does not reject application tags
    /// throughout the document.
    /// Tags consumed by enabled preprocessing, such as `!include`, retain that
    /// behavior before this check.
    ///
    /// Serde can match fields without calling `deserialize_struct`: structs using
    /// `#[serde(flatten)]` enter through `deserialize_map`, while internally tagged
    /// and untagged enums buffer their fields first. Adjacently tagged enums also
    /// buffer content that precedes the discriminator. Fields handled by those
    /// paths, including structs nested in their buffers, are not covered by this
    /// option; it is not a document-wide guarantee against tagged field names.
    ///
    /// Default: true.
    #[cfg_attr(
        feature = "serde_derived_types",
        serde(default = "default_reject_tagged_field_names")
    )]
    pub reject_tagged_field_names: bool,

    /// Handling of NaN, positive/negative infinity, and decimal literals that overflow
    /// `f64` (e.g. `1e999`) in `deserialize_any`.
    ///
    /// If left at its default, this uses [`Self::reject_non_finite_typeless_float`],
    /// preserving rejection by default and string conversion when that legacy flag is
    /// false. Explicit [`NonFiniteFloatPolicy::PassThrough`], [`NonFiniteFloatPolicy::Reject`],
    /// and [`NonFiniteFloatPolicy::AsString`] values override the legacy flag.
    ///
    /// `PassThrough` lets float-capable visitors preserve non-finite values, including
    /// untagged enums and flattened float fields. `serde_json::Value` instead converts
    /// them to `Null` without an error. `Reject` prevents that loss; `AsString` passes
    /// canonical strings (`.nan`, `.inf`, `-.inf`) to the visitor.
    ///
    /// Untagged quoted scalars and `!!str` scalars remain strings under every policy.
    /// Direct `f32`/`f64` targets are unaffected: YAML non-finite spellings remain
    /// accepted, while overflowing decimal/exponential literals remain invalid.
    #[cfg_attr(feature = "serde_derived_types", serde(default))]
    pub non_finite_float_policy: NonFiniteFloatPolicy,

    /// Legacy handling of non-finite floats in `deserialize_any`.
    ///
    /// When [`Self::non_finite_float_policy`] is left at its default, true (the default)
    /// rejects non-finite floats and false converts them to canonical strings.
    /// An explicit policy overrides this flag.
    ///
    /// [`Options::default()`] sets this to true. When deserializing options with
    /// `serde_derived_types`, an omitted flag defaults to false for compatibility
    /// with 1.3.0. If the policy is also omitted, non-finite values become strings.
    ///
    /// Use [`Self::non_finite_float_policy`] with [`NonFiniteFloatPolicy::Reject`] instead
    /// of true, or [`NonFiniteFloatPolicy::AsString`] instead of false.
    #[deprecated(since = "2.0.0", note = "use non_finite_float_policy instead")]
    #[cfg_attr(feature = "serde_derived_types", serde(default))]
    pub reject_non_finite_typeless_float: bool,

    /// If true (default), public APIs that have access to the original YAML input
    /// will wrap returned errors with a snippet wrapper, enabling rustc-like snippet
    /// rendering when a location is available.
    pub with_snippet: bool,

    /// Horizontal crop radius (in character columns) when rendering snippet diagnostics.
    ///
    /// The renderer crops all displayed lines (including the context lines) to the same
    /// column window around the reported error column, so they stay vertically aligned.
    ///
    /// If set to `0`, snippet wrapping is disabled (the original, unwrapped error is returned).
    pub crop_radius: usize,

    /// Indentation requirement for the parsed document.
    pub require_indent: RequireIndent,

    /// Optional include resolver callback.
    ///
    /// When provided, it can push parsers onto the internal parser stack to resolve `!include`
    ///-like constructs.
    #[cfg(feature = "include")]
    #[cfg_attr(feature = "serde_derived_types", serde(skip))]
    pub include_resolver: Option<IncludeResolverCallback>,

    /// A map of properties to substitute in scalar values.
    /// Used for docker-compose-style interpolation like `${VAR}`.
    #[cfg(feature = "properties")]
    #[cfg_attr(feature = "serde_derived_types", serde(skip))]
    pub property_map: Option<Rc<HashMap<String, String>>>,

    /// Which property-interpolation syntaxes are recognized.
    /// Defaults to [`PropertySyntax::Braced`] (only `${NAME}`).
    #[cfg(feature = "properties")]
    #[cfg_attr(feature = "serde_derived_types", serde(default))]
    pub property_syntax: PropertySyntax,

    /// Preserve local YAML tags as single-entry maps in `deserialize_any`.
    /// Defaults to `false`.
    ///
    /// Both operations in this configuration take `5` as their argument.
    ///
    /// ```yaml
    /// - !Add 5
    /// - !Subtract 5
    /// ```
    ///
    /// When reading directly into `Vec<Operation>`, Serde can use the tags to
    /// select `Operation::Add(5)` and `Operation::Subtract(5)`. A configuration
    /// layer may first load the YAML into a generic value such as
    /// `serde_json::Value` or Figment's `Value`. With the default settings,
    /// that value holds `[5, 5]`. By the time the application asks for
    /// `Vec<Operation>`, the information that distinguished the variants is gone.
    ///
    /// Set `local_tags_as_maps` to `true` to keep the variant names in the
    /// intermediate value. The same input then yields
    /// `[{"Add": 5}, {"Subtract": 5}]`.
    /// Each entry has the shape Serde expects for an externally tagged enum,
    /// so the application can recover both operations from the stored value.
    ///
    /// When Serde buffers an untagged enum or a field collected by
    /// `#[serde(flatten)]`, simple, unrecognized tags can appear as maps even
    /// with this option disabled. For example, `!Add 5` becomes `{"Add": 5}`.
    ///
    /// The map key is the resolved local tag name with the leading `!` removed.
    /// Its value is the tagged scalar, sequence, or mapping. A bare `!` yields
    /// a string, sequence, or mapping according to the node's kind. Global tags
    /// such as `!!str` keep their usual meaning. Tag validation and configured
    /// include resolution happen first.
    ///
    /// # When to leave this disabled
    ///
    /// - **Your application treats local tags as annotations.** For
    ///   `port: !note 8080`, a generic value normally holds the integer `8080` at
    ///   `port`. Enabling this option puts `{"note": 8080}` there, so code that
    ///   reads `port` as a number will fail. This also applies to a field of type
    ///   `serde_json::Value` inside a typed struct.
    /// - **You want the tag only in the metadata of [`Tagged<serde_json::Value>`](crate::Tagged).**
    ///   Reading `!T 5` into this type yields `Tagged(5, Some("!T"))`
    ///   by default. Enabling this option also puts the tag name in the inner value
    ///   as a map key, yielding `Tagged({"T": 5}, Some("!T"))`. Reading the same
    ///   input as `Tagged<u32>` yields `Tagged(5, Some("!T"))` with either setting.
    /// - **An enum needs its discriminator after buffering or type erasure.** An
    ///   internally tagged enum read directly can use the `kind` field in
    ///   `!ns:meta {kind: Known}`. If a generic value or a type-erasing adapter
    ///   exposes `{"ns:meta": {"kind": "Known"}}`, Serde can report a missing
    ///   `kind`. Serde also buffers enums with a final `#[serde(untagged)]`
    ///   fallback. If that variant accepts the map, the read can silently select
    ///   the fallback even though `kind` says `Known`.
    /// - **Your generic values need converted robotics units.** With the `robotics`
    ///   feature and [`Self::angle_conversions`] enabled, `!degrees 180` normally
    ///   yields π radians. Enabling this option yields `{"degrees": 180}` in a
    ///   generic value. A direct `f64` read still yields π.
    ///
    /// Each tag exposed as a map also adds one level of nesting during
    /// deserialization. A document that previously fit within [`Budget::max_depth`]
    /// can exceed that limit when it contains nested tagged values.
    #[cfg_attr(feature = "serde_derived_types", serde(default))]
    pub local_tags_as_maps: bool,
}

#[cfg(feature = "include")]
pub type IncludeResolverCallback = Rc<
    std::cell::RefCell<
        dyn for<'res> FnMut(
                crate::input_source::IncludeRequest<'res>,
            )
                -> Result<crate::ResolvedInclude, crate::IncludeResolveError>
            + 'static,
    >,
>;

pub type BudgetReportCallback =
    Rc<std::cell::RefCell<dyn FnMut(crate::budget::BudgetReport) + 'static>>;

impl Options {
    pub(crate) fn parser_options(&self) -> granit_parser::Options {
        let default_budget = Budget::default();
        let budget = self.budget.as_ref().unwrap_or(&default_budget);
        let mut parser_options = budget.parser_options();
        #[cfg(feature = "comments")]
        {
            parser_options.emit_comments = self.emit_comments;
        }
        parser_options.strict_indentation = self.strict_indentation;
        parser_options.bom_inside_document_as_content = self.bom_inside_document_as_content;
        parser_options
    }

    pub(crate) fn validate(&self) -> Result<(), crate::de_error::Error> {
        self.require_indent.validate()
    }

    /// Registers a budget-report callback. Any closure can be used,  including ones that
    /// capture state from the surrounding scope.
    ///
    /// The callback is invoked with the final [`crate::budget::BudgetReport`] after parsing
    /// completes, both on success and when the budget is breached.
    ///
    /// ```rust
    /// use serde_saphyr::options;
    /// use serde_saphyr::budget::BudgetReport;
    ///
    /// let options = options! {}.with_budget_report(|report: BudgetReport| {
    ///     // e.g. update your state / emit metrics / log the report
    ///     let _ = report;
    /// });
    /// ```
    #[must_use]
    pub fn with_budget_report<F>(mut self, cb: F) -> Self
    where
        F: FnMut(crate::budget::BudgetReport) + 'static,
    {
        self.budget_report_cb = Some(Rc::new(std::cell::RefCell::new(cb)));
        self
    }

    /// Installs a property map used for `${NAME}` interpolation.
    ///
    /// [`PropertySyntax::DockerCompose`] and [`PropertySyntax::DockerComposeWithNulls`] also
    /// interpolate quoted and block string values; the other modes interpolate only plain scalars.
    ///
    /// This is the intended public API for the `properties` feature. It consumes the provided
    /// [`HashMap`] and stores it in the internal shared representation used by nested
    /// deserializers, so callers do not have to construct `Rc` or `Some(...)` manually.
    ///
    /// ```rust
    /// # #[cfg(feature = "properties")]
    /// # {
    /// use std::collections::HashMap;
    /// use serde_saphyr::Options;
    ///
    /// let mut properties = HashMap::new();
    /// properties.insert("MODE".to_string(), "production".to_string());
    ///
    /// let options = Options::default().with_properties(properties);
    /// # let _ = options;
    /// # }
    /// ```
    #[cfg(feature = "properties")]
    #[must_use]
    pub fn with_properties(mut self, properties: HashMap<String, String>) -> Self {
        self.property_map = Some(Rc::new(properties));
        self
    }

    /// Sets the include resolver callback to be used during parsing.
    ///
    /// This method is for advances use cases. If you just want to include files from the
    /// filesystem, use [`Options::with_filesystem_root`] instead that will use [`crate::SafeFileResolver`]
    ///
    /// The callback is invoked each time the parser encounters a `!include` tag. It receives an
    /// [`crate::input_source::IncludeRequest`] describing the requested include target, the source
    /// that triggered it, the include stack, and the source location. The callback must then
    /// return either a [`crate::ResolvedInclude`] or a [`crate::IncludeResolveError`].
    ///
    /// This is useful for virtual filesystems, embedded configuration bundles, network-backed
    /// resolvers, or custom caching layers.
    ///
    /// ```rust
    /// # #[cfg(feature = "include")]
    /// # {
    /// use serde::Deserialize;
    /// use serde_saphyr::{
    ///     from_str_with_options, options, IncludeRequest, IncludeResolveError, InputSource,
    ///     ResolvedInclude,
    /// };
    ///
    /// #[derive(Debug, Deserialize, PartialEq)]
    /// struct Config {
    ///     users: Vec<User>,
    /// }
    ///
    /// #[derive(Debug, Deserialize, PartialEq)]
    /// struct User {
    ///     name: String,
    /// }
    ///
    /// let root_yaml = "users: !include virtual://users.yaml\n";
    /// let users_yaml = "- name: Alice\n- name: Bob\n";
    ///
    /// let options = options! {}.with_include_resolver(|req: IncludeRequest<'_>| {
    ///     assert_eq!(req.spec, "virtual://users.yaml");
    ///     assert_eq!(req.from_name, "<input>");
    ///
    ///     if req.spec == "virtual://users.yaml" {
    ///         Ok(ResolvedInclude::new(
    ///             req.spec,
    ///             "virtual users",
    ///             InputSource::from_string(users_yaml.to_owned()),
    ///         ))
    ///     } else {
    ///         Err(IncludeResolveError::Message(format!("unknown include: {}", req.spec)))
    ///     }
    /// });
    ///
    /// let config: Config = from_str_with_options(root_yaml, options).unwrap();
    /// assert_eq!(config.users.len(), 2);
    /// assert_eq!(config.users[0].name, "Alice");
    /// # }
    /// ```
    #[cfg(feature = "include")]
    #[must_use]
    pub fn with_include_resolver<F>(mut self, cb: F) -> Self
    where
        F: for<'res> FnMut(
                crate::input_source::IncludeRequest<'res>,
            )
                -> Result<crate::ResolvedInclude, crate::IncludeResolveError>
            + 'static,
    {
        self.include_resolver = Some(Rc::new(std::cell::RefCell::new(cb)));
        self
    }

    /// Configures a [`crate::SafeFileResolver`] rooted at `path` for `!include` lookups.
    ///
    /// This is a convenience for:
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "include_fs")]
    /// # fn main() -> Result<(), std::io::Error> {
    /// # use serde_saphyr::{options, SafeFileResolver};
    /// let options = options! {}
    ///     .with_include_resolver(SafeFileResolver::new("./configs")?.into_callback());
    /// # let _ = options;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// It enables filesystem-backed `!include` resolution.
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "include_fs")]
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use serde::Deserialize;
    /// use serde_saphyr::{from_str_with_options, options};
    ///
    /// #[derive(Debug, Deserialize)]
    /// struct User {
    ///     name: String,
    /// }
    ///
    /// #[derive(Debug, Deserialize)]
    /// struct Config {
    ///     users: Vec<User>,
    /// }
    ///
    /// let yaml = "users: !include#users value.yaml\n";
    /// let options = options! {}.with_filesystem_root("./examples")?;
    /// let config: Config = from_str_with_options(yaml, options)?;
    /// # let _ = config;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "include_fs")]
    pub fn with_filesystem_root<P>(self, path: P) -> io::Result<Self>
    where
        P: AsRef<Path>,
    {
        Ok(self.with_include_resolver(crate::SafeFileResolver::new(path)?.into_callback()))
    }
}

#[allow(deprecated)] // Preserve the legacy default while the new field is unconfigured.
impl Default for Options {
    fn default() -> Self {
        Self {
            budget: Some(Budget::default()),
            budget_report: None,
            budget_report_cb: None,
            #[cfg(feature = "comments")]
            emit_comments: true,
            strict_indentation: false,
            bom_inside_document_as_content: false,
            duplicate_keys: DuplicateKeyPolicy::Error,
            merge_keys: MergeKeyPolicy::Merge,
            alias_limits: AliasLimits::default(),
            legacy_octal_numbers: false,
            leading_zero_numbers_as_strings: false,
            allow_numeric_underscores: true,
            lowercase_radix_prefixes_only: false,
            strict_booleans: false,
            null_as_string: false,
            angle_conversions: false,
            ignore_binary_tag_for_string: false,
            no_schema: false,
            reject_unsupported_tags: false,
            reject_tagged_field_names: true,
            reject_non_finite_typeless_float: true,
            with_snippet: true,
            crop_radius: 64,
            require_indent: RequireIndent::Unchecked,

            #[cfg(feature = "include")]
            include_resolver: None,
            #[cfg(feature = "properties")]
            property_map: None,
            #[cfg(feature = "properties")]
            property_syntax: PropertySyntax::Braced,
            non_finite_float_policy: NonFiniteFloatPolicy::default(),
            local_tags_as_maps: false,
        }
    }
}

#[allow(deprecated)] // Include the legacy setting in diagnostics.
impl std::fmt::Debug for Options {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("Options");
        debug
            .field("budget", &self.budget)
            .field("budget_report", &self.budget_report)
            .field(
                "budget_report_cb",
                &if self.budget_report_cb.is_some() {
                    "set"
                } else {
                    "none"
                },
            );
        #[cfg(feature = "comments")]
        debug.field("emit_comments", &self.emit_comments);
        debug
            .field("strict_indentation", &self.strict_indentation)
            .field(
                "bom_inside_document_as_content",
                &self.bom_inside_document_as_content,
            )
            .field("duplicate_keys", &self.duplicate_keys)
            .field("merge_keys", &self.merge_keys)
            .field("alias_limits", &self.alias_limits)
            .field("legacy_octal_numbers", &self.legacy_octal_numbers)
            .field(
                "leading_zero_numbers_as_strings",
                &self.leading_zero_numbers_as_strings,
            )
            .field("allow_numeric_underscores", &self.allow_numeric_underscores)
            .field(
                "lowercase_radix_prefixes_only",
                &self.lowercase_radix_prefixes_only,
            )
            .field("strict_booleans", &self.strict_booleans)
            .field("null_as_string", &self.null_as_string)
            .field(
                "ignore_binary_tag_for_string",
                &self.ignore_binary_tag_for_string,
            )
            .field("angle_conversions", &self.angle_conversions)
            .field("no_schema", &self.no_schema)
            .field("local_tags_as_maps", &self.local_tags_as_maps)
            .field("reject_unsupported_tags", &self.reject_unsupported_tags)
            .field("reject_tagged_field_names", &self.reject_tagged_field_names)
            .field("non_finite_float_policy", &self.non_finite_float_policy)
            .field(
                "reject_non_finite_typeless_float",
                &self.reject_non_finite_typeless_float,
            )
            .field("with_snippet", &self.with_snippet)
            .field("crop_radius", &self.crop_radius)
            .field("require_indent", &self.require_indent)
            .field("include_resolver", &{
                #[cfg(feature = "include")]
                {
                    if self.include_resolver.is_some() {
                        "set"
                    } else {
                        "none"
                    }
                }
                #[cfg(not(feature = "include"))]
                {
                    "disabled"
                }
            })
            .field("property_map", &{
                #[cfg(feature = "properties")]
                {
                    if self.property_map.is_some() {
                        "set"
                    } else {
                        "none"
                    }
                }
                #[cfg(not(feature = "properties"))]
                {
                    "disabled"
                }
            })
            .field("property_syntax", &{
                #[cfg(feature = "properties")]
                {
                    format!("{:?}", self.property_syntax)
                }
                #[cfg(not(feature = "properties"))]
                {
                    "disabled".to_string()
                }
            })
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "include_fs")]
    use crate::input_source::{IncludeRequest, InputSource};
    #[cfg(feature = "include_fs")]
    use std::path::PathBuf;
    #[cfg(feature = "include_fs")]
    use tempfile::tempdir;

    #[test]
    fn test_options_default() {
        let opts = Options::default();
        assert!(opts.budget.is_some());
        assert!(opts.budget_report.is_none());
        assert!(opts.budget_report_cb.is_none());
        #[cfg(feature = "comments")]
        assert!(opts.emit_comments);
        assert!(!opts.strict_indentation);
        assert!(!opts.bom_inside_document_as_content);
        assert!(matches!(opts.duplicate_keys, DuplicateKeyPolicy::Error));
        assert!(matches!(opts.merge_keys, MergeKeyPolicy::Merge));
        assert_eq!(opts.alias_limits.max_total_replayed_events, 1_000_000);
        assert!(!opts.legacy_octal_numbers);
        assert!(!opts.leading_zero_numbers_as_strings);
        assert!(opts.allow_numeric_underscores);
        assert!(!opts.lowercase_radix_prefixes_only);
        assert!(!opts.strict_booleans);
        assert!(!opts.null_as_string);
        assert!(!opts.ignore_binary_tag_for_string);
        assert!(!opts.angle_conversions);
        assert!(!opts.no_schema);
        assert!(!opts.reject_unsupported_tags);
        assert!(!opts.local_tags_as_maps);
        assert!(opts.reject_tagged_field_names);
        assert_eq!(
            opts.non_finite_float_policy,
            NonFiniteFloatPolicy::default()
        );
        assert!(opts.with_snippet);
        assert_eq!(opts.crop_radius, 64);
        assert_eq!(opts.require_indent, RequireIndent::Unchecked);

        #[cfg(feature = "include")]
        assert!(opts.include_resolver.is_none());
        #[cfg(feature = "properties")]
        {
            assert!(opts.property_map.is_none());
            assert_eq!(opts.property_syntax, PropertySyntax::Braced);
        }
    }

    #[cfg(feature = "serde_derived_types")]
    #[test]
    fn duplicate_key_policy_serde_uses_snake_case() {
        assert_eq!(
            serde_json::to_string(&DuplicateKeyPolicy::FirstWins).unwrap(),
            "\"first_wins\""
        );
        assert!(matches!(
            serde_json::from_str::<DuplicateKeyPolicy>("\"last_wins\"").unwrap(),
            DuplicateKeyPolicy::LastWins
        ));
        assert!(serde_json::from_str::<DuplicateKeyPolicy>("\"FirstWins\"").is_err());
    }

    #[cfg(feature = "serde_derived_types")]
    #[test]
    fn local_tags_as_maps_defaults_to_false_in_older_options() {
        let opts = crate::options! { local_tags_as_maps: true };
        let mut fields = serde_json::to_value(opts).unwrap();
        let fields = fields.as_object_mut().unwrap();
        assert_eq!(fields.shift_remove("local_tags_as_maps"), Some(true.into()));

        let sequence = serde_json::Value::Array(fields.values().cloned().collect());
        let restored: Options = serde_json::from_value(sequence).unwrap();
        assert!(!restored.local_tags_as_maps);
        assert!(restored.reject_tagged_field_names);
    }

    #[cfg(feature = "serde_derived_types")]
    #[test]
    fn null_as_string_serde_roundtrip_and_missing_default() {
        let options = crate::options! { null_as_string: true };
        let mut json = serde_json::to_value(&options).unwrap();
        let restored: Options = serde_json::from_value(json.clone()).unwrap();
        assert!(restored.null_as_string);

        json.as_object_mut().unwrap().remove("null_as_string");
        let legacy: Options = serde_json::from_value(json).unwrap();
        assert!(!legacy.null_as_string);
    }

    #[cfg(feature = "serde_derived_types")]
    #[test]
    fn tagged_field_name_policy_serde_roundtrip_and_missing_default() {
        for enabled in [true, false] {
            let options = crate::options! { reject_tagged_field_names: enabled };
            let mut json = serde_json::to_value(&options).unwrap();
            let restored: Options = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(restored.reject_tagged_field_names, enabled);

            json.as_object_mut()
                .unwrap()
                .remove("reject_tagged_field_names");
            let legacy: Options = serde_json::from_value(json).unwrap();
            assert!(legacy.reject_tagged_field_names);
        }
    }

    #[cfg(feature = "serde_derived_types")]
    #[test]
    fn strict_indentation_serde_roundtrip_and_missing_default() {
        let options = crate::options! { strict_indentation: true };
        let mut json = serde_json::to_value(&options).unwrap();
        let restored: Options = serde_json::from_value(json.clone()).unwrap();
        assert!(restored.strict_indentation);

        json.as_object_mut().unwrap().remove("strict_indentation");
        let legacy: Options = serde_json::from_value(json).unwrap();
        assert!(!legacy.strict_indentation);
    }

    #[cfg(feature = "serde_derived_types")]
    #[test]
    fn bom_inside_document_serde_roundtrip_and_missing_default() {
        let options = crate::options! { bom_inside_document_as_content: true };
        let mut json = serde_json::to_value(&options).unwrap();
        let restored: Options = serde_json::from_value(json.clone()).unwrap();
        assert!(restored.bom_inside_document_as_content);

        json.as_object_mut()
            .unwrap()
            .remove("bom_inside_document_as_content");
        let legacy: Options = serde_json::from_value(json).unwrap();
        assert!(!legacy.bom_inside_document_as_content);
    }

    #[cfg(feature = "serde_derived_types")]
    #[test]
    fn non_finite_float_policy_serde_uses_snake_case() {
        for (policy, name) in [
            (NonFiniteFloatPolicy::Default, "default"),
            (NonFiniteFloatPolicy::PassThrough, "pass_through"),
            (NonFiniteFloatPolicy::Reject, "reject"),
            (NonFiniteFloatPolicy::AsString, "as_string"),
        ] {
            let json = serde_json::to_string(&policy).unwrap();
            assert_eq!(json, format!("\"{name}\""));
            assert_eq!(
                serde_json::from_str::<NonFiniteFloatPolicy>(&json).unwrap(),
                policy
            );
        }
        assert!(serde_json::from_str::<NonFiniteFloatPolicy>("\"PassThrough\"").is_err());
    }

    #[test]
    fn test_options_debug_format() {
        let opts = Options::default();
        let debug_str = format!("{:?}", opts);
        assert!(debug_str.contains("Options"));
        assert!(debug_str.contains("budget"));
        assert!(debug_str.contains("budget_report_cb: \"none\""));
        #[cfg(feature = "comments")]
        assert!(debug_str.contains("emit_comments: true"));
        assert!(debug_str.contains("strict_indentation: false"));
        assert!(debug_str.contains("bom_inside_document_as_content: false"));
        assert!(debug_str.contains("reject_unsupported_tags: false"));
        assert!(debug_str.contains("reject_tagged_field_names: true"));
        assert!(debug_str.contains("non_finite_float_policy: Default"));
        assert!(debug_str.contains("reject_non_finite_typeless_float: true"));

        #[cfg(feature = "include")]
        assert!(debug_str.contains("include_resolver: \"none\""));
        #[cfg(feature = "properties")]
        {
            assert!(debug_str.contains("property_map: \"none\""));
        }
        #[cfg(not(feature = "properties"))]
        {
            assert!(debug_str.contains("property_map: \"disabled\""));
        }

        // Test with callback
        let opts_with_cb = opts.with_budget_report(|_| {});
        let debug_str_cb = format!("{:?}", opts_with_cb);
        assert!(debug_str_cb.contains("budget_report_cb: \"set\""));
    }

    #[cfg(feature = "properties")]
    #[test]
    fn test_with_properties_sets_property_map() {
        let mut properties = std::collections::HashMap::new();
        properties.insert("MODE".to_string(), "production".to_string());

        let opts = Options::default().with_properties(properties);

        assert_eq!(
            opts.property_map.as_deref().unwrap().get("MODE"),
            Some(&"production".to_string())
        );
    }

    #[test]
    fn test_alias_limits_default() {
        let limits = AliasLimits::default();
        assert_eq!(limits.max_total_replayed_events, 1_000_000);
        assert_eq!(limits.max_replay_stack_depth, 64);
        assert_eq!(limits.max_alias_expansions_per_anchor, usize::MAX);
    }

    #[test]
    fn test_alias_limits_macro() {
        let limits = crate::alias_limits! {
            max_total_replayed_events: 42,
            max_replay_stack_depth: 7,
        };
        assert_eq!(limits.max_total_replayed_events, 42);
        assert_eq!(limits.max_replay_stack_depth, 7);
        assert_eq!(limits.max_alias_expansions_per_anchor, usize::MAX);

        let opts = crate::options! {
            alias_limits: crate::alias_limits! {
                max_alias_expansions_per_anchor: 9,
            },
        };
        assert_eq!(opts.alias_limits.max_alias_expansions_per_anchor, 9);
    }

    #[cfg(feature = "include_fs")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_with_filesystem_root_sets_include_resolver() {
        let root = PathBuf::from(".");
        let opts = Options::default().with_filesystem_root(&root).unwrap();
        assert!(opts.include_resolver.is_some());
    }

    #[cfg(feature = "include_fs")]
    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_with_filesystem_root_uses_reader_default_for_regular_files() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("child.yaml"), "value: 1\n").unwrap();

        let opts = Options::default().with_filesystem_root(dir.path()).unwrap();
        let mut resolver = opts
            .include_resolver
            .as_ref()
            .expect("resolver set")
            .borrow_mut();
        let resolved = resolver(IncludeRequest {
            spec: "child.yaml",
            from_name: "<input>",
            from_id: None,
            stack: vec!["<input>".to_string()],
            location: crate::Location::UNKNOWN,
            size_remaining: None,
        })
        .unwrap();

        assert!(matches!(resolved.source, InputSource::Reader(_)));
    }
}
