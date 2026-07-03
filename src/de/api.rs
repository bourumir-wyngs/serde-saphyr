use std::io::Read;

use serde_core::de::DeserializeOwned;

use super::with_deserializer::{deserialize_with_scope_and_null_policy, normalize_str_input};
use super::{Error, Ev, Events, Options, ring_reader};
use crate::budget::EnforcingPolicy;
use crate::live_events::LiveEvents;
use crate::parse_scalars::scalar_document_is_empty_or_null;

#[cfg(all(feature = "deserialize", feature = "include"))]
pub(crate) fn resolver_from_options<'a>(
    options: Options,
) -> Option<Box<crate::input_source::IncludeResolver<'a>>> {
    options.include_resolver.clone().map(|rc_refcell| {
        Box::new(move |req: crate::input_source::IncludeRequest<'_>| rc_refcell.borrow_mut()(req))
            as Box<crate::input_source::IncludeResolver<'a>>
    })
}

/// Deserialize any `T: serde::de::Deserialize<'de>` directly from a YAML string.
///
/// This is the simplest entry point; it parses a single YAML document. If the
/// input contains multiple documents, this returns an error advising to use
/// [`from_multiple`] or [`from_multiple_with_options`].
///
/// This function supports both owned types (like `String`) and borrowed types
/// (like `&str`). For borrowed types, the deserialized value's lifetime is tied
/// to the input string's lifetime.
///
/// **Note**: Borrowing only works for simple plain scalars that don't require
/// any transformation (no multi-line folding, no escape processing). For
/// transformed strings, deserialization to `&str` will fail with a helpful
/// error message suggesting to use `String` or `Cow<str>` instead.
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
///
/// Example: read a structure with borrowed string fields.
///
/// Borrowed strings are supported when deserializing from an in-memory input (`from_str` / `from_slice`),
/// and only when the scalar exists verbatim in the input (i.e., no escape processing, folding, or other
/// normalization is required). If the YAML scalar requires transformation, deserializing into `&str`
/// fails with an error suggesting `String` or `Cow<str>`.
///
/// Note: reader-based entry points like [`from_reader`] require `DeserializeOwned` and therefore cannot
/// return values that borrow from the input.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Data<'a> {
///     name: &'a str,
///     value: i32,
/// }
///
/// let yaml = "name: hello\nvalue: 42\n";
///
/// let data: Data = serde_saphyr::from_str(yaml).unwrap();
/// assert_eq!(data.name, "hello");
/// assert_eq!(data.value, 42);
/// ```
#[cfg(feature = "deserialize")]
pub fn from_str<'de, T>(input: &'de str) -> Result<T, Error>
where
    T: serde_core::de::Deserialize<'de>,
{
    from_str_with_options(input, Options::default())
}

#[allow(deprecated)]
#[cfg(feature = "deserialize")]
fn from_str_with_options_impl<'de, T>(input: &'de str, options: Options) -> Result<T, Error>
where
    T: serde_core::de::Deserialize<'de>,
{
    super::with_deserializer::with_deserializer_from_str_with_options(input, options, |de| {
        T::deserialize(de)
    })
}

/// Deserialize a single YAML document with configurable [`Options`].
///
/// This function supports both owned types (like `String`) and borrowed types
/// (like `&str`). For borrowed types, the deserialized value's lifetime is tied
/// to the input string's lifetime.
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
/// let options = serde_saphyr::options! {
///     budget: serde_saphyr::budget! {
///         max_anchors: 200,
///     },
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
/// };
/// let cfg: Config = serde_saphyr::from_str_with_options(yaml, options).unwrap();
/// assert_eq!(cfg.retries, 5);
/// ```
#[allow(deprecated)]
#[cfg(feature = "deserialize")]
pub fn from_str_with_options<'de, T>(input: &'de str, options: Options) -> Result<T, Error>
where
    T: serde_core::de::Deserialize<'de>,
{
    from_str_with_options_impl(input, options)
}

#[cfg(feature = "deserialize")]
pub(crate) fn maybe_with_snippet(
    err: Error,
    input: &str,
    with_snippet: bool,
    crop_radius: usize,
) -> Error {
    if !(with_snippet && crop_radius > 0 && err.location().is_some()) {
        return err;
    }

    err.with_snippet(input, crop_radius)
}

