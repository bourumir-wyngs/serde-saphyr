use serde::de::{Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};
use std::ops::Deref;

// Flow hints and block-string hints: we use newtype-struct names.
// These are consumed by the YAML serializer (see `ser.rs`) via Serde's data model.
pub(crate) const NAME_LIT_STR: &str = "__yaml_lit_str";
pub(crate) const NAME_FOLD_STR: &str = "__yaml_fold_str";

/// Force a YAML block literal string using the `|` style.
///
/// Emits the inner `&str` as a block scalar that preserves newlines exactly
/// as written. Each line is indented one level deeper than the surrounding
/// indentation where the value appears.
///
/// In short: use [LitStr] (|) to preserve line breaks exactly; use [FoldStr] (>) when you want
/// readers to display line breaks as spaces (soft-wrapped paragraphs).
///
/// See also: [FoldStr], [LitString], [FoldString].
///
/// Behavior
/// - Uses YAML's literal block style: a leading `|` followed by newline.
/// - Newlines are preserved verbatim by YAML consumers.
/// - Indentation is handled automatically by the serializer.
/// - Works in mapping values, sequence items, and at the top level.
///
/// Examples
///
/// Top-level literal block string:
/// ```rust
/// let long = "line 1\nline 2\n".repeat(20);
/// let out = serde_saphyr::to_string(&serde_saphyr::LitStr(&long)).unwrap();
/// assert!(out.starts_with("|\n  "));
/// ```
///
/// As a mapping value:
/// ```rust
/// use serde::Serialize;
/// #[derive(Serialize)]
/// struct S { note: serde_saphyr::LitStr<'static> }
/// let s = S { note: serde_saphyr::LitStr("a\nb") };
/// let out = serde_saphyr::to_string(&s).unwrap();
/// assert_eq!(out, "note: |-\n  a\n  b\n");
/// ```
#[derive(Clone, Copy)]
pub struct LitStr<'a>(pub &'a str);

/// Owned-string variant of [LitStr] that forces a YAML block literal string using the `|` style.
///
/// This works the same as [LitStr] but takes ownership of a String. Useful when you already
/// have an owned String and want to avoid borrowing lifetimes.
///
/// See also: [FoldStr], [FoldString].
///
/// Example
/// ```rust
/// let out = serde_saphyr::to_string(&serde_saphyr::LitString("line 1\nline 2".to_string())).unwrap();
/// assert_eq!(out, "|-\n  line 1\n  line 2\n");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LitString(pub String);

/// Force a YAML folded block string using the `>` style.
///
/// Emits the inner `&str` as a block scalar that suggests folding line breaks
/// to spaces for display by YAML consumers (empty lines are kept as paragraph
/// breaks). The serializer writes each line on its own; the folding behavior is
/// applied by consumers of the YAML, not during serialization.
///
/// In short: use [FoldStr] (>) for human-readable paragraphs that may soft-wrap; use
/// [LitStr] (|) when you need to preserve line breaks exactly as written.
///
/// See also: [LitStr], [LitString], [FoldString].
///
/// Behavior
/// - Uses YAML's folded block style: a leading `>` followed by newline.
/// - Intended for human-readable paragraphs where soft-wrapping is desirable.
/// - Indentation is handled automatically by the serializer.
/// - Works in mapping values, sequence items, and at the top level.
///
/// Examples
///
/// Top-level folded block string:
/// ```rust
/// let out = serde_saphyr::to_string(&serde_saphyr::FoldStr("line 1\nline 2")).unwrap();
/// assert_eq!(out, ">\n  line 1\n  line 2\n");
/// ```
///
/// As a mapping value:
/// ```rust
/// use serde::Serialize;
/// #[derive(Serialize)]
/// struct S { note: serde_saphyr::FoldStr<'static> }
/// let s = S { note: serde_saphyr::FoldStr("a\nb") };
/// let out = serde_saphyr::to_string(&s).unwrap();
/// assert_eq!(out, "note: >\n  a\n  b\n");
/// ```
#[derive(Clone, Copy)]
pub struct FoldStr<'a>(pub &'a str);

/// Owned-string variant of [FoldStr] that forces a YAML folded block string using the `>` style.
///
/// Same behavior as [FoldStr] but owns a String.
///
/// See also: [LitStr], [LitString].
///
/// Example
/// ```rust
/// let out = serde_saphyr::to_string(&serde_saphyr::FoldString("line 1\nline 2".to_string())).unwrap();
/// assert_eq!(out, ">\n  line 1\n  line 2\n");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoldString(pub String);

impl<'a> Serialize for LitStr<'a> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        // Always delegate decision to the YAML serializer so it can apply options.
        s.serialize_newtype_struct(NAME_LIT_STR, &self.0)
    }
}
impl Serialize for LitString {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(NAME_LIT_STR, &self.0)
    }
}
impl<'a> Serialize for FoldStr<'a> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(NAME_FOLD_STR, &self.0)
    }
}
impl Serialize for FoldString {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(NAME_FOLD_STR, &self.0)
    }
}

// Deserialization for owned block string wrappers: delegate to String
impl<'de> Deserialize<'de> for LitString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        String::deserialize(deserializer).map(LitString)
    }
}
impl<'de> Deserialize<'de> for FoldString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        String::deserialize(deserializer).map(FoldString)
    }
}

// ------------------------------------------------------------
// Convenience conversions / comparisons for block-string wrappers
// ------------------------------------------------------------

impl<'a> From<&'a str> for LitStr<'a> {
    fn from(s: &'a str) -> Self {
        LitStr(s)
    }
}

impl<'a> From<&'a str> for FoldStr<'a> {
    fn from(s: &'a str) -> Self {
        FoldStr(s)
    }
}

impl From<String> for LitString {
    fn from(s: String) -> Self {
        LitString(s)
    }
}

impl From<String> for FoldString {
    fn from(s: String) -> Self {
        FoldString(s)
    }
}

impl Deref for LitString {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for FoldString {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> Deref for LitStr<'a> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a> Deref for FoldStr<'a> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl LitString {
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl FoldString {
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl PartialEq<FoldString> for LitString {
    fn eq(&self, other: &FoldString) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<LitString> for FoldString {
    fn eq(&self, other: &LitString) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<String> for LitString {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<String> for FoldString {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<&str> for LitString {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<&str> for FoldString {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<str> for LitString {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<str> for FoldString {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}
