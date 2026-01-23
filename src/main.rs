#![forbid(unsafe_code)]

use std::process::exit;

use serde::de::IgnoredAny;
use serde_saphyr::{Error, Options, from_str_with_options};

fn usage() -> &'static str {
    "Usage: serde-saphyr [--plain] <path>\n\
\n\
Reads the YAML file at <path> and prints a budget summary.\n\
It can also be used as a YAML validator.\n\
\n\
Options:\n\
  --plain   Disable miette formatting and print errors in plain text"
}

/// Read YAML file and print budget summary. This tool allows to check approximate
/// typical budget requirements for your YAML files. Single parameter is the file name.
fn main() {
    let mut plain = false;
    let mut path: Option<String> = None;

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--plain" => plain = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ if arg.starts_with('-') => {
                eprintln!("Unknown option: {arg}\n\n{}", usage());
                exit(1);
            }
            _ => {
                if path.is_some() {
                    eprintln!("Unexpected extra argument: {arg}\n\n{}", usage());
                    exit(1);
                }
                path = Some(arg);
            }
        }
    }

    let path = match path {
        Some(path) => path,
        None => {
            eprintln!("{}", usage());
            exit(1);
        }
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Failed to read {path}: {err}");
            exit(2);
        }
    };

    let options = if plain {
        Options {
            // Plain mode uses serde-saphyr's own snippet rendering.
            with_snippet: true,
            ..Options::default()
        }
    } else {
        Options {
            // When using miette, use miette's snippet rendering instead of serde-saphyr's.
            // Otherwise, keep serde-saphyr snippets enabled.
            with_snippet: cfg!(feature = "miette") == false,
            ..Options::default()
        }
    }.with_budget_report(|report|
        {
            match serde_saphyr::to_string(report) {
                Ok(serialized) => println!("Budget report:\n{serialized}"),
                Err(err) => eprintln!("Failed to serialize budget report: {err}"),
            }
        }
    );

    let r: Result<IgnoredAny, Error> = from_str_with_options(&content, options);

    if let Err(err) = r {
        if plain {
            eprintln!("{path} invalid:\n{err}");
            exit(3);
        }

        #[cfg(feature = "miette")]
        {
            let report = serde_saphyr::miette::to_miette_report(&err, &content, &path);
            // `Debug` formatting uses miette's graphical reporter.
            eprintln!("{report:?}");
            exit(3);
        }

        #[cfg(not(feature = "miette"))]
        {
            eprintln!("{path} invalid:\n{err}");
            exit(3);
        }
    }
}
