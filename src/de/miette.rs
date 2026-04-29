//! `miette` integration.
//!
//! This module is feature-gated behind the `miette` feature.

use std::fmt;
use std::sync::Arc;

use miette::{Diagnostic, LabeledSpan, NamedSource, SourceSpan};

use crate::Error;
use crate::Location;
use crate::de_error::CroppedRegion;
use crate::de_snippet::sanitize_terminal_snippet_preserve_len;
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
pub fn to_miette_report(err: &Error, source: &str, file: &str) -> miette::Report {
    to_miette_report_with_formatter(err, source, file, RenderOptions::default().formatter)
}

pub fn to_miette_report_with_formatter(
    err: &Error,
    source: &str,
    file: &str,
    formatter: &dyn MessageFormatter,
) -> miette::Report {
    let sanitized_source = sanitize_terminal_snippet_preserve_len(source.to_owned());
    let src = Arc::new(NamedSource::new(file, sanitized_source));
    let diag = build_diagnostic(err, src, formatter, &[]);
    miette::Report::new(diag)
}

#[derive(Clone, Debug)]
struct ErrorDiagnostic {
    message: String,
    src: Arc<NamedSource<String>>,
    labels: Vec<LabeledSpan>,
    related: Vec<ErrorDiagnostic>,
}

impl fmt::Display for ErrorDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ErrorDiagnostic {}

impl Diagnostic for ErrorDiagnostic {
    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&*self.src)
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
        #[cfg(feature = "garde")]
        Error::ValidationError { issues, locations } => {
            let mut related = Vec::new();
            for issue in issues {
                related.push(build_validation_entry_diagnostic(
                    &src,
                    &issue.path,
                    &issue.display_entry(),
                    locations,
                    regions,
                ));
            }

            ErrorDiagnostic {
                message: format!(
                    "validation failed{}",
                    if related.len() == 1 {
                        ""
                    } else {
                        " (multiple errors)"
                    }
                ),
                src,
                labels: Vec::new(),
                related,
            }
        }

        #[cfg(feature = "garde")]
        Error::ValidationErrors { errors } => {
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
                message: format!("validation failed for {} document(s)", errors.len()),
                src,
                labels: Vec::new(),
                related,
            }
        }

        #[cfg(feature = "validator")]
        Error::ValidatorError { issues, locations } => {
            let mut related = Vec::new();

            for issue in issues {
                related.push(build_validation_entry_diagnostic(
                    &src,
                    &issue.path,
                    &issue.display_entry(),
                    locations,
                    regions,
                ));
            }

            ErrorDiagnostic {
                message: format!(
                    "validation failed{}",
                    if related.len() == 1 {
                        ""
                    } else {
                        " (multiple errors)"
                    }
                ),
                src,
                labels: Vec::new(),
                related,
            }
        }

        #[cfg(feature = "validator")]
        Error::ValidatorErrors { errors } => {
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
                message: format!("validation failed for {} document(s)", errors.len()),
                src,
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

            for region in snippet_regions {
                let key = (
                    region.source_name.as_str(),
                    region.location.source_id(),
                    region.start_line,
                    region.end_line,
                );

                if used_regions.insert(key) {
                    let (synthetic_src, span) =
                        get_source_and_span(&diag.src, &region.location, snippet_regions);
                    if let Some(span) = span {
                        diag.related.push(ErrorDiagnostic {
                            message: "included from here".to_owned(),
                            src: synthetic_src,
                            labels: vec![LabeledSpan::new_with_span(None, span)],
                            related: Vec::new(),
                        });
                    }
                }
            }

            diag
        }

        Error::AliasError { msg: _, locations } => {
            let (actual_src, labels) = build_alias_labels(
                &src,
                locations.reference_location,
                locations.defined_location,
                regions,
            );

            ErrorDiagnostic {
                message: formatter.format_message(err).into_owned(),
                src: actual_src,
                labels,
                related: Vec::new(),
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
                    Some(formatter.format_message(other).into_owned()),
                    span,
                ));
            }

            ErrorDiagnostic {
                message: formatter.format_message(other).into_owned(),
                src: actual_src,
                labels,
                related: Vec::new(),
            }
        }
    }
}

#[cfg(any(feature = "garde", feature = "validator"))]
fn build_validation_entry_diagnostic(
    src: &Arc<NamedSource<String>>,
    path_key: &PathKey,
    entry: &str,
    locations: &PathMap,
    regions: &[CroppedRegion],
) -> ErrorDiagnostic {
    let original_leaf = path_key
        .leaf_string()
        .unwrap_or_else(|| "<root>".to_string());

    let (locs, resolved_leaf) = locations
        .search(path_key)
        .or_else(|| {
            // If the exact path isn't recorded (common when custom deserialization transforms the
            // YAML shape, e.g. sequence -> map keyed by derived IDs), fall back to the nearest
            // ancestor path that has a known location so we can still render a useful snippet.
            let mut p = path_key.parent();
            while let Some(cur) = p {
                if let Some(found) = locations.search(&cur) {
                    return Some(found);
                }
                p = cur.parent();
            }
            None
        })
        .unwrap_or((Locations::UNKNOWN, original_leaf));

    let ref_loc = locs.reference_location;
    let def_loc = locs.defined_location;

    let resolved_path = format_path_with_resolved_leaf(path_key, &resolved_leaf);
    let base_msg = format!("validation error: {entry} for `{resolved_path}`");

    let (actual_src, labels) = build_validation_labels(src, ref_loc, def_loc, regions);

    ErrorDiagnostic {
        message: base_msg,
        src: actual_src,
        labels,
        related: Vec::new(),
    }
}

