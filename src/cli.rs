use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;

use serde_core::de::IgnoredAny;

use crate::de::budget::{BudgetBreach, BudgetReport};
use crate::{Error, from_str_with_options};

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
        Some(breach) => {
            out.push_str("breached:\n");
            format_budget_breach(&mut out, breach);
        }
        None => out.push_str("breached: null\n"),
    }

    let _ = writeln!(out, "events: {}", report.events);
    let _ = writeln!(out, "aliases: {}", report.aliases);
    let _ = writeln!(out, "anchors: {}", report.anchors);
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
            out.push_str("  type: events\n");
            let _ = writeln!(out, "  events: {events}");
        }
        BudgetBreach::Aliases { aliases } => {
            out.push_str("  type: aliases\n");
            let _ = writeln!(out, "  aliases: {aliases}");
        }
        BudgetBreach::Anchors { anchors } => {
            out.push_str("  type: anchors\n");
            let _ = writeln!(out, "  anchors: {anchors}");
        }
        BudgetBreach::Depth { depth } => {
            out.push_str("  type: depth\n");
            let _ = writeln!(out, "  depth: {depth}");
        }
        BudgetBreach::InclusionDepth { depth } => {
            out.push_str("  type: inclusion_depth\n");
            let _ = writeln!(out, "  depth: {depth}");
        }
        BudgetBreach::Documents { documents } => {
            out.push_str("  type: documents\n");
            let _ = writeln!(out, "  documents: {documents}");
        }
        BudgetBreach::Nodes { nodes } => {
            out.push_str("  type: nodes\n");
            let _ = writeln!(out, "  nodes: {nodes}");
        }
        BudgetBreach::ScalarBytes { total_scalar_bytes } => {
            out.push_str("  type: scalar_bytes\n");
            let _ = writeln!(out, "  total_scalar_bytes: {total_scalar_bytes}");
        }
        BudgetBreach::CommentBytes {
            total_comment_bytes,
        } => {
            out.push_str("  type: comment_bytes\n");
            let _ = writeln!(out, "  total_comment_bytes: {total_comment_bytes}");
        }
        BudgetBreach::MergeKeys { merge_keys } => {
            out.push_str("  type: merge_keys\n");
            let _ = writeln!(out, "  merge_keys: {merge_keys}");
        }
        BudgetBreach::AliasAnchorRatio { aliases, anchors } => {
            out.push_str("  type: alias_anchor_ratio\n");
            let _ = writeln!(out, "  aliases: {aliases}");
            let _ = writeln!(out, "  anchors: {anchors}");
        }
        BudgetBreach::SequenceUnbalanced => {
            out.push_str("  type: sequence_unbalanced\n");
        }
        BudgetBreach::InputBytes { input_bytes } => {
            out.push_str("  type: input_bytes\n");
            let _ = writeln!(out, "  input_bytes: {input_bytes}");
        }
    }
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

    let path = match path {
        Some(path) => path,
        None => {
            let _ = writeln!(stderr, "{}", usage());
            return 1;
        }
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) => {
            let _ = writeln!(stderr, "Failed to read {path}: {err}");
            return 2;
        }
    };

    let buffered_output = Rc::new(RefCell::new(Vec::<String>::new()));
    let budget_output = Rc::clone(&buffered_output);

    #[allow(deprecated)]
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
    }
    .with_budget_report(move |report| {
        let formatted = format_budget_report(&report);
        budget_output
            .borrow_mut()
            .push(format!("Budget report:\n{formatted}"));
    });

    if let Some(path) = include_path {
        options = match options.with_filesystem_root(&path) {
            Ok(options) => options,
            Err(err) => {
                let _ = writeln!(stderr, "Failed to configure include root {path}: {err}");
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
            let _ = writeln!(stderr, "{path} invalid:\n{err}");
            return 3;
        }

        #[cfg(feature = "miette")]
        {
            let report = crate::miette::to_miette_report(&err, &content, &path);
            // `Debug` formatting uses miette's graphical reporter.
            let _ = writeln!(stderr, "{report:?}");
            return 3;
        }

        #[cfg(not(feature = "miette"))]
        {
            let _ = writeln!(stderr, "{path} invalid:\n{err}");
            return 3;
        }
    }

    0
}
