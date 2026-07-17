use super::api::{ReaderSnippetContext, StrSnippetContext};
use super::with_deserializer::{
    enforce_single_document_and_finish, normalize_str_input, run_with_document_scope,
};
use crate::budget::EnforcingPolicy;
use crate::de::{Error, Ev, Events, Options};
#[cfg(feature = "garde")]
use crate::de_error::collect_garde_issues;
#[cfg(feature = "validator")]
use crate::de_error::collect_validator_issues;
use crate::de_error::redact_issue;
use crate::live_events::LiveEvents;
use crate::parse_scalars::scalar_document_is_empty_or_null;
use crate::path_map::PathMap;
use serde_core::de::DeserializeOwned;
use std::io::Read;

#[cfg(feature = "garde")]
use garde::Validate;
#[cfg(feature = "validator")]
use validator::Validate as ValidatorValidate;

use crate::de_error::ValidationSource;

#[cfg(any(feature = "garde", feature = "validator"))]
fn synthesized_null_error_should_be_eof(error: &Error) -> bool {
    !error.is_validation_error()
}

#[cfg(feature = "garde")]
fn garde_validation_error(report: garde::Report, locations: &PathMap) -> Error {
    Error::validation_error(
        ValidationSource::Garde,
        collect_garde_issues(&report)
            .into_iter()
            .map(redact_issue)
            .collect(),
        locations.clone(),
    )
}

#[cfg(feature = "validator")]
fn validator_validation_error(errors: validator::ValidationErrors, locations: &PathMap) -> Error {
    Error::validation_error(
        ValidationSource::Validator,
        collect_validator_issues(&errors)
            .into_iter()
            .map(redact_issue)
            .collect(),
        locations.clone(),
    )
}

/// Deserialize a single YAML document with configurable [`Options`], and also
/// return a map from validation paths to source [`Location`]s.
fn from_str_with_options_and_path_recorder_validated<T, F>(
    input: &str,
    options: Options,
    validate: F,
) -> Result<T, Error>
where
    T: DeserializeOwned,
    F: FnOnce(&T, &PathMap) -> Result<(), Error>,
{
    let input = normalize_str_input(input);
    let snippet_ctx = StrSnippetContext::new(input, options.with_snippet, options.crop_radius);
    let cfg = crate::de::Cfg::from_options(&options);
    let mut src = LiveEvents::from_str(input, options);
    let mut recorder = crate::path_map::PathRecorder::new();
    let wrap_err = |e, src: &LiveEvents<'_>| snippet_ctx.attach_snippet(e, src);

    let value = run_with_document_scope(
        &mut src,
        |src| {
            let value = crate::de::with_root_redaction(
                crate::de::YamlDeserializer::new_with_path_recorder(src, cfg, &mut recorder),
                |de| T::deserialize(de),
            )?;
            validate(&value, &recorder.map)?;
            Ok(value)
        },
        wrap_err,
        synthesized_null_error_should_be_eof,
    )?;

    enforce_single_document_and_finish(
        &mut src,
        "use from_multiple or from_multiple_with_options",
        wrap_err,
    )?;

    Ok(value)
}

/// Deserialize a single YAML document from a YAML string and validate it with `garde`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "garde")]
pub fn from_str_valid<T>(input: &str) -> Result<T, Error>
where
    T: DeserializeOwned + garde::Validate,
    <T as garde::Validate>::Context: Default,
{
    from_str_with_options_valid(input, Options::default())
}

/// Deserialize a single YAML document with configurable [`Options`] and validate it with `garde`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "garde")]
pub fn from_str_with_options_valid<T>(input: &str, options: Options) -> Result<T, Error>
where
    T: DeserializeOwned + garde::Validate,
    <T as garde::Validate>::Context: Default,
{
    from_str_with_options_and_path_recorder_validated::<T, _>(input, options, |value, locs| {
        Validate::validate(value).map_err(|report| garde_validation_error(report, locs))
    })
}

