use crate::ReaderSnippetContext;
use crate::budget::EnforcingPolicy;
use crate::de::{Error, Ev, Events, Options};
#[cfg(feature = "garde")]
use crate::de_error::collect_garde_issues;
#[cfg(feature = "validator")]
use crate::de_error::collect_validator_issues;
use crate::de_error::redact_issue;
use crate::live_events::LiveEvents;
use crate::maybe_with_snippet_from_events;
use crate::parse_scalars::scalar_is_nullish;
use crate::properties_redaction::with_interp_redaction_scope;
use serde::de::DeserializeOwned;
use std::io::Read;

#[cfg(feature = "garde")]
use garde::Validate;
#[cfg(feature = "validator")]
use validator::Validate as ValidatorValidate;

/// Deserialize a single YAML document with configurable [`Options`], and also
/// return a map from validation paths to source [`Location`]s.
#[cfg(feature = "garde")]
#[allow(deprecated)]
fn from_str_with_options_and_path_recorder_garde_valid<'de, T, F>(
    input: &'de str,
    options: Options,
    validate: F,
) -> Result<(T, LiveEvents<'de>), Error>
where
    T: DeserializeOwned + garde::Validate,
    F: FnOnce(&T) -> Result<(), garde::Report>,
{
    let input = if let Some(rest) = input.strip_prefix('\u{FEFF}') {
        rest
    } else {
        input
    };

    let with_snippet = options.with_snippet;
    let crop_radius = options.crop_radius;
    let cfg = crate::de::Cfg::from_options(&options);
    let mut src = LiveEvents::from_str(input, options, false);
    let mut recorder = crate::path_map::PathRecorder::new();

    let value_res = crate::anchor_store::with_document_scope(|| {
        with_interp_redaction_scope(|| {
            let value = crate::de::with_root_redaction(
                crate::de::YamlDeserializer::new_with_path_recorder(&mut src, cfg, &mut recorder),
                |de| T::deserialize(de),
            )?;
            validate(&value).map_err(|report| Error::ValidationError {
                issues: collect_garde_issues(&report)
                    .into_iter()
                    .map(redact_issue)
                    .collect(),
                locations: recorder.map.clone(),
            })?;
            Ok(value)
        })
    });
    let value = match value_res {
        Ok(v) => v,
        Err(e) => {
            if src.synthesized_null_emitted() {
                let err = Error::eof().with_location(src.last_location());
                return Err(maybe_with_snippet_from_events(
                    err,
                    input,
                    &src,
                    with_snippet,
                    crop_radius,
                ));
            } else {
                return Err(maybe_with_snippet_from_events(
                    e,
                    input,
                    &src,
                    with_snippet,
                    crop_radius,
                ));
            }
        }
    };

    match src.peek() {
        Ok(Some(_)) => {
            let err = Error::multiple_documents("use from_multiple or from_multiple_with_options")
                .with_location(src.last_location());
            return Err(maybe_with_snippet_from_events(
                err,
                input,
                &src,
                with_snippet,
                crop_radius,
            ));
        }
        Ok(None) => {}
        Err(e) => {
            if !src.seen_doc_end() {
                return Err(maybe_with_snippet_from_events(
                    e,
                    input,
                    &src,
                    with_snippet,
                    crop_radius,
                ));
            }
        }
    }

    Ok((value, src))
}

#[cfg(feature = "validator")]
#[allow(deprecated)]
fn from_str_with_options_and_path_recorder_validator_valid<'de, T>(
    input: &'de str,
    options: Options,
) -> Result<(T, LiveEvents<'de>), Error>
where
    T: DeserializeOwned + ValidatorValidate,
{
    let input = if let Some(rest) = input.strip_prefix('\u{FEFF}') {
        rest
    } else {
        input
    };

    let with_snippet = options.with_snippet;
    let crop_radius = options.crop_radius;
    let cfg = crate::de::Cfg::from_options(&options);
    let mut src = LiveEvents::from_str(input, options, false);
    let mut recorder = crate::path_map::PathRecorder::new();

    let value_res = crate::anchor_store::with_document_scope(|| {
        with_interp_redaction_scope(|| {
            let value = crate::de::with_root_redaction(
                crate::de::YamlDeserializer::new_with_path_recorder(&mut src, cfg, &mut recorder),
                |de| T::deserialize(de),
            )?;
            ValidatorValidate::validate(&value).map_err(|errors| Error::ValidatorError {
                issues: collect_validator_issues(&errors)
                    .into_iter()
                    .map(redact_issue)
                    .collect(),
                locations: recorder.map.clone(),
            })?;
            Ok(value)
        })
    });
    let value = match value_res {
        Ok(v) => v,
        Err(e) => {
            if src.synthesized_null_emitted() {
                let err = Error::eof().with_location(src.last_location());
                return Err(maybe_with_snippet_from_events(
                    err,
                    input,
                    &src,
                    with_snippet,
                    crop_radius,
                ));
            } else {
                return Err(maybe_with_snippet_from_events(
                    e,
                    input,
                    &src,
                    with_snippet,
                    crop_radius,
                ));
            }
        }
    };

    match src.peek() {
        Ok(Some(_)) => {
            let err = Error::multiple_documents("use from_multiple or from_multiple_with_options")
                .with_location(src.last_location());
            return Err(maybe_with_snippet_from_events(
                err,
                input,
                &src,
                with_snippet,
                crop_radius,
            ));
        }
        Ok(None) => {}
        Err(e) => {
            if !src.seen_doc_end() {
                return Err(maybe_with_snippet_from_events(
                    e,
                    input,
                    &src,
                    with_snippet,
                    crop_radius,
                ));
            }
        }
    }

    Ok((value, src))
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
    let with_snippet = options.with_snippet;
    let crop_radius = options.crop_radius;
    let (v, mut src) =
        from_str_with_options_and_path_recorder_garde_valid::<T, _>(input, options, |value| {
            Validate::validate(value)
        })?;
    if let Err(e) = src.finish() {
        return Err(maybe_with_snippet_from_events(
            e,
            input,
            &src,
            with_snippet,
            crop_radius,
        ));
    }
    Ok(v)
}

/// Deserialize a single YAML document with configurable [`Options`] and validate it with `garde` in context [`<T as garde::Validate>::Context`].
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
    let with_snippet = options.with_snippet;
    let crop_radius = options.crop_radius;
    let (v, mut src) =
        from_str_with_options_and_path_recorder_garde_valid::<T, _>(input, options, |value| {
            Validate::validate_with(value, context)
        })?;
    if let Err(e) = src.finish() {
        return Err(maybe_with_snippet_from_events(
            e,
            input,
            &src,
            with_snippet,
            crop_radius,
        ));
    }
    Ok(v)
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

/// Deserialize multiple YAML documents with configurable [`Options`] and validate each with `garde`.
/// The error message will contain a snippet with exact location information, and if the
/// invalid value comes from anchor, serde-saphyr will also tell where it is defined.
#[cfg(feature = "garde")]
#[allow(deprecated)]
pub fn from_multiple_with_options_valid<T>(input: &str, options: Options) -> Result<Vec<T>, Error>
where
    T: DeserializeOwned + garde::Validate,
    <T as garde::Validate>::Context: Default,
{
    let with_snippet = options.with_snippet;
    let crop_radius = options.crop_radius;

    let cfg = crate::de::Cfg::from_options(&options);
    let mut src = LiveEvents::from_str(input, options, false);
    let mut values = Vec::new();
    let mut validation_errors: Vec<Error> = Vec::new();

    loop {
        match src.peek()? {
            // Skip documents that are explicit null-like scalars ("", "~", or "null").
            Some(Ev::Scalar {
                value: s,
                style,
                tag,
                ..
            }) if *tag == crate::tags::SfTag::Null
                || (*tag != crate::tags::SfTag::String && scalar_is_nullish(s, style)) =>
            {
                let _ = src.next()?; // consume the null scalar document
                continue;
            }
            Some(_) => {
                let mut recorder = crate::path_map::PathRecorder::new();
                let value_res = crate::anchor_store::with_document_scope(|| {
                    with_interp_redaction_scope(|| {
                        let value = crate::de::with_root_redaction(
                            crate::de::YamlDeserializer::new_with_path_recorder(
                                &mut src,
                                cfg,
                                &mut recorder,
                            ),
                            |de| T::deserialize(de),
                        )?;
                        Validate::validate(&value).map_err(|report| Error::ValidationError {
                            issues: collect_garde_issues(&report)
                                .into_iter()
                                .map(redact_issue)
                                .collect(),
                            locations: recorder.map.clone(),
                        })?;
                        Ok(value)
                    })
                });
                let value = match value_res {
                    Ok(v) => v,
                    Err(Error::ValidationError { issues, locations }) => {
                        let err = Error::ValidationError { issues, locations };
                        validation_errors.push(maybe_with_snippet_from_events(
                            err,
                            input,
                            &src,
                            with_snippet,
                            crop_radius,
                        ));
                        continue;
                    }
                    Err(e) => {
                        return Err(maybe_with_snippet_from_events(
                            e,
                            input,
                            &src,
                            with_snippet,
                            crop_radius,
                        ));
                    }
                };

                values.push(value);
            }
            None => break,
        }
    }

    if let Err(e) = src.finish() {
        return Err(maybe_with_snippet_from_events(
            e,
            input,
            &src,
            with_snippet,
            crop_radius,
        ));
    }

    if validation_errors.is_empty() {
        Ok(values)
    } else {
        Err(Error::ValidationErrors {
            errors: validation_errors,
        })
    }
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

/// Deserialize a single YAML document from a reader with options and validate it with `garde`.
/// Snippets are attached on a best-effort basis for streamed root input, are available for
/// included text sources, and may be unavailable for included reader sources.
#[cfg(feature = "garde")]
#[allow(deprecated)]
pub fn from_reader_with_options_valid<R: std::io::Read, T>(
    reader: R,
    options: Options,
) -> Result<T, Error>
where
    T: DeserializeOwned + garde::Validate,
    <T as garde::Validate>::Context: Default,
{
    let cfg = crate::de::Cfg::from_options(&options);
    let (snippet_ctx, ring_handle) =
        ReaderSnippetContext::new(reader, options.with_snippet, options.crop_radius);
    let mut src = LiveEvents::from_reader(ring_handle, options, false, EnforcingPolicy::AllContent);

    let mut recorder = crate::path_map::PathRecorder::new();

    let value_res = crate::anchor_store::with_document_scope(|| {
        with_interp_redaction_scope(|| {
            let value = crate::de::with_root_redaction(
                crate::de::YamlDeserializer::new_with_path_recorder(&mut src, cfg, &mut recorder),
                |de| T::deserialize(de),
            )?;
            Validate::validate(&value).map_err(|report| Error::ValidationError {
                issues: collect_garde_issues(&report)
                    .into_iter()
                    .map(redact_issue)
                    .collect(),
                locations: recorder.map.clone(),
            })?;
            Ok(value)
        })
    });
    let value = match value_res {
        Ok(v) => v,
        Err(e) => {
            if src.synthesized_null_emitted() {
                // If the only thing in the input was an empty document (synthetic null),
                // surface this as an EOF error to preserve expected error semantics
                // for incompatible target types (e.g., bool).
                return Err(snippet_ctx
                    .attach_snippet(Error::eof().with_location(src.last_location()), &src));
            } else {
                return Err(snippet_ctx.attach_snippet(e, &src));
            }
        }
    };

    // After finishing first document, peek ahead to detect either another document/content
    // or trailing garbage. If a scan error occurs but we have seen a DocumentEnd ("..."),
    // ignore the trailing garbage. Otherwise, surface the error.
    match src.peek() {
        Ok(Some(_)) => {
            return Err(snippet_ctx.attach_snippet(
                Error::multiple_documents(
                    "use read_valid or read_with_options_valid to obtain the iterator",
                )
                .with_location(src.last_location()),
                &src,
            ));
        }
        Ok(None) => {}
        Err(e) => {
            if src.seen_doc_end() {
                // Trailing garbage after a proper document end marker is ignored.
            } else {
                return Err(snippet_ctx.attach_snippet(e, &src));
            }
        }
    }

    if let Err(e) = src.finish() {
        return Err(snippet_ctx.attach_snippet(e, &src));
    }
    Ok(value)
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
    read_with_options_valid(reader, Default::default())
}

/// Create an iterator over validated YAML documents from a reader with configurable options.
/// Root streamed input gets snippets on a best-effort basis; included text sources retain full
/// snippets, while included reader sources may not have snippet text available.
#[cfg(feature = "garde")]
#[allow(deprecated)]
pub fn read_with_options_valid<'a, R, T>(
    reader: &'a mut R,
    options: Options,
) -> impl Iterator<Item = Result<T, Error>> + 'a
where
    R: Read + 'a,
    T: DeserializeOwned + garde::Validate + 'a,
    <T as garde::Validate>::Context: Default,
{
    struct ReadValidIter<'a, R, T> {
        snippet_ctx: ReaderSnippetContext<&'a mut R>,
        src: LiveEvents<'a>, // borrows from `reader`
        cfg: crate::de::Cfg,
        finished: bool,
        _marker: std::marker::PhantomData<T>,
    }

    impl<'a, R, T> Iterator for ReadValidIter<'a, R, T>
    where
        R: Read + 'a,
        T: DeserializeOwned + garde::Validate + 'a,
        <T as garde::Validate>::Context: Default,
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
                        let mut recorder = crate::path_map::PathRecorder::new();
                        let value_res = crate::anchor_store::with_document_scope(|| {
                            with_interp_redaction_scope(|| {
                                let value = T::deserialize(
                                    crate::de::YamlDeserializer::new_with_path_recorder(
                                        &mut self.src,
                                        self.cfg,
                                        &mut recorder,
                                    ),
                                )?;
                                Validate::validate(&value).map_err(|report| {
                                    Error::ValidationError {
                                        issues: collect_garde_issues(&report)
                                            .into_iter()
                                            .map(redact_issue)
                                            .collect(),
                                        locations: recorder.map.clone(),
                                    }
                                })?;
                                Ok(value)
                            })
                        });
                        let value = match value_res {
                            Ok(v) => v,
                            Err(e) => {
                                let err = self.snippet_ctx.attach_snippet(e, &self.src);
                                // After a deserialization error, skip remaining events in the
                                // current document and try to recover at the next document boundary.
                                if !self.src.skip_to_next_document() {
                                    self.finished = true;
                                }
                                return Some(Err(err));
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
    let src = LiveEvents::from_reader(ring_handle, options, false, EnforcingPolicy::PerDocument);

    ReadValidIter::<R, T> {
        snippet_ctx,
        src,
        cfg,
        finished: false,
        _marker: std::marker::PhantomData,
    }
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
    let with_snippet = options.with_snippet;
    let crop_radius = options.crop_radius;
    let (v, mut src) =
        from_str_with_options_and_path_recorder_validator_valid::<T>(input, options)?;
    if let Err(e) = src.finish() {
        return Err(maybe_with_snippet_from_events(
            e,
            input,
            &src,
            with_snippet,
            crop_radius,
        ));
    }
    Ok(v)
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
#[allow(deprecated)]
pub fn from_multiple_with_options_validate<T>(
    input: &str,
    options: Options,
) -> Result<Vec<T>, Error>
where
    T: DeserializeOwned + ValidatorValidate,
{
    let with_snippet = options.with_snippet;
    let crop_radius = options.crop_radius;

    let cfg = crate::de::Cfg::from_options(&options);
    let mut src = LiveEvents::from_str(input, options, false);
    let mut values = Vec::new();
    let mut validation_errors: Vec<Error> = Vec::new();

    loop {
        match src.peek()? {
            // Skip documents that are explicit null-like scalars ("", "~", or "null").
            Some(Ev::Scalar {
                value: s,
                style,
                tag,
                ..
            }) if *tag == crate::tags::SfTag::Null
                || (*tag != crate::tags::SfTag::String && scalar_is_nullish(s, style)) =>
            {
                let _ = src.next()?; // consume the null scalar document
                continue;
            }
            Some(_) => {
                let mut recorder = crate::path_map::PathRecorder::new();
                let value_res = crate::anchor_store::with_document_scope(|| {
                    with_interp_redaction_scope(|| {
                        let value = crate::de::with_root_redaction(
                            crate::de::YamlDeserializer::new_with_path_recorder(
                                &mut src,
                                cfg,
                                &mut recorder,
                            ),
                            |de| T::deserialize(de),
                        )?;
                        ValidatorValidate::validate(&value).map_err(|errors| {
                            Error::ValidatorError {
                                issues: collect_validator_issues(&errors)
                                    .into_iter()
                                    .map(redact_issue)
                                    .collect(),
                                locations: recorder.map.clone(),
                            }
                        })?;
                        Ok(value)
                    })
                });
                let value = match value_res {
                    Ok(v) => v,
                    Err(Error::ValidatorError { issues, locations }) => {
                        let err = Error::ValidatorError { issues, locations };
                        validation_errors.push(maybe_with_snippet_from_events(
                            err,
                            input,
                            &src,
                            with_snippet,
                            crop_radius,
                        ));
                        continue;
                    }
                    Err(e) => {
                        return Err(maybe_with_snippet_from_events(
                            e,
                            input,
                            &src,
                            with_snippet,
                            crop_radius,
                        ));
                    }
                };

                values.push(value);
            }
            None => break,
        }
    }

    if let Err(e) = src.finish() {
        return Err(maybe_with_snippet_from_events(
            e,
            input,
            &src,
            with_snippet,
            crop_radius,
        ));
    }

    if validation_errors.is_empty() {
        Ok(values)
    } else {
        Err(Error::ValidatorErrors {
            errors: validation_errors,
        })
    }
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
#[allow(deprecated)]
pub fn from_reader_with_options_validate<R: std::io::Read, T>(
    reader: R,
    options: Options,
) -> Result<T, Error>
where
    T: DeserializeOwned + ValidatorValidate,
{
    let cfg = crate::de::Cfg::from_options(&options);
    let (snippet_ctx, ring_handle) =
        ReaderSnippetContext::new(reader, options.with_snippet, options.crop_radius);
    let mut src = LiveEvents::from_reader(ring_handle, options, false, EnforcingPolicy::AllContent);

    let mut recorder = crate::path_map::PathRecorder::new();

    let value_res = crate::anchor_store::with_document_scope(|| {
        with_interp_redaction_scope(|| {
            let value = crate::de::with_root_redaction(
                crate::de::YamlDeserializer::new_with_path_recorder(&mut src, cfg, &mut recorder),
                |de| T::deserialize(de),
            )?;
            ValidatorValidate::validate(&value).map_err(|errors| Error::ValidatorError {
                issues: collect_validator_issues(&errors)
                    .into_iter()
                    .map(redact_issue)
                    .collect(),
                locations: recorder.map.clone(),
            })?;
            Ok(value)
        })
    });
    let value = match value_res {
        Ok(v) => v,
        Err(e) => {
            if src.synthesized_null_emitted() {
                // If the only thing in the input was an empty document (synthetic null),
                // surface this as an EOF error to preserve expected error semantics
                // for incompatible target types (e.g., bool).
                return Err(snippet_ctx
                    .attach_snippet(Error::eof().with_location(src.last_location()), &src));
            } else {
                return Err(snippet_ctx.attach_snippet(e, &src));
            }
        }
    };

    // After finishing first document, peek ahead to detect either another document/content
    // or trailing garbage. If a scan error occurs but we have seen a DocumentEnd ("..."),
    // ignore the trailing garbage. Otherwise, surface the error.
    match src.peek() {
        Ok(Some(_)) => {
            return Err(snippet_ctx.attach_snippet(
                Error::multiple_documents(
                    "use read_validate or read_with_options_validate to obtain the iterator",
                )
                .with_location(src.last_location()),
                &src,
            ));
        }
        Ok(None) => {}
        Err(e) => {
            if src.seen_doc_end() {
                // Trailing garbage after a proper document end marker is ignored.
            } else {
                return Err(snippet_ctx.attach_snippet(e, &src));
            }
        }
    }

    if let Err(e) = src.finish() {
        return Err(snippet_ctx.attach_snippet(e, &src));
    }
    Ok(value)
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
    read_with_options_validate(reader, Default::default())
}

/// Create an iterator over validated YAML documents from a reader with configurable options.
/// Root streamed input gets snippets on a best-effort basis; included text sources retain full
/// snippets, while included reader sources may not have snippet text available.
#[cfg(feature = "validator")]
#[allow(deprecated)]
pub fn read_with_options_validate<'a, R, T>(
    reader: &'a mut R,
    options: Options,
) -> impl Iterator<Item = Result<T, Error>> + 'a
where
    R: Read + 'a,
    T: DeserializeOwned + ValidatorValidate + 'a,
{
    struct ReadValidateIter<'a, R, T> {
        snippet_ctx: ReaderSnippetContext<&'a mut R>,
        src: LiveEvents<'a>, // borrows from `reader`
        cfg: crate::de::Cfg,
        finished: bool,
        _marker: std::marker::PhantomData<T>,
    }

    impl<'a, R, T> Iterator for ReadValidateIter<'a, R, T>
    where
        R: Read + 'a,
        T: DeserializeOwned + ValidatorValidate + 'a,
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
                        let mut recorder = crate::path_map::PathRecorder::new();
                        let value_res = crate::anchor_store::with_document_scope(|| {
                            with_interp_redaction_scope(|| {
                                let value = T::deserialize(
                                    crate::de::YamlDeserializer::new_with_path_recorder(
                                        &mut self.src,
                                        self.cfg,
                                        &mut recorder,
                                    ),
                                )?;
                                ValidatorValidate::validate(&value).map_err(|errors| {
                                    Error::ValidatorError {
                                        issues: collect_validator_issues(&errors)
                                            .into_iter()
                                            .map(redact_issue)
                                            .collect(),
                                        locations: recorder.map.clone(),
                                    }
                                })?;
                                Ok(value)
                            })
                        });
                        let value = match value_res {
                            Ok(v) => v,
                            Err(e) => {
                                let err = self.snippet_ctx.attach_snippet(e, &self.src);
                                // After a deserialization error, skip remaining events in the
                                // current document and try to recover at the next document boundary.
                                if !self.src.skip_to_next_document() {
                                    self.finished = true;
                                }
                                return Some(Err(err));
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
    let src = LiveEvents::from_reader(ring_handle, options, false, EnforcingPolicy::PerDocument);

    ReadValidateIter::<R, T> {
        snippet_ctx,
        src,
        cfg,
        finished: false,
        _marker: std::marker::PhantomData,
    }
}
