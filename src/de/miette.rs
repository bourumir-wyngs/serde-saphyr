//! `miette` integration.
//!
//! This module is feature-gated behind the `miette` feature.

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use miette::{Diagnostic, LabeledSpan, NamedSource, SourceCode, SourceSpan, SpanContents};

use crate::Error;
use crate::Location;
use crate::de_error::{
    CroppedRegion, distinct_alias_error_location, render_line_offset, render_message_text,
    sanitize_message_text,
};
use crate::de_snippet::sanitize_terminal_snippet_preserve_len;
use crate::localizer::Localizer;
use crate::{MessageFormatter, RenderOptions};
#[cfg(any(feature = "garde", feature = "validator"))]
use crate::{
    location::Locations,
    path_map::{PathKey, PathMap, format_path_with_resolved_leaf},
};

/// Convert a deserialization [`Error`] into a `miette::Report`.
///
/// This function takes the YAML `source` and a display `file` name/path.
///
/// # Example
///
/// ```rust,no_run
/// let yaml = "definitely\n";
///
/// let err = serde_saphyr::from_str::<bool>(yaml).expect_err("bool parse error expected");
/// let report = serde_saphyr::miette::to_miette_report(&err, yaml, "config.yaml");
///
/// // `Debug` formatting uses miette's graphical reporter.
/// eprintln!("{report:?}");
/// ```
///
/// Notes:
/// - `serde-saphyr::Error` intentionally does not retain the full input text.
///   This helper owns a copy of `source` to build a standalone `miette::Report`.
/// - If the error has no known location/span, the report will not include labels.
#[must_use]
pub fn to_miette_report(err: &Error, source: &str, file: &str) -> miette::Report {
    to_miette_report_with_options(err, source, file, RenderOptions::default())
}

/// Like [`to_miette_report`], with custom messages and localization.
///
/// Labels, include context, and individual validation diagnostics use the formatter's
/// [`Localizer`]. Validation entries also honor external-message overrides. The
/// multi-document validation summary uses [`MessageFormatter::format_message`].
#[must_use]
pub fn to_miette_report_with_formatter(
    err: &Error,
    source: &str,
    file: &str,
    formatter: &dyn MessageFormatter,
) -> miette::Report {
    to_miette_report_with_options(err, source, file, RenderOptions::new(formatter))
}

/// Like [`to_miette_report`], with deferred rendering options.
///
/// `line_offset` adds the number of lines preceding the root YAML fragment to
/// displayed line numbers. `source` must remain the YAML fragment that was parsed;
/// locations and byte spans in `err` are unchanged. Included sources retain their
/// own line numbers. Extremely large offsets are capped to the supported line
/// number range, as with [`Error::render_with_options`].
///
/// `source_name`, when set, overrides both `file` and stored snippet names for the
/// root source. Included sources retain their own names. Control characters in
/// display names are escaped.
///
/// Setting [`crate::SnippetMode::Off`] uses plain rendering, including location
/// suffixes, without source snippets or labels.
///
/// ```rust
/// let yaml = "definitely\n";
/// let err = serde_saphyr::from_str::<bool>(yaml).unwrap_err();
/// let options = serde_saphyr::render_options! {
///     line_offset: 9999,
///     source_name: Some("article.md"),
/// };
/// // The YAML fragment starts at line 10000 in the surrounding document.
/// let report = serde_saphyr::miette::to_miette_report_with_options(
///     &err, yaml, "<input>", options,
/// );
/// ```
#[must_use]
pub fn to_miette_report_with_options(
    err: &Error,
    source: &str,
    file: &str,
    options: RenderOptions<'_>,
) -> miette::Report {
    if options.snippets == crate::SnippetMode::Off {
        return miette::Report::msg(err.render_with_options(options));
    }
    let sanitized_source = sanitize_terminal_snippet_preserve_len(source.to_owned());
    let sanitized_file = sanitize_message_text(Cow::Borrowed(file)).into_owned();
    let src = Arc::new(NamedSource::new(sanitized_file, sanitized_source));
    let mut diag = build_diagnostic(err, src, options.formatter, &[]);
    let source_name = options
        .source_name
        .map(|name| Arc::<str>::from(sanitize_message_text(Cow::Borrowed(name)).into_owned()));
    diag.apply_source_options(
        render_line_offset(err, options.line_offset),
        source_name.as_ref(),
    );
    miette::Report::new(diag)
}

#[derive(Clone, Debug)]
struct ErrorDiagnostic {
    message: String,
    src: Arc<NamedSource<String>>,
    source_id: u32,
    line_offset: u64,
    source_name: Option<Arc<str>>,
    labels: Vec<LabeledSpan>,
    related: Vec<ErrorDiagnostic>,
}

impl fmt::Display for ErrorDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ErrorDiagnostic {}

impl ErrorDiagnostic {
    fn apply_source_options(&mut self, line_offset: u64, source_name: Option<&Arc<str>>) {
        self.line_offset = if self.source_id <= 1 { line_offset } else { 0 };
        self.source_name = if self.source_id <= 1 {
            source_name.cloned()
        } else {
            None
        };
        for related in &mut self.related {
            related.apply_source_options(line_offset, source_name);
        }
    }
}

impl SourceCode for ErrorDiagnostic {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn SpanContents<'a> + 'a>, miette::MietteError> {
        let contents = self
            .src
            .read_span(span, context_lines_before, context_lines_after)?;
        // Miette adds one and context line counts to this zero-based line number.
        // Leave room for all source lines, even when the requested offset exceeds
        // the platform's range. The offset never changes the source allocation.
        let max_offset = usize::MAX.saturating_sub(self.src.inner().len().saturating_add(1));
        let offset = usize::try_from(self.line_offset)
            .unwrap_or(usize::MAX)
            .min(max_offset);
        let line = contents.line().saturating_add(offset);
        Ok(Box::new(RenderedSpanContents {
            contents,
            line,
            name: self.source_name.as_deref(),
        }))
    }
}

struct RenderedSpanContents<'a> {
    contents: Box<dyn SpanContents<'a> + 'a>,
    line: usize,
    name: Option<&'a str>,
}

impl<'a> SpanContents<'a> for RenderedSpanContents<'a> {
    fn data(&self) -> &'a [u8] {
        self.contents.data()
    }

    fn span(&self) -> &SourceSpan {
        self.contents.span()
    }

    fn name(&self) -> Option<&str> {
        self.name.or_else(|| self.contents.name())
    }

    fn line(&self) -> usize {
        self.line
    }

    fn column(&self) -> usize {
        self.contents.column()
    }

    fn line_count(&self) -> usize {
        self.contents.line_count()
    }

    fn language(&self) -> Option<&str> {
        self.contents.language()
    }
}

impl Diagnostic for ErrorDiagnostic {
    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(self)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        if self.labels.is_empty() {
            None
        } else {
            Some(Box::new(self.labels.clone().into_iter()))
        }
    }

    fn related(&self) -> Option<Box<dyn Iterator<Item = &dyn Diagnostic> + '_>> {
        if self.related.is_empty() {
            return None;
        }
        Some(Box::new(self.related.iter().map(|d| d as &dyn Diagnostic)))
    }
}