#[cfg(feature = "deserialize")]
pub(crate) struct RootFragment<'a> {
    pub text: &'a str,
    pub start_line: usize,
    pub source_name: &'a str,
}

#[cfg(feature = "deserialize")]
pub(crate) struct StrSnippetContext<'a> {
    input: &'a str,
    with_snippet: bool,
    crop_radius: usize,
}

#[cfg(feature = "deserialize")]
impl<'a> StrSnippetContext<'a> {
    pub(crate) fn new(input: &'a str, with_snippet: bool, crop_radius: usize) -> Self {
        Self {
            input,
            with_snippet,
            crop_radius,
        }
    }

    pub(crate) fn attach_snippet(&self, err: Error, src: &LiveEvents<'_>) -> Error {
        maybe_with_snippet_from_events(err, self.input, src, self.with_snippet, self.crop_radius)
    }
}

#[cfg(feature = "deserialize")]
pub(crate) struct ReaderSnippetContext<R> {
    shared_ring: ring_reader::SharedRingReader<R>,
    with_snippet: bool,
    crop_radius: usize,
}

#[cfg(feature = "deserialize")]
impl<R: Read> ReaderSnippetContext<R> {
    pub(crate) fn new(
        reader: R,
        with_snippet: bool,
        crop_radius: usize,
    ) -> (Self, ring_reader::SharedRingReaderHandle<R>) {
        let shared_ring = ring_reader::SharedRingReader::new(reader);
        let ring_handle = ring_reader::SharedRingReaderHandle::new(&shared_ring);
        (
            Self {
                shared_ring,
                with_snippet,
                crop_radius,
            },
            ring_handle,
        )
    }

    pub(crate) fn attach_snippet(&self, err: Error, src: &LiveEvents<'_>) -> Error {
        if !self.with_snippet || self.crop_radius == 0 {
            return err;
        }

        match self.shared_ring.get_recent() {
            Ok(snapshot) => {
                let text = String::from_utf8_lossy(&snapshot.bytes);
                let root = RootFragment {
                    text: text.as_ref(),
                    start_line: snapshot.start_line,
                    source_name: "input",
                };
                maybe_with_snippet_from_events_and_root_fragment(
                    err,
                    Some(&root),
                    text.as_ref(),
                    src,
                    self.with_snippet,
                    self.crop_radius,
                )
            }
            Err(_) => err,
        }
    }
}

#[cfg(all(feature = "deserialize", feature = "include"))]
fn with_root_additional_snippet(
    err: Error,
    root: Option<&RootFragment<'_>>,
    input: &str,
    location: &crate::Location,
    crop_radius: usize,
) -> Error {
    match root {
        Some(root) => err.with_additional_snippet_offset_named(
            root.text,
            root.start_line,
            root.source_name,
            location,
            crop_radius,
        ),
        None => err.with_additional_snippet_named(input, "input", location, crop_radius),
    }
}

#[cfg(all(feature = "deserialize", feature = "include"))]
fn recorded_source_snippet_chain<'a>(
    events: &'a crate::live_events::LiveEvents<'_>,
    location: &crate::Location,
) -> Option<Vec<&'a crate::include_stack::RecordedSource>> {
    let chain = events.recorded_source_chain(location.source_id());
    // Bail unless the innermost source has recorded text — the snippet renderer needs it.
    chain.first()?.text.as_deref()?;
    Some(chain)
}

#[cfg(all(feature = "deserialize", feature = "include"))]
fn with_recorded_source_snippets(
    err: Error,
    root: Option<&RootFragment<'_>>,
    input: &str,
    chain: &[&crate::include_stack::RecordedSource],
    crop_radius: usize,
) -> Error {
    let Some(current) = chain.first() else {
        return with_root_or_input_snippet(err, root, input, crop_radius);
    };
    let Some(source_text) = current.text.as_deref() else {
        return with_root_or_input_snippet(err, root, input, crop_radius);
    };
    let mut err_with_snippet =
        err.with_snippet_named(source_text, current.name.as_str(), crop_radius);

    for window in chain.windows(2) {
        let child = window[0];
        let parent = window[1];
        if child.include_location == crate::Location::UNKNOWN {
            continue;
        }

        match parent.text.as_deref() {
            Some(parent_text) => {
                err_with_snippet = err_with_snippet.with_additional_snippet_named(
                    parent_text,
                    parent.name.as_str(),
                    &child.include_location,
                    crop_radius,
                );
            }
            None if parent.parent_source_id.is_none() => {
                err_with_snippet = with_root_additional_snippet(
                    err_with_snippet,
                    root,
                    input,
                    &child.include_location,
                    crop_radius,
                );
            }
            None => {}
        }
    }
    err_with_snippet
}

