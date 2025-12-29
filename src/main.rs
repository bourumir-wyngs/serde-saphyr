#![forbid(unsafe_code)]

use std::process::exit;
use serde_json::Value;
use serde_saphyr::{Options, from_str_with_options, Error};

fn report_budget(report: &serde_saphyr::budget::BudgetReport) {
    match serde_saphyr::to_string(report) {
        Ok(serialized) => println!("Budget report:\n{serialized}"),
        Err(err) => eprintln!("Failed to serialize budget report: {err}"),
    }
}

/// Read YAML file and print budget summary. This tool allows to check approximate
/// typical budget requirements for your YAML files. Single parameter is the file name.
fn main() {
    let path = match std::env::args()
        .nth(1)
        .ok_or("This program calculates budget to parse the given YAML file, \
        can also be used as YAML validator. Expected a path to a YAML file as the first argument") {
        Ok(path) => path,
        Err(err) => {
            eprintln!("{err}");
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

    let options = Options {
        budget_report: Some(report_budget),
        ..Options::default()
    };

    let r: Result<Value, Error> = from_str_with_options(&content, options);

    if let Err(err) = r {
        eprintln!("{path} invalid:\n{err}");
        exit(3);
    }
}