fn build_diagnostic(
    err: &Error,
    src: Arc<NamedSource<String>>,
    formatter: &dyn MessageFormatter,
    regions: &[CroppedRegion],
) -> ErrorDiagnostic {
    match err {
        #[cfg(any(feature = "garde", feature = "validator"))]
        Error::ValidationError {
            source,
            issues,
            locations,
        } => {
            let l10n = formatter.localizer();
            let mut related = Vec::new();
            for issue in issues {
                let entry = issue.display_entry_overridden(l10n, source.external_message_source());
                related.push(build_validation_entry_diagnostic(
                    &src,
                    l10n,
                    &issue.path,
                    &entry,
                    locations,
                    regions,
                ));
            }

            ErrorDiagnostic {
                message: sanitize_message_text(l10n.validation_failed(issues.len())).into_owned(),
                src,
                source_id: 1,
                line_offset: 0,
                source_name: None,
                labels: Vec::new(),
                related,
            }
        }

        #[cfg(any(feature = "garde", feature = "validator"))]
        Error::ValidationErrors { errors, .. } => {
            let mut related = Vec::new();
            for e in errors {
                related.push(build_diagnostic(
                    e.without_snippet(),
                    Arc::clone(&src),
                    formatter,
                    regions,
                ));
            }

            ErrorDiagnostic {
                message: render_message_text(formatter, err).into_owned(),
                src,
                source_id: 1,
                line_offset: 0,
                source_name: None,
                labels: Vec::new(),
                related,
            }
        }

        Error::WithSnippet {
            error,
            regions: snippet_regions,
            ..
        } => {
            let mut diag =
                build_diagnostic(error.without_snippet(), src, formatter, snippet_regions);

            let mut used_regions = std::collections::HashSet::new();
            if let Some(locs) = error.locations() {
                insert_selected_region_key(
                    &mut used_regions,
                    snippet_regions,
                    &locs.reference_location,
                );
                insert_selected_region_key(
                    &mut used_regions,
                    snippet_regions,
                    &locs.defined_location,
                );
            } else if let Some(location) = error.location() {
                insert_selected_region_key(&mut used_regions, snippet_regions, &location);
            }
            #[cfg(any(feature = "garde", feature = "validator"))]
            if let Error::ValidationError {
                issues, locations, ..
            } = error.without_snippet()
            {
                // `Error::locations()` exposes only the first validation issue.
                // Every issue's use and definition windows already have labels.
                for issue in issues {
                    if let Some((locs, _)) = locations.search_with_ancestor_fallback(&issue.path) {
                        insert_selected_region_key(
                            &mut used_regions,
                            snippet_regions,
                            &locs.reference_location,
                        );
                        insert_selected_region_key(
                            &mut used_regions,
                            snippet_regions,
                            &locs.defined_location,
                        );
                    }
                }
            }
            if let Some(location) = distinct_alias_error_location(error) {
                insert_selected_region_key(&mut used_regions, snippet_regions, &location);
            }

            for region in snippet_regions {
                let key = region_key(region);

                if used_regions.insert(key) {
                    let (synthetic_src, span) =
                        get_source_and_span(&diag.src, &region.location, snippet_regions);
                    if let Some(span) = span {
                        diag.related.push(ErrorDiagnostic {
                            message: sanitize_message_text(
                                formatter.localizer().included_from_here(),
                            )
                            .into_owned(),
                            src: synthetic_src,
                            source_id: region.location.source_id(),
                            line_offset: 0,
                            source_name: None,
                            labels: vec![LabeledSpan::new_with_span(None, span)],
                            related: Vec::new(),
                        });
                    }
                }
            }

            diag
        }

        Error::AliasError { locations, .. } => {
            let l10n = formatter.localizer();
            let (actual_src, source_id, mut labels, mut related) = build_dual_location_labels(
                &src,
                locations.reference_location,
                locations.defined_location,
                regions,
                l10n,
                l10n.anchor_defined_here().as_ref(),
            );
            let message = render_message_text(formatter, err).into_owned();

            if let Some(location) = distinct_alias_error_location(err) {
                let (error_src, span) = get_source_and_span(&src, &location, regions);
                if let Some(span) = span {
                    let label = LabeledSpan::new_with_span(
                        Some(sanitize_message_text(l10n.error_here()).into_owned()),
                        span,
                    );
                    // Cropped regions can share a filename while having different
                    // padded contents and offsets. A label must use its own source.
                    if source_id == location.source_id() && sources_match(&actual_src, &error_src) {
                        labels.push(label);
                    } else {
                        related.push(ErrorDiagnostic {
                            message: message.clone(),
                            src: error_src,
                            source_id: location.source_id(),
                            line_offset: 0,
                            source_name: None,
                            labels: vec![label],
                            related: Vec::new(),
                        });
                    }
                }
            }

            ErrorDiagnostic {
                message,
                src: actual_src,
                source_id,
                line_offset: 0,
                source_name: None,
                labels,
                related,
            }
        }

        other => {
            let mut labels = Vec::new();
            let (actual_src, span) = if let Some(loc) = other.location() {
                get_source_and_span(&src, &loc, regions)
            } else {
                (Arc::clone(&src), None)
            };

            if let Some(span) = span {
                labels.push(LabeledSpan::new_with_span(
                    Some(render_message_text(formatter, other).into_owned()),
                    span,
                ));
            }

            ErrorDiagnostic {
                message: render_message_text(formatter, other).into_owned(),
                src: actual_src,
                source_id: other.location().map_or(0, |location| location.source_id()),
                line_offset: 0,
                source_name: None,
                labels,
                related: Vec::new(),
            }
        }
    }
}

fn region_key(region: &CroppedRegion) -> (&str, u32, usize, usize) {
    (
        region.source_name.as_str(),
        region.location.source_id(),
        region.start_line,
        region.end_line,
    )
}

fn insert_selected_region_key<'a>(
    used_regions: &mut std::collections::HashSet<(&'a str, u32, usize, usize)>,
    regions: &'a [CroppedRegion],
    location: &Location,
) {
    if let Some(region) = select_region_for_location(regions, location) {
        used_regions.insert(region_key(region));
    }
}

#[cfg(any(feature = "garde", feature = "validator"))]
fn build_validation_entry_diagnostic(
    src: &Arc<NamedSource<String>>,
    l10n: &dyn Localizer,
    path_key: &PathKey,
    entry: &str,
    locations: &PathMap,
    regions: &[CroppedRegion],
) -> ErrorDiagnostic {
    let original_leaf = path_key
        .leaf_string()
        .unwrap_or_else(|| l10n.root_path_label().into_owned());

    let (locs, resolved_leaf) = locations
        .search_with_ancestor_fallback(path_key)
        .unwrap_or((Locations::UNKNOWN, original_leaf));

    let ref_loc = locs.reference_location;
    let def_loc = locs.defined_location;

    let resolved_path = format_path_with_resolved_leaf(path_key, &resolved_leaf);
    let base_msg = sanitize_message_text(Cow::Owned(
        l10n.validation_base_message(entry, &resolved_path),
    ))
    .into_owned();

    let (actual_src, source_id, labels, related) = build_dual_location_labels(
        src,
        ref_loc,
        def_loc,
        regions,
        l10n,
        l10n.defined_window().as_ref(),
    );

    ErrorDiagnostic {
        message: base_msg,
        src: actual_src,
        source_id,
        line_offset: 0,
        source_name: None,
        labels,
        related,
    }
}

fn build_dual_location_labels(
    src: &Arc<NamedSource<String>>,
    ref_loc: Location,
    def_loc: Location,
    regions: &[CroppedRegion],
    l10n: &dyn Localizer,
    definition_label: &str,
) -> (
    Arc<NamedSource<String>>,
    u32,
    Vec<LabeledSpan>,
    Vec<ErrorDiagnostic>,
) {
    let mut labels = Vec::new();
    let mut related = Vec::new();

    let primary_loc = if ref_loc == Location::UNKNOWN {
        def_loc
    } else {
        ref_loc
    };
    let (primary_src, span) = get_source_and_span(src, &primary_loc, regions);

    if let Some(span) = span {
        let label = if ref_loc == Location::UNKNOWN {
            l10n.defined_window()
        } else {
            l10n.value_used_here()
        };
        labels.push(LabeledSpan::new_with_span(
            Some(sanitize_message_text(label).into_owned()),
            span,
        ));
    }

    if def_loc != Location::UNKNOWN && def_loc != primary_loc {
        let (def_src, def_span) = get_source_and_span(src, &def_loc, regions);
        if let Some(span) = def_span {
            let definition_label =
                sanitize_message_text(Cow::Borrowed(definition_label)).into_owned();
            let label = LabeledSpan::new_with_span(Some(definition_label.clone()), span);
            if primary_loc.source_id() == def_loc.source_id()
                && sources_match(&primary_src, &def_src)
            {
                labels.push(label);
            } else {
                related.push(ErrorDiagnostic {
                    message: definition_label,
                    src: def_src,
                    source_id: def_loc.source_id(),
                    line_offset: 0,
                    source_name: None,
                    labels: vec![label],
                    related: Vec::new(),
                });
            }
        }
    }

    (primary_src, primary_loc.source_id(), labels, related)
}

fn sources_match(left: &Arc<NamedSource<String>>, right: &Arc<NamedSource<String>>) -> bool {
    Arc::ptr_eq(left, right) || (left.name() == right.name() && left.inner() == right.inner())
}

