//! `miette` integration.
//!
//! This module is feature-gated behind the `miette` feature.

use std::fmt;
use std::sync::Arc;

use miette::{Diagnostic, LabeledSpan, NamedSource, SourceSpan};

use crate::Error;
use crate::Location;
#[cfg(any(feature = "garde", feature = "validator"))]
use crate::path_map::{PathKey, PathMap, format_path_with_resolved_leaf};
#[cfg(feature = "garde")]
use crate::path_map::path_key_from_garde;

#[cfg(feature = "validator")]
use validator::{ValidationErrors, ValidationErrorsKind};

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
                let path_key = path_key_from_garde(path);
                related.push(build_validation_entry_diagnostic(
                    &src,
                    &path_key,
                    &entry.to_string(),
                    referenced,
                    defined,
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
                related.push(build_diagnostic(e.without_snippet(), Arc::clone(&src)));
            }

            ErrorDiagnostic {
                message: format!("validation failed for {} document(s)", errors.len()),
                src,
                labels: Vec::new(),
                related,
            }
        }

        #[cfg(feature = "validator")]
        Error::ValidatorError {
            errors,
            referenced,
            defined,
        } => {
            let entries = collect_validator_entries(errors);
            let mut related = Vec::new();

            for (path, entry) in entries {
                related.push(build_validation_entry_diagnostic(
                    &src,
                    &path,
                    &entry,
                    referenced,
                    defined,
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

#[cfg(any(feature = "garde", feature = "validator"))]
fn build_validation_entry_diagnostic(
    src: &Arc<NamedSource<String>>,
    path_key: &PathKey,
    entry: &str,
    referenced: &PathMap,
    defined: &PathMap,
) -> ErrorDiagnostic {
    let original_leaf = path_key
        .leaf_string()
        .unwrap_or_else(|| "<root>".to_string());

    let (ref_loc, ref_leaf) = referenced
        .search(path_key)
        .unwrap_or((Location::UNKNOWN, original_leaf.clone()));
    let (def_loc, def_leaf) = defined
        .search(path_key)
        .unwrap_or((Location::UNKNOWN, original_leaf.clone()));

    let resolved_leaf = if ref_loc != Location::UNKNOWN {
        ref_leaf
    } else if def_loc != Location::UNKNOWN {
        def_leaf
    } else {
        original_leaf
    };

    let resolved_path = format_path_with_resolved_leaf(path_key, &resolved_leaf);
    let base_msg = format!("validation error: {entry} for `{resolved_path}`");

    let labels = build_validation_labels(src, ref_loc, def_loc);

    ErrorDiagnostic {
        message: base_msg,
        src: Arc::clone(src),
        labels,
        related: Vec::new(),
    }
}

#[cfg(any(feature = "garde", feature = "validator"))]
fn build_validation_labels(
    src: &Arc<NamedSource<String>>,
    ref_loc: Location,
    def_loc: Location,
) -> Vec<LabeledSpan> {
    let mut labels = Vec::new();

    // Primary label: use-site (alias/merge) if known, otherwise definition.
    if ref_loc != Location::UNKNOWN {
        if let Some(span) = to_source_span(src, &ref_loc) {
            labels.push(LabeledSpan::new_with_span(
                Some("the value is used here".to_owned()),
                span,
            ));
        }
    } else if def_loc != Location::UNKNOWN {
        if let Some(span) = to_source_span(src, &def_loc) {
            labels.push(LabeledSpan::new_with_span(
                Some("defined here".to_owned()),
                span,
            ));
        }
    }

    // Secondary label: definition site when it is distinct and known.
    if def_loc != Location::UNKNOWN && def_loc != ref_loc {
        if let Some(span) = to_source_span(src, &def_loc) {
            labels.push(LabeledSpan::new_with_span(Some("defined here".to_owned()), span));
        }
    }

    labels
}

#[cfg(feature = "validator")]
fn collect_validator_entries(errors: &ValidationErrors) -> Vec<(PathKey, String)> {
    let mut out = Vec::new();
    let root = PathKey::empty();
    collect_validator_entries_inner(errors, &root, &mut out);
    out
}

#[cfg(feature = "validator")]
fn collect_validator_entries_inner(
    errors: &ValidationErrors,
    path: &PathKey,
    out: &mut Vec<(PathKey, String)>,
) {
    for (field, kind) in errors.errors() {
        let field_path = path.clone().join(*field);
        match kind {
            ValidationErrorsKind::Field(entries) => {
                for entry in entries {
                    out.push((field_path.clone(), entry.to_string()));
                }
            }
            ValidationErrorsKind::Struct(inner) => {
                collect_validator_entries_inner(inner, &field_path, out);
            }
            ValidationErrorsKind::List(list) => {
                for (idx, inner) in list {
                    let index_path = field_path.clone().join(*idx);
                    collect_validator_entries_inner(inner, &index_path, out);
                }
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

        #[cfg(feature = "validator")]
        Error::ValidatorError { errors, .. } => format!("validation error: {errors}"),
        #[cfg(feature = "validator")]
        Error::ValidatorErrors { errors } => {
            format!("validation failed for {} document(s)", errors.len())
        }
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
        let src = Arc::new(NamedSource::new("config.yaml".to_owned(), yaml.to_owned()));

        let use_offset = yaml.find("*A").unwrap();
        let def_offset = yaml.find("\"x\"").unwrap();

        let referenced_loc = Location {
            line: 3,
            column: 15,
            span: crate::Span {
                offset: use_offset,
                len: 2,
            },
        };
        let defined_loc = Location {
            line: 2,
            column: 18,
            span: crate::Span {
                offset: def_offset,
                len: 3,
            },
        };

        let mut referenced = PathMap::new();
        let mut defined = PathMap::new();

        // Validation path uses snake_case (`second_string`), but the YAML key is camelCase.
        // We insert the recorded YAML spelling so `PathMap::search()` resolves the leaf.
        let yaml_path = PathKey::empty().join("secondString");
        referenced.insert(yaml_path.clone(), referenced_loc);
        defined.insert(yaml_path, defined_loc);

        let err = Error::ValidatorError {
            errors,
            referenced,
            defined,
        };

        let diag = build_diagnostic(&err, Arc::clone(&src));
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
}
