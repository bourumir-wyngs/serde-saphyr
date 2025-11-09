// Serialization public API is defined at crate root; wrappers are re-exported.
use crate::budget::EnforcingPolicy;
use crate::de::{Ev, Events};
use crate::live_events::LiveEvents;
use crate::parse_scalars::scalar_is_nullish;
pub use crate::serializer_options::SerializerOptions;
pub use anchors::{ArcAnchor, ArcWeakAnchor, RcAnchor, RcWeakAnchor};
pub use de::{Budget, DuplicateKeyPolicy, Error, Location, Options};
pub use ser::{Commented, FlowMap, FlowSeq, FoldStr, LitStr};
use serde::de::DeserializeOwned;
use std::io::Read;

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

pub(crate) mod ser_quoting;

#[cfg(feature = "robotics")]
pub mod angles_conversions;
mod buffered_input;
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

/// Deprecated: use `to_fmt_writer` or `to_io_writer`
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

    let cfg = crate::de::Cfg::from_options(&options);
    // Do not stop at DocumentEnd; we'll probe for trailing content/errors explicitly.
    let mut src = LiveEvents::from_str(input, options.budget, options.alias_limits, false);
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
    let mut src = LiveEvents::from_str(input, options.budget, options.alias_limits, false);
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
/// let reader = std::io::Cursor::new(big.as_bytes().to_owned());
/// let _value: Value = serde_saphyr::from_reader(reader).unwrap();
/// ```
pub fn from_reader<'a, R: std::io::Read + 'a, T: DeserializeOwned>(reader: R) -> Result<T, Error> {
    from_reader_with_options(reader, Options::default())
}

/// Deserialize a single YAML document from any `std::io::Read` with configurable `Options`.
///
/// This is the reader-based counterpart to [`from_str_with_options`]. It consumes a
/// byte-oriented reader, decodes it to UTF-8, and streams events into the deserializer.
///
/// Notes on limits and large inputs
/// - Parsing limits: Use [`Options::budget`] to constrain YAML complexity (events, nodes,
///   nesting depth, total scalar bytes, number of documents, anchors, aliases, etc.). These
///   limits are enforced during parsing and are enabled by default via `Options::default()`.
/// - Byte-level input cap: A hard cap on input bytes is enforced via `Options::budget.max_reader_input_bytes`.
///   The default budget sets this to 256 MiB. You can override it by customizing `Options::budget`.
///   When the cap is exceeded, deserialization fails early with a budget error.
///
/// Example: limit raw input bytes and customize options
/// ```rust
/// use std::io::{Read, Cursor};
/// use serde::Deserialize;
/// use serde_saphyr::{Budget, Options};
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Point { x: i32, y: i32 }
///
/// let yaml = "x: 3\ny: 4\n";
/// let reader = Cursor::new(yaml.as_bytes());
///
/// // Cap the reader to at most 1 KiB of input bytes.
/// let capped = reader.take(1024);
///
/// // Tighten the parsing budget as well (optional).
/// let mut opts = Options::default();
/// opts.budget = Some(Budget { max_events: 10_000, ..Budget::default() });
///
/// let p: Point = serde_saphyr::from_reader_with_options(capped, opts).unwrap();
/// assert_eq!(p, Point { x: 3, y: 4 });
/// ```
///
/// Error behavior
/// - If an empty document is provided (no content), a type-mismatch (eof) error is returned when
///   attempting to deserialize into non-null-like targets.
/// - If the reader contains multiple documents, an error is returned suggesting the
///   `read`/`read_with_options` iterator APIs.
/// - If `Options::budget` is set and a limit is exceeded, an error is returned early.
pub fn from_reader_with_options<'a, R: std::io::Read + 'a, T: DeserializeOwned>(
    reader: R,
    options: Options,
) -> Result<T, Error> {
    let cfg = crate::de::Cfg::from_options(&options);
    let mut src = LiveEvents::from_reader(
        reader,
        options.budget,
        options.alias_limits,
        false,
        EnforcingPolicy::AllContent,
    );
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
                "multiple YAML documents detected; use read or read_with_options to obtain the iterator",
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

    src.finish()?;
    Ok(value)
}

