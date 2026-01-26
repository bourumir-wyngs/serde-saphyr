use std::collections::BTreeMap;

use serde::Deserialize;
use serde::de::IgnoredAny;

use serde_saphyr::options::DuplicateKeyPolicy;
use serde_saphyr::{from_str, from_str_with_options};

#[derive(Deserialize)]
struct MergeDoc<T> {
    #[serde(flatten)]
    #[allow(dead_code)]
    rest: BTreeMap<String, IgnoredAny>,
    target: T,
}

#[test]
fn merge_expands_nested_mappings() {
    let yaml = r#"
base1: &B1 { a: 1, b: 2 }
base2: &B2
  <<: { c: 3 }
  d: 4
target:
  <<: [*B1, *B2]
  e: 5
"#;

    let doc: MergeDoc<BTreeMap<String, i32>> = from_str(yaml).expect("merge must deserialize");
    let mut expected = BTreeMap::new();
    expected.insert("a".to_string(), 1);
    expected.insert("b".to_string(), 2);
    expected.insert("c".to_string(), 3);
    expected.insert("d".to_string(), 4);
    expected.insert("e".to_string(), 5);
    assert_eq!(doc.target, expected);
}

#[test]
fn merge_conflicts_skip_duplicates_by_default() {
    let yaml = r#"
base1: &B1 { a: 1, b: 2 }
base2: &B2 { b: 20 }
target:
  <<: [*B1, *B2]
"#;

    let doc: MergeDoc<BTreeMap<String, i32>> = from_str(yaml).expect("merge must skip duplicates");
    assert_eq!(doc.target.get("a"), Some(&1));
    assert_eq!(doc.target.get("b"), Some(&20));
}

#[test]
fn merge_respects_first_wins_policy() {
    let yaml = r#"
base1: &B1 { a: 1, b: 2 }
base2: &B2 { b: 20, c: 3 }
target:
  <<: [*B1, *B2]
"#;

    let options = serde_saphyr::options! {
        duplicate_keys: DuplicateKeyPolicy::FirstWins,
    };

    let doc: MergeDoc<BTreeMap<String, i32>> =
        from_str_with_options(yaml, options).expect("merge must honor FirstWins");
    assert_eq!(doc.target.get("b"), Some(&20));
    assert_eq!(doc.target.get("c"), Some(&3));
}

#[test]
fn merge_respects_last_wins_policy() {
    let yaml = r#"
base1: &B1 { a: 1, b: 2 }
base2: &B2 { b: 20, c: 3 }
target:
  <<: [*B1, *B2]
"#;

    let options = serde_saphyr::options! {
        duplicate_keys: DuplicateKeyPolicy::LastWins,
    };

    let doc: MergeDoc<BTreeMap<String, i32>> =
        from_str_with_options(yaml, options).expect("merge must honor LastWins");
    assert_eq!(doc.target.get("b"), Some(&20));
    assert_eq!(doc.target.get("c"), Some(&3));
}

#[test]
fn merge_rejects_non_mapping_value() {
    let yaml = r#"
target:
  <<: 42
  other: 1
"#;

    let result: Result<MergeDoc<BTreeMap<String, i32>>, _> = from_str(yaml);
    assert!(result.is_err(), "non-mapping merge value must error");
}

#[test]
fn quoted_merge_key_is_literal() {
    let yaml = r#"
"<<": 1
other: 2
"#;

    let map: BTreeMap<String, i32> = from_str(yaml).expect("quoted key must deserialize");
    assert_eq!(map.get("<<"), Some(&1));
    assert_eq!(map.get("other"), Some(&2));
}

#[test]
fn merge_explicit_fields_override_with_first_wins() {
    let yaml = r#"
base: &B { shared: 1, untouched: 3 }
target:
  shared: 10
  own: 5
  <<: *B
"#;

    let options = serde_saphyr::options! {
        duplicate_keys: DuplicateKeyPolicy::FirstWins,
    };

    let doc: MergeDoc<BTreeMap<String, i32>> =
        from_str_with_options(yaml, options).expect("explicit fields must win");
    assert_eq!(doc.target.get("shared"), Some(&10));
    assert_eq!(doc.target.get("untouched"), Some(&3));
    assert_eq!(doc.target.get("own"), Some(&5));
}

#[test]
fn merge_keys_expand_in_reverse_order() {
    let yaml = r#"
base1: &B1 { shared: 1, from_one: 10 }
base2: &B2 { shared: 2, from_two: 20 }
base3: &B3 { shared: 3, from_three: 30 }
target:
  <<: *B1
  <<: *B2
  <<: *B3
"#;

    let options = serde_saphyr::options! {
        duplicate_keys: DuplicateKeyPolicy::FirstWins,
    };

    let doc: MergeDoc<BTreeMap<String, i32>> =
        from_str_with_options(yaml, options).expect("merges must expand");
    assert_eq!(doc.target.get("shared"), Some(&3));
    assert_eq!(doc.target.get("from_one"), Some(&10));
    assert_eq!(doc.target.get("from_two"), Some(&20));
    assert_eq!(doc.target.get("from_three"), Some(&30));
}

#[test]
fn merge_sequence_applies_last_mapping_last() {
    let yaml = r#"
target:
  <<: [ { shared: 1, first: 10 }, { shared: 2, second: 20 } ]
"#;

    let options = serde_saphyr::options! {
        duplicate_keys: DuplicateKeyPolicy::FirstWins,
    };

    let doc: MergeDoc<BTreeMap<String, i32>> =
        from_str_with_options(yaml, options).expect("sequence merges must expand");
    assert_eq!(doc.target.get("shared"), Some(&2));
    assert_eq!(doc.target.get("first"), Some(&10));
    assert_eq!(doc.target.get("second"), Some(&20));
}
