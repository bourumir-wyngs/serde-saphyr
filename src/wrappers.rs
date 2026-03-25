use serde::de::{Deserialize, Deserializer};

/// Force a sequence to be emitted in flow style: `[a, b, c]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowSeq<T>(pub T);

/// Force a mapping to be emitted in flow style: `{k1: v1, k2: v2}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowMap<T>(pub T);

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

/// Attach an inline YAML comment to a value when serializing.
///
/// This wrapper lets you annotate a scalar with an inline YAML comment that is
/// emitted after the value when using block style. The typical form is:
/// `value # comment`. This is the most useful when deserializing the anchor
/// reference (so a human reader can see what a referenced value represents).
///
/// Behavior
/// - Block style (default): the comment appears after the scalar on the same line.
/// - Flow style (inside `[ ... ]` or `{ ... }`): comments are suppressed to keep
///   the flow representation compact and unambiguous.
/// - Complex values (sequences/maps/structs): the comment is ignored; only the
///   inner value is serialized to preserve indentation and layout.
/// - Newlines in comments are sanitized to spaces so the comment remains on a
///   single line (e.g., "a\nb" becomes "a b").
/// - Deserialization of `Commented<T>` ignores comments: it behaves like `T` and
///   produces an empty comment string.
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
/// ignored for complex inner values. Value with `Commented` wrapper will be
/// deserialized correctly as well, but deserializing comments is currently not
/// supported.
/// ```
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

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Commented<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        T::deserialize(deserializer).map(|v| Commented(v, String::new()))
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for SpaceAfter<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        T::deserialize(deserializer).map(SpaceAfter)
    }
}

#[cfg(all(test, feature = "deserialize"))]
mod tests {
    use serde::Deserialize;

    use crate::{Commented, FlowMap, FlowSeq, SpaceAfter};

    #[derive(Debug, Deserialize, PartialEq)]
    struct WrappersDoc {
        seq: FlowSeq<Vec<u32>>,
        map: FlowMap<std::collections::BTreeMap<String, u32>>,
        after: SpaceAfter<String>,
        commented: Commented<bool>,
    }

    #[test]
    fn wrappers_remain_deserializable_without_serialize() {
        let value: WrappersDoc = crate::from_str(
            "seq: [1, 2]\nmap: {a: 1}\nafter: hello\ncommented: true\n",
        )
        .unwrap();

        assert_eq!(value.seq, FlowSeq(vec![1, 2]));
        assert_eq!(value.after, SpaceAfter("hello".to_string()));
        assert_eq!(value.commented, Commented(true, String::new()));
        assert_eq!(value.map.0.get("a"), Some(&1));
    }
}