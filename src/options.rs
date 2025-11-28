use crate::budget::Budget;
use serde::{Deserialize, Serialize};

/// Duplicate key handling policy for mappings.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DuplicateKeyPolicy {
    /// Error out on encountering a duplicate key.
    Error,
    /// First key wins: later duplicate pairs are skipped (key+value are consumed and ignored).
    FirstWins,
    /// Last key wins: later duplicate pairs are passed through (default Serde targets typically overwrite).
    LastWins,
}

/// Limits applied to alias replay to harden against alias bombs.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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
/// Use this to configure duplicate-key policy, alias-replay limits, and an
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
/// let options = Options {
///     budget: Some(Budget {
///         max_documents: 2,
///         ..Budget::default()
///     }),
///     duplicate_keys: DuplicateKeyPolicy::LastWins,
///     ..Options::default()
/// };
///
/// let cfg: Config = from_str_with_options(yaml, options).unwrap();
/// assert_eq!(cfg.name, "My Application");
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Options {
    /// Optional YAML budget to enforce before parsing (counts raw parser events).
    pub budget: Option<Budget>,
    /// Optional callback invoked with the final budget report after parsing.
    /// It is invoked both when parsing is successful and when budget was breached.
    #[serde(skip)]
    pub budget_report: Option<fn(&crate::budget::BudgetReport)>,
    /// Policy for duplicate keys.
    pub duplicate_keys: DuplicateKeyPolicy,
    /// Limits for alias replay to harden against alias bombs.
    pub alias_limits: AliasLimits,
    /// Enable legacy octal parsing where values starting with `00` are treated as base-8.
    /// They are deprecated in YAML 1.2. Default: false.
    pub legacy_octal_numbers: bool,
    /// If true, interpret only the exact literals `true` and `false` as booleans.
    /// YAML 1.1 forms like `yes`/`no`/`on`/`off` will be rejected and not inferred.
    /// Default: false (accept YAML 1.1 boolean forms).
    pub strict_booleans: bool,
    /// When a field marked with the `!!binary` tag is deserialized into a `String`,
    /// `serde-saphyr` normally expects the value to be base64-encoded UTF-8.
    /// If you want to treat the value as a plain string and ignore the `!!binary` tag,
    /// set this to `true` (the default is `false`).
    pub ignore_binary_tag_for_string: bool,
    /// Activates YAML conventions common in robotics community. These extensions support
    /// conversion functions (deg, rad) and simple mathematical expressions such as deg(180),
    /// rad(pi), 1 + 2*(3 - 4/5), or rad(pi/2). [robotics] feature must also be enabled.
    pub angle_conversions: bool,
    /// If true, values that can be parsed as booleans or numbers are rejected as
    /// unquoted strings. This flag is intended for teams that want to enforce
    /// compatibility with YAML parsers that infer types from unquoted values,
    /// requiring such strings to be explicitly quoted.
    /// The default is false (a number or boolean will be stored in the string
    /// field exactly as provided, without quoting).
    pub no_schema: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            budget: Some(Budget::default()),
            budget_report: None,
            duplicate_keys: DuplicateKeyPolicy::Error,
            alias_limits: AliasLimits::default(),
            legacy_octal_numbers: false,
            strict_booleans: false,
            angle_conversions: false,
            ignore_binary_tag_for_string: false,
            no_schema: false,
        }
    }
}
