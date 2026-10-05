#![cfg(all(feature = "deserialize", not(feature = "comments")))]

use serde_saphyr::budget::BudgetReport;
use serde_saphyr::{
    Options, from_reader_with_options, from_slice_with_options, from_str_with_options,
};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn comments_are_ignored_for_string_slice_and_reader_inputs() {
    const YAML: &str = "# document\nfirst: &items # anchor\n  - 1 # one\n  # separator\n  - 2\nsecond: *items # alias\nflow: [3, # flow item\n  4]\n";
    let expected = serde_json::json!({
        "first": [1, 2],
        "second": [1, 2],
        "flow": [3, 4],
    });

    let from_string: serde_json::Value = from_str_with_options(YAML, Options::default()).unwrap();
    let from_slice: serde_json::Value =
        from_slice_with_options(YAML.as_bytes(), Options::default()).unwrap();
    let from_reader: serde_json::Value =
        from_reader_with_options(YAML.as_bytes(), Options::default()).unwrap();

    assert_eq!(from_string, expected);
    assert_eq!(from_slice, expected);
    assert_eq!(from_reader, expected);
}

fn parse_with_report(yaml: &str, options: Options) -> (String, BudgetReport) {
    let report = Rc::new(RefCell::new(None));
    let captured_report = Rc::clone(&report);
    let options =
        options.with_budget_report(move |value| *captured_report.borrow_mut() = Some(value));
    let value = from_str_with_options(yaml, options).expect("YAML should parse within budget");
    let report = report
        .borrow_mut()
        .take()
        .expect("budget report should be delivered");
    (value, report)
}

#[test]
fn ignored_comments_do_not_consume_the_event_budget() {
    let (_, baseline) = parse_with_report("value\n", Options::default());
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! { max_events: baseline.events },
    };
    let (value, report) = parse_with_report("# first\n# second\nvalue # trailing\n", options);

    assert_eq!(value, "value");
    assert_eq!(report.events, baseline.events);
    assert!(report.breached.is_none());
}
