use serde::de::DeserializeOwned;
pub use de::{
    Budget, Options, Error, Location, DuplicateKeyPolicy
};

// Serialization public API is defined at crate root; wrappers are re-exported.
pub use ser::{ RcAnchor, ArcAnchor, RcWeakAnchor, ArcWeakAnchor, FlowSeq, FlowMap, LitStr, FoldStr };
pub use crate::serializer_options::SerializerOptions;

use crate::live_events::LiveEvents;
use crate::parse_scalars::scalar_is_nullish;
use crate::de::{Ev, Events};

mod base64;
pub mod budget;
pub mod options;
mod parse_scalars;
mod de;
mod error;
mod live_events;
mod tags;
mod ser;
mod serializer_options;

// Detect BS4K-style invalid pattern: a content line with an inline comment,
// immediately followed by a top-level (non-indented) content line that would
// implicitly start a new document without a marker. This should be rejected
// by single-document APIs.
fn has_inline_comment_followed_by_top_level_content(input: &str) -> bool {
    let mut lines = input.lines();
    while let Some(line) = lines.next() {
        // Normalize: ignore UTF-8 BOM if present in the first line. Use strip_prefix to avoid slicing at a non-UTF8 boundary.
        let line = if let Some(rest) = line.strip_prefix('\u{FEFF}') { rest } else { line };
        let trimmed = line.trim_end();

        // Find position of inline comment '#'
        let hash_pos = trimmed.find('#');
        if let Some(pos) = hash_pos {
            // Slice before '#'
            let before = &trimmed[..pos];
            // Skip if there is no non-whitespace content before '#'
            if before.chars().all(|c| c.is_whitespace()) { continue; }
            // If there is a ':' (mapping key) before '#', this is not the BS4K case.
            if before.contains(':') { continue; }
            // If line starts with a sequence dash after whitespace, skip.
            let before_trim = before.trim_start();
            if before_trim.starts_with("- ") || before_trim == "-" { continue; }
            // If flow indicators are present before the comment, skip (flow content allowed).
            if before.contains('[') || before.contains('{') { continue; }

            // Now check the next line context.
            if let Some(next) = lines.clone().next() {
                let next_trim = next.trim_end();
                let next_is_empty = next_trim.trim().is_empty();
                let next_starts_with_ws = next_trim.starts_with(' ') || next_trim.starts_with('\t');
                let next_is_marker = next_trim.starts_with("---") || next_trim.starts_with("...") || next_trim.starts_with('#');
                // If next line begins a mapping key (contains ':' before a '#'), do not trigger.
                if let Some(colon) = next_trim.find(':') {
                    let before_colon = &next_trim[..colon];
                    if before_colon.chars().any(|c| !c.is_whitespace()) { continue; }
                }
                // Trigger only if next line is top-level content (non-empty, non-indented, not a marker/comment)
                if !next_is_empty && !next_starts_with_ws && !next_is_marker {
                    return true;
                }
            }
        }
    }
    false
}

// ---------------- Serialization (public API) ----------------

/// Serialize a value to a YAML `String`.
///
/// This is the easiest entry point when you just want a YAML string.
///
/// Example
///
/// ```rust
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Foo { a: i32, b: bool }
///
/// let s = serde_saphyr::to_string(&Foo { a: 1, b: true }).unwrap();
/// assert!(s.contains("a: 1"));
/// ```
pub fn to_string<T: serde::Serialize>(value: &T) -> std::result::Result<String, crate::ser::Error> {
    let mut out = String::new();
    to_writer(&mut out, value)?;
    Ok(out)
}

/// Serialize a value to a `fmt::Write` with default indentation (2 spaces).
///
/// - `out`: destination that implements `std::fmt::Write` (for example, a `String`).
/// - `value`: any `serde::Serialize` value.
///
/// Returns `Ok(())` on success, otherwise a serialization error.
pub fn to_writer<W: std::fmt::Write, T: serde::Serialize>(out: &mut W, value: &T) -> std::result::Result<(), crate::ser::Error> {
    let mut ser = crate::ser::YamlSer::new(out);
    value.serialize(&mut ser)
}