/// Deserialize a single YAML document with configurable [`Options`] and validate it with
/// `garde` using the context associated with `T`'s [`garde::Validate`] implementation.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "garde")]
pub fn from_str_with_options_context_valid<T>(
    input: &str,
    options: Options,
    context: &<T as garde::Validate>::Context,
) -> Result<T, Error>
where
    T: DeserializeOwned + garde::Validate,
{
    from_str_with_options_and_path_recorder_validated::<T, _>(input, options, |value, locs| {
        Validate::validate_with(value, context)
            .map_err(|report| garde_validation_error(report, locs))
    })
}

/// Deserialize multiple YAML documents from a YAML string and validate each with `garde`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "garde")]
pub fn from_multiple_valid<T: DeserializeOwned + garde::Validate>(
    input: &str,
) -> Result<Vec<T>, Error>
where
    <T as garde::Validate>::Context: Default,
{
    from_multiple_with_options_valid(input, Options::default())
}

fn from_multiple_with_options_validated<T, F>(
    input: &str,
    options: Options,
    source: ValidationSource,
    validate: F,
) -> Result<Vec<T>, Error>
where
    T: DeserializeOwned,
    F: Fn(&T, &PathMap) -> Result<(), Error>,
{
    let input = normalize_str_input(input);
    let snippet_ctx = StrSnippetContext::new(input, options.with_snippet, options.crop_radius);
    let cfg = crate::de::Cfg::from_options(&options);
    let mut src = LiveEvents::from_str(input, options);
    let mut values = Vec::new();
    let mut validation_errors: Vec<Error> = Vec::new();
    let wrap_err = |e, src: &LiveEvents<'_>| snippet_ctx.attach_snippet(e, src);

    loop {
        let peeked = match src.peek() {
            Ok(peeked) => peeked,
            Err(e) => return Err(wrap_err(e, &src)),
        };

        match peeked {
            // Skip documents that are explicit null-like scalars ("", "~", or "null").
            Some(Ev::Scalar {
                value: s,
                style,
                tag,
                ..
            }) if scalar_document_is_empty_or_null(tag, s, style) => {
                let _ = src.next()?; // consume the null scalar document
                continue;
            }
            Some(_) => {
                let mut recorder = crate::path_map::PathRecorder::new();
                let value_res: Result<T, Error> = run_with_document_scope(
                    &mut src,
                    |src| {
                        let value = crate::de::with_root_redaction(
                            crate::de::YamlDeserializer::new_with_path_recorder(
                                src,
                                cfg,
                                &mut recorder,
                            ),
                            |de| T::deserialize(de),
                        )?;
                        validate(&value, &recorder.map)?;
                        Ok(value)
                    },
                    |e, _| e,
                    |_| false,
                );
                let value = match value_res {
                    Ok(v) => v,
                    Err(e) if e.is_validation_error() => {
                        validation_errors.push(wrap_err(e, &src));
                        continue;
                    }
                    Err(e) => {
                        return Err(wrap_err(e, &src));
                    }
                };

                values.push(value);
            }
            None => break,
        }
    }

    if let Err(e) = src.finish() {
        return Err(wrap_err(e, &src));
    }

    if validation_errors.is_empty() {
        Ok(values)
    } else {
        Err(Error::validation_errors(source, validation_errors))
    }
}

/// Deserialize multiple YAML documents with configurable [`Options`] and validate each with `garde`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "garde")]
pub fn from_multiple_with_options_valid<T>(input: &str, options: Options) -> Result<Vec<T>, Error>
where
    T: DeserializeOwned + garde::Validate,
    <T as garde::Validate>::Context: Default,
{
    from_multiple_with_options_validated(input, options, ValidationSource::Garde, |value, locs| {
        Validate::validate(value).map_err(|report| garde_validation_error(report, locs))
    })
}

/// Deserialize a single YAML document from bytes and validate it with `garde`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "garde")]
pub fn from_slice_valid<T: DeserializeOwned + garde::Validate>(bytes: &[u8]) -> Result<T, Error>
where
    <T as garde::Validate>::Context: Default,
{
    from_slice_with_options_valid(bytes, Options::default())
}