fn select_region_for_location<'a>(
    regions: &'a [CroppedRegion],
    location: &Location,
) -> Option<&'a CroppedRegion> {
    if *location == Location::UNKNOWN {
        return None;
    }

    let line = location.line as usize;
    let location_source_id = location.source_id();
    regions
        .iter()
        .find(|r| r.location == *location)
        .or_else(|| {
            regions.iter().find(|r| {
                location_source_id != 0
                    && r.location.source_id() == location_source_id
                    && r.start_line <= line
                    && line <= r.end_line
            })
        })
        .or_else(|| {
            regions.iter().find(|r| {
                r.start_line <= line
                    && line <= r.end_line
                    && (location_source_id == 0
                        || r.location.source_id() == 0
                        || r.location.source_id() == location_source_id)
            })
        })
        .or_else(|| {
            // A location in an included source without retained text must not fall back to a
            // window of another source (such as the include site in the root).
            regions.first().filter(|r| {
                location_source_id <= 1
                    || r.location.source_id() == 0
                    || r.location.source_id() == location_source_id
            })
        })
}

fn get_source_and_span(
    src: &Arc<NamedSource<String>>,
    location: &Location,
    regions: &[CroppedRegion],
) -> (Arc<NamedSource<String>>, Option<SourceSpan>) {
    if *location == Location::UNKNOWN {
        return (Arc::clone(src), None);
    }

    let region = select_region_for_location(regions, location);

    if let Some(region) = region {
        let start_line = region.start_line.saturating_sub(1);
        let mut padded_text = String::new();
        for _ in 0..start_line {
            padded_text.push('\n');
        }
        padded_text.push_str(&region.text);
        let padded_text = sanitize_terminal_snippet_preserve_len(padded_text);

        let source_name =
            sanitize_message_text(Cow::Borrowed(region.source_name.as_str())).into_owned();
        let synthetic_src = Arc::new(NamedSource::new(source_name, padded_text.clone()));

        let mut byte_off = 0;
        let mut found = false;
        for (i, line) in padded_text.split_terminator('\n').enumerate() {
            if i + 1 == location.line as usize {
                let col = location.column as usize;
                let char_off = col.saturating_sub(1);
                let line_byte_off = line
                    .char_indices()
                    .nth(char_off)
                    .map_or(line.len(), |(idx, _)| idx);
                byte_off += line_byte_off;
                found = true;
                break;
            }
            byte_off += line.len() + 1; // +1 for \n
        }

        if found {
            let mut char_len = location.span().len() as usize;
            if char_len == 0 {
                char_len = 1;
            }
            let src_len = padded_text.len();
            if byte_off > src_len {
                return (Arc::clone(src), to_source_span(src, location));
            }
            let remainder = &padded_text[byte_off..];
            let byte_len = remainder
                .char_indices()
                .nth(char_len)
                .map_or(remainder.len(), |(idx, _)| idx)
                .max(1);
            let clamped_len = byte_len.min(src_len.saturating_sub(byte_off));
            return (
                synthetic_src,
                Some(SourceSpan::new(byte_off.into(), clamped_len)),
            );
        }
    }

    // `src` is the root document. Source id 1 is the root and 0 is unknown; any other id is an
    // included source with no retained text, whose location must not be mapped onto the root.
    if location.source_id() > 1 {
        return (Arc::clone(src), None);
    }

    (Arc::clone(src), to_source_span(src, location))
}

fn to_source_span(src: &NamedSource<String>, location: &Location) -> Option<SourceSpan> {
    if *location == Location::UNKNOWN {
        return None;
    }

    let (byte_off, mut byte_len): (usize, usize) = if let (Some(off), Some(len)) =
        (location.span().byte_offset(), location.span().byte_len())
    {
        (off as usize, len as usize)
    } else {
        // The parser provides character-based offsets/lengths, while miette expects
        // byte offsets into the UTF-8 source. Convert here using the available source.
        fn char_range_to_byte_range(
            s: &str,
            char_offset: usize,
            char_len: usize,
        ) -> Option<(usize, usize)> {
            // Start byte index for the given character offset
            let start_byte = if char_offset == 0 {
                0
            } else {
                match s.char_indices().nth(char_offset) {
                    Some((i, _)) => i,
                    None if char_offset == s.chars().count() => s.len(),
                    None => return None,
                }
            };

            // End in characters (exclusive)
            let end_char = char_offset.saturating_add(char_len);

            // If end past the last char, clamp to the end of the string in bytes
            let end_byte = match s.char_indices().nth(end_char) {
                Some((i, _)) => i,
                None => s.len(),
            };

            Some((start_byte, end_byte.saturating_sub(start_byte)))
        }

        let char_off = location.span().offset() as usize;
        let mut char_len = location.span().len() as usize;
        if char_len == 0 {
            char_len = 1;
        }

        char_range_to_byte_range(src.inner(), char_off, char_len)?
    };

    if byte_len == 0 {
        byte_len = 1;
    }

    // Clamp to the actual input, to avoid panics and invalid spans.
    let src_len = src.inner().len();
    if byte_off > src_len {
        return None;
    }
    byte_len = byte_len.min(src_len.saturating_sub(byte_off));

    Some(SourceSpan::new(byte_off.into(), byte_len))
}

#[cfg(all(test, feature = "miette"))]
mod tests {
    use super::*;

    #[derive(Debug, serde::Deserialize)]
    struct AliasMappingConfig {
        #[serde(rename = "base")]
        _base: std::collections::BTreeMap<String, String>,
        #[serde(rename = "copy")]
        _copy: AliasPort,
    }

    #[derive(Debug, serde::Deserialize)]
    struct AliasPort {
        #[serde(rename = "port")]
        _port: u16,
    }

