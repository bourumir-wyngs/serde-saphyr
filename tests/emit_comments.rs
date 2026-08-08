#![cfg(feature = "deserialize")]

use serde_saphyr::budget::{BudgetBreach, BudgetReport};
use serde_saphyr::granit_parser::ErrorKind;
use serde_saphyr::{Error, ExternalMessageSource, Options, from_str_with_options};
use std::cell::RefCell;
use std::rc::Rc;

fn parse_string_with_report(yaml: &str, options: Options) -> (String, BudgetReport) {
    let reports = Rc::new(RefCell::new(Vec::new()));
    let callback_reports = Rc::clone(&reports);
    let options =
        options.with_budget_report(move |report| callback_reports.borrow_mut().push(report));

    let value = from_str_with_options(yaml, options).expect("YAML should parse within budget");
    let report = reports
        .borrow_mut()
        .pop()
        .expect("a successful budgeted parse should deliver a report");
    (value, report)
}

#[test]
fn disabled_comment_emission_uses_retained_data_budget_accounting() {
    const WITHOUT_COMMENTS: &str = "value\n";
    const WITH_COMMENTS: &str = "# leading one\n# leading two\nvalue # trailing\n";

    let (_, baseline) = parse_string_with_report(WITHOUT_COMMENTS, serde_saphyr::options! {});

    let retained_comments = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_total_comment_bytes: 0,
        },
    };
    let error = from_str_with_options::<String>(WITH_COMMENTS, retained_comments)
        .expect_err("the fixture should exceed a zero-byte retained-comment budget");
    assert!(matches!(
        error.without_snippet(),
        Error::Budget {
            breach: BudgetBreach::CommentBytes {
                total_comment_bytes,
            },
            ..
        } if *total_comment_bytes > 0
    ));

    let options = serde_saphyr::options! {
        emit_comments: false,
        budget: serde_saphyr::budget! {
            max_events: baseline.events,
            max_total_comment_bytes: 0,
        },
    };
    let (value, report) = parse_string_with_report(WITH_COMMENTS, options);

    assert_eq!(value, "value");
    assert_eq!(report.events, baseline.events);
    assert_eq!(report.total_comment_bytes, 0);
    assert!(report.breached.is_none());
}

#[test]
fn disabled_comment_emission_makes_buffered_comment_limit_ineffective() {
    const YAML: &str = "key: # buffered\nnext: value\n";

    let retained_comments = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_buffered_comment_events: 0,
        },
    };
    let error = from_str_with_options::<serde_json::Value>(YAML, retained_comments)
        .expect_err("the fixture should require one buffered comment");
    assert!(matches!(
        error.without_snippet(),
        Error::ExternalMessage { source, .. }
            if matches!(source.as_ref(), ExternalMessageSource::Parser(error)
                if matches!(error.kind(), ErrorKind::TooManyComments))
    ));

    let suppressed_comments = serde_saphyr::options! {
        emit_comments: false,
        budget: serde_saphyr::budget! {
            max_buffered_comment_events: 0,
        },
    };
    let value = from_str_with_options::<serde_json::Value>(YAML, suppressed_comments)
        .expect("a buffered-comment limit should have no effect when comments are suppressed");

    assert_eq!(value, serde_json::json!({ "key": null, "next": "value" }));
}
