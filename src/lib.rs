// Serialization public API is defined at crate root; wrappers are re-exported.
pub use de::{Budget, DuplicateKeyPolicy, Error, Location, Options};
use serde::de::DeserializeOwned;
pub use crate::serializer_options::SerializerOptions;
pub use anchors::{ArcAnchor, ArcWeakAnchor, RcAnchor, RcWeakAnchor};
pub use ser::{FlowMap, FlowSeq, FoldStr, LitStr};
use crate::bs4k::find_bs4k_issue_location;
use crate::de::{Ev, Events};
use crate::live_events::LiveEvents;
use crate::parse_scalars::scalar_is_nullish;

mod anchor_store;
mod anchors;
mod base64;
pub mod budget;
mod de;
mod error;
mod live_events;
pub mod options;
mod parse_scalars;
mod ser;

pub mod ser_error;

mod serializer_options;
mod tags;

pub (crate) mod ser_quoting;

#[cfg(feature = "robotics")]
pub mod angles_conversions;
mod bs4k;

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
    to_fmt_writer(&mut out, value)?;
    Ok(out)
}

/// DEPRECATED: use `to_fmt_writer` or `to_io_writer`
/// Kept for a transition release to avoid instant breakage.
#[deprecated(
    since = "0.0.7",
    note = "Use `to_fmt_writer` for `fmt::Write` (String, fmt::Formatter) or `to_io_writer` for files/sockets."
)]
pub fn to_writer<W: std::fmt::Write, T: serde::Serialize>(
    output: &mut W,
    value: &T,
) -> std::result::Result<(), crate::ser::Error> {
    let mut ser = crate::ser::YamlSer::new(output);
    value.serialize(&mut ser)
}

/// Serialize a value as YAML into any [`fmt::Write`] target.
pub fn to_fmt_writer<W: std::fmt::Write, T: serde::Serialize>(
    output: &mut W,
    value: &T,
) -> std::result::Result<(), crate::ser::Error> {
    to_fmt_writer_with_options(output, value, SerializerOptions::default())
}

/// Serialize a value as YAML into any [`io::Write`] target.
pub fn to_io_writer<W: std::io::Write, T: serde::Serialize>(
    output: &mut W,
    value: &T,
) -> std::result::Result<(), crate::ser::Error> {
    to_io_writer_with_options(output, value, SerializerOptions::default())
}

/// Serialize a value as YAML into any [`fmt::Write`] target, with options.
/// Options are consumed because anchor generator may be taken from them.
pub fn to_fmt_writer_with_options<W: std::fmt::Write, T: serde::Serialize>(
    output: &mut W,
    value: &T,
    mut options: SerializerOptions,
) -> std::result::Result<(), crate::ser::Error> {
    let mut ser = crate::ser::YamlSer::with_options(output, &mut options);
    value.serialize(&mut ser)
}

/// Serialize a value as YAML into any [`io::Write`] target, with options.
/// Options are consumed because anchor generator may be taken from them.
pub fn to_io_writer_with_options<W: std::io::Write, T: serde::Serialize>(
    output: &mut W,
    value: &T,
    mut options: SerializerOptions,
) -> std::result::Result<(), crate::ser::Error> {
    struct Adapter<'a, W: std::io::Write> {
        output: &'a mut W,
        last_err: Option<std::io::Error>,
    }
    impl<'a, W: std::io::Write> std::fmt::Write for Adapter<'a, W> {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            if let Err(e) = self.output.write_all(s.as_bytes()) {
                self.last_err = Some(e);
                return Err(std::fmt::Error);
            }
            Ok(())
        }
        fn write_char(&mut self, c: char) -> std::fmt::Result {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            self.write_str(s)
        }
    }
    let mut adapter = Adapter {
        output: output,
        last_err: None,
    };
    let mut ser = crate::ser::YamlSer::with_options(&mut adapter, &mut options);
    match value.serialize(&mut ser) {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Some(io_error) = adapter.last_err.take() {
                return Err(crate::ser::Error::from(io_error));
            }
            Err(e)
        }
    }
}

