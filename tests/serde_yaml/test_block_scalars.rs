#![allow(clippy::derive_partial_eq_without_eq)]

use std::collections::BTreeMap;
use indoc::indoc;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize, PartialEq)]
struct Scalars {
    folded: String,
    folded_strip: String,
    folded_keep: String,
    literal: String,
    literal_strip: String,
    literal_keep: String,
}

#[test]
fn test_block_scalars() {
    let yaml = indoc! {
        "
        folded: >
          line1
          line2
        folded_strip: >-
          line1
          line2
        folded_keep: >+
          line1
          line2

        literal: |
          line1
          line2
        literal_strip: |-
          line1
          line2
        literal_keep: |+
          line1
          line2

        "
    };

    let expected = Scalars {
        folded: "line1 line2\n".to_owned(),
        folded_strip: "line1 line2".to_owned(),
        folded_keep: "line1 line2\n\n".to_owned(),
        literal: "line1\nline2\n".to_owned(),
        literal_strip: "line1\nline2".to_owned(),
        literal_keep: "line1\nline2\n\n".to_owned(),
    };

    let result: Scalars = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(expected, result);
}


#[test]
fn test_block_scalars_2() {
    let yaml = indoc! {
        "
        literal_clip: |
          foo
          bar
        literal_strip: |-
          foo
          bar
        literal_keep: |+
          foo
          bar

        folded_clip: >
          foo
          bar
        folded_strip: >-
          foo
          bar
        folded_keep: >+
          foo
          bar
        "
    };
    let data: BTreeMap<String, Value> = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(data.get("literal_clip").unwrap(), "foo\nbar\n");
    assert_eq!(data.get("literal_strip").unwrap(), "foo\nbar");
    assert_eq!(data.get("literal_keep").unwrap(), "foo\nbar\n\n");
    assert_eq!(data.get("folded_clip").unwrap(), "foo bar\n");
    assert_eq!(data.get("folded_strip").unwrap(), "foo bar");
    assert_eq!(data.get("folded_keep").unwrap(), "foo bar\n");
}