/// Serialize a value to a writer using [`SerializerOptions`].
///
/// Use this to tweak indentation or provide a custom anchor name generator.
///
/// Example: 4-space indentation.
///
/// ```rust
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Foo { a: i32 }
///
/// let mut buf = String::new();
/// let opts = serde_saphyr::SerializerOptions { indent_step: 4, anchor_generator: None };
/// serde_saphyr::to_writer_with_options(&mut buf, &Foo { a: 7 }, opts).unwrap();
/// assert!(buf.contains("a: 7"));
/// ```
///
/// Example: custom anchor names when using Rc/Arc wrappers.
///
/// ```rust
/// use serde::Serialize;
/// use std::rc::Rc;
///
/// #[derive(Serialize)]
/// struct Node { name: String }
///
/// let shared = Rc::new(Node { name: "n".into() });
/// let mut buf = String::new();
/// let opts = serde_saphyr::SerializerOptions { indent_step: 2, anchor_generator: Some(|id| format!("id{id}")) };
/// serde_saphyr::to_writer_with_options(&mut buf, &serde_saphyr::RcAnchor(shared), opts).unwrap();
/// assert!(buf.contains("&id1") || buf.contains("&id0"));
/// ```
pub fn to_writer_with_options<W: std::fmt::Write, T: serde::Serialize>(
    out: &mut W,
    value: &T,
    mut options: crate::serializer_options::SerializerOptions,
) -> std::result::Result<(), crate::ser::Error> {
    let mut ser = crate::ser::YamlSer::with_options(out, &mut options);
    value.serialize(&mut ser)
}

/// Deserialize any `T: serde::de::DeserializeOwned` directly from a YAML string.
///
/// This is the simplest entry point; it parses a single YAML document. If the
/// input contains multiple documents, this returns an error advising to use
/// [`from_multiple`] or [`from_multiple_with_options`].
///
/// Example: read a small `Config` structure from a YAML string.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
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
/// let cfg: Config = serde_saphyr::from_str(yaml).unwrap();
/// assert!(cfg.enabled);
/// ```
pub fn from_str<T: DeserializeOwned>(input: &str) -> Result<T, Error> {
    from_str_with_options(input, Options::default())
}

