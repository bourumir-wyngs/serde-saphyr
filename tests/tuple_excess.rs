#![cfg(feature = "deserialize")]

use std::fmt;

use serde::Deserialize;
use serde::de::{SeqAccess, Visitor};

#[test]
fn top_level_tuple_excess_reports_invalid_length() {
    let err = serde_saphyr::from_str::<(i32, i32)>("[1, 2, 3]").unwrap_err();

    assert_invalid_length(&err, "invalid length 3");
}

#[test]
fn nested_tuple_excess_reports_invalid_length() {
    #[derive(Debug, Deserialize)]
    struct Doc {
        #[allow(dead_code)]
        pair: (i32, i32),
        #[allow(dead_code)]
        tail: i32,
    }

    let err = serde_saphyr::from_str::<Doc>("pair: [1, 2, 3]\ntail: 4\n").unwrap_err();

    assert_invalid_length(&err, "invalid length 3");
}

#[derive(Debug, PartialEq)]
struct FirstOnly(i32);

impl<'de> Deserialize<'de> for FirstOnly {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(FirstOnlyVisitor)
    }
}

struct FirstOnlyVisitor;

impl<'de> Visitor<'de> for FirstOnlyVisitor {
    type Value = FirstOnly;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a sequence whose first value is used")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let first = seq.next_element()?.unwrap_or_default();
        Ok(FirstOnly(first))
    }
}

#[test]
fn early_returning_sequence_visitor_does_not_desync_parent_map() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Doc {
        seq: FirstOnly,
        tail: i32,
    }

    let doc: Doc = serde_saphyr::from_str("seq: [1, 2, 3]\ntail: 4\n").unwrap();

    assert_eq!(
        doc,
        Doc {
            seq: FirstOnly(1),
            tail: 4,
        }
    );
}

fn assert_invalid_length(err: &serde_saphyr::Error, expected: &str) {
    let err = err.without_snippet();
    match err {
        serde_saphyr::Error::Message { msg, .. } => {
            assert!(
                msg.contains(expected),
                "expected `{expected}` in invalid length error, got `{msg}`"
            );
        }
        other => panic!("expected invalid length message, got {other:?}"),
    }
}
