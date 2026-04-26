use std::cell::RefCell;
use std::rc::Rc;

use serde::de::IgnoredAny;

use crate::{Error, from_str_with_options};

enum BufferedOutput {
    Stdout(String),
    Stderr(String),
}

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

    let buffered_output = Rc::new(RefCell::new(Vec::<BufferedOutput>::new()));
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
    .with_budget_report(move |report| match crate::to_string(&report) {
        Ok(serialized) => budget_output
            .borrow_mut()
            .push(BufferedOutput::Stdout(format!(
                "Budget report:\n{serialized}"
            ))),
        Err(err) => budget_output
            .borrow_mut()
            .push(BufferedOutput::Stderr(format!(
                "Failed to serialize budget report: {err}"
            ))),
    });

    if let Some(path) = include_path {
        options = options
            .with_filesystem_root(&path)
            .expect("failed to create filesystem include resolver");
    }

    let result: Result<IgnoredAny, Error> = from_str_with_options(&content, options);

    for message in std::mem::take(&mut *buffered_output.borrow_mut()) {
        match message {
            BufferedOutput::Stdout(message) => {
                let _ = writeln!(stdout, "{message}");
            }
            BufferedOutput::Stderr(message) => {
                let _ = writeln!(stderr, "{message}");
            }
        }
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