/// Create an iterator over YAML documents from any `std::io::Read` using default options.
///
/// This is a convenience wrapper around [`read_with_options`], equivalent to
/// `read_with_options(reader, Options::default())`.
///
/// - It streams the reader without loading the whole input into memory.
/// - Each item produced by the returned iterator is one deserialized YAML document of type `T`.
/// - Documents that are completely empty or null-like (e.g., `"", ~, null`) are skipped.
///
/// Generic parameters
/// - `R`: the concrete reader type implementing [`std::io::Read`]. You almost never need to
///   write this explicitly; the compiler will infer it from the `reader` you pass. When using
///   turbofish, write `_` to let the compiler infer `R`.
/// - `T`: the type to deserialize each YAML document into. Must implement [`serde::de::DeserializeOwned`].
///
/// Lifetimes
/// - `'a`: the lifetime of the returned iterator, tied to the lifetime of the provided `reader`.
///   The iterator cannot outlive the reader it was created from.
///
/// Limits and budget
/// - Uses `Options::default()`, which enables a YAML parsing budget by default. This enforces
///   limits such as maximum events, nodes, nesting depth, total scalar bytes
///   Maximal input size limit is turned to off, as input for the streaming reader may
///   potentially be indefinite. To customize these, call [`read_with_options`] and set
///   `Options::budget.max_reader_input_bytes` in the provided `Options`.
/// - Alias replay limits are also enforced with their default values to mitigate alias bombs.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Simple { id: usize }
///
/// let yaml = b"id: 1\n---\nid: 2\n";
/// let mut reader = std::io::Cursor::new(&yaml[..]);
///
/// // Type `T` is inferred from the collection target (Vec<Simple>).
/// let values: Vec<Simple> = serde_saphyr::read(&mut reader)
///     .map(|r| r.unwrap())
///     .collect();
/// assert_eq!(values.len(), 2);
/// assert_eq!(values[0].id, 1);
/// ```
///
/// Specifying only `T` with turbofish and letting `R` be inferred using `_`:
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Simple { id: usize }
///
/// let yaml = b"id: 10\n---\nid: 20\n";
/// let mut reader = std::io::Cursor::new(&yaml[..]);
///
/// // First turbofish parameter is R (reader type), `_` lets the compiler infer it.
/// let iter = serde_saphyr::read::<_, Simple>(&mut reader);
/// let ids: Vec<usize> = iter.map(|res| res.unwrap().id).collect();
/// assert_eq!(ids, vec![10, 20]);
/// ```
///
/// - Each `next()` yields either `Ok(T)` for a successfully deserialized document or `Err(Error)`
///   if parsing fails or a limit is exceeded. After an error, the iterator ends.
/// - Empty/null-like documents are skipped and produce no items.
pub fn read<'a, R, T>(reader: &'a mut R) -> Box<dyn Iterator<Item = Result<T, Error>> + 'a>
where
    R: Read + 'a,
    T: DeserializeOwned + 'a,
{
    Box::new(read_with_options(
        reader,
        Options {
            budget: Some(Budget {
                max_reader_input_bytes: None,
                ..Budget::default()
            }),
            ..Options::default()
        },
    ))
}