/// Deprecated: use `to_fmt_writer_with_options` for `fmt::Write` or `to_io_writer_with_options` for `io::Write`.
#[deprecated(
    since = "0.0.7",
    note = "Use `to_fmt_writer_with_options` for fmt::Write or `to_io_writer_with_options` for io::Write."
)]
pub fn to_writer_with_options<W: std::fmt::Write, T: serde::Serialize>(
    output: &mut W,
    value: &T,
    options: SerializerOptions,
) -> std::result::Result<(), crate::ser::Error> {
    to_fmt_writer_with_options(output, value, options)
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
    let input = if let Some(rest) = input.strip_prefix('\u{FEFF}') {
        rest
    } else {
        input
    };
    // Tripwire for debugging: inputs with "] ]" should be rejected in single-doc API.
    if input.contains("] ]") {
        return Err(
            Error::msg("unexpected trailing closing delimiter").with_location(Location::UNKNOWN)
        );
    }
    // Heuristic rejection for BS4K-style invalid input: a plain scalar line with an inline
    // comment followed by an unindented content line starting a new scalar without a document
    // marker. This must be rejected in single-document APIs.
    if let Some(location) = find_bs4k_issue_location(input) {
        return Err(Error::msg("invalid plain scalar: inline comment cannot be \
        followed by a new top-level scalar line without a document marker (bs4k)")
            .with_location(location));
    }

    let cfg = crate::de::Cfg::from_options(&options);
    // Do not stop at DocumentEnd; we'll probe for trailing content/errors explicitly.
    let mut src = LiveEvents::new(input, options.budget, options.alias_limits, false);
    let value_res = crate::anchor_store::with_document_scope(|| {
        T::deserialize(crate::de::Deser::new(&mut src, cfg))
    });
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
            return Err(Error::msg("unexpected trailing closing delimiter")
                .with_location(src.last_location()));
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
    let input = if let Some(rest) = input.strip_prefix('\u{FEFF}') {
        rest
    } else {
        input
    };
    let cfg = crate::de::Cfg::from_options(&options);
    let mut src = LiveEvents::new(input, options.budget, options.alias_limits, false);
    let mut values = Vec::new();

    loop {
        match src.peek()? {
            // Skip documents that are explicit null-like scalars ("", "~", or "null").
            Some(Ev::Scalar {
                value: s, style, ..
            }) if scalar_is_nullish(s, style) => {
                let _ = src.next()?; // consume the null scalar document
                // Do not push anything for this document; move to the next one.
                continue;
            }
            Some(_) => {
                let value = crate::anchor_store::with_document_scope(|| {
                    T::deserialize(crate::de::Deser::new(&mut src, cfg))
                })?;
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

/// Serialize multiple documents into a YAML string.
///
/// Serializes each value in the provided slice as an individual YAML document.
/// Documents are separated by a standard YAML document start marker ("---\n").
/// No marker is emitted before the first document.
///
/// Example
///
/// ```rust
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Point { x: i32 }
///
/// let docs = vec![Point { x: 1 }, Point { x: 2 }];
/// let out = serde_saphyr::to_string_multiple(&docs).unwrap();
/// assert_eq!(out, "x: 1\n---\nx: 2\n");
/// ```
pub fn to_string_multiple<T: serde::Serialize>(
    values: &[T],
) -> std::result::Result<String, crate::ser::Error> {
    let mut out = String::new();
    let mut first = true;
    for v in values {
        if !first {
            out.push_str("---\n");
        }
        first = false;
        to_fmt_writer(&mut out, v)?;
    }
    Ok(out)
}

/// Deserialize a single YAML document from any `std::io::Read`.
///
/// The entire reader is read into memory (buffered) and then deserialized
/// using the same logic as [`from_slice`]. This function is convenient when
/// your YAML input comes from a file or any other IO stream.
///
/// Example
///
/// ```rust
/// use serde::{Deserialize, Serialize};
/// use std::collections::HashMap;
/// use serde_json::Value;
///
/// #[derive(Debug, PartialEq, Serialize, Deserialize)]
/// struct Point {
///     x: i32,
///     y: i32,
/// }
///
/// let yaml = "x: 3\ny: 4\n";
/// let reader = std::io::Cursor::new(yaml.as_bytes());
/// let p: Point = serde_saphyr::from_reader(reader).unwrap();
/// assert_eq!(p, Point { x: 3, y: 4 });
///
/// // It also works for dynamic values like serde_json::Value
/// let mut big = String::new();
/// let mut i = 0usize;
/// while big.len() < 64 * 1024 { big.push_str(&format!("k{0}: v{0}\n", i)); i += 1; }
/// let reader = std::io::Cursor::new(big.as_bytes());
/// let _value: Value = serde_saphyr::from_reader(reader).unwrap();
/// ```
/// Create a YAML Deserializer from any `std::io::Read`.
///
/// This reads the entire reader into memory and exposes a Serde Deserializer
/// over it.
///
/// Example
///
/// ```rust
/// use serde::{Deserialize, Serialize};
/// use std::collections::HashMap;
/// use serde_json::Value;
///
/// #[derive(Debug, PartialEq, Serialize, Deserialize)]
/// struct Point { x: i32, y: i32 }
///
/// // As a Deserializer
/// let yaml = "x: 3\ny: 4\n";
/// let reader = std::io::Cursor::new(yaml.as_bytes());
/// let p: Point = serde_saphyr::from_reader(reader).unwrap();
/// assert_eq!(p, Point { x: 3, y: 4 });
///
/// // Directly to a value via unwrap::<T>()
/// let mut big = String::new();
/// let mut i = 0usize;
/// while big.len() < 64 * 1024 { big.push_str(&format!("k{0}: v{0}\n", i)); i += 1; }
/// let reader = std::io::Cursor::new(big.as_bytes());
/// let _value: Value = serde_saphyr::from_reader(reader).unwrap();
/// ```
pub fn from_reader<R: std::io::Read, T: DeserializeOwned>(mut reader: R) -> Result<T, Error> {
    let mut buf = String::new();
    match reader.read_to_string(&mut buf) {
        Ok(_) => { from_str::<T>(&buf) }
        Err(error) => {Err(Error::IOError { cause: error })}
    }
}