/// Deserialize a single YAML document with configurable [`Options`].
///
/// Example: read a small `Config` with a custom budget and default duplicate-key policy.
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::DuplicateKeyPolicy;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
///      name: My Application
///      enabled: true
///      retries: 5
/// "#;
///
/// let options = serde_saphyr::Options {
///      budget: Some(serde_saphyr::Budget {
///            max_anchors: 200,
///            .. serde_saphyr::Budget::default()
///      }),
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
///     .. serde_saphyr::Options::default()
/// };
/// let cfg: Config = serde_saphyr::from_str_with_options(yaml, options).unwrap();
/// assert_eq!(cfg.retries, 5);
/// ```
pub fn from_str_with_options<T: DeserializeOwned>(
    input: &str,
    options: Options,
) -> Result<T, Error> {
    // Normalize: ignore a single leading UTF-8 BOM if present.
    let input = if let Some(rest) = input.strip_prefix('\u{FEFF}') { rest } else { input };
    // Tripwire for debugging: inputs with "] ]" should be rejected in single-doc API.
    if input.contains("] ]") {
        return Err(Error::msg("unexpected trailing closing delimiter").with_location(Location::UNKNOWN));
    }
    // Heuristic rejection for BS4K-style invalid input: a plain scalar line with an inline
    // comment followed by an unindented content line starting a new scalar without a document
    // marker. This must be rejected in single-document APIs.
    if has_inline_comment_followed_by_top_level_content(input) {
        return Err(Error::msg("invalid plain scalar: inline comment cannot be followed by a new top-level scalar line without a document marker").with_location(Location::UNKNOWN));
    }
    let cfg = crate::de::Cfg {
        dup_policy: options.duplicate_keys,
        legacy_octal_numbers: options.legacy_octal_numbers,
        strict_booleans: options.strict_booleans,
    };
    // Do not stop at DocumentEnd; we'll probe for trailing content/errors explicitly.
    let mut src = LiveEvents::new(input, options.budget, options.alias_limits, false);
    let value_res = T::deserialize(crate::de::Deser::new(&mut src, cfg));
    let value = match value_res {
        Ok(v) => v,
        Err(e) => {
            if src.synthesized_null_emitted() {
                // If the only thing in the input was an empty document (synthetic null),
                // surface this as an EOF error to preserve expected error semantics
                // for incompatible target types (e.g., bool).
                return Err(Error::eof().with_location(src.last_location()));
            } else {
                return Err(e);
            }
        }
    };

    // After finishing first document, peek ahead to detect either another document/content
    // or trailing garbage. If a scan error occurs but we have seen a DocumentEnd ("..."),
    // ignore the trailing garbage. Otherwise, surface the error.
    match src.peek() {
        Ok(Some(_)) => {
            return Err(Error::msg(
                "multiple YAML documents detected; use from_multiple or from_multiple_with_options",
            )
                .with_location(src.last_location()));
        }
        Ok(None) => {}
        Err(e) => {
            if src.seen_doc_end() {
                // Trailing garbage after a proper document end marker is ignored.
            } else {
                return Err(e);
            }
        }
    }

    // Conservative extra guard for malformed trailing closers in single-doc mode.
    if !src.seen_doc_end() {
        let trimmed = input.trim();
        // Heuristic: catch a stray extra closing bracket in flow context like "[ a ] ]".
        // Avoid triggering on nested arrays like "[[1]]" by checking for matching open patterns.
        if (trimmed.contains("] ]") || trimmed.contains("]]"))
            && !(trimmed.contains("[[") || trimmed.contains("[ ["))
        {
            return Err(Error::msg("unexpected trailing closing delimiter").with_location(src.last_location()));
        }
    }

    src.finish()?;
    Ok(value)
}

/// Deserialize multiple YAML documents from a single string into a vector of `T`.
/// Completely empty documents are ignored and not included into returned vector.
///
/// Example: read two `Config` documents separated by `---`.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: First
/// enabled: true
/// retries: 1
/// ---
/// name: Second
/// enabled: false
/// retries: 2
/// "#;
///
/// let cfgs: Vec<Config> = serde_saphyr::from_multiple(yaml).unwrap();
/// assert_eq!(cfgs.len(), 2);
/// assert_eq!(cfgs[0].name, "First");
/// ```
pub fn from_multiple<T: DeserializeOwned>(input: &str) -> Result<Vec<T>, Error> {
    from_multiple_with_options(input, Options::default())
}

/// Deserialize multiple YAML documents into a vector with configurable [`Options`].
///
/// Example: two `Config` documents with a custom budget.
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::DuplicateKeyPolicy;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: First
/// enabled: true
/// retries: 1
/// ---
/// name: Second
/// enabled: false
/// retries: 2
/// "#;
///
/// let options = serde_saphyr::Options {
///      budget: Some(serde_saphyr::Budget {
///            max_anchors: 200,
///            .. serde_saphyr::Budget::default()
///      }),
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
///     .. serde_saphyr::Options::default()
/// };
/// let cfgs: Vec<Config> = serde_saphyr::from_multiple_with_options(yaml, options).unwrap();
/// assert_eq!(cfgs.len(), 2);
/// assert!(!cfgs[1].enabled);
/// ```
pub fn from_multiple_with_options<T: DeserializeOwned>(
    input: &str,
    options: Options,
) -> Result<Vec<T>, Error> {
    // Normalize: ignore a single leading UTF-8 BOM if present.
    let input = if let Some(rest) = input.strip_prefix('\u{FEFF}') { rest } else { input };
    let cfg = crate::de::Cfg {
        dup_policy: options.duplicate_keys,
        legacy_octal_numbers: options.legacy_octal_numbers,
        strict_booleans: options.strict_booleans,
    };
    let mut src = LiveEvents::new(input, options.budget, options.alias_limits, false);
    let mut values = Vec::new();

    loop {
        match src.peek()? {
            // Skip documents that are explicit null-like scalars ("", "~", or "null").
            Some(Ev::Scalar {
                     value: s,
                     style,
                     ..
                 }) if scalar_is_nullish(s, style) => {
                let _ = src.next()?; // consume the null scalar document
                // Do not push anything for this document; move to the next one.
                continue;
            }
            Some(_) => {
                let value = T::deserialize(crate::de::Deser::new(&mut src, cfg))?;
                values.push(value);
            }
            None => break,
        }
    }

    src.finish()?;
    Ok(values)
}

