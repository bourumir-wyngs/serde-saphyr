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

#[test]
fn sibling_newtypes_do_not_add_up() {
    let yaml = format!("[{}]", vec!["1"; 100].join(", "));
    let values: Vec<Inner> = serde_saphyr::from_str_with_options(&yaml, with_max_depth(2)).unwrap();
    assert_eq!(values.len(), 100);
}
