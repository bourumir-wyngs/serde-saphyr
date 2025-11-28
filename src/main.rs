use serde_json::Value;
use serde_saphyr::{Options, from_str_with_options};

fn report_budget(report: &serde_saphyr::budget::BudgetReport) {
    match serde_saphyr::to_string(report) {
        Ok(serialized) => println!("Budget report:\n{serialized}"),
        Err(err) => eprintln!("Failed to serialize budget report: {err}"),
    }
}

/// Read YAML file and print budget summary. This tool allows to check approximate
/// typical budget requirements for your YAML files. Single parameter is the file name.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| "expected a path to a YAML file as the first argument")?;

    let content = std::fs::read_to_string(&path)?;

    let options = Options {
        budget_report: Some(report_budget),
        ..Options::default()
    };

    let _: Value = from_str_with_options(&content, options)?;

    Ok(())
}
