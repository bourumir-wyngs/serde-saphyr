#![cfg(feature = "deserialize")]

use serde::Deserialize;
use serde_saphyr::Error;
use serde_saphyr::budget::BudgetBreach;

/// A newtype that holds itself: each level is a newtype struct, which reads no YAML event.
#[derive(Debug, Deserialize)]
struct Chain(#[allow(dead_code)] Option<Box<Chain>>);

#[derive(Debug, Deserialize, PartialEq)]
struct Inner(u32);

#[derive(Debug, Deserialize, PartialEq)]
struct Outer(Inner);

fn with_max_depth(max_depth: usize) -> serde_saphyr::Options {
    serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_depth: max_depth,
        },
    }
}

fn depth_breach(err: &Error) -> Option<usize> {
    match err.without_snippet() {
        Error::Budget {
            breach: BudgetBreach::Depth { depth },
            ..
        } => Some(*depth),
        _ => None,
    }
}

#[test]
fn a_self_nesting_newtype_stops_at_max_depth() {
    // Without a depth charge this re-enters the same `[]` node until the stack overflows.
    let err = serde_saphyr::from_str::<Chain>("[]").unwrap_err();
    assert!(
        depth_breach(&err).is_some(),
        "want a depth budget error, got {err:?}"
    );

    let err =
        serde_saphyr::from_str_with_options::<Chain>("&a [*a]", with_max_depth(16)).unwrap_err();
    assert_eq!(depth_breach(&err), Some(17), "got {err:?}");
}

#[test]
fn nested_newtypes_within_max_depth_still_parse() {
    let value: Outer = serde_saphyr::from_str_with_options("7", with_max_depth(2)).unwrap();
    assert_eq!(value, Outer(Inner(7)));

    let err = serde_saphyr::from_str_with_options::<Outer>("7", with_max_depth(1)).unwrap_err();
    assert_eq!(depth_breach(&err), Some(2), "got {err:?}");
}

/// Deserialize `yaml` as `T` with a budget-report callback; return the error's depth
/// and the breach the report recorded.
fn depth_breach_and_report<T: serde::de::DeserializeOwned>(
    yaml: &str,
    max_depth: usize,
) -> (Option<usize>, Option<BudgetBreach>) {
    let observed = std::rc::Rc::new(std::cell::RefCell::new(None));
    let observed_by_callback = std::rc::Rc::clone(&observed);
    let options = with_max_depth(max_depth).with_budget_report(move |report| {
        *observed_by_callback.borrow_mut() = report.breached;
    });

    let err = match serde_saphyr::from_str_with_options::<T>(yaml, options) {
        Ok(_) => panic!("expected a depth budget error"),
        Err(err) => err,
    };
    let reported = observed.borrow_mut().take();
    (depth_breach(&err), reported)
}

/// The newtype level that the parser never counts.
#[derive(Debug, Deserialize)]
struct W(#[allow(dead_code)] Vec<u8>);

#[derive(Debug, Deserialize)]
struct W2(#[allow(dead_code)] W);

#[derive(Debug, Deserialize)]
struct Item {
    #[allow(dead_code)]
    v: W2,
}

#[derive(Debug, Deserialize)]
struct Doc {
    #[allow(dead_code)]
    item: Item,
}

#[test]
fn newtype_depth_breach_is_in_budget_report() {
    let (depth, reported) = depth_breach_and_report::<Outer>("7", 1);
    assert_eq!(depth, Some(2));
    assert!(
        matches!(reported, Some(BudgetBreach::Depth { depth: 2 })),
        "got {reported:?}"
    );
}

#[test]
fn newtype_carried_sequence_depth_breach_is_in_budget_report() {
    // The parser sees depth 1; the sequence inside `W` is level 2 for the deserializer.
    let (depth, reported) = depth_breach_and_report::<W>("[1]", 1);
    assert_eq!(depth, Some(2));
    assert!(
        matches!(reported, Some(BudgetBreach::Depth { depth: 2 })),
        "got {reported:?}"
    );
}

#[test]
fn merged_newtype_depth_breach_is_in_budget_report() {
    // The parser stays within 4 levels; the merged `v` adds two newtype levels.
    let yaml = "base: &b {v: [1]}\nitem:\n  <<: *b\n";
    let (depth, reported) = depth_breach_and_report::<Doc>(yaml, 4);
    assert!(depth.is_some(), "want a depth budget error");
    assert!(
        matches!(reported, Some(BudgetBreach::Depth { .. })),
        "got {reported:?}"
    );
}

#[test]
fn streaming_reader_stops_after_a_depth_breach() {
    // A depth breach is recorded in the budget, so the reader ends instead of
    // recovering at the next document, as it does for parser-level budget breaches.
    let mut reader = std::io::Cursor::new(b"7\n---\n8\n".to_vec());
    let mut iter = serde_saphyr::read_with_options::<_, Outer>(&mut reader, with_max_depth(1));

    let first = iter.next().expect("expected first result");
    let err = first.expect_err("first document should fail");
    assert_eq!(depth_breach(&err), Some(2), "got {err:?}");
    assert!(
        iter.next().is_none(),
        "iterator should stop after the depth breach"
    );
}

#[test]
fn sibling_newtypes_do_not_add_up() {
    let yaml = format!("[{}]", vec!["1"; 100].join(", "));
    let values: Vec<Inner> = serde_saphyr::from_str_with_options(&yaml, with_max_depth(2)).unwrap();
    assert_eq!(values.len(), 100);
}