/// Create an iterator over YAML documents from any `std::io::Read`, with configurable options.
///
/// This is the multi-document counterpart to [`from_reader_with_options`]. It does not load
/// the entire input into memory. Instead, it streams the reader, deserializing one document
/// at a time into values of type `T`, yielding them through the returned iterator. Documents
/// that are completely empty or null-like (e.g., `""`, `~`, or `null`) are skipped.
///
/// Generic parameters
/// - `R`: the concrete reader type that implements [`std::io::Read`]. You rarely need to spell
///   this out; it is almost always inferred from the `reader` value you pass in. When using
///   turbofish, you can write `_` for this parameter to let the compiler infer it.
/// - `T`: the type to deserialize each YAML document into. This must implement [`serde::de::DeserializeOwned`].
///
/// Lifetimes
/// - `'a`: the lifetime of the returned iterator. It is tied to the lifetime of the provided
///   `reader` value because the iterator borrows internal state that references the reader.
///   In practice, this means the iterator cannot outlive the reader it was created from.
///
/// Limits and budget
/// - All parsing limits configured via [`Options::budget`] (such as maximum events, nodes,
///   nesting depth, total scalar bytes) are enforced while streaming. A hard input-byte cap
///   is also enforced via `Budget::max_reader_input_bytes` (256 MiB by default), set this
///   to None if you need a streamer to exist for arbitrary long time.
/// - Alias replay limits from [`Options::alias_limits`] are also enforced to mitigate alias bombs.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Simple { id: usize }
///
/// let yaml = b"id: 1\n---\nid: 2\n";
/// let mut reader = std::io::Cursor::new(&yaml[..]);
///
/// // Type `T` is inferred from the collection target (Vec<Simple>).
/// let values: Vec<Simple> = serde_saphyr::read(&mut reader)
///     .map(|r| r.unwrap())
///     .collect();
/// assert_eq!(values.len(), 2);
/// assert_eq!(values[0].id, 1);
/// ```
///
/// Specifying only `T` with turbofish and letting `R` be inferred using `_`:
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Simple { id: usize }
///
/// let yaml = b"id: 10\n---\nid: 20\n";
/// let mut reader = std::io::Cursor::new(&yaml[..]);
///
/// // First turbofish parameter is R (reader type) which we let the compiler infer via `_`.
/// let iter = serde_saphyr::read_with_options::<_, Simple>(&mut reader, serde_saphyr::Options::default());
/// let ids: Vec<usize> = iter.map(|res| res.unwrap().id).collect();
/// assert_eq!(ids, vec![10, 20]);
/// ```
///
/// - Each `next()` yields either `Ok(T)` for a successfully deserialized document or `Err(Error)`
///   if parsing fails or a budget/alias limit is exceeded. After an error, the iterator ends.
/// - Empty/null-like documents are skipped and produce no items.
pub fn read_with_options<'a, R, T>(
    reader: &'a mut R, // iterator must not outlive this borrow
    options: Options,
) -> impl Iterator<Item = Result<T, Error>> + 'a
where
    R: Read + 'a,
    T: DeserializeOwned + 'a,
{
    struct ReadIter<'a, T> {
        src: LiveEvents<'a>, // borrows from `reader`
        cfg: crate::de::Cfg,
        finished: bool,
        _marker: std::marker::PhantomData<T>,
    }

    impl<'a, T> Iterator for ReadIter<'a, T>
    where
        T: DeserializeOwned + 'a,
    {
        type Item = Result<T, Error>;

        fn next(&mut self) -> Option<Self::Item> {
            if self.finished {
                return None;
            }
            loop {
                match self.src.peek() {
                    Ok(Some(Ev::Scalar { value, style, .. }))
                        if scalar_is_nullish(value, style) =>
                    {
                        let _ = self.src.next();
                        continue;
                    }
                    Ok(Some(_)) => {
                        let res = crate::anchor_store::with_document_scope(|| {
                            T::deserialize(crate::de::Deser::new(&mut self.src, self.cfg))
                        });
                        return Some(res);
                    }
                    Ok(None) => {
                        self.finished = true;
                        if let Err(e) = self.src.finish() {
                            return Some(Err(e));
                        }
                        return None;
                    }
                    Err(e) => {
                        self.finished = true;
                        let _ = self.src.finish();
                        return Some(Err(e));
                    }
                }
            }
        }
    }

    let cfg = crate::de::Cfg::from_options(&options);
    let src = LiveEvents::from_reader(
        reader,
        options.budget,
        options.alias_limits,
        false,
        EnforcingPolicy::PerDocument,
    );

    ReadIter::<T> {
        src,
        cfg,
        finished: false,
        _marker: std::marker::PhantomData,
    }
}
