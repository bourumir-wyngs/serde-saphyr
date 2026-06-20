use serde_core::de::{self, Deserialize, Deserializer, Visitor};
use std::fmt;
use std::marker::PhantomData;

/// Force a sequence to be emitted in flow style: `[a, b, c]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowSeq<T>(pub T);

/// Force a mapping to be emitted in flow style: `{k1: v1, k2: v2}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowMap<T>(pub T);

/// Force a string value to be emitted in double-quoted style.
///
/// This wrapper is transparent during deserialization: the inner value is
/// deserialized normally and placed into `DoubleQuoted<T>`. `DoubleQuoted<T>` implements
/// Serde traits only for string-like `T` values.
///
/// ```rust
/// # #[cfg(feature = "serialize")]
/// # {
/// use serde::Serialize;
/// use serde_saphyr::DoubleQuoted;
///
/// #[derive(Serialize)]
/// struct Config {
///     value: DoubleQuoted<String>,
/// }
///
/// let cfg = Config { value: DoubleQuoted("plain text".to_string()) };
/// let yaml = serde_saphyr::to_string(&cfg).unwrap();
/// assert_eq!(yaml, "value: \"plain text\"\n");
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoubleQuoted<T>(pub T);

/// Force a string value to be emitted in a single-quoted style. This provides additional
/// safety constraints, as serializer rejects control characters and other values that require
/// double-quoted string escaping.
///
/// This wrapper is transparent during deserialization: the inner value is
/// deserialized normally and placed into `SingleQuoted<T>`. `SingleQuoted<T>` implements
/// Serde traits only for string-like `T` values.
///
/// Serialization fails if the value cannot be represented
/// safely in YAML single-quoted style.
///
/// ```rust
/// # #[cfg(feature = "serialize")]
/// # {
/// use serde::Serialize;
/// use serde_saphyr::SingleQuoted;
///
/// #[derive(Serialize)]
/// struct Config {
///     value: SingleQuoted<String>,
/// }
///
/// let cfg = Config { value: SingleQuoted("plain text".to_string()) };
/// let yaml = serde_saphyr::to_string(&cfg).unwrap();
/// assert_eq!(yaml, "value: 'plain text'\n");
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SingleQuoted<T>(pub T);

/// Add an empty line after the wrapped value when serializing.
///
/// This wrapper is transparent during deserialization and can be nested with
/// other wrappers like `Commented`, `FlowSeq`, etc.
/// ```rust
/// # #[cfg(feature = "serialize")]
/// # {
/// use serde::Serialize;
/// use serde_saphyr::SpaceAfter;
///
/// #[derive(Serialize)]
/// struct Config {
///     first: SpaceAfter<i32>,
///     second: i32,
/// }
///
/// let cfg = Config { first: SpaceAfter(1), second: 2 };
/// let yaml = serde_saphyr::to_string(&cfg).unwrap();
/// // The output will have an empty line after "first: 1"
/// # }
/// ```
/// **Important:** Avoid using this wrapper with `LitStr`/`LitString` as it may add the empty
/// line to the string content. For `FoldStr`/`FoldString` and other YAML values
/// (e.g. `key: value`, quoted scalars), the extra empty line is cosmetic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpaceAfter<T>(pub T);

/// Serialize `None` as YAML tilde (`~`) while otherwise behaving like `Option<T>`.
///
/// `Some(value)` is serialized transparently as `value`. `None` is serialized
/// as `~` instead of the serializer's regular null spelling. Deserialization
/// delegates to `Option<T>`, so `~`, `null`, and empty YAML values all become
/// `NullableTilde(None)`.
///
/// ```rust
/// # #[cfg(feature = "serialize")]
/// # {
/// use serde::Serialize;
/// use serde_saphyr::NullableTilde;
///
/// #[derive(Serialize)]
/// struct Config {
///     maybe: NullableTilde<String>,
/// }
///
/// let cfg = Config { maybe: NullableTilde(None) };
/// let yaml = serde_saphyr::to_string(&cfg).unwrap();
/// assert_eq!(yaml, "maybe: ~\n");
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NullableTilde<T>(pub Option<T>);

