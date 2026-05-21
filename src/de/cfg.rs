use super::options::{DuplicateKeyPolicy, MergeKeyPolicy, Options};

/// Small immutable runtime configuration that `YamlDeserializer` needs.
#[derive(Copy, Clone)]
pub(crate) struct Cfg {
    /// Policy to apply for duplicate mapping keys.
    pub(crate) dup_policy: DuplicateKeyPolicy,
    /// Policy for YAML merge keys (`<<`).
    pub(crate) merge_keys: MergeKeyPolicy,
    /// If true, accept legacy octal numbers that start with `0`.
    pub(crate) legacy_octal_numbers: bool,
    /// If true, only accept exact literals `true`/`false` as booleans.
    pub(crate) strict_booleans: bool,
    /// If true, ROS-compliant angle resolver is enabled
    pub(crate) angle_conversions: bool,
    /// Ignore !!binary for string
    pub(crate) ignore_binary_tag_for_string: bool,
    /// Do not take into String type that looks like number or boolean (require quoting)
    pub(crate) no_schema: bool,
}

impl Cfg {
    #[inline]
    #[allow(deprecated)]
    pub(crate) fn from_options(options: &Options) -> Self {
        Self {
            dup_policy: options.duplicate_keys,
            merge_keys: options.merge_keys,
            legacy_octal_numbers: options.legacy_octal_numbers,
            strict_booleans: options.strict_booleans,
            angle_conversions: options.angle_conversions,
            ignore_binary_tag_for_string: options.ignore_binary_tag_for_string,
            no_schema: options.no_schema,
        }
    }
}
