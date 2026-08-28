#![cfg(feature = "deserialize")]

use serde_saphyr::budget::{BudgetBreach, BudgetReport};
use serde_saphyr::{Error, from_reader_with_options, from_str_with_options};
use std::cell::RefCell;
use std::io::Cursor;
use std::rc::Rc;

const PAYLOAD: &str = "payload";
const NESTED_ANCHORS: &str = "\
value: &outer [&middle [&inner [payload]]]
alias: *outer
";
const NESTED_ANCHORED_MAPPINGS: &str = "\
value: &outer
  middle: &middle
    inner: &inner
      payload: text
alias: *outer
";

fn unwrap_snippet(error: &Error) -> &Error {
    match error {
        Error::WithSnippet { error, .. } => unwrap_snippet(error),
        other => other,
    }
}

#[test]
fn borrowed_nested_anchors_obey_recorded_event_budget() {
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_recorded_anchor_events: 14,
            max_recorded_anchor_bytes: usize::MAX,
        },
    };

    let error = from_str_with_options::<serde_json::Value>(NESTED_ANCHORS, options)
        .expect_err("the fifteenth retained copy must breach the event budget");

    assert!(matches!(
        unwrap_snippet(&error),
        Error::Budget {
            breach: BudgetBreach::RecordedAnchorEvents {
                recorded_anchor_events: 15,
            },
            ..
        }
    ));
}

#[test]
fn borrowed_anchor_payloads_do_not_consume_owned_byte_budget() {
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_recorded_anchor_events: 15,
            max_recorded_anchor_bytes: 0,
        },
    };

    let value = from_str_with_options::<serde_json::Value>(NESTED_ANCHORS, options)
        .expect("cloning borrowed payload references must not consume the owned-byte budget");

    assert_eq!(value["value"], value["alias"]);
}

#[test]
fn borrowed_nested_mappings_use_the_same_recorded_event_budget() {
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_recorded_anchor_events: 20,
            max_recorded_anchor_bytes: usize::MAX,
        },
    };

    let error = from_str_with_options::<serde_json::Value>(NESTED_ANCHORED_MAPPINGS, options)
        .expect_err("the twenty-first retained mapping event must breach the budget");

    assert!(matches!(
        unwrap_snippet(&error),
        Error::Budget {
            breach: BudgetBreach::RecordedAnchorEvents {
                recorded_anchor_events: 21,
            },
            ..
        }
    ));
}

#[test]
fn reader_owned_scalars_obey_recorded_byte_budget() {
    let copied_payload_bytes = PAYLOAD.len() * 3;
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_recorded_anchor_events: usize::MAX,
            max_recorded_anchor_bytes: copied_payload_bytes - 1,
        },
    };

    let reader = Cursor::new(NESTED_ANCHORS.as_bytes());
    let error = from_reader_with_options::<_, serde_json::Value>(reader, options)
        .expect_err("the third owned payload clone must breach the byte budget");

    assert!(matches!(
        unwrap_snippet(&error),
        Error::Budget {
            breach: BudgetBreach::RecordedAnchorBytes {
                recorded_anchor_bytes,
            },
            ..
        } if *recorded_anchor_bytes == copied_payload_bytes
    ));
}

#[test]
fn exact_reader_anchor_usage_is_reported_and_aliases_still_replay() {
    let copied_payload_bytes = PAYLOAD.len() * 3;
    let reports = Rc::new(RefCell::new(Vec::<BudgetReport>::new()));
    let callback_reports = Rc::clone(&reports);
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_recorded_anchor_events: 15,
            max_recorded_anchor_bytes: copied_payload_bytes,
        },
    }
    .with_budget_report(move |report| callback_reports.borrow_mut().push(report));

    let reader = Cursor::new(NESTED_ANCHORS.as_bytes());
    let value = from_reader_with_options::<_, serde_json::Value>(reader, options)
        .expect("usage equal to both limits must remain valid");

    assert_eq!(value["value"], value["alias"]);
    let reports = reports.borrow();
    assert_eq!(reports.len(), 1);
    assert!(reports[0].breached.is_none());
    assert_eq!(reports[0].recorded_anchor_events, 15);
    assert_eq!(reports[0].recorded_anchor_bytes, copied_payload_bytes);
}
