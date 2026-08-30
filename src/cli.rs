use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;

use serde_core::de::IgnoredAny;

use crate::de::budget::{BudgetBreach, BudgetReport};
use crate::{Budget, Error, from_str_with_options};

fn usage() -> &'static str {
    "Usage: serde-saphyr [--plain] [--include <path>] <path>\n\
\n\
Reads the YAML file at <path> and prints a budget summary.\n\
It can also be used as a YAML validator.\n\
\n\
Options:\n\
  --plain           Disable miette formatting and print errors in plain text\n\
  --include <path>  Configure parser to allow file inclusion from <path> directory"
}

fn format_budget_report(report: &BudgetReport) -> String {
    let mut out = String::new();

    match &report.breached {
        Some(BudgetBreach::SequenceUnbalanced) => out.push_str("breached: SequenceUnbalanced\n"),
        Some(breach) => format_budget_breach(&mut out, breach),
        None => out.push_str("breached: null\n"),
    }

    let _ = writeln!(out, "events: {}", report.events);
    let _ = writeln!(out, "aliases: {}", report.aliases);
    let _ = writeln!(out, "anchors: {}", report.anchors);
    let _ = writeln!(
        out,
        "recorded_anchor_events: {}",
        report.recorded_anchor_events
    );
    let _ = writeln!(
        out,
        "recorded_anchor_bytes: {}",
        report.recorded_anchor_bytes
    );
    let _ = writeln!(out, "documents: {}", report.documents);
    let _ = writeln!(out, "nodes: {}", report.nodes);
    let _ = writeln!(out, "max_depth: {}", report.max_depth);
    let _ = writeln!(out, "total_scalar_bytes: {}", report.total_scalar_bytes);
    let _ = writeln!(out, "total_comment_bytes: {}", report.total_comment_bytes);
    let _ = writeln!(out, "merge_keys: {}", report.merge_keys);

    out
}

fn format_budget_breach(out: &mut String, breach: &BudgetBreach) {
    match breach {
        BudgetBreach::Events { events } => {
            out.push_str("breached:\n  Events:\n");
            let _ = writeln!(out, "    events: {events}");
        }
        BudgetBreach::Aliases { aliases } => {
            out.push_str("breached:\n  Aliases:\n");
            let _ = writeln!(out, "    aliases: {aliases}");
        }
        BudgetBreach::Anchors { anchors } => {
            out.push_str("breached:\n  Anchors:\n");
            let _ = writeln!(out, "    anchors: {anchors}");
        }
        BudgetBreach::RecordedAnchorEvents {
            recorded_anchor_events,
        } => {
            out.push_str("breached:\n  RecordedAnchorEvents:\n");
            let _ = writeln!(out, "    recorded_anchor_events: {recorded_anchor_events}");
        }
        BudgetBreach::RecordedAnchorBytes {
            recorded_anchor_bytes,
        } => {
            out.push_str("breached:\n  RecordedAnchorBytes:\n");
            let _ = writeln!(out, "    recorded_anchor_bytes: {recorded_anchor_bytes}");
        }
        BudgetBreach::Depth { depth } => {
            out.push_str("breached:\n  Depth:\n");
            let _ = writeln!(out, "    depth: {depth}");
        }
        BudgetBreach::InclusionDepth { depth } => {
            out.push_str("breached:\n  InclusionDepth:\n");
            let _ = writeln!(out, "    depth: {depth}");
        }
        BudgetBreach::Documents { documents } => {
            out.push_str("breached:\n  Documents:\n");
            let _ = writeln!(out, "    documents: {documents}");
        }
        BudgetBreach::Nodes { nodes } => {
            out.push_str("breached:\n  Nodes:\n");
            let _ = writeln!(out, "    nodes: {nodes}");
        }
        BudgetBreach::ScalarBytes { total_scalar_bytes } => {
            out.push_str("breached:\n  ScalarBytes:\n");
            let _ = writeln!(out, "    total_scalar_bytes: {total_scalar_bytes}");
        }
        BudgetBreach::CommentBytes {
            total_comment_bytes,
        } => {
            out.push_str("breached:\n  CommentBytes:\n");
            let _ = writeln!(out, "    total_comment_bytes: {total_comment_bytes}");
        }
        BudgetBreach::MergeKeys { merge_keys } => {
            out.push_str("breached:\n  MergeKeys:\n");
            let _ = writeln!(out, "    merge_keys: {merge_keys}");
        }
        BudgetBreach::AliasAnchorRatio { aliases, anchors } => {
            out.push_str("breached:\n  AliasAnchorRatio:\n");
            let _ = writeln!(out, "    aliases: {aliases}");
            let _ = writeln!(out, "    anchors: {anchors}");
        }
        BudgetBreach::SequenceUnbalanced => {
            out.push_str("breached: SequenceUnbalanced\n");
        }
        BudgetBreach::InputBytes { input_bytes } => {
            out.push_str("breached:\n  InputBytes:\n");
            let _ = writeln!(out, "    input_bytes: {input_bytes}");
        }
        BudgetBreach::PropertyExpansionDepth { depth, max_depth } => {
            out.push_str("breached:\n  PropertyExpansionDepth:\n");
            let _ = writeln!(out, "    depth: {depth}");
            let _ = writeln!(out, "    max_depth: {max_depth}");
        }
        BudgetBreach::PropertyInterpolationWork { work, max_work } => {
            out.push_str("breached:\n  PropertyInterpolationWork:\n");
            let _ = writeln!(out, "    work: {work}");
            let _ = writeln!(out, "    max_work: {max_work}");
        }
    }
}

