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

use serde::de::{self, Deserializer, IntoDeserializer};
use serde::{Deserialize, Serialize};

use crate::Location;

/// A value paired with source locations describing where it came from. Spanned location
/// is specified in character positions and, when possible, in byte offsets as well (byte offsets
/// are available for a string source but not from the reader.
///
/// # Example
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
/// # Location semantics for YAML aliases and merges
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
///
/// # Limitation with `#[serde(flatten)]`, `#[serde(untagged)]`, and `#[serde(tag = "...")]`
///
/// When `Spanned<T>` is used inside a struct with `#[serde(flatten)]`, or inside
/// variants of `#[serde(untagged)]` or `#[serde(tag = "...")]` enums, deserialization
/// **succeeds** but **location information is lost**: both `referenced` and `defined`
/// will be `Location::UNKNOWN` (line 0, column 0).
///
/// This is because serde buffers values through a generic `ContentDeserializer` in
/// these cases, which discards the YAML deserializer context needed to capture spans.
///
/// ## Workaround for untagged/internally-tagged enums: Wrap the entire enum
///
/// Instead of putting `Spanned<T>` inside each variant, wrap the whole enum:
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::Spanned;
///
/// #[derive(Debug, Deserialize)]
/// #[serde(untagged)]
/// pub enum Payload {
///     StringVariant { message: String },
///     IntVariant { count: u32 },
/// }
///
/// // Use Spanned<Payload> instead of Spanned<T> inside variants
/// let yaml = "message: hello";
/// let result: Spanned<Payload> = serde_saphyr::from_str(yaml).unwrap();
/// assert_eq!(result.referenced.line(), 1);
/// ```
///
/// ## Alternative: Use externally tagged enums (serde default)
///
/// Externally tagged enums (the default) work with `Spanned<T>` inside variants:
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::Spanned;
///
/// #[derive(Debug, Deserialize)]
/// pub enum Payload {
///     StringVariant { message: Spanned<String> },
///     IntVariant { count: Spanned<u32> },
/// }
///
/// let yaml = "StringVariant:\n  message: hello";
/// let result: Payload = serde_saphyr::from_str(yaml).unwrap();
/// match result {
///     Payload::StringVariant { message } => {
///         assert_eq!(&message.value, "hello");
///         assert_eq!(message.referenced.line(), 2);
///     }
///     _ => panic!("Expected StringVariant"),
/// }
/// ```
///
/// ## Alternative: Use adjacently tagged enums
///
/// Adjacently tagged enums also work with `Spanned<T>` inside variants:
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::Spanned;
///
/// #[derive(Debug, Deserialize)]
/// #[serde(tag = "type", content = "data")]
/// pub enum Payload {
///     StringVariant { message: Spanned<String> },
///     IntVariant { count: Spanned<u32> },
/// }
///
/// let yaml = "type: StringVariant\ndata:\n  message: hello";
/// let result: Payload = serde_saphyr::from_str(yaml).unwrap();
/// match result {
///     Payload::StringVariant { message } => {
///         assert_eq!(&message.value, "hello");
///         assert_eq!(message.referenced.line(), 3);
///     }
///     _ => panic!("Expected StringVariant"),
/// }
/// ```
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
                // Call deserialize_any so that:
                // - Our YAML SpannedDeser calls deserialize_struct → visit_map with the
                //   synthesized {value, referenced, defined} map → full location info.
                // - serde's ContentDeserializer (used by #[serde(flatten)]) calls
                //   visit_map with the buffered Content::Map → ReprOrPlainVisitor::visit_map.
                // - serde's ContentDeserializer with a plain scalar calls visit_u64/visit_str/
                //   etc. → ReprOrPlainVisitor plain-value fallbacks with Location::UNKNOWN.
                deserializer.deserialize_any(ReprOrPlainVisitor::<T>(std::marker::PhantomData))
            }
        }

        /// Visitor that handles both the normal YAML path (visit_map with synthesized
        /// {value, referenced, defined} fields) and the flattened/content path where
        /// serde's ContentDeserializer calls visit_* with a plain or map value.
        struct ReprOrPlainVisitor<T>(std::marker::PhantomData<T>);

        impl<'de, T> de::Visitor<'de> for ReprOrPlainVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = Spanned<T>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a value or a span-aware map with value/referenced/defined fields")
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                #[derive(Deserialize)]
                struct Repr<T> {
                    value: T,
                    referenced: Location,
                    defined: Location,
                }

                Repr::<T>::deserialize(de::value::MapAccessDeserializer::new(map))
                    .map(|repr| Spanned::new(repr.value, repr.referenced, repr.defined))
            }

            // Fallback handlers for plain values arriving via ContentDeserializer
            // when Spanned<T> is inside a #[serde(flatten)] struct.
            // Location information is unavailable in this path; Location::UNKNOWN is used.

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_i8<E: de::Error>(self, v: i8) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_i16<E: de::Error>(self, v: i16) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_i32<E: de::Error>(self, v: i32) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_u8<E: de::Error>(self, v: u8) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_u16<E: de::Error>(self, v: u16) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_u32<E: de::Error>(self, v: u32) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_f32<E: de::Error>(self, v: f32) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_char<E: de::Error>(self, v: char) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                T::deserialize(de::value::BytesDeserializer::new(v))
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_byte_buf<E: de::Error>(self, v: Vec<u8>) -> Result<Self::Value, E> {
                T::deserialize(de::value::BytesDeserializer::new(&v))
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                T::deserialize(().into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                T::deserialize(().into_deserializer())
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
            }
            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                T::deserialize(de::value::SeqAccessDeserializer::new(seq))
                    .map(|val| Spanned::new(val, Location::UNKNOWN, Location::UNKNOWN))
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
