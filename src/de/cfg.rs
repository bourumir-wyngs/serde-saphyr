use super::options::{DuplicateKeyPolicy, MergeKeyPolicy, Options};
use super::{Error, Location};
use crate::budget::BudgetBreach;
use crate::scalar::Schema;

/// Small immutable runtime configuration that `YamlDeserializer` needs.
#[derive(Copy, Clone)]
pub(crate) struct Cfg {
    /// Policy to apply for duplicate mapping keys.
    pub(crate) dup_policy: DuplicateKeyPolicy,
    /// Policy for YAML merge keys (`<<`).
    pub(crate) merge_keys: MergeKeyPolicy,
    /// Effective scalar vocabulary, after resolving the legacy option flags.
    pub(crate) schema: Schema,
    /// Vocabulary used by `no_schema` string checks. The compatibility schema
    /// applies only `strict_booleans` to those checks, ignoring the octal flag.
    pub(crate) string_schema: Schema,
    /// If true, ROS-compliant angle resolver is enabled
    pub(crate) angle_conversions: bool,
    /// Ignore !!binary for string
    pub(crate) ignore_binary_tag_for_string: bool,
    /// Do not take into String type that looks like number or boolean (require quoting)
    pub(crate) no_schema: bool,
    /// If true, `deserialize_any` errors on a non-finite float instead of converting it to a
    /// canonical string.
    pub(crate) reject_non_finite_typeless_float: bool,
    /// Maximum container depth from the configured budget. `None` means budget enforcement
    /// is disabled for deserializer recursion.
    pub(crate) max_depth: Option<usize>,
    /// Current container depth for the recursive Serde deserializer.
    pub(crate) depth: usize,
}

impl Cfg {
    #[inline]
    #[allow(deprecated)] // Resolve old flags only when no schema was selected.
    pub(crate) fn from_options(options: &Options) -> Self {
        let schema = options
            .schema
            .for_deserializer(options.strict_booleans, options.legacy_octal_numbers);
        let mut string_schema = schema;
        if let Schema::Specific {
            legacy_octal_numbers,
            ..
        } = &mut string_schema
        {
            // Preserve the historical no_schema policy for both explicit Specific
            // and deprecated options: octal syntax only affects numeric parsing.
            *legacy_octal_numbers = false;
        }
        Self {
            dup_policy: options.duplicate_keys,
            merge_keys: options.merge_keys,
            schema,
            string_schema,
            angle_conversions: options.angle_conversions,
            ignore_binary_tag_for_string: options.ignore_binary_tag_for_string,
            no_schema: options.no_schema,
            reject_non_finite_typeless_float: options.reject_non_finite_typeless_float,
            max_depth: options.budget.as_ref().map(|budget| budget.max_depth),
            depth: 0,
        }
    }

    pub(crate) fn enter_container(self, location: Location) -> Result<Self, Error> {
        let depth = self.depth.saturating_add(1);
        if let Some(max_depth) = self.max_depth
            && depth > max_depth
        {
            return Err(crate::de_error::budget_error(BudgetBreach::Depth { depth })
                .with_location(location));
        }
        Ok(Self { depth, ..self })
    }
}
