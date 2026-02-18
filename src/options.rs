use std::rc::Rc;
use crate::budget::Budget;
use serde::{Deserialize, Serialize};

/// Duplicate key handling policy for mappings.
#[non_exhaustive]
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
#[derive(Clone, Serialize, Deserialize)]
pub struct Options {
    /// Optional YAML budget to enforce before parsing (counts raw parser events).
    pub budget: Option<Budget>,
    /// Optional callback invoked with the final budget report after parsing.
    /// It is invoked both when parsing is successful and when budget was breached.
    #[serde(skip)]
    pub budget_report: Option<fn(&crate::budget::BudgetReport)>,

    /// Invoked both when parsing is successful and when budget was breached.
    #[serde(skip)]
    pub budget_report_cb: Option<BudgetReportCallback>,

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
}

pub type BudgetReportCallback =
   Rc<std::cell::RefCell<dyn FnMut(crate::budget::BudgetReport) + 'static>>;

impl Options {
    /// Registers a budget-report callback. Any closure can be used,  including ones that
    /// capture state from the surrounding scope.
    ///
    /// The callback is invoked with the final [`crate::budget::BudgetReport`] after parsing
    /// completes, both on success and when the budget is breached.
    ///
    /// ```rust
    /// use serde_saphyr::Options;
    /// use serde_saphyr::budget::BudgetReport;
    ///
    /// let options = Options::default().with_budget_report(|report: BudgetReport| {
    ///     // e.g. update your state / emit metrics / log the report
    ///     let _ = report;
    /// });
    /// ```
    #[allow(deprecated)]
    pub fn with_budget_report<F>(mut self, cb: F) -> Self
    where
        F: FnMut(crate::budget::BudgetReport) + 'static,
    {
        self.budget_report_cb = Some(Rc::new(std::cell::RefCell::new(cb)));
        self
    }
}

impl Default for Options {
    #[allow(deprecated)]
    fn default() -> Self {
        Self {
            budget: Some(Budget::default()),
            budget_report: None,
            budget_report_cb: None,
            duplicate_keys: DuplicateKeyPolicy::Error,
            alias_limits: AliasLimits::default(),
            legacy_octal_numbers: false,
            strict_booleans: false,
            angle_conversions: false,
            ignore_binary_tag_for_string: false,
            no_schema: false,
            with_snippet: true,
            crop_radius: 64,
        }
    }
}

impl std::fmt::Debug for Options {
    #[allow(deprecated)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Options")
            .field("budget", &self.budget)
            .field("budget_report", &self.budget_report)
            .field("budget_report_cb", &if self.budget_report_cb.is_some() { "set" } else { "none" })
            .field("duplicate_keys", &self.duplicate_keys)
            .field("alias_limits", &self.alias_limits)
            .field("legacy_octal_numbers", &self.legacy_octal_numbers)
            .field("strict_booleans", &self.strict_booleans)
            .field("ignore_binary_tag_for_string", &self.ignore_binary_tag_for_string)
            .field("angle_conversions", &self.angle_conversions)
            .field("no_schema", &self.no_schema)
            .field("with_snippet", &self.with_snippet)
            .field("crop_radius", &self.crop_radius)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_options_default() {
        let opts = Options::default();
        assert!(opts.budget.is_some());
        assert!(opts.budget_report.is_none());
        assert!(opts.budget_report_cb.is_none());
        assert!(matches!(opts.duplicate_keys, DuplicateKeyPolicy::Error));
        assert_eq!(opts.alias_limits.max_total_replayed_events, 1_000_000);
        assert!(!opts.legacy_octal_numbers);
        assert!(!opts.strict_booleans);
        assert!(!opts.ignore_binary_tag_for_string);
        assert!(!opts.angle_conversions);
        assert!(!opts.no_schema);
        assert!(opts.with_snippet);
        assert_eq!(opts.crop_radius, 64);
    }

    #[test]
    fn test_options_debug_format() {
        let opts = Options::default();
        let debug_str = format!("{:?}", opts);
        assert!(debug_str.contains("Options"));
        assert!(debug_str.contains("budget"));
        assert!(debug_str.contains("budget_report_cb: \"none\""));
        
        // Test with callback
        let opts_with_cb = opts.with_budget_report(|_| {});
        let debug_str_cb = format!("{:?}", opts_with_cb);
        assert!(debug_str_cb.contains("budget_report_cb: \"set\""));
    }
    
    #[test]
    fn test_alias_limits_default() {
        let limits = AliasLimits::default();
        assert_eq!(limits.max_total_replayed_events, 1_000_000);
        assert_eq!(limits.max_replay_stack_depth, 64);
        assert_eq!(limits.max_alias_expansions_per_anchor, usize::MAX);
    }
}