/// Attach an inline YAML comment to a value when serializing.
///
/// This wrapper lets you annotate a scalar with an inline YAML comment that is
/// emitted after the value when using block style. The typical form is:
/// `value # comment`. This is the most useful when deserializing the anchor
/// reference (so a human reader can see what a referenced value represents).
///
/// Comment is also captured into its field when deserializing YAML.
///
/// Behavior
/// - Block style (default): the comment appears after the scalar on the same line.
/// - Flow style (inside `[ ... ]` or `{ ... }`): comments are suppressed to keep
///   the flow representation compact and unambiguous.
/// - Complex values (sequences/maps/structs): the comment is ignored; only the
///   inner value is serialized to preserve indentation and layout.
/// - Newlines in comments are sanitized to spaces so the comment remains on a
///   single line (e.g., "a\nb" becomes "a b").
/// - Deserialization of `Commented<T>` captures nearby source comments when the
///   `serde-saphyr` deserializer can provide them. Other deserializers treat it
///   transparently and produce an empty comment string.
/// - Comment capture is use-site oriented for replayed YAML. Comments from an
///   anchor definition are not copied through aliases or merge keys; a field
///   materialized by `<<: *defaults` does not inherit comments that were written
///   above the field inside `&defaults`.
/// - For container values, comments attached to the parent value itself are
///   captured only by `Commented<Container>` and are not inherited by the first
///   child field or element. A comment inside the container, directly above a
///   child key or element, is captured by that child.
/// - When an alias to a container is used as a nested value, leading comments
///   above the alias follow the same inside-container rule. For example,
///   `root:\n  # comment\n  *defaults` leaves the comment available to the
///   expanded container's first child rather than capturing it on the alias use.
///
/// Examples
///
/// Basic scalar with a comment in block style:
/// ```rust
/// # #[cfg(feature = "serialize")]
/// # {
/// use serde::Serialize;
///
/// // Re-exported from the crate root
/// use serde_saphyr::Commented;
///
/// let out = serde_saphyr::to_string(&Commented(42, "answer".to_string())).unwrap();
/// assert_eq!(out, "42 # answer\n");
/// # }
/// ```
///
/// As a mapping value, still inline:
/// ```rust
/// # #[cfg(feature = "serialize")]
/// # {
/// use serde::Serialize;
/// use serde_saphyr::Commented;
///
/// #[derive(Serialize)]
/// struct S { xn: Commented<i32> }
///
/// let s = S { xn: Commented(5, "send five starships first".into()) };
/// let out = serde_saphyr::to_string(&s).unwrap();
/// assert_eq!(out, "xn: 5 # send five starships first\n");
/// # }
/// ```
///
/// *Important*: Comments are suppressed in flow contexts (no `#` appears), and
/// ignored for complex inner values during serialization. During deserialization,
/// parent-side comments on a container such as `root: # comment` are captured by
/// `Commented<Container>` only; comments inside the container, directly above the
/// first child key or element, remain available to that child. The same applies
/// to leading comments above a nested alias whose target is a container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commented<T>(pub T, pub String);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for FlowSeq<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        T::deserialize(deserializer).map(FlowSeq)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for FlowMap<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        T::deserialize(deserializer).map(FlowMap)
    }
}

impl<'de, T> Deserialize<'de> for DoubleQuoted<T>
where
    T: Deserialize<'de> + AsRef<str>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        T::deserialize(deserializer).map(DoubleQuoted)
    }
}

impl<'de, T> Deserialize<'de> for SingleQuoted<T>
where
    T: Deserialize<'de> + AsRef<str>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        T::deserialize(deserializer).map(SingleQuoted)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Commented<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct CommentedVisitor<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for CommentedVisitor<T> {
            type Value = Commented<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a commented YAML value")
            }

            fn visit_newtype_struct<D>(
                self,
                deserializer: D,
            ) -> std::result::Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                T::deserialize(deserializer).map(|value| Commented(value, String::new()))
            }

            fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let value = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let comment = seq.next_element()?.unwrap_or_default();
                Ok(Commented(value, comment))
            }
        }

        deserializer.deserialize_newtype_struct("__yaml_commented", CommentedVisitor(PhantomData))
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for SpaceAfter<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        T::deserialize(deserializer).map(SpaceAfter)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for NullableTilde<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(NullableTilde)
    }
}

#[cfg(all(test, feature = "deserialize"))]
mod tests {
    use serde::Deserialize;

    use crate::{
        Commented, DoubleQuoted, FlowMap, FlowSeq, NullableTilde, SingleQuoted, SpaceAfter,
    };

    #[derive(Debug, Deserialize, PartialEq)]
    struct WrappersDoc {
        seq: FlowSeq<Vec<u32>>,
        map: FlowMap<std::collections::BTreeMap<String, u32>>,
        after: SpaceAfter<String>,
        nullable_tilde_none: NullableTilde<String>,
        nullable_tilde_some: NullableTilde<String>,
        commented: Commented<bool>,
        double_quoted: DoubleQuoted<String>,
        single_quoted: SingleQuoted<String>,
    }

    #[test]
    fn wrappers_remain_deserializable_without_serialize() {
        let value: WrappersDoc = crate::from_str(
            "seq: [1, 2]\nmap: {a: 1}\nafter: hello\nnullable_tilde_none: ~\nnullable_tilde_some: value\ncommented: true\ndouble_quoted: value\nsingle_quoted: value\n",
        )
        .unwrap();

        assert_eq!(value.seq, FlowSeq(vec![1, 2]));
        assert_eq!(value.after, SpaceAfter("hello".to_string()));
        assert_eq!(value.nullable_tilde_none, NullableTilde(None));
        assert_eq!(
            value.nullable_tilde_some,
            NullableTilde(Some("value".to_string()))
        );
        assert_eq!(value.commented, Commented(true, String::new()));
        assert_eq!(value.double_quoted, DoubleQuoted("value".to_string()));
        assert_eq!(value.single_quoted, SingleQuoted("value".to_string()));
        assert_eq!(value.map.0.get("a"), Some(&1));
    }
}