/// Check the input's filesystem-reported size against the reader byte limit.
fn check_input_file_size(path: &str, budget: &Budget) -> std::io::Result<()> {
    let Some(limit) = budget.max_reader_input_bytes else {
        return Ok(());
    };
    let file_size = std::fs::metadata(path)?.len();
    let limit_u64 = u64::try_from(limit).unwrap_or(u64::MAX);
    if file_size > limit_u64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::FileTooLarge,
            format!("input size limit of {limit} bytes exceeded"),
        ));
    }
    Ok(())
}

/// Run the serde-saphyr CLI with explicit arguments and output streams.
pub fn run<I, S, Stdout, Stderr>(args: I, stdout: &mut Stdout, stderr: &mut Stderr) -> i32
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    Stdout: std::io::Write,
    Stderr: std::io::Write,
{
    let mut plain = false;
    let mut path: Option<String> = None;
    let mut include_path: Option<String> = None;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let arg = arg.as_ref();
        match arg {
            "--plain" => plain = true,
            "--include" => {
                include_path = match args.next() {
                    Some(path) if path.as_ref().starts_with('-') => {
                        let _ = writeln!(stderr, "Missing path for --include\n\n{}", usage());
                        return 1;
                    }
                    Some(path) => Some(path.as_ref().to_owned()),
                    None => {
                        let _ = writeln!(stderr, "Missing path for --include\n\n{}", usage());
                        return 1;
                    }
                };
            }
            "--help" | "-h" => {
                let _ = writeln!(stdout, "{}", usage());
                return 0;
            }
            _ if arg.starts_with('-') => {
                let _ = writeln!(stderr, "Unknown option: {arg}\n\n{}", usage());
                return 1;
            }
            _ => {
                if path.is_some() {
                    let _ = writeln!(stderr, "Unexpected extra argument: {arg}\n\n{}", usage());
                    return 1;
                }
                path = Some(arg.to_owned());
            }
        }
    }

    let Some(path) = path else {
        let _ = writeln!(stderr, "{}", usage());
        return 1;
    };
    let safe_path =
        crate::de_error::sanitize_message_text(std::borrow::Cow::Borrowed(path.as_str()));

    let mut options = if plain {
        crate::options! {
            // Plain mode uses serde-saphyr's own snippet rendering.
            with_snippet: true,
        }
    } else {
        crate::options! {
            // When using miette, use miette's snippet rendering instead of serde-saphyr's.
            // Otherwise, keep serde-saphyr snippets enabled.
            with_snippet: cfg!(feature = "miette") == false,
        }
    };

    if let Some(budget) = options.budget.as_ref()
        && let Err(err) = check_input_file_size(&path, budget)
    {
        let _ = writeln!(stderr, "Failed to read {safe_path}: {err}");
        return 2;
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) => {
            let _ = writeln!(stderr, "Failed to read {safe_path}: {err}");
            return 2;
        }
    };

    let buffered_output = Rc::new(RefCell::new(Vec::<String>::new()));
    let budget_output = Rc::clone(&buffered_output);

    options = options.with_budget_report(move |report| {
        let formatted = format_budget_report(&report);
        budget_output
            .borrow_mut()
            .push(format!("Budget report:\n{formatted}"));
    });

    if let Some(path) = include_path {
        let safe_include_path =
            crate::de_error::sanitize_message_text(std::borrow::Cow::Borrowed(path.as_str()));
        options = match options.with_filesystem_root(&path) {
            Ok(options) => options,
            Err(err) => {
                let _ = writeln!(
                    stderr,
                    "Failed to configure include root {safe_include_path}: {err}"
                );
                return 2;
            }
        };
    }

    let result: Result<IgnoredAny, Error> = from_str_with_options(&content, options);

    for message in std::mem::take(&mut *buffered_output.borrow_mut()) {
        let _ = writeln!(stdout, "{message}");
    }

    if let Err(err) = result {
        if plain {
            let _ = writeln!(stderr, "{safe_path} invalid:\n{err}");
            return 3;
        }

        #[cfg(feature = "miette")]
        {
            let report = crate::miette::to_miette_report(&err, &content, safe_path.as_ref());
            // `Debug` formatting uses miette's graphical reporter.
            let _ = writeln!(stderr, "{report:?}");
            return 3;
        }

        #[cfg(not(feature = "miette"))]
        {
            let _ = writeln!(stderr, "{safe_path} invalid:\n{err}");
            return 3;
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn report_with_breach(breached: BudgetBreach) -> BudgetReport {
        BudgetReport {
            breached: Some(breached),
            events: 1,
            aliases: 2,
            anchors: 3,
            recorded_anchor_events: 4,
            recorded_anchor_bytes: 5,
            documents: 6,
            nodes: 7,
            max_depth: 8,
            total_scalar_bytes: 9,
            total_comment_bytes: 10,
            merge_keys: 11,
        }
    }

    #[test]
    fn format_budget_report_without_breach() {
        let formatted = format_budget_report(&BudgetReport {
            breached: None,
            events: 10,
            aliases: 0,
            anchors: 1,
            recorded_anchor_events: 2,
            recorded_anchor_bytes: 3,
            documents: 4,
            nodes: 5,
            max_depth: 6,
            total_scalar_bytes: 7,
            total_comment_bytes: 8,
            merge_keys: 9,
        });

        assert!(formatted.contains("breached: null"));
        assert!(formatted.contains("events: 10"));
        assert!(formatted.contains("recorded_anchor_events: 2"));
        assert!(formatted.contains("recorded_anchor_bytes: 3"));
        assert!(formatted.contains("total_comment_bytes: 8"));
    }

    #[test]
    fn format_budget_report_covers_all_breach_variants() {
        let cases = [
            (
                report_with_breach(BudgetBreach::Events { events: 11 }),
                "  Events:",
                "    events: 11",
            ),
            (
                report_with_breach(BudgetBreach::Aliases { aliases: 12 }),
                "  Aliases:",
                "    aliases: 12",
            ),
            (
                report_with_breach(BudgetBreach::Anchors { anchors: 13 }),
                "  Anchors:",
                "    anchors: 13",
            ),
            (
                report_with_breach(BudgetBreach::RecordedAnchorEvents {
                    recorded_anchor_events: 14,
                }),
                "  RecordedAnchorEvents:",
                "    recorded_anchor_events: 14",
            ),
            (
                report_with_breach(BudgetBreach::RecordedAnchorBytes {
                    recorded_anchor_bytes: 15,
                }),
                "  RecordedAnchorBytes:",
                "    recorded_anchor_bytes: 15",
            ),
            (
                report_with_breach(BudgetBreach::Depth { depth: 14 }),
                "  Depth:",
                "    depth: 14",
            ),
            (
                report_with_breach(BudgetBreach::InclusionDepth { depth: 15 }),
                "  InclusionDepth:",
                "    depth: 15",
            ),
            (
                report_with_breach(BudgetBreach::Documents { documents: 16 }),
                "  Documents:",
                "    documents: 16",
            ),
            (
                report_with_breach(BudgetBreach::Nodes { nodes: 17 }),
                "  Nodes:",
                "    nodes: 17",
            ),
            (
                report_with_breach(BudgetBreach::ScalarBytes {
                    total_scalar_bytes: 18,
                }),
                "  ScalarBytes:",
                "    total_scalar_bytes: 18",
            ),
            (
                report_with_breach(BudgetBreach::CommentBytes {
                    total_comment_bytes: 19,
                }),
                "  CommentBytes:",
                "    total_comment_bytes: 19",
            ),
            (
                report_with_breach(BudgetBreach::MergeKeys { merge_keys: 20 }),
                "  MergeKeys:",
                "    merge_keys: 20",
            ),
            (
                report_with_breach(BudgetBreach::AliasAnchorRatio {
                    aliases: 21,
                    anchors: 22,
                }),
                "  AliasAnchorRatio:",
                "    anchors: 22",
            ),
            (
                report_with_breach(BudgetBreach::SequenceUnbalanced),
                "breached: SequenceUnbalanced",
                "nodes: 7",
            ),
            (
                report_with_breach(BudgetBreach::InputBytes { input_bytes: 23 }),
                "  InputBytes:",
                "    input_bytes: 23",
            ),
            (
                report_with_breach(BudgetBreach::PropertyExpansionDepth {
                    depth: 25,
                    max_depth: 24,
                }),
                "  PropertyExpansionDepth:",
                "    max_depth: 24",
            ),
            (
                report_with_breach(BudgetBreach::PropertyInterpolationWork {
                    work: 27,
                    max_work: 26,
                }),
                "  PropertyInterpolationWork:",
                "    max_work: 26",
            ),
        ];

        for (report, expected_type, expected_value) in cases {
            let formatted = format_budget_report(&report);
            assert!(formatted.contains(expected_type), "{formatted}");
            assert!(formatted.contains(expected_value), "{formatted}");
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn input_file_size_check_uses_reader_budget_limit() {
        let mut file = tempfile::NamedTempFile::new().expect("create temp file");
        write!(file, "1234").expect("write temp file");
        let mut budget = Budget {
            max_reader_input_bytes: Some(4),
            ..Budget::default()
        };
        check_input_file_size(file.path().to_str().unwrap(), &budget)
            .expect("a file at the limit should be accepted");

        budget.max_reader_input_bytes = Some(3);
        let error = check_input_file_size(file.path().to_str().unwrap(), &budget)
            .expect_err("a file above the limit should be rejected");
        assert_eq!(error.kind(), std::io::ErrorKind::FileTooLarge);

        budget.max_reader_input_bytes = None;
        check_input_file_size(file.path().to_str().unwrap(), &budget)
            .expect("a disabled reader limit should accept the file");
    }

    #[cfg(feature = "serde_derived_types")]
    #[test]
    fn format_budget_report_matches_serde_output() {
        let reports = [
            BudgetReport {
                breached: None,
                events: 10,
                aliases: 0,
                anchors: 1,
                recorded_anchor_events: 2,
                recorded_anchor_bytes: 3,
                documents: 4,
                nodes: 5,
                max_depth: 6,
                total_scalar_bytes: 7,
                total_comment_bytes: 8,
                merge_keys: 9,
            },
            report_with_breach(BudgetBreach::Events { events: 11 }),
            report_with_breach(BudgetBreach::AliasAnchorRatio {
                aliases: 21,
                anchors: 22,
            }),
            report_with_breach(BudgetBreach::SequenceUnbalanced),
            report_with_breach(BudgetBreach::InputBytes { input_bytes: 23 }),
            report_with_breach(BudgetBreach::PropertyExpansionDepth {
                depth: 25,
                max_depth: 24,
            }),
            report_with_breach(BudgetBreach::PropertyInterpolationWork {
                work: 27,
                max_work: 26,
            }),
        ];

        for report in reports {
            assert_eq!(
                format_budget_report(&report),
                crate::to_string(&report).unwrap()
            );
        }
    }
}