/// Deserialize a single YAML document from bytes and validate it with `garde`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "garde")]
pub fn from_slice_with_options_valid<T: DeserializeOwned + garde::Validate>(
    bytes: &[u8],
    options: Options,
) -> Result<T, Error>
where
    <T as garde::Validate>::Context: Default,
{
    let s = std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8Input)?;
    from_str_with_options_valid(s, options)
}

/// Deserialize multiple YAML documents from bytes with options and validate each with `garde`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "garde")]
pub fn from_slice_multiple_with_options_valid<T>(
    bytes: &[u8],
    options: Options,
) -> Result<Vec<T>, Error>
where
    T: DeserializeOwned + garde::Validate,
    <T as garde::Validate>::Context: Default,
{
    let s = std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8Input)?;
    from_multiple_with_options_valid(s, options)
}

/// Deserialize a single YAML document from a reader and validate it with `garde`.
/// Snippets are attached on a best-effort basis for streamed root input, are available for
/// included text sources, and may be unavailable for included reader sources.
#[cfg(feature = "garde")]
pub fn from_reader_valid<R: std::io::Read, T>(reader: R) -> Result<T, Error>
where
    T: DeserializeOwned + garde::Validate,
    <T as garde::Validate>::Context: Default,
{
    from_reader_with_options_valid(reader, Options::default())
}

fn from_reader_with_options_validated<R, T, F>(
    reader: R,
    options: Options,
    validate: F,
    multiple_documents_hint: &'static str,
) -> Result<T, Error>
where
    R: Read,
    T: DeserializeOwned,
    F: FnOnce(&T, &PathMap) -> Result<(), Error>,
{
    let cfg = crate::de::Cfg::from_options(&options);
    let (snippet_ctx, ring_handle) =
        ReaderSnippetContext::new(reader, options.with_snippet, options.crop_radius);
    let mut src = LiveEvents::from_reader(ring_handle, options, EnforcingPolicy::AllContent);

    let mut recorder = crate::path_map::PathRecorder::new();
    let wrap_err = |e, src: &LiveEvents<'_>| snippet_ctx.attach_snippet(e, src);

    let value = run_with_document_scope(
        &mut src,
        |src| {
            let value = crate::de::with_root_redaction(
                crate::de::YamlDeserializer::new_with_path_recorder(src, cfg, &mut recorder),
                |de| T::deserialize(de),
            )?;
            validate(&value, &recorder.map)?;
            Ok(value)
        },
        wrap_err,
        synthesized_null_error_should_be_eof,
    )?;

    enforce_single_document_and_finish(&mut src, multiple_documents_hint, wrap_err)?;
    Ok(value)
}

/// Deserialize a single YAML document from a reader with options and validate it with `garde`.
/// Snippets are attached on a best-effort basis for streamed root input, are available for
/// included text sources, and may be unavailable for included reader sources.
#[cfg(feature = "garde")]
pub fn from_reader_with_options_valid<R: std::io::Read, T>(
    reader: R,
    options: Options,
) -> Result<T, Error>
where
    T: DeserializeOwned + garde::Validate,
    <T as garde::Validate>::Context: Default,
{
    from_reader_with_options_validated(
        reader,
        options,
        |value, locs| {
            Validate::validate(value).map_err(|report| garde_validation_error(report, locs))
        },
        "use read_valid or read_with_options_valid to obtain the iterator",
    )
}

/// Create an iterator over validated YAML documents from a reader.
/// Root streamed input gets snippets on a best-effort basis; included text sources retain full
/// snippets, while included reader sources may not have snippet text available.
#[cfg(feature = "garde")]
pub fn read_valid<'a, R, T>(reader: &'a mut R) -> impl Iterator<Item = Result<T, Error>> + 'a
where
    R: Read + 'a,
    T: DeserializeOwned + garde::Validate + 'a,
    <T as garde::Validate>::Context: Default,
{
    read_with_options_valid(reader, Options::default())
}