#[cfg(all(feature = "deserialize", feature = "include"))]
fn with_root_or_input_snippet(
    err: Error,
    root: Option<&RootFragment<'_>>,
    input: &str,
    crop_radius: usize,
) -> Error {
    match root {
        Some(root) => {
            err.with_snippet_offset_named(root.text, root.start_line, root.source_name, crop_radius)
        }
        None => maybe_with_snippet(err, input, true, crop_radius),
    }
}

#[cfg(feature = "deserialize")]
pub(crate) fn maybe_with_snippet_from_events_and_root_fragment(
    err: Error,
    root: Option<&RootFragment<'_>>,
    input: &str,
    #[allow(unused_variables)] events: &crate::live_events::LiveEvents<'_>,
    with_snippet: bool,
    crop_radius: usize,
) -> Error {
    if !(with_snippet && crop_radius > 0 && err.location().is_some()) {
        return err;
    }

    #[cfg(feature = "include")]
    if let Some(loc) = err.location()
        && let Some(chain) = recorded_source_snippet_chain(events, &loc)
    {
        return with_recorded_source_snippets(err, root, input, &chain, crop_radius);
    }

    match root {
        Some(root) => {
            err.with_snippet_offset_named(root.text, root.start_line, root.source_name, crop_radius)
        }
        None => maybe_with_snippet(err, input, with_snippet, crop_radius),
    }
}

#[cfg(feature = "deserialize")]
pub(crate) fn maybe_with_snippet_from_events(
    err: Error,
    input: &str,
    #[allow(unused_variables)] events: &crate::live_events::LiveEvents<'_>,
    with_snippet: bool,
    crop_radius: usize,
) -> Error {
    maybe_with_snippet_from_events_and_root_fragment(
        err,
        None,
        input,
        events,
        with_snippet,
        crop_radius,
    )
}

/// Deserialize multiple YAML documents from a single string into a vector of `T`.
/// Completely empty documents are ignored and not included in the returned vector.
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
#[cfg(feature = "deserialize")]
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
/// let options = serde_saphyr::options! {
///     budget: serde_saphyr::budget! {
///         max_anchors: 200,
///     },
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
/// };
/// let cfgs: Vec<Config> = serde_saphyr::from_multiple_with_options(yaml, options).unwrap();
/// assert_eq!(cfgs.len(), 2);
/// assert!(!cfgs[1].enabled);
/// ```
#[allow(deprecated)]
#[cfg(feature = "deserialize")]
pub fn from_multiple_with_options<T: DeserializeOwned>(
    input: &str,
    options: Options,
) -> Result<Vec<T>, Error> {
    let input = normalize_str_input(input);
    let snippet_ctx = StrSnippetContext::new(input, options.with_snippet, options.crop_radius);
    let cfg = crate::de::Cfg::from_options(&options);
    let mut src = LiveEvents::from_str(input, options);
    let mut values = Vec::new();
    let wrap_err = |e, src: &LiveEvents<'_>| snippet_ctx.attach_snippet(e, src);

    loop {
        match src.peek() {
            // Skip documents that are explicit null-like scalars ("", "~", or "null").
            Ok(Some(Ev::Scalar {
                value: s,
                style,
                tag,
                ..
            })) if scalar_document_is_empty_or_null(tag, s, style) => {
                let _ = src.next()?; // consume the null scalar document
                // Do not push anything for this document; move to the next one.
                continue;
            }
            Ok(Some(_)) => {
                let value = deserialize_with_scope_and_null_policy(
                    &mut src,
                    cfg,
                    |de| T::deserialize(de),
                    wrap_err,
                    |_| false,
                )?;
                values.push(value);
            }
            Ok(None) => break,
            Err(e) => {
                return Err(wrap_err(e, &src));
            }
        }
    }

    if let Err(e) = src.finish() {
        return Err(wrap_err(e, &src));
    }
    Ok(values)
}