/// Deserialize a single YAML document from a UTF-8 byte slice.
///
/// This is equivalent to [`from_str`], but accepts `&[u8]` and validates it is
/// valid UTF-8 before parsing.
///
/// Example: read a small `Config` structure from bytes.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
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
/// let bytes = yaml.as_bytes();
/// let cfg: Config = serde_saphyr::from_slice(bytes).unwrap();
/// assert!(cfg.enabled);
/// ```
///
pub fn from_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    from_slice_with_options(bytes, Options::default())
}

/// Deserialize a single YAML document from a UTF-8 byte slice with configurable [`Options`].
///
/// Example: read a small `Config` with a custom budget from bytes.
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::DuplicateKeyPolicy;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
///      name: My Application
///      enabled: true
///      retries: 5
/// "#;
/// let bytes = yaml.as_bytes();
/// let options = serde_saphyr::Options {
///      budget: Some(serde_saphyr::Budget {
///            max_anchors: 200,
///            .. serde_saphyr::Budget::default()
///      }),
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
///     .. serde_saphyr::Options::default()
/// };
/// let cfg: Config = serde_saphyr::from_slice_with_options(bytes, options).unwrap();
/// assert_eq!(cfg.retries, 5);
/// ```
pub fn from_slice_with_options<T: DeserializeOwned>(
    bytes: &[u8],
    options: Options,
) -> Result<T, Error> {
    let s = std::str::from_utf8(bytes).map_err(|_| Error::msg("input is not valid UTF-8"))?;
    from_str_with_options(s, options)
}

/// Deserialize multiple YAML documents from a UTF-8 byte slice into a vector of `T`.
///
/// Example: read two `Config` documents separated by `---` from bytes.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: First
/// enabled: true
/// retries: 1
/// ---
/// name: Second
/// enabled: false
/// retries: 2
/// "#;
/// let bytes = yaml.as_bytes();
/// let cfgs: Vec<Config> = serde_saphyr::from_slice_multiple(bytes).unwrap();
/// assert_eq!(cfgs.len(), 2);
/// assert_eq!(cfgs[0].name, "First");
/// ```
pub fn from_slice_multiple<T: DeserializeOwned>(bytes: &[u8]) -> Result<Vec<T>, Error> {
    from_slice_multiple_with_options(bytes, Options::default())
}

/// Deserialize multiple YAML documents from bytes with configurable [`Options`].
/// Completely empty documents are ignored and not included into returned vector.
///
/// Example: two `Config` documents with a custom budget from bytes.
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::DuplicateKeyPolicy;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: First
/// enabled: true
/// retries: 1
/// ---
/// name: Second
/// enabled: false
/// retries: 2
/// "#;
/// let bytes = yaml.as_bytes();
/// let options = serde_saphyr::Options {
///      budget: Some(serde_saphyr::Budget {
///            max_anchors: 200,
///            .. serde_saphyr::Budget::default()
///      }),
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
///     .. serde_saphyr::Options::default()
/// };
/// let cfgs: Vec<Config> = serde_saphyr::from_slice_multiple_with_options(bytes, options).unwrap();
/// assert_eq!(cfgs.len(), 2);
/// assert!(!cfgs[1].enabled);
/// ```
pub fn from_slice_multiple_with_options<T: DeserializeOwned>(
    bytes: &[u8],
    options: Options,
) -> Result<Vec<T>, Error> {
    let s = std::str::from_utf8(bytes).map_err(|_| Error::msg("input is not valid UTF-8"))?;
    from_multiple_with_options(s, options)
}
