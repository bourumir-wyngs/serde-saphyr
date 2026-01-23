use crate::budget::EnforcingPolicy;
use crate::live_events::LiveEvents;

use super::{Cfg, Error, Events, Options, YamlDeserializer};

fn normalize_str_input(input: &str) -> &str {
    // Normalize: ignore a single leading UTF-8 BOM if present.
    input.strip_prefix('\u{FEFF}').unwrap_or(input)
}

fn deserialize_with_scope<R, F, W>(
    src: &mut LiveEvents,
    cfg: Cfg,
    f: F,
    wrap_err: W,
) -> Result<R, Error>
where
    for<'de> F: FnOnce(crate::Deserializer<'de>) -> Result<R, Error>,
    W: Fn(Error) -> Error,
{
    let value_res = crate::anchor_store::with_document_scope(|| f(YamlDeserializer::new(src, cfg)));
    match value_res {
        Ok(v) => Ok(v),
        Err(e) => {
            if src.synthesized_null_emitted() {
                // If the only thing in the input was an empty document (synthetic null),
                // surface this as an EOF error to preserve expected error semantics
                // for incompatible target types (e.g., bool).
                Err(wrap_err(Error::eof().with_location(src.last_location())))
            } else {
                Err(wrap_err(e))
            }
        }
    }
}

fn enforce_single_document_and_finish<W>(
    src: &mut LiveEvents,
    multiple_docs_msg: &'static str,
    wrap_err: W,
) -> Result<(), Error>
where
    W: Fn(Error) -> Error,
{
    // After finishing first document, peek ahead to detect either another document/content
    // or trailing garbage. If a scan error occurs but we have seen a DocumentEnd ("..."),
    // ignore the trailing garbage. Otherwise, surface the error.
    match src.peek() {
        Ok(Some(_)) => {
            return Err(wrap_err(
                Error::msg(multiple_docs_msg).with_location(src.last_location()),
            ));
        }
        Ok(None) => {}
        Err(e) => {
            if src.seen_doc_end() {
                // Trailing garbage after a proper document end marker is ignored.
            } else {
                return Err(wrap_err(e));
            }
        }
    }

    src.finish().map_err(wrap_err)
}

/// Create a streaming [`crate::Deserializer`] for a YAML string and run a closure against it.
///
/// This is useful for tooling that needs access to the underlying Serde deserializer,
/// such as wrappers that report unknown/ignored fields (e.g. the `serde_ignored` crate)
/// or wrappers that augment error paths.
///
/// The deserializer borrows internal parsing state, so it cannot be returned directly.
/// Instead, you provide a closure `f` that performs the desired deserialization.
#[allow(deprecated)]
pub fn with_deserializer_from_str_with_options<R, F>(
    input: &str,
    options: Options,
    f: F,
) -> Result<R, Error>
where
    for<'de> F: FnOnce(crate::Deserializer<'de>) -> Result<R, Error>,
{
    let input = normalize_str_input(input);

    let with_snippet = options.with_snippet;
    let crop_radius = options.crop_radius;

    let cfg = Cfg::from_options(&options);
    // Do not stop at DocumentEnd; we'll probe for trailing content/errors explicitly.
    let mut src = LiveEvents::from_str(
        input,
        options.budget,
        options.budget_report,
        options.budget_report_cb,
        options.alias_limits,
        false,
    );

    let wrap_err = |e| crate::maybe_with_snippet(e, input, with_snippet, crop_radius);

    let value = deserialize_with_scope(&mut src, cfg, f, wrap_err)?;
    enforce_single_document_and_finish(
        &mut src,
        "multiple YAML documents detected; use from_multiple or from_multiple_with_options",
        wrap_err,
    )?;
    Ok(value)
}

/// Convenience wrapper around [`with_deserializer_from_str_with_options`] using
/// [`Options::default`].
pub fn with_deserializer_from_str<R, F>(input: &str, f: F) -> Result<R, Error>
where
    for<'de> F: FnOnce(crate::Deserializer<'de>) -> Result<R, Error>,
{
    with_deserializer_from_str_with_options(input, Options::default(), f)
}

/// Create a streaming [`crate::Deserializer`] for a UTF-8 byte slice and run a closure against it.
///
/// This is equivalent to [`with_deserializer_from_str`], but validates the input is UTF-8.
pub fn with_deserializer_from_slice<R, F>(bytes: &[u8], f: F) -> Result<R, Error>
where
    for<'de> F: FnOnce(crate::Deserializer<'de>) -> Result<R, Error>,
{
    with_deserializer_from_slice_with_options(bytes, Options::default(), f)
}

/// Create a streaming [`crate::Deserializer`] for a UTF-8 byte slice with configurable [`Options`]
/// and run a closure against it.
pub fn with_deserializer_from_slice_with_options<R, F>(
    bytes: &[u8],
    options: Options,
    f: F,
) -> Result<R, Error>
where
    for<'de> F: FnOnce(crate::Deserializer<'de>) -> Result<R, Error>,
{
    let s = std::str::from_utf8(bytes).map_err(|_| Error::msg("input is not valid UTF-8"))?;
    with_deserializer_from_str_with_options(s, options, f)
}

/// Create a streaming [`crate::Deserializer`] for any [`std::io::Read`] and run a closure against it.
///
/// This is the reader-based counterpart to [`with_deserializer_from_str`]. It consumes a
/// byte-oriented reader, decodes it to UTF-8, and streams events into the deserializer.
pub fn with_deserializer_from_reader<R, Out, F>(reader: R, f: F) -> Result<Out, Error>
where
    R: std::io::Read,
    for<'de> F: FnOnce(crate::Deserializer<'de>) -> Result<Out, Error>,
{
    with_deserializer_from_reader_with_options(reader, Options::default(), f)
}

/// Create a streaming [`crate::Deserializer`] for any [`std::io::Read`] with configurable [`Options`]
/// and run a closure against it.
#[allow(deprecated)]
pub fn with_deserializer_from_reader_with_options<R, Out, F>(
    reader: R,
    options: Options,
    f: F,
) -> Result<Out, Error>
where
    R: std::io::Read,
    for<'de> F: FnOnce(crate::Deserializer<'de>) -> Result<Out, Error>,
{
    let cfg = Cfg::from_options(&options);
    let mut src = LiveEvents::from_reader(
        reader,
        options.budget,
        options.budget_report,
        options.budget_report_cb,
        options.alias_limits,
        false,
        EnforcingPolicy::AllContent,
    );

    let wrap_err = |e| e;

    let value = deserialize_with_scope(&mut src, cfg, f, wrap_err)?;
    enforce_single_document_and_finish(
        &mut src,
        "multiple YAML documents detected; use read or read_with_options to obtain the iterator",
        wrap_err,
    )?;
    Ok(value)
}