/// Deserialize a single YAML document from a UTF-8 byte slice.
///
/// UTF-8 only (due borrowing). For UTF-16 input, use [`from_reader`] instead:
/// `let reader = std::io::Cursor::new(bytes);`
/// `let cfg: Config = serde_saphyr::from_reader(reader)?;`
///
/// This is equivalent to [`from_str`], but accepts `&[u8]` and validates it is
/// valid UTF-8 before parsing.
///
/// This function supports both owned types (like `String`) and borrowed types
/// (like `&str`). For borrowed types, the deserialized value's lifetime is tied
/// to the input byte slice's lifetime.
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
#[cfg(feature = "deserialize")]
pub fn from_slice<'de, T>(bytes: &'de [u8]) -> Result<T, Error>
where
    T: serde_core::Deserialize<'de>,
{
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
/// let options = serde_saphyr::options! {
///     budget: serde_saphyr::budget! {
///         max_anchors: 200,
///     },
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
/// };
/// let cfg: Config = serde_saphyr::from_slice_with_options(bytes, options).unwrap();
/// assert_eq!(cfg.retries, 5);
/// ```
#[cfg(feature = "deserialize")]
pub fn from_slice_with_options<'de, T>(bytes: &'de [u8], options: Options) -> Result<T, Error>
where
    T: serde_core::Deserialize<'de>,
{
    let s = std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8Input)?;
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
#[cfg(feature = "deserialize")]
pub fn from_slice_multiple<T: DeserializeOwned>(bytes: &[u8]) -> Result<Vec<T>, Error> {
    from_slice_multiple_with_options(bytes, Options::default())
}

/// Deserialize multiple YAML documents from bytes with configurable [`Options`].
/// Completely empty documents are ignored and not included in the returned vector.
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
/// let options = serde_saphyr::options! {
///     budget: serde_saphyr::budget! {
///         max_anchors: 200,
///     },
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
/// };
/// let cfgs: Vec<Config> = serde_saphyr::from_slice_multiple_with_options(bytes, options).unwrap();
/// assert_eq!(cfgs.len(), 2);
/// assert!(!cfgs[1].enabled);
/// ```
#[cfg(feature = "deserialize")]
pub fn from_slice_multiple_with_options<T: DeserializeOwned>(
    bytes: &[u8],
    options: Options,
) -> Result<Vec<T>, Error> {
    let s = std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8Input)?;
    from_multiple_with_options(s, options)
}

/// Deserialize a single YAML document from any `std::io::Read`.
///
/// Reader-based entry points accept BOM-marked UTF-8, UTF-16LE, and UTF-16BE. If no
/// recognized BOM is present, the input bytes are treated as UTF-8.
///
/// This method parses as it reads, without loading the entire input into memory first. Hence,
/// budget limits protect against large (potentially malicious) input.
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
#[cfg(feature = "deserialize")]
pub fn from_reader<'a, R: std::io::Read + 'a, T: DeserializeOwned>(reader: R) -> Result<T, Error> {
    from_reader_with_options(reader, Options::default())
}

/// Deserialize a single YAML document from any `std::io::Read` with configurable `Options`.
///
/// This is the reader-based counterpart to [`from_str_with_options`]. It consumes a
/// byte-oriented reader and streams events into the deserializer. BOM-marked
/// UTF-8, UTF-16LE, and UTF-16BE inputs are transcoded to UTF-8 internally
/// before parsing. If no recognized BOM is present, the input bytes are
/// treated as UTF-8.
///
/// This method parses as it reads, without loading the entire input into memory first. Hence,
/// budget limits protect against large (potentially malicious) input.
///
/// Notes on limits and large inputs
/// - Parsing limits: Use [`Options::budget`] to constrain YAML complexity (events, nodes,
///   nesting depth, total scalar bytes, total comment bytes, number of documents, anchors,
///   aliases, etc.). These
///   limits are enforced during parsing and are enabled by default via `Options::default()`.
/// - Byte-level input cap: `Budget::max_reader_input_bytes` is enforced while reading.
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
/// let opts = serde_saphyr::options! {
///     budget: serde_saphyr::budget! {
///         max_events: 10_000,
///         max_reader_input_bytes: Some(1024),
///     },
/// };
///
/// let p: Point = serde_saphyr::from_reader_with_options(reader, opts).unwrap();
/// assert_eq!(p, Point { x: 3, y: 4 });
/// ```
///
/// Error behavior
/// - If an empty document is provided (no content), a type-mismatch (eof) error is returned when
///   attempting to deserialize into non-null-like targets.
/// - If the reader contains multiple documents, an error is returned suggesting the
///   `read`/`read_with_options` iterator APIs.
/// - If `Options::budget` is set and a limit is exceeded, an error is returned early.
#[allow(deprecated)]
#[cfg(feature = "deserialize")]
pub fn from_reader_with_options<'a, R: std::io::Read + 'a, T: DeserializeOwned>(
    reader: R,
    options: Options,
) -> Result<T, Error> {
    super::with_deserializer::with_deserializer_from_reader_with_options(reader, options, |de| {
        T::deserialize(de)
    })
}

