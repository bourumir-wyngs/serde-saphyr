#![cfg(feature = "deserialize")]

use std::fmt;

use serde::Deserialize;
use serde::de::{SeqAccess, Visitor};

#[test]
fn top_level_tuple_excess_reports_invalid_length() {
    let err = serde_saphyr::from_str::<(i32, i32)>("[1, 2, 3]").unwrap_err();

    assert_invalid_length(&err, 3);
}

#[test]
fn short_tuples_report_structured_lengths() {
    for (yaml, len) in [("[]", 0), ("[1]", 1)] {
        for err in [
            serde_saphyr::from_str::<(i32, i32)>(yaml).unwrap_err(),
            serde_saphyr::from_reader::<_, (i32, i32)>(yaml.as_bytes()).unwrap_err(),
        ] {
            assert_invalid_length(&err, len);
        }
    }
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

    let yaml = "pair: [1, 2, 3]\ntail: 4\n";
    for err in [
        serde_saphyr::from_str::<Doc>(yaml).unwrap_err(),
        serde_saphyr::from_reader::<_, Doc>(yaml.as_bytes()).unwrap_err(),
    ] {
        assert_invalid_length(&err, 3);
        let location = err.location().expect("tuple field location");
        assert_eq!((location.line(), location.column()), (1, 1));
        let locations = err.locations().expect("tuple field locations");
        assert_eq!(locations.reference_location, location);
        assert_eq!(locations.defined_location, location);
        assert_eq!(
            err.without_snippet().to_string(),
            "invalid length 3, expected a tuple of size 2 at line 1, column 1"
        );
        assert!(err.to_string().contains(" --> "), "{err}");
    }
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

#[track_caller]
fn assert_invalid_length(err: &serde_saphyr::Error, expected_len: usize) {
    let err = err.without_snippet();
    match err {
        serde_saphyr::Error::SerdeInvalidLength { len, expected, .. } => {
            assert_eq!(*len, expected_len);
            assert_eq!(expected, "a tuple of size 2");
        }
        other => panic!("expected a structured invalid length error, got {other:?}"),
    }
}