fn read_with_options_validated<'a, R, T, F>(
    reader: &'a mut R,
    options: Options,
    validate: F,
) -> impl Iterator<Item = Result<T, Error>> + 'a
where
    R: Read + 'a,
    T: DeserializeOwned + 'a,
    F: Fn(&T, &PathMap) -> Result<(), Error> + 'a,
{
    struct ReadValidatedIter<'a, R, T, F> {
        snippet_ctx: ReaderSnippetContext<&'a mut R>,
        src: LiveEvents<'a>, // borrows from `reader`
        cfg: crate::de::Cfg,
        validate: F,
        finished: bool,
        _marker: std::marker::PhantomData<T>,
    }

    impl<'a, R, T, F> Iterator for ReadValidatedIter<'a, R, T, F>
    where
        R: Read + 'a,
        T: DeserializeOwned + 'a,
        F: Fn(&T, &PathMap) -> Result<(), Error> + 'a,
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
                    }
                    Ok(Some(_)) => {
                        let mut recorder = crate::path_map::PathRecorder::new();
                        let validate = &self.validate;
                        let value_res = run_with_document_scope(
                            &mut self.src,
                            |src| {
                                let value = crate::de::with_root_redaction(
                                    crate::de::YamlDeserializer::new_with_path_recorder(
                                        src,
                                        self.cfg,
                                        &mut recorder,
                                    ),
                                    T::deserialize,
                                )?;
                                validate(&value, &recorder.map)?;
                                Ok(value)
                            },
                            |e, src| self.snippet_ctx.attach_snippet(e, src),
                            |_| false,
                        );
                        let value = match value_res {
                            Ok(v) => v,
                            Err(e) => {
                                // After a deserialization error, skip remaining events in the
                                // current document and try to recover at the next document boundary.
                                if !self.src.skip_to_next_document() {
                                    self.finished = true;
                                    let _ = self.src.finish();
                                }
                                return Some(Err(e));
                            }
                        };

                        return Some(Ok(value));
                    }
                    Ok(None) => {
                        self.finished = true;
                        if let Err(e) = self.src.finish() {
                            return Some(Err(self.snippet_ctx.attach_snippet(e, &self.src)));
                        }
                        return None;
                    }
                    Err(e) => {
                        self.finished = true;
                        let _ = self.src.finish();
                        return Some(Err(self.snippet_ctx.attach_snippet(e, &self.src)));
                    }
                }
            }
        }
    }

    let cfg = crate::de::Cfg::from_options(&options);
    let (snippet_ctx, ring_handle) =
        ReaderSnippetContext::new(reader, options.with_snippet, options.crop_radius);
    let src = LiveEvents::from_reader(ring_handle, options, EnforcingPolicy::PerDocument);

    ReadValidatedIter::<R, T, F> {
        snippet_ctx,
        src,
        cfg,
        validate,
        finished: false,
        _marker: std::marker::PhantomData,
    }
}

/// Create an iterator over validated YAML documents from a reader with configurable options.
/// Root streamed input gets snippets on a best-effort basis; included text sources retain full
/// snippets, while included reader sources may not have snippet text available.
#[cfg(feature = "garde")]
pub fn read_with_options_valid<'a, R, T>(
    reader: &'a mut R,
    options: Options,
) -> impl Iterator<Item = Result<T, Error>> + 'a
where
    R: Read + 'a,
    T: DeserializeOwned + garde::Validate + 'a,
    <T as garde::Validate>::Context: Default,
{
    read_with_options_validated(reader, options, |value, locs| {
        Validate::validate(value).map_err(|report| garde_validation_error(report, locs))
    })
}

/// Deserialize a single YAML document from a YAML string and validate it with `validator`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "validator")]
pub fn from_str_validate<T>(input: &str) -> Result<T, Error>
where
    T: DeserializeOwned + ValidatorValidate,
{
    from_str_with_options_validate(input, Options::default())
}

/// Deserialize a single YAML document with configurable [`Options`] and validate it with `validator`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "validator")]
pub fn from_str_with_options_validate<T>(input: &str, options: Options) -> Result<T, Error>
where
    T: DeserializeOwned + ValidatorValidate,
{
    from_str_with_options_and_path_recorder_validated::<T, _>(input, options, |value, locs| {
        ValidatorValidate::validate(value)
            .map_err(|errors| validator_validation_error(errors, locs))
    })
}