    fn labeled_text<'a>(diagnostic: &'a ErrorDiagnostic, label: &LabeledSpan) -> &'a str {
        &diagnostic.src.inner()[label.offset()..label.offset() + label.len()]
    }

    fn assert_display_offset(original: &ErrorDiagnostic, shifted: &ErrorDiagnostic, offset: usize) {
        assert_source_display(original, shifted, offset, None);
    }

    fn assert_source_display(
        original: &ErrorDiagnostic,
        shifted: &ErrorDiagnostic,
        offset: usize,
        source_name: Option<&str>,
    ) {
        assert_eq!(original.message, shifted.message);
        assert_eq!(original.src.inner(), shifted.src.inner());
        assert_eq!(original.src.name(), shifted.src.name());
        assert_eq!(original.source_id, shifted.source_id);
        assert_eq!(original.labels, shifted.labels);
        for label in &original.labels {
            let before = original.read_span(label.inner(), 1, 1).unwrap();
            let after = shifted.read_span(label.inner(), 1, 1).unwrap();
            let expected_offset = if original.source_id <= 1 { offset } else { 0 };
            assert_eq!(after.line(), before.line() + expected_offset);
            assert_eq!(after.column(), before.column());
            assert_eq!(after.data(), before.data());
            assert_eq!(after.span(), before.span());
            assert_eq!(after.line_count(), before.line_count());
            let expected_name = if original.source_id <= 1 {
                source_name.or_else(|| before.name())
            } else {
                before.name()
            };
            assert_eq!(after.name(), expected_name);
        }
        assert_eq!(original.related.len(), shifted.related.len());
        for (before, after) in original.related.iter().zip(&shifted.related) {
            assert_source_display(before, after, offset, source_name);
        }
    }

    fn graphical_report(report: &miette::Report) -> String {
        let mut output = String::new();
        miette::GraphicalReportHandler::new()
            .with_theme(miette::GraphicalTheme::none())
            .render_report(&mut output, report.as_ref())
            .unwrap();
        output
    }

    #[test]
    fn line_offset_shifts_miette_headers_and_gutters_without_changing_spans() {
        let yaml = "éighty\n";
        let error =
            crate::from_str_with_options::<bool>(yaml, crate::options! { with_snippet: false })
                .unwrap_err();
        let original_location = error.location();
        let baseline = to_miette_report(&error, yaml, "article.md");
        let shifted = to_miette_report_with_options(
            &error,
            yaml,
            "article.md",
            crate::render_options! { line_offset: 9999 },
        );
        assert_display_offset(
            baseline.downcast_ref::<ErrorDiagnostic>().unwrap(),
            shifted.downcast_ref::<ErrorDiagnostic>().unwrap(),
            9999,
        );
        assert_eq!(error.location(), original_location);
        let rendered = graphical_report(&shifted);
        assert!(rendered.contains("article.md:10000:1"), "{rendered}");
        assert!(rendered.contains("10000 | éighty"), "{rendered}");
    }

    #[test]
    fn source_name_overrides_explicit_file_and_stored_names_without_changing_spans() {
        let yaml = "éighty\n";
        for error in [
            crate::from_str::<bool>(yaml).unwrap_err(),
            crate::from_reader::<_, bool>(yaml.as_bytes()).unwrap_err(),
        ] {
            let original_error = format!("{error:?}");
            for error in [&error, error.without_snippet()] {
                let baseline = to_miette_report(error, yaml, "fallback.yaml");
                for (source_name, expected_name) in [
                    (Some("article.md"), Some("article.md")),
                    (
                        Some("article\n\u{1b}😀.md\\raw"),
                        Some(r"article\n\u{1b}😀.md\raw"),
                    ),
                    (Some(""), Some("")),
                    (None, None),
                ] {
                    let report = to_miette_report_with_options(
                        error,
                        yaml,
                        "fallback.yaml",
                        crate::render_options! { line_offset: 9999, source_name: source_name },
                    );
                    assert_source_display(
                        baseline.downcast_ref::<ErrorDiagnostic>().unwrap(),
                        report.downcast_ref::<ErrorDiagnostic>().unwrap(),
                        9999,
                        expected_name,
                    );
                    if source_name == Some("article.md") {
                        let rendered = graphical_report(&report);
                        assert!(rendered.contains("article.md:10000:1"), "{rendered}");
                        assert!(rendered.contains("10000 | éighty"), "{rendered}");
                        assert!(!rendered.contains("fallback.yaml"), "{rendered}");
                    }
                }
            }
            assert_eq!(format!("{error:?}"), original_error);
        }
    }

    #[test]
    fn source_name_applies_to_all_root_alias_regions() {
        let yaml = concat!(
            "base: &b\n  port: eighty\n",
            "# gap\n# gap\n# gap\n# gap\n# gap\n# gap\n",
            "copy: *b\n",
        );
        for error in [
            crate::from_str::<AliasMappingConfig>(yaml).unwrap_err(),
            crate::from_reader::<_, AliasMappingConfig>(yaml.as_bytes()).unwrap_err(),
        ] {
            let baseline = to_miette_report(&error, yaml, "fallback.yaml");
            assert!(baseline.related().is_some());
            let report = to_miette_report_with_options(
                &error,
                yaml,
                "fallback.yaml",
                crate::render_options! {
                    line_offset: 9999,
                    source_name: Some("article.md"),
                },
            );
            assert_source_display(
                baseline.downcast_ref::<ErrorDiagnostic>().unwrap(),
                report.downcast_ref::<ErrorDiagnostic>().unwrap(),
                9999,
                Some("article.md"),
            );
            let rendered = graphical_report(&report);
            assert!(!rendered.contains("<input>"), "{rendered}");
            assert!(!rendered.contains("fallback.yaml"), "{rendered}");
        }
    }

    #[test]
    fn line_offset_covers_alias_crops_from_strings_and_readers() {
        let yaml = "base: &b\n  port: eighty\ncopy: *b\n";
        for error in [
            crate::from_str::<AliasMappingConfig>(yaml).unwrap_err(),
            crate::from_reader::<_, AliasMappingConfig>(yaml.as_bytes()).unwrap_err(),
        ] {
            for error in [&error, error.without_snippet()] {
                let baseline = to_miette_report(error, yaml, "article.md");
                let shifted = to_miette_report_with_options(
                    error,
                    yaml,
                    "article.md",
                    crate::render_options! { line_offset: 9999 },
                );
                assert_display_offset(
                    baseline.downcast_ref::<ErrorDiagnostic>().unwrap(),
                    shifted.downcast_ref::<ErrorDiagnostic>().unwrap(),
                    9999,
                );
            }
        }
    }

    #[rstest::rstest]
    #[case::unknown_root_source(0)]
    #[case::known_root_source(1)]
    fn line_offset_preserves_included_coordinates_even_for_identical_named_sources(
        #[case] source_id: u32,
    ) {
        let yaml = "wrong\n";
        let root = Location {
            line: 1,
            column: 1,
            span: crate::Span::new(0, 5),
            source_id,
        };
        let included = Location {
            source_id: 2,
            ..root
        };
        let error = Error::WithSnippet {
            error: Box::new(Error::AliasError {
                msg: "invalid value".to_owned(),
                error: Box::new(Error::Message {
                    msg: "invalid value".to_owned(),
                    location: included,
                }),
                locations: crate::Locations {
                    reference_location: root,
                    defined_location: included,
                },
            }),
            crop_radius: 2,
            regions: vec![
                CroppedRegion::new(yaml, "same.yaml", 1, 1, root),
                CroppedRegion::new(yaml, "same.yaml", 1, 1, included),
            ],
        };
        let baseline = to_miette_report(&error, yaml, "same.yaml");
        let shifted = to_miette_report_with_options(
            &error,
            yaml,
            "same.yaml",
            crate::render_options! { line_offset: 9999 },
        );
        let diagnostic = shifted.downcast_ref::<ErrorDiagnostic>().unwrap();
        assert_eq!(diagnostic.labels.len(), 1);
        assert_eq!(diagnostic.related.len(), 1);
        assert_eq!(diagnostic.related[0].source_id, 2);
        assert_display_offset(
            baseline.downcast_ref::<ErrorDiagnostic>().unwrap(),
            diagnostic,
            9999,
        );
        let rendered = graphical_report(&shifted);
        assert!(rendered.contains("same.yaml:10000:1"), "{rendered}");
        assert!(rendered.contains("same.yaml:1:1"), "{rendered}");
        let renamed = to_miette_report_with_options(
            &error,
            yaml,
            "same.yaml",
            crate::render_options! {
                line_offset: 9999,
                source_name: Some("article.md"),
            },
        );
        assert_source_display(
            baseline.downcast_ref::<ErrorDiagnostic>().unwrap(),
            renamed.downcast_ref::<ErrorDiagnostic>().unwrap(),
            9999,
            Some("article.md"),
        );
        let rendered = graphical_report(&renamed);
        assert!(rendered.contains("article.md:10000:1"), "{rendered}");
        assert!(rendered.contains("same.yaml:1:1"), "{rendered}");
    }

    #[test]
    fn huge_line_offset_does_not_pad_source_or_overflow_miette() {
        let yaml = "wrong\n";
        let error = crate::from_str::<bool>(yaml).unwrap_err();
        let report = to_miette_report_with_options(
            &error,
            yaml,
            "article.md",
            crate::render_options! { line_offset: u64::MAX },
        );
        let diagnostic = report.downcast_ref::<ErrorDiagnostic>().unwrap();
        assert_eq!(diagnostic.src.inner().len(), yaml.len());
        assert!(graphical_report(&report).len() < 2000);
    }

    #[test]
    fn disabled_miette_snippets_preserve_plain_location_offsets() {
        let yaml = concat!(
            "base: &b\n  port: eighty\n",
            "# gap\n# gap\n# gap\n# gap\n# gap\n# gap\n",
            "copy: *b\n",
        );
        let error = crate::from_str::<AliasMappingConfig>(yaml).unwrap_err();
        let options = crate::render_options! {
            line_offset: 9999,
            snippets: crate::SnippetMode::Off,
            source_name: Some("renamed.md"),
        };
        let report = to_miette_report_with_options(&error, yaml, "article.md", options);
        assert!(report.related().is_none());
        assert!(report.source_code().is_none());
        assert!(report.labels().is_none());
        assert_eq!(report.to_string(), error.render_with_options(options));
        let rendered = graphical_report(&report);
        assert!(rendered.contains("invalid u16"), "{rendered}");
        assert!(rendered.contains("used at line 10008"), "{rendered}");
        assert!(rendered.contains("defined at line 10001"), "{rendered}");
        assert!(!rendered.contains("copy: *b"), "{rendered}");
        assert!(!rendered.contains("renamed.md"), "{rendered}");
    }

    #[test]
    fn mapping_alias_labels_the_actual_failing_value() {
        let yaml = "base: &b\n  port: eighty\ncopy: *b\n";
        let error = crate::from_str::<AliasMappingConfig>(yaml).unwrap_err();
        let src = Arc::new(NamedSource::new("config.yaml", yaml.to_owned()));

        for error in [&error, error.without_snippet()] {
            let diagnostic = build_diagnostic(
                error,
                Arc::clone(&src),
                RenderOptions::default().formatter,
                &[],
            );
            assert_eq!(diagnostic.labels.len(), 3);
            assert!(diagnostic.related.is_empty());
            let label = diagnostic
                .labels
                .iter()
                .find(|label| label.label() == Some("the error occurred here"))
                .expect("the failing value must have its own label");
            assert_eq!(labeled_text(&diagnostic, label), "eighty");
        }
    }

    #[test]
    #[allow(deprecated)] // Populates the legacy msg field when constructing an alias error.
    fn unknown_alias_reference_shows_definition_and_distinct_value_once() {
        let yaml = "base: value\nport: eighty\n";
        let definition = Location {
            line: 1,
            column: 7,
            span: crate::Span::new(6, 5),
            source_id: 0,
        };
        let failing_value = Location {
            line: 2,
            column: 7,
            span: crate::Span::new(yaml.find("eighty").unwrap() as u64, 6),
            source_id: 0,
        };
        let error = Error::AliasError {
            msg: "invalid value".to_owned(),
            error: Box::new(Error::Message {
                msg: "invalid value".to_owned(),
                location: failing_value,
            }),
            locations: crate::location::Locations {
                reference_location: Location::UNKNOWN,
                defined_location: definition,
            },
        };
        let src = Arc::new(NamedSource::new("config.yaml", yaml.to_owned()));
        let diagnostic = build_diagnostic(&error, src, RenderOptions::default().formatter, &[]);

        assert_eq!(diagnostic.labels.len(), 2);
        assert_eq!(diagnostic.labels[0].label(), Some("defined here"));
        assert_eq!(labeled_text(&diagnostic, &diagnostic.labels[0]), "value");
        assert_eq!(
            diagnostic.labels[1].label(),
            Some("the error occurred here")
        );
        assert_eq!(labeled_text(&diagnostic, &diagnostic.labels[1]), "eighty");
        assert!(diagnostic.related.is_empty());
    }

    #[test]
    fn distant_alias_value_uses_its_own_cropped_source() {
        let mut yaml = "base: &b\n".to_owned();
        for index in 0..30 {
            yaml.push_str(&format!("  before_{index}: ok\n"));
        }
        yaml.push_str("  port: eighty\n");
        for index in 0..30 {
            yaml.push_str(&format!("  after_{index}: ok\n"));
        }
        yaml.push_str("copy: *b\n");
        let error = crate::from_str::<AliasMappingConfig>(&yaml).unwrap_err();
        let src = Arc::new(NamedSource::new("config.yaml", yaml.clone()));
        let diagnostic = build_diagnostic(&error, src, RenderOptions::default().formatter, &[]);

        assert_eq!(diagnostic.labels.len(), 1);
        assert_eq!(labeled_text(&diagnostic, &diagnostic.labels[0]), "*b");
        assert!(!diagnostic.src.inner().contains("eighty"));
        assert_eq!(
            diagnostic.related.len(),
            2,
            "the failing value's region must not be repeated as included-from-here"
        );
        let definition = diagnostic
            .related
            .iter()
            .find(|related| related.labels[0].label() == Some("anchor defined here"))
            .expect("the anchor definition needs its own source");
        assert!(definition.src.inner()[definition.labels[0].offset()..].starts_with("before_0"));
        let underlying = diagnostic
            .related
            .iter()
            .find(|related| related.labels[0].label() == Some("the error occurred here"))
            .expect("the failing value needs its own source");
        assert_eq!(diagnostic.src.name(), underlying.src.name());
        assert_ne!(diagnostic.src.inner(), underlying.src.inner());
        assert_eq!(underlying.labels.len(), 1);
        let label = &underlying.labels[0];
        assert_eq!(label.label(), Some("the error occurred here"));
        assert_eq!(labeled_text(underlying, label), "eighty");
        assert_eq!(underlying.src.inner()[..label.offset()].lines().count(), 32);
    }

    #[test]
    fn overlapping_alias_crops_keep_each_label_on_its_actual_source() {
        let yaml = concat!(
            "base: &b\n",
            "  first: ok\n",
            "  port: eighty\n",
            "copy: *b\n",
            "tail_1: ok\n",
            "tail_2: ok\n",
            "tail_3: ok\n",
        );
        let error = crate::from_str::<AliasMappingConfig>(yaml).unwrap_err();
        let src = Arc::new(NamedSource::new("config.yaml", yaml.to_owned()));
        let diagnostic = build_diagnostic(&error, src, RenderOptions::default().formatter, &[]);

        assert_eq!(diagnostic.labels.len(), 1);
        assert_eq!(labeled_text(&diagnostic, &diagnostic.labels[0]), "*b");
        assert_eq!(diagnostic.related.len(), 2);
        let definition = &diagnostic.related[0];
        assert_eq!(definition.labels[0].label(), Some("anchor defined here"));
        assert!(definition.src.inner()[definition.labels[0].offset()..].starts_with("first"));
        let underlying = &diagnostic.related[1];
        assert_eq!(
            underlying.labels[0].label(),
            Some("the error occurred here")
        );
        assert_eq!(labeled_text(underlying, &underlying.labels[0]), "eighty");

        // All three windows contain the use, definition, and error lines, but
        // different context prefixes still require different byte offsets.
        for related in &diagnostic.related {
            assert_eq!(diagnostic.src.name(), related.src.name());
            assert_ne!(diagnostic.src.inner(), related.src.inner());
            assert!(related.src.inner().contains("copy: *b"));
        }
    }

    #[test]
    fn alias_value_label_is_localized_and_sanitized_without_reformatting_message() {
        #[derive(Default)]
        struct Formatter {
            calls: std::cell::Cell<usize>,
        }

        impl crate::Localizer for Formatter {
            fn error_here(&self) -> Cow<'static, str> {
                Cow::Borrowed("failure\n\u{1b}]0;owned\u{7}")
            }
        }

        impl MessageFormatter for Formatter {
            fn localizer(&self) -> &dyn crate::Localizer {
                self
            }

            fn format_message<'a>(&self, error: &'a Error) -> Cow<'a, str> {
                assert!(matches!(error, Error::InvalidScalar { ty: "u16", .. }));
                self.calls.set(self.calls.get() + 1);
                Cow::Borrowed("custom port error")
            }
        }

        let yaml = "base: &b\n  port: eighty\ncopy: *b\n";
        let error = crate::from_str::<AliasMappingConfig>(yaml).unwrap_err();
        let src = Arc::new(NamedSource::new("config.yaml", yaml.to_owned()));
        let formatter = Formatter::default();
        let diagnostic = build_diagnostic(&error, src, &formatter, &[]);

        assert_eq!(diagnostic.message, "custom port error");
        assert_eq!(formatter.calls.get(), 1);
        let label = diagnostic
            .labels
            .iter()
            .find(|label| label.label() == Some(r"failure\n\u{1b}]0;owned\u{7}"))
            .expect("localized label must escape control characters");
        assert_eq!(labeled_text(&diagnostic, label), "eighty");
    }

    #[cfg(any(feature = "garde", feature = "validator"))]
    #[test]
    fn validation_message_fields_are_sanitized() {
        let path = PathKey::empty().join("field\n\u{1b}");
        let error = Error::ValidationError {
            source: crate::de_error::ValidationSource::Validator,
            issues: vec![
                crate::de_error::ValidationIssue::new(path, "bad")
                    .with_message("invalid\nvalue\u{1b}]0;owned\u{7}"),
            ],
            locations: PathMap::new(),
        };
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", "field: bad\n".to_owned()));

        let diagnostic = build_diagnostic(&error, src, RenderOptions::default().formatter, &[]);
        assert_eq!(diagnostic.related.len(), 1);
        let message = &diagnostic.related[0].message;
        assert!(
            message.contains(r"invalid\nvalue\u{1b}]0;owned\u{7}"),
            "{message:?}"
        );
        assert!(message.contains(r"field\n\u{1b}"), "{message:?}");
        assert!(!message.contains("invalid\nvalue\u{1b}"), "{message:?}");
    }

    #[test]
    fn cropped_region_source_names_are_sanitized() {
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", "bad\n".to_owned()));
        let location = Location::new(1, 1);
        let region =
            CroppedRegion::new("bad\n", "source\n\u{1b}]0;owned\u{7}.yaml", 1, 1, location);

        let (selected, _) = get_source_and_span(&src, &location, &[region]);
        assert_eq!(selected.name(), r"source\n\u{1b}]0;owned\u{7}.yaml");
    }

    #[test]
    fn get_source_and_span_prefers_region_with_matching_source_id() {
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", "root: 1\n".to_owned()));
        let location = Location {
            line: 2,
            column: 7,
            span: crate::Span::new(0, 5),
            source_id: 2,
        };

        let regions = vec![
            CroppedRegion {
                text: "a: 1\nbad: parent_value\n".to_string(),
                source_name: "parent.yaml".to_string(),
                start_line: 1,
                end_line: 2,
                location: Location {
                    line: 2,
                    column: 6,
                    span: crate::Span::UNKNOWN,
                    source_id: 1,
                },
            },
            CroppedRegion {
                text: "x: 1\nbad: child_value\n".to_string(),
                source_name: "child.yaml".to_string(),
                start_line: 1,
                end_line: 2,
                location: Location {
                    line: 2,
                    column: 6,
                    span: crate::Span::UNKNOWN,
                    source_id: 2,
                },
            },
        ];

        let (picked_src, picked_span) = get_source_and_span(&src, &location, &regions);
        assert!(picked_src.inner().contains("child_value"));
        assert!(picked_span.is_some());
    }

    #[test]
    fn get_source_and_span_keeps_line_fallback_when_source_id_unknown() {
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", "root: 1\n".to_owned()));
        let location = Location {
            line: 2,
            column: 7,
            span: crate::Span::new(0, 5),
            source_id: 0,
        };

        let regions = vec![CroppedRegion {
            text: "x: 1\nbad: region_value\n".to_string(),
            source_name: "region.yaml".to_string(),
            start_line: 1,
            end_line: 2,
            location: Location {
                line: 2,
                column: 6,
                span: crate::Span::UNKNOWN,
                source_id: 33,
            },
        }];

        let (picked_src, picked_span) = get_source_and_span(&src, &location, &regions);
        assert!(picked_src.inner().contains("region_value"));
        assert!(picked_span.is_some());
    }

    #[test]
    fn get_source_and_span_uses_source_id_to_disambiguate_overlapping_lines_between_root_and_include()
     {
        let src: Arc<NamedSource<String>> = Arc::new(NamedSource::new(
            "root.yaml",
            "root: 1\nconflict: root_value\n".to_owned(),
        ));
        let location = Location {
            line: 2,
            column: 11,
            span: crate::Span::new(0, 10),
            source_id: 1,
        };

        let regions = vec![
            CroppedRegion {
                text: "root: 1\nconflict: root_value\n".to_string(),
                source_name: "root.yaml".to_string(),
                start_line: 1,
                end_line: 2,
                location: Location {
                    line: 2,
                    column: 11,
                    span: crate::Span::UNKNOWN,
                    source_id: 1,
                },
            },
            CroppedRegion {
                text: "foo: 1\nconflict: include_value\n".to_string(),
                source_name: "child.yaml".to_string(),
                start_line: 1,
                end_line: 2,
                location: Location {
                    line: 2,
                    column: 11,
                    span: crate::Span::UNKNOWN,
                    source_id: 2,
                },
            },
        ];

        let (picked_src, picked_span) = get_source_and_span(&src, &location, &regions);
        assert!(picked_src.inner().contains("root_value"));
        assert!(!picked_src.inner().contains("include_value"));
        assert!(picked_span.is_some());
    }

    #[test]
    fn get_source_and_span_clamps_span_at_region_eof() {
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", "a:\n".to_owned()));
        let location = Location {
            line: 1,
            column: 3,
            span: crate::Span::new(0, 0),
            source_id: 1,
        };

        let regions = vec![CroppedRegion {
            text: "a:".to_string(),
            source_name: "snippet.yaml".to_string(),
            start_line: 1,
            end_line: 1,
            location: Location {
                line: 1,
                column: 1,
                span: crate::Span::UNKNOWN,
                source_id: 1,
            },
        }];

        let (_picked_src, picked_span) = get_source_and_span(&src, &location, &regions);
        let picked_span = picked_span.expect("expected a span");
        assert_eq!(picked_span.offset(), 2);
        assert_eq!(picked_span.len(), 0);
    }

    #[test]
    fn get_source_and_span_sanitizes_raw_region_text() {
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", "root: 1\n".to_owned()));
        let location = Location {
            line: 1,
            column: 8,
            span: crate::Span::new(7, 10),
            source_id: 1,
        };
        let regions = vec![CroppedRegion {
            text: "field: \x1B]0;malicious title\x07\n".to_string(),
            source_name: "snippet.yaml".to_string(),
            start_line: 1,
            end_line: 1,
            location,
        }];

        let (picked_src, picked_span) = get_source_and_span(&src, &location, &regions);

        assert!(picked_span.is_some());
        assert!(!picked_src.inner().contains("\x1B]0;"));
        assert!(!picked_src.inner().contains('\x07'));
    }

    #[test]
    fn with_snippet_skips_primary_region_from_related_entries() {
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", "bad: value\n".to_owned()));

        let err = Error::WithSnippet {
            error: Box::new(Error::Message {
                msg: "invalid value".to_owned(),
                location: Location {
                    line: 1,
                    column: 6,
                    span: crate::Span::new(5, 5),
                    source_id: 1,
                },
            }),
            crop_radius: 2,
            regions: vec![CroppedRegion {
                text: "bad: value\n".to_string(),
                source_name: "input.yaml".to_string(),
                start_line: 1,
                end_line: 1,
                location: Location {
                    line: 1,
                    column: 6,
                    span: crate::Span::UNKNOWN,
                    source_id: 1,
                },
            }],
        };

        let diag = build_diagnostic(
            &err,
            Arc::clone(&src),
            RenderOptions::default().formatter,
            &[],
        );

        assert!(
            diag.related.is_empty(),
            "primary snippet region should not be repeated as included-from-here"
        );
    }

    #[cfg(any(feature = "garde", feature = "validator"))]
    #[rstest::rstest]
    #[case::without_include(false)]
    #[case::with_include(true)]
    fn validation_snippet_regions_are_not_include_notes(#[case] with_include: bool) {
        let yaml = "first: bad\n# 2\n# 3\n# 4\n# 5\n# 6\n# 7\n# 8\nsecond: wrong\n";
        let source_name = if with_include {
            "child.yaml"
        } else {
            "input.yaml"
        };
        let source_id = if with_include { 2 } else { 1 };
        let mut issues = Vec::new();
        let mut locations = PathMap::new();
        for (field, value, line, column) in [("first", "bad", 1, 8), ("second", "wrong", 9, 9)] {
            let path = PathKey::empty().join(field);
            let location = Location {
                line,
                column,
                span: crate::Span::new(yaml.find(value).unwrap() as u64, value.len() as u64),
                source_id,
            };
            issues.push(crate::de_error::ValidationIssue::new(
                path.clone(),
                "invalid",
            ));
            locations.insert(
                path,
                Locations {
                    reference_location: location,
                    defined_location: location,
                },
            );
        }
        let mut error = Error::ValidationError {
            source: crate::de_error::ValidationSource::Validator,
            issues,
            locations,
        }
        .with_snippet_named(yaml, source_name, 1);
        let Error::WithSnippet { regions, .. } = &mut error else {
            panic!("expected snippet regions");
        };
        assert_eq!(regions.len(), 2);
        assert!(regions[0].end_line < regions[1].start_line);

        let root_yaml = "config: !include child.yaml\n";
        if with_include {
            regions.push(CroppedRegion::new(
                root_yaml,
                "root.yaml",
                1,
                1,
                Location {
                    line: 1,
                    column: 9,
                    span: crate::Span::new(8, 19),
                    source_id: 1,
                },
            ));
        }
        let (file, source) = if with_include {
            ("root.yaml", root_yaml)
        } else {
            (source_name, yaml)
        };
        let src = Arc::new(NamedSource::new(file, source.to_owned()));
        let diagnostic = build_diagnostic(&error, src, RenderOptions::default().formatter, &[]);

        assert_eq!(diagnostic.related.len(), 2 + usize::from(with_include));
        for (entry, value) in diagnostic.related.iter().zip(["bad", "wrong"]) {
            assert!(entry.message.starts_with("validation error:"));
            assert_eq!(entry.labels.len(), 1);
            assert_eq!(labeled_text(entry, &entry.labels[0]), value);
            assert!(entry.related.is_empty());
        }
        if with_include {
            let include = &diagnostic.related[2];
            assert_eq!(include.message, "included from here");
            assert_eq!(include.src.name(), "root.yaml");
            assert_eq!(
                labeled_text(include, &include.labels[0]),
                "!include child.yaml"
            );
        }
        let shifted = to_miette_report_with_options(
            &error,
            source,
            file,
            crate::render_options! { line_offset: 9999 },
        );
        assert_display_offset(
            &diagnostic,
            shifted.downcast_ref::<ErrorDiagnostic>().unwrap(),
            9999,
        );
        let renamed = to_miette_report_with_options(
            &error,
            source,
            file,
            crate::render_options! {
                line_offset: 9999,
                source_name: Some("article.md"),
            },
        );
        assert_source_display(
            &diagnostic,
            renamed.downcast_ref::<ErrorDiagnostic>().unwrap(),
            9999,
            Some("article.md"),
        );
    }

    #[test]
    fn with_snippet_keeps_non_primary_related_entry_for_same_name_different_sources() {
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", "root: 1\n".to_owned()));

        let err = Error::WithSnippet {
            error: Box::new(Error::Message {
                msg: "invalid value".to_owned(),
                location: Location {
                    line: 2,
                    column: 6,
                    span: crate::Span::new(0, 5),
                    source_id: 2,
                },
            }),
            crop_radius: 2,
            regions: vec![
                CroppedRegion {
                    text: "x: 1\nbad: parent_value\n".to_string(),
                    source_name: "dup.yaml".to_string(),
                    start_line: 1,
                    end_line: 2,
                    location: Location {
                        line: 2,
                        column: 6,
                        span: crate::Span::UNKNOWN,
                        source_id: 1,
                    },
                },
                CroppedRegion {
                    text: "x: 1\nbad: child_value\n".to_string(),
                    source_name: "dup.yaml".to_string(),
                    start_line: 1,
                    end_line: 2,
                    location: Location {
                        line: 2,
                        column: 6,
                        span: crate::Span::UNKNOWN,
                        source_id: 2,
                    },
                },
            ],
        };

        let diag = build_diagnostic(
            &err,
            Arc::clone(&src),
            RenderOptions::default().formatter,
            &[],
        );
        assert!(diag.src.inner().contains("child_value"));
        assert_eq!(diag.related.len(), 1);
        assert!(
            diag.related
                .iter()
                .any(|d| d.src.inner().contains("parent_value"))
        );
        assert!(
            !diag
                .related
                .iter()
                .any(|d| d.src.inner().contains("child_value"))
        );
    }

    #[test]
    fn basic_error_has_primary_label_span() {
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", "a: definitely\n".to_owned()));
        let err = Error::Message {
            msg: "invalid bool".to_owned(),
            location: Location {
                line: 1,
                column: 4,
                span: crate::Span::new("a: definitely\n".find("definitely").unwrap() as u64, 3),
                source_id: 0,
            },
        };

        let diag = build_diagnostic(
            &err,
            Arc::clone(&src),
            RenderOptions::default().formatter,
            &[],
        );
        let labels: Vec<_> = diag.labels().unwrap().collect();
        assert_eq!(labels.len(), 1);
        assert_eq!(
            labels[0].inner().offset(),
            err.location().unwrap().span().offset() as usize
        );
    }

    #[test]
    fn non_ascii_prefix_char_offsets_convert_to_byte_offsets() {
        // Three Greek letters (non-ASCII, multi-byte in UTF-8) followed by ASCII "def".
        let yaml = "αβγdef\n";
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", yaml.to_owned()));

        let ascii_slice = "def";
        let byte_off = yaml.find(ascii_slice).expect("substring present");
        // Character-based offset for the start of "def"
        let char_off = yaml[..byte_off].chars().count();

        let err = Error::Message {
            msg: "invalid".to_owned(),
            location: Location {
                line: 1,
                // Column is 1-indexed and character-based; set consistently with the span
                column: (char_off as u32) + 1,
                span: crate::Span::new(char_off as u64, ascii_slice.len() as u64),
                source_id: 0,
            },
        };

        let diag = build_diagnostic(
            &err,
            Arc::clone(&src),
            RenderOptions::default().formatter,
            &[],
        );
        let labels: Vec<_> = diag.labels().unwrap().collect();
        assert_eq!(labels.len(), 1);
        // miette expects byte offsets; ensure we converted from chars to bytes correctly
        assert_eq!(labels[0].inner().offset(), byte_off);
        assert_eq!(labels[0].inner().len(), ascii_slice.len());
    }

    #[test]
    fn non_ascii_token_itself_converts_correctly() {
        let yaml = "a: áé\n"; // value contains two non-ASCII letters
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", yaml.to_owned()));

        let value_chars = "áé";
        let start_byte = yaml.find(value_chars).unwrap();
        let start_char = yaml[..start_byte].chars().count();

        // Span over the two non-ASCII characters in character units
        let err = Error::Message {
            msg: "invalid".to_owned(),
            location: Location {
                line: 1,
                column: (start_char as u32) + 1,
                span: crate::Span::new(start_char as u64, value_chars.chars().count() as u64),
                source_id: 0,
            },
        };

        let diag = build_diagnostic(
            &err,
            Arc::clone(&src),
            RenderOptions::default().formatter,
            &[],
        );
        let labels: Vec<_> = diag.labels().unwrap().collect();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].inner().offset(), start_byte);
        assert_eq!(labels[0].inner().len(), value_chars.len()); // bytes
    }

    #[test]
    fn zero_length_span_highlights_one_char() {
        let yaml = "key: value\n";
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", yaml.to_owned()));
        let start_byte = yaml.find("value").unwrap();
        let start_char = yaml[..start_byte].chars().count();

        // Zero-length in characters
        let err = Error::Message {
            msg: "invalid".to_owned(),
            location: Location {
                line: 1,
                column: (start_char as u32) + 1,
                span: crate::Span::new(start_char as u64, 0),
                source_id: 0,
            },
        };

        let diag = build_diagnostic(
            &err,
            Arc::clone(&src),
            RenderOptions::default().formatter,
            &[],
        );
        let labels: Vec<_> = diag.labels().unwrap().collect();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].inner().offset(), start_byte);
        assert_eq!(labels[0].inner().len(), 1);
    }

    #[test]
    fn span_past_end_is_clamped() {
        let yaml = "hello"; // 5 bytes, 5 chars
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", yaml.to_owned()));
        // Start at char 3 (the 'l'), but ask for a very long span
        let start_char = 3usize;
        let start_byte = yaml.char_indices().nth(start_char).map(|(i, _)| i).unwrap();

        let err = Error::Message {
            msg: "invalid".to_owned(),
            location: Location {
                line: 1,
                column: (start_char as u32) + 1,
                span: crate::Span::new(start_char as u64, 1000),
                source_id: 0,
            },
        };

        let diag = build_diagnostic(
            &err,
            Arc::clone(&src),
            RenderOptions::default().formatter,
            &[],
        );
        let labels: Vec<_> = diag.labels().unwrap().collect();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].inner().offset(), start_byte);
        // Clamped to end of string
        assert_eq!(labels[0].inner().len(), yaml.len() - start_byte);
    }

    #[test]
    fn span_at_source_end_keeps_zero_length_label() {
        let yaml = "αβ\n";
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", yaml.to_owned()));
        let end_char = yaml.chars().count();

        let err = Error::Eof {
            location: Location {
                line: 2,
                column: 1,
                span: crate::Span::new(end_char as u64, 0),
                source_id: 0,
            },
        };

        let diag = build_diagnostic(
            &err,
            Arc::clone(&src),
            RenderOptions::default().formatter,
            &[],
        );
        let labels: Vec<_> = diag.labels().unwrap().collect();

        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].inner().offset(), yaml.len());
        assert_eq!(labels[0].inner().len(), 0);
    }

    #[test]
    fn multiline_offset_after_newline() {
        let yaml = "α\nβ\nxyz\n"; // 1-char lines, then ascii line
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("input.yaml", yaml.to_owned()));
        let target = "xyz";
        let start_byte = yaml.find(target).unwrap();
        let start_char = yaml[..start_byte].chars().count();

        let err = Error::Message {
            msg: "invalid".to_owned(),
            location: Location {
                line: 3,
                column: 1,
                span: crate::Span::new(start_char as u64, target.chars().count() as u64),
                source_id: 0,
            },
        };

        let diag = build_diagnostic(
            &err,
            Arc::clone(&src),
            RenderOptions::default().formatter,
            &[],
        );
        let labels: Vec<_> = diag.labels().unwrap().collect();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].inner().offset(), start_byte);
        assert_eq!(labels[0].inner().len(), target.len());
    }

    #[cfg(feature = "validator")]
    #[test]
    fn validator_validation_error_has_use_and_definition_labels() {
        use validator::Validate;

        #[derive(Debug, Validate)]
        struct Cfg {
            #[validate(length(min = 2))]
            second_string: String,
        }

        let cfg = Cfg {
            second_string: "x".to_owned(),
        };
        let errors = cfg.validate().expect_err("validation error expected");

        // Simulate the alias case:
        // - use-site is at `secondString: *A`
        // - definition-site is at `firstString: &A "x"`
        let yaml = "\nfirstString: &A \"x\"\nsecondString: *A\n";
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("config.yaml", yaml.to_owned()));

        let use_offset = yaml.find("*A").unwrap();
        let def_offset = yaml.find("\"x\"").unwrap();

        let referenced_loc = Location {
            line: 3,
            column: 15,
            span: crate::Span::new(use_offset as u64, 2),
            source_id: 0,
        };
        let defined_loc = Location {
            line: 2,
            column: 18,
            span: crate::Span::new(def_offset as u64, 3),
            source_id: 0,
        };

        let mut locations = PathMap::new();

        // Validation path uses snake_case (`second_string`), but the YAML key is camelCase.
        // We insert the recorded YAML spelling so `PathMap::search()` resolves the leaf.
        let yaml_path = PathKey::empty().join("secondString");
        locations.insert(
            yaml_path,
            Locations {
                reference_location: referenced_loc,
                defined_location: defined_loc,
            },
        );

        let err = Error::ValidationError {
            source: crate::de_error::ValidationSource::Validator,
            issues: crate::de_error::collect_validator_issues(&errors),
            locations,
        };

        let diag = build_diagnostic(
            &err,
            Arc::clone(&src),
            RenderOptions::default().formatter,
            &[],
        );
        assert_eq!(diag.message, "validation failed");
        assert_eq!(diag.related.len(), 1);

        let labels = &diag.related[0].labels;
        assert_eq!(labels.len(), 2, "expected 2 labels, got: {labels:?}");

        let label_debug = format!("{labels:?}");
        assert!(
            label_debug.contains("the value is used here"),
            "expected use-site label, got: {label_debug}"
        );
        assert!(
            label_debug.contains("defined here"),
            "expected definition-site label, got: {label_debug}"
        );
    }

    #[cfg(feature = "garde")]
    #[test]
    fn garde_validation_error_has_use_and_definition_labels() {
        use garde::Validate;

        #[derive(Debug, Validate)]
        struct Cfg {
            #[garde(length(min = 2))]
            second_string: String,
        }

        let cfg = Cfg {
            second_string: "x".to_owned(),
        };
        let report = cfg.validate().expect_err("validation error expected");

        // Simulate the alias case:
        // - use-site is at `secondString: *A`
        // - definition-site is at `firstString: &A "x"`
        let yaml = "\nfirstString: &A \"x\"\nsecondString: *A\n";
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("config.yaml", yaml.to_owned()));

        let use_offset = yaml.find("*A").unwrap();
        let def_offset = yaml.find("\"x\"").unwrap();

        let referenced_loc = Location {
            line: 3,
            column: 15,
            span: crate::Span::new(use_offset as u64, 2),
            source_id: 0,
        };
        let defined_loc = Location {
            line: 2,
            column: 18,
            span: crate::Span::new(def_offset as u64, 3),
            source_id: 0,
        };

        let mut locations = PathMap::new();

        // Validation path uses snake_case (`second_string`), but the YAML key is camelCase.
        // We insert the recorded YAML spelling so `PathMap::search()` resolves the leaf.
        let yaml_path = PathKey::empty().join("secondString");
        locations.insert(
            yaml_path,
            Locations {
                reference_location: referenced_loc,
                defined_location: defined_loc,
            },
        );

        let err = Error::ValidationError {
            source: crate::de_error::ValidationSource::Garde,
            issues: crate::de_error::collect_garde_issues(&report),
            locations,
        };

        let diag = build_diagnostic(
            &err,
            Arc::clone(&src),
            RenderOptions::default().formatter,
            &[],
        );
        assert_eq!(diag.message, "validation failed");
        assert_eq!(diag.related.len(), 1);

        let labels = &diag.related[0].labels;
        assert_eq!(labels.len(), 2, "expected 2 labels, got: {labels:?}");

        let label_debug = format!("{labels:?}");
        assert!(
            label_debug.contains("the value is used here"),
            "expected use-site label, got: {label_debug}"
        );
        assert!(
            label_debug.contains("defined here"),
            "expected definition-site label, got: {label_debug}"
        );
    }

    #[test]
    #[allow(deprecated)] // Populates the legacy msg field when constructing an alias error.
    fn alias_error_has_use_and_definition_labels() {
        use crate::location::Locations;

        // Simulate an alias error where:
        // - use-site is at `value: *anchor`
        // - definition-site is at `anchor: &anchor "bad"`
        let yaml = "anchor: &a \"bad\"\nvalue: *a\n";
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("config.yaml", yaml.to_owned()));

        let use_offset = yaml.find("*a").unwrap();
        let def_offset = yaml.find("\"bad\"").unwrap();

        let referenced_loc = Location {
            line: 2,
            column: 8,
            span: crate::Span::new(use_offset as u64, 2),
            source_id: 0,
        };
        let defined_loc = Location {
            line: 1,
            column: 13,
            span: crate::Span::new(def_offset as u64, 5),
            source_id: 0,
        };

        let err = Error::AliasError {
            msg: "invalid value for alias".to_owned(),
            error: Box::new(Error::msg("invalid value for alias")),
            locations: Locations {
                reference_location: referenced_loc,
                defined_location: defined_loc,
            },
        };

        let diag = build_diagnostic(
            &err,
            Arc::clone(&src),
            RenderOptions::default().formatter,
            &[],
        );
        assert_eq!(diag.message, "invalid value for alias");

        let labels = &diag.labels;
        assert_eq!(labels.len(), 2, "expected 2 labels, got: {labels:?}");

        let label_debug = format!("{labels:?}");
        assert!(
            label_debug.contains("the value is used here"),
            "expected use-site label, got: {label_debug}"
        );
        assert!(
            label_debug.contains("anchor defined here"),
            "expected definition-site label, got: {label_debug}"
        );
    }

    #[test]
    #[allow(deprecated)] // Populates the legacy msg field when constructing an alias error.
    fn alias_error_with_same_locations_has_single_label() {
        use crate::location::Locations;

        let yaml = "value: \"bad\"\n";
        let src: Arc<NamedSource<String>> =
            Arc::new(NamedSource::new("config.yaml", yaml.to_owned()));

        let offset = yaml.find("\"bad\"").unwrap();
        let loc = Location {
            line: 1,
            column: 8,
            span: crate::Span::new(offset as u64, 5),
            source_id: 0,
        };

        let err = Error::AliasError {
            msg: "invalid value".to_owned(),
            error: Box::new(Error::msg("invalid value")),
            locations: Locations {
                reference_location: loc,
                defined_location: loc,
            },
        };

        let diag = build_diagnostic(
            &err,
            Arc::clone(&src),
            RenderOptions::default().formatter,
            &[],
        );
        assert_eq!(diag.message, "invalid value");

        // When both locations are the same, should only have one label
        let labels = &diag.labels;
        assert_eq!(
            labels.len(),
            1,
            "expected 1 label when locations are same, got: {labels:?}"
        );
    }
}
