use crate::budget::Budget;

/// Duplicate key handling policy for mappings.
#[derive(Clone, Copy, Debug)]
pub enum DuplicateKeyPolicy {
    /// Error out on encountering a duplicate key.
    Error,
    /// First key wins: later duplicate pairs are skipped (key+value are consumed and ignored).
    FirstWins,
    /// Last key wins: later duplicate pairs are passed through (default Serde targets typically overwrite).
    LastWins,
}

/// Limits applied to alias replay to harden against alias bombs.
#[derive(Clone, Copy, Debug)]
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
/// Example: parse a small `Config` using a custom `Options`.
///
/// ```rust
/// use serde::Deserialize;///
///
/// use serde_saphyr::sf_serde::DuplicateKeyPolicy;
///
/// #[derive(Deserialize)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
///     name: My Application
///     enabled: true
///     retries: 5
/// "#;
///
/// let options = serde_saphyr::Options {
///      budget: Some(serde_saphyr::Budget {
///            max_documents: 2,
///            .. serde_saphyr::Budget::default()
///      }),
///     // default is error
///     duplicate_keys: DuplicateKeyPolicy::LastWins,
///     .. serde_saphyr::Options::default()
/// };
///
/// let cfg: Config = serde_saphyr::from_str_with_options(yaml, options).unwrap();
/// assert_eq!(cfg.name, "My Application");
/// ```
#[derive(Clone, Debug)]
pub struct Options {
    /// Optional YAML budget to enforce before parsing (counts raw parser events).
    pub budget: Option<Budget>,
    /// Policy for duplicate keys.
    pub duplicate_keys: DuplicateKeyPolicy,
    /// Limits for alias replay to harden against alias bombs.
    pub alias_limits: AliasLimits,
    /// Enable legacy octal parsing where values starting with `00` are treated as base-8.
    /// They are deprecated in YAML 1.2. Default: false.
    pub legacy_octal_numbers: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            budget: Some(Budget::default()),
            duplicate_keys: DuplicateKeyPolicy::Error,
            alias_limits: AliasLimits::default(),
            legacy_octal_numbers: false,
        }
    }
}