/// Create an iterator over YAML documents from any `std::io::Read` using default options.
///
/// This is a convenience wrapper around [`read_with_options`] that uses the
/// same defaults as [`Options::default`] **except** it disables the
/// `max_reader_input_bytes` budget to better support long-lived streams.
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
/// - Uses the same limits as `Options::default()` (events, nodes, nesting depth, total scalar
///   bytes, total comment bytes) and the default alias-replay caps. The only change is that
///   `Budget::max_reader_input_bytes` is set to `None` so the streaming iterator can handle
///   arbitrarily long inputs. To customize these limits, call [`read_with_options`] and set
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
///
/// *Note* Some content of the next document is read before the current parsed document is emitted.
/// Hence, while streaming is good for safely parsing large files with multiple documents without
/// loading it into RAM in advance, it does not emit each document exactly
/// after `---`  is encountered.
#[cfg(feature = "deserialize")]
pub fn read<'a, R, T>(reader: &'a mut R) -> Box<dyn Iterator<Item = Result<T, Error>> + 'a>
where
    R: Read + 'a,
    T: DeserializeOwned + 'a,
{
    Box::new(read_with_options(
        reader,
        crate::options! {
            budget: crate::budget! {
                max_reader_input_bytes: None,
            },
        },
    ))
}

/// Create an iterator over YAML documents from any `std::io::Read`, with configurable options.
///
/// This is the multi-document counterpart to [`from_reader_with_options`]. It does not load
/// the entire input into memory. Instead, it streams the reader, deserializing one document
/// at a time into values of type `T`, yielding them through the returned iterator. Documents
/// that are completely empty or null-like (e.g., `""`, `~`, or `null`) are skipped.
/// Like [`from_reader_with_options`], BOM-marked UTF-8, UTF-16LE, and UTF-16BE
/// inputs are transcoded to UTF-8 internally before parsing. If no recognized
/// BOM is present, the input bytes are treated as UTF-8.
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
///   nesting depth, total scalar bytes, total comment bytes) are enforced while streaming. The
///   reader input-byte cap is also enforced via `Budget::max_reader_input_bytes` (256 MiB by
///   default). Set this to `None` if the stream may legitimately run without a fixed byte cap.
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
///   if parsing or deserialization fails.
/// - After a **deserialization error** (e.g., type mismatch, missing field), the iterator
///   automatically recovers by skipping to the next document boundary (`---`) and continues
///   iteration. This allows processing subsequent valid documents even when some fail.
/// - After a **syntax error** or **budget/alias limit exceeded**, the iterator ends because
///   the parser state may be unrecoverable.
/// - Empty/null-like documents are skipped and produce no items.
#[allow(deprecated)]
#[cfg(feature = "deserialize")]
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
                    Ok(Some(Ev::Scalar {
                        value, style, tag, ..
                    })) if scalar_document_is_empty_or_null(tag, value, style) => {
                        let _ = self.src.next();
                        continue;
                    }
                    Ok(Some(_)) => {
                        let res = deserialize_with_scope_and_null_policy(
                            &mut self.src,
                            self.cfg,
                            |de| T::deserialize(de),
                            |e, _| e,
                            |_| false,
                        );
                        if res.is_err() {
                            // After a deserialization error, skip remaining events in the
                            // current document and try to recover at the next document boundary.
                            // If no next document is found, mark as finished.
                            if !self.src.skip_to_next_document() {
                                self.finished = true;
                            }
                        }
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
    let src = LiveEvents::from_reader(reader, options, EnforcingPolicy::PerDocument);

    ReadIter::<T> {
        src,
        cfg,
        finished: false,
        _marker: std::marker::PhantomData,
    }
}
