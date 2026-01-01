//! `miette` integration.
//!
//! This module is feature-gated behind the `miette` feature.

use std::fmt;
use std::sync::Arc;

use miette::{Diagnostic, LabeledSpan, NamedSource, SourceSpan};

use crate::Error;
use crate::Location;

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
    let src = Arc::new(NamedSource::new(file.to_owned(), source.to_owned()));
    let diag = build_diagnostic(err.without_snippet(), src);
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
        Some(Box::new(
            self.related.iter().map(|d| d as &dyn Diagnostic),
        ))
    }
}

fn build_diagnostic(err: &Error, src: Arc<NamedSource<String>>) -> ErrorDiagnostic {
    match err {
        #[cfg(feature = "garde")]
        Error::ValidationError {
            report,
            referenced,
            defined,
        } => {
            let mut related = Vec::new();
            for (path, entry) in report.iter() {
                let original_leaf = garde_leaf_segment(path).unwrap_or("<root>").to_owned();

                let (ref_loc, ref_leaf) = referenced
                    .search(path)
                    .unwrap_or((Location::UNKNOWN, original_leaf.clone()));
                let (def_loc, def_leaf) = defined
                    .search(path)
                    .unwrap_or((Location::UNKNOWN, original_leaf.clone()));

                let resolved_leaf = if ref_loc != Location::UNKNOWN {
                    ref_leaf
                } else if def_loc != Location::UNKNOWN {
                    def_leaf
                } else {
                    original_leaf
                };

                let resolved_path = format_garde_path_with_resolved_leaf(path, &resolved_leaf);
                let base_msg = format!("validation error: {entry} for `{resolved_path}`");

                let mut labels = Vec::new();

                // Primary label: use-site (alias/merge) if known, otherwise definition.
                if ref_loc != Location::UNKNOWN {
                    if let Some(span) = to_source_span(&src, &ref_loc) {
                        labels.push(LabeledSpan::new_with_span(
                            Some("the value is used here".to_owned()),
                            span,
                        ));
                    }
                } else if def_loc != Location::UNKNOWN {
                    if let Some(span) = to_source_span(&src, &def_loc) {
                        labels.push(LabeledSpan::new_with_span(
                            Some("defined here".to_owned()),
                            span,
                        ));
                    }
                }

                // Secondary label: definition site when it is distinct and known.
                if def_loc != Location::UNKNOWN && def_loc != ref_loc {
                    if let Some(span) = to_source_span(&src, &def_loc) {
                        labels.push(LabeledSpan::new_with_span(
                            Some("defined here".to_owned()),
                            span,
                        ));
                    }
                }

                related.push(ErrorDiagnostic {
                    message: base_msg,
                    src: Arc::clone(&src),
                    labels,
                    related: Vec::new(),
                });
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
                related.push(build_diagnostic(e.without_snippet(), Arc::clone(&src)));
            }

            ErrorDiagnostic {
                message: format!("validation failed for {} document(s)", errors.len()),
                src,
                labels: Vec::new(),
                related,
            }
        }

        Error::WithSnippet { error, .. } => build_diagnostic(error.without_snippet(), src),

        other => {
            let mut labels = Vec::new();
            if let Some(loc) = other.location() {
                if let Some(span) = to_source_span(&src, &loc) {
                    labels.push(LabeledSpan::new_with_span(None, span));
                }
            }

            ErrorDiagnostic {
                message: message_without_location(other),
                src,
                labels,
                related: Vec::new(),
            }
        }
    }
}

fn to_source_span(src: &NamedSource<String>, location: &Location) -> Option<SourceSpan> {
    if *location == Location::UNKNOWN {
        return None;
    }

    let offset = location.span().offset();
    let mut len = location.span().len();
    if len == 0 {
        len = 1;
    }

    // Clamp to the actual input, to avoid panics and invalid spans.
    let src_len = src.inner().len();
    if offset > src_len {
        return None;
    }
    let len = len.min(src_len.saturating_sub(offset));

    Some(SourceSpan::new(offset.into(), len.into()))
}

fn message_without_location(err: &Error) -> String {
    match err {
        Error::Message { msg, .. } => msg.clone(),
        Error::HookError { msg, .. } => msg.clone(),
        Error::Eof { .. } => "unexpected end of input".to_owned(),
        Error::Unexpected { expected, .. } => format!("unexpected event: expected {expected}"),
        Error::ContainerEndMismatch { .. } => "list or mapping end with no start".to_owned(),
        Error::UnknownAnchor { id, .. } => format!("alias references unknown anchor id {id}"),
        Error::Budget { breach, .. } => format!("YAML budget breached: {breach:?}"),
        Error::QuotingRequired { value, .. } => {
            format!("The string value [{value}] must be quoted")
        }
        Error::IOError { cause } => format!("IO error: {cause}"),
        Error::WithSnippet { error, .. } => message_without_location(error),

        #[cfg(feature = "garde")]
        Error::ValidationError { report, .. } => format!("validation error: {report}"),
        #[cfg(feature = "garde")]
        Error::ValidationErrors { errors } => {
            format!("validation failed for {} document(s)", errors.len())
        }
    }
}

#[cfg(feature = "garde")]
fn garde_leaf_segment(path: &garde::error::Path) -> Option<&str> {
    path.__iter().next().map(|(_k, s)| s.as_str())
}

#[cfg(feature = "garde")]
fn format_garde_path_with_resolved_leaf(path: &garde::error::Path, resolved_leaf: &str) -> String {
    // garde paths are stored leaf-first.
    let mut segs: Vec<(garde::error::Kind, &str)> =
        path.__iter().map(|(k, s)| (k, s.as_str())).collect();
    if let Some((_k, leaf)) = segs.first_mut() {
        *leaf = resolved_leaf;
    }
    segs.reverse();

    let mut out = String::new();
    for (kind, seg) in segs {
        match kind {
            garde::error::Kind::None | garde::error::Kind::Key => {
                if out.is_empty() {
                    out.push_str(seg);
                } else {
                    out.push('.');
                    out.push_str(seg);
                }
            }
            garde::error::Kind::Index => {
                out.push('[');
                out.push_str(seg);
                out.push(']');
            }
        }
    }
    if out.is_empty() {
        "<root>".to_owned()
    } else {
        out
    }
}

#[cfg(all(test, feature = "miette"))]
mod tests {
    use super::*;

    #[test]
    fn basic_error_has_primary_label_span() {
        let src = Arc::new(NamedSource::new(
            "input.yaml".to_owned(),
            "a: definitely\n".to_owned(),
        ));
        let err = Error::Message {
            msg: "invalid bool".to_owned(),
            location: Location {
                line: 1,
                column: 4,
                span: crate::Span {
                    offset: "a: definitely\n".find("definitely").unwrap(),
                    len: 3,
                },
            },
        };

        let diag = build_diagnostic(&err, Arc::clone(&src));
        let labels: Vec<_> = diag.labels().unwrap().collect();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].inner().offset(), err.location().unwrap().span().offset());
    }
}
