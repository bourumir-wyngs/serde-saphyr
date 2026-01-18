//! Span-aware wrapper types.
//!
//! `Spanned<T>` lets you deserialize a value together with the source location
//! (line/column) of the YAML node it came from.
//!
//! This is especially useful for config validation errors, where you want to
//! point at the exact place in the YAML. Many configuration errors are not kind
//! of "invalid YAML" but rather "valid YAML, still invalid value". Using
//! Spanned allows to tell where the invalid value comes from.
//!
//! ```rust
//! use serde::Deserialize;
//!
//! #[derive(Debug, Deserialize)]
//! struct Cfg {
//!     timeout: serde_saphyr::Spanned<u64>,
//! }
//!
//! let cfg: Cfg = serde_saphyr::from_str("timeout: 5\n").unwrap();
//! assert_eq!(cfg.timeout.value, 5);
//! assert_eq!(cfg.timeout.referenced.line(), 1);
//! assert_eq!(cfg.timeout.referenced.column(), 10);
//! ```

use serde::{Deserialize, Serialize};
use serde::de::{self, Deserializer};

use crate::Location;

/// A value paired with source locations describing where it came from.
///
/// Example
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize)]
/// struct Cfg {
///     timeout: serde_saphyr::Spanned<u64>,
/// }
///
/// let cfg: Cfg = serde_saphyr::from_str("timeout: 5\n").unwrap();
/// assert_eq!(cfg.timeout.value, 5);
/// assert_eq!(cfg.timeout.referenced.line(), 1);
/// assert_eq!(cfg.timeout.referenced.column(), 10);
/// ```
///
/// Location semantics for YAML aliases and merges
///
/// `Spanned<T>` exposes two locations:
///
/// - `referenced`: where the value is referenced/used in the YAML.
///   - For aliases (`*a`): this is the location of the alias token.
///   - For merge-derived values (`<<`): this is the location of the merge entry
///     (typically the `<<: *a` site).
/// - `defined`: where the value is defined in YAML.
///   - For plain values: equals `referenced`.
///   - For aliases: points to the anchored definition.
///   - For merge-derived values: points to the originating scalar in the merged
///     mapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spanned<T> {
    pub value: T,
    pub referenced: Location,
    pub defined: Location,
}

impl<T> Spanned<T> {
    pub const fn new(value: T, referenced: Location, defined: Location) -> Self {
        Self {
            value,
            referenced,
            defined,
        }
    }
}

impl<'de, T> Deserialize<'de> for Spanned<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SpannedVisitor<T>(std::marker::PhantomData<T>);

        impl<'de, T> de::Visitor<'de> for SpannedVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = Spanned<T>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a span-aware newtype wrapper")
            }

            fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                #[derive(Deserialize)]
                struct Repr<T> {
                    value: T,
                    referenced: Location,
                    defined: Location,
                }

                let repr = Repr::<T>::deserialize(deserializer)?;
                Ok(Spanned::new(repr.value, repr.referenced, repr.defined))
            }
        }

        deserializer
            .deserialize_newtype_struct("__yaml_spanned", SpannedVisitor(std::marker::PhantomData))
    }
}

impl<T> Serialize for Spanned<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // `Spanned<T>` is a deserialization helper that records source locations.
        // When serializing, we emit the wrapped value only.
        self.value.serialize(serializer)
    }
}