#[cfg(any(feature = "garde", feature = "validator"))]
fn build_validation_labels(
    src: &Arc<NamedSource<String>>,
    ref_loc: Location,
    def_loc: Location,
    regions: &[CroppedRegion],
) -> (Arc<NamedSource<String>>, Vec<LabeledSpan>) {
    let mut labels = Vec::new();

    let primary_loc = if ref_loc != Location::UNKNOWN {
        ref_loc
    } else {
        def_loc
    };
    let (primary_src, span) = get_source_and_span(src, &primary_loc, regions);

    if let Some(span) = span {
        labels.push(LabeledSpan::new_with_span(
            Some(if ref_loc != Location::UNKNOWN {
                "the value is used here".to_owned()
            } else {
                "defined here".to_owned()
            }),
            span,
        ));
    }

    if def_loc != Location::UNKNOWN && def_loc != ref_loc {
        let (def_src, def_span) = get_source_and_span(src, &def_loc, regions);
        if (Arc::ptr_eq(&primary_src, &def_src) || def_src.name() == primary_src.name())
            && let Some(span) = def_span
        {
            labels.push(LabeledSpan::new_with_span(
                Some("defined here".to_owned()),
                span,
            ));
        }
    }

    (primary_src, labels)
}

/// Build labels for an alias error with both reference and defined locations.
fn build_alias_labels(
    src: &Arc<NamedSource<String>>,
    ref_loc: Location,
    def_loc: Location,
    regions: &[CroppedRegion],
) -> (Arc<NamedSource<String>>, Vec<LabeledSpan>) {
    let mut labels = Vec::new();

    let primary_loc = if ref_loc != Location::UNKNOWN {
        ref_loc
    } else {
        def_loc
    };
    let (primary_src, span) = get_source_and_span(src, &primary_loc, regions);

    if let Some(span) = span {
        labels.push(LabeledSpan::new_with_span(
            Some(if ref_loc != Location::UNKNOWN {
                "the value is used here".to_owned()
            } else {
                "defined here".to_owned()
            }),
            span,
        ));
    }

    if def_loc != Location::UNKNOWN && def_loc != ref_loc {
        let (def_src, def_span) = get_source_and_span(src, &def_loc, regions);
        if (Arc::ptr_eq(&primary_src, &def_src) || def_src.name() == primary_src.name())
            && let Some(span) = def_span
        {
            labels.push(LabeledSpan::new_with_span(
                Some("anchor defined here".to_owned()),
                span,
            ));
        }
    }

    (primary_src, labels)
}

fn get_source_and_span(
    src: &Arc<NamedSource<String>>,
    location: &Location,
    regions: &[CroppedRegion],
) -> (Arc<NamedSource<String>>, Option<SourceSpan>) {
    if *location == Location::UNKNOWN {
        return (Arc::clone(src), None);
    }

    let line = location.line as usize;
    let location_source_id = location.source_id();
    let region = regions
        .iter()
        .find(|r| {
            location_source_id != 0
                && r.location.source_id() == location_source_id
                && r.start_line <= line
                && line <= r.end_line
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
        .or_else(|| regions.first());

    if let Some(region) = region {
        let start_line = region.start_line.saturating_sub(1);
        let mut padded_text = String::new();
        for _ in 0..start_line {
            padded_text.push('\n');
        }
        padded_text.push_str(&region.text);

        let synthetic_src = Arc::new(NamedSource::new(
            region.source_name.as_str(),
            padded_text.clone(),
        ));

        let mut byte_off = 0;
        let mut found = false;
        for (i, line) in padded_text.split_terminator('\n').enumerate() {
            if i + 1 == location.line as usize {
                let col = location.column as usize;
                let char_off = col.saturating_sub(1);
                let line_byte_off = line
                    .char_indices()
                    .nth(char_off)
                    .map(|(idx, _)| idx)
                    .unwrap_or(line.len());
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
                .map(|(idx, _)| idx)
                .unwrap_or(remainder.len())
                .max(1);
            let clamped_len = byte_len.min(src_len.saturating_sub(byte_off));
            return (
                synthetic_src,
                Some(SourceSpan::new(byte_off.into(), clamped_len)),
            );
        }
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
                s.char_indices().nth(char_offset).map(|(i, _)| i)?
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
    fn with_snippet_keeps_related_entries_for_same_name_different_sources() {
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
        assert_eq!(diag.related.len(), 2);
        assert!(
            diag.related
                .iter()
                .any(|d| d.src.inner().contains("parent_value"))
        );
        assert!(
            diag.related
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

        let err = Error::ValidatorError {
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
        assert_eq!(
            diag.message,
            "invalid value for alias (defined at line 1, column 13)"
        );

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