/// Deserialize multiple YAML documents from a YAML string and validate each with `validator`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "validator")]
pub fn from_multiple_validate<T: DeserializeOwned + ValidatorValidate>(
    input: &str,
) -> Result<Vec<T>, Error> {
    from_multiple_with_options_validate(input, Options::default())
}

/// Deserialize multiple YAML documents with configurable [`Options`] and validate each with `validator`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "validator")]
pub fn from_multiple_with_options_validate<T>(
    input: &str,
    options: Options,
) -> Result<Vec<T>, Error>
where
    T: DeserializeOwned + ValidatorValidate,
{
    from_multiple_with_options_validated(
        input,
        options,
        ValidationSource::Validator,
        |value, locs| {
            ValidatorValidate::validate(value)
                .map_err(|errors| validator_validation_error(errors, locs))
        },
    )
}

/// Deserialize a single YAML document from bytes and validate it with `validator`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "validator")]
pub fn from_slice_validate<T: DeserializeOwned + ValidatorValidate>(
    bytes: &[u8],
) -> Result<T, Error> {
    from_slice_with_options_validate(bytes, Options::default())
}

/// Deserialize a single YAML document from bytes and validate it with `validator`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "validator")]
pub fn from_slice_with_options_validate<T: DeserializeOwned + ValidatorValidate>(
    bytes: &[u8],
    options: Options,
) -> Result<T, Error> {
    let s = std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8Input)?;
    from_str_with_options_validate(s, options)
}

/// Deserialize multiple YAML documents from bytes with options and validate each with `validator`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "validator")]
pub fn from_slice_multiple_with_options_validate<T>(
    bytes: &[u8],
    options: Options,
) -> Result<Vec<T>, Error>
where
    T: DeserializeOwned + ValidatorValidate,
{
    let s = std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8Input)?;
    from_multiple_with_options_validate(s, options)
}

/// Deserialize a single YAML document from a reader and validate it with `validator`.
/// Snippets are attached on a best-effort basis for streamed root input, are available for
/// included text sources, and may be unavailable for included reader sources.
#[cfg(feature = "validator")]
pub fn from_reader_validate<R: std::io::Read, T>(reader: R) -> Result<T, Error>
where
    T: DeserializeOwned + ValidatorValidate,
{
    from_reader_with_options_validate(reader, Options::default())
}

/// Deserialize a single YAML document from a reader with options and validate it with `validator`.
/// Snippets are attached on a best-effort basis for streamed root input, are available for
/// included text sources, and may be unavailable for included reader sources.
#[cfg(feature = "validator")]
pub fn from_reader_with_options_validate<R: std::io::Read, T>(
    reader: R,
    options: Options,
) -> Result<T, Error>
where
    T: DeserializeOwned + ValidatorValidate,
{
    from_reader_with_options_validated(
        reader,
        options,
        |value, locs| {
            ValidatorValidate::validate(value)
                .map_err(|errors| validator_validation_error(errors, locs))
        },
        "use read_validate or read_with_options_validate to obtain the iterator",
    )
}

/// Create an iterator over validated YAML documents from a reader.
/// Root streamed input gets snippets on a best-effort basis; included text sources retain full
/// snippets, while included reader sources may not have snippet text available.
#[cfg(feature = "validator")]
pub fn read_validate<'a, R, T>(reader: &'a mut R) -> impl Iterator<Item = Result<T, Error>> + 'a
where
    R: Read + 'a,
    T: DeserializeOwned + ValidatorValidate + 'a,
{
    read_with_options_validate(reader, Options::default())
}

/// Create an iterator over validated YAML documents from a reader with configurable options.
/// Root streamed input gets snippets on a best-effort basis; included text sources retain full
/// snippets, while included reader sources may not have snippet text available.
#[cfg(feature = "validator")]
pub fn read_with_options_validate<'a, R, T>(
    reader: &'a mut R,
    options: Options,
) -> impl Iterator<Item = Result<T, Error>> + 'a
where
    R: Read + 'a,
    T: DeserializeOwned + ValidatorValidate + 'a,
{
    read_with_options_validated(reader, options, |value, locs| {
        ValidatorValidate::validate(value)
            .map_err(|errors| validator_validation_error(errors, locs))
    })
}
