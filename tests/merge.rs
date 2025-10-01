use std::collections::BTreeMap;

use serde::Deserialize;
use serde::de::IgnoredAny;

use serde_saphyr::options::DuplicateKeyPolicy;
use serde_saphyr::{Options, from_str, from_str_with_options};

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
fn merge_errors_on_conflict_by_default() {
    let yaml = r#"
base1: &B1 { a: 1, b: 2 }
base2: &B2 { b: 20 }
target:
  <<: [*B1, *B2]
"#;

    let result: Result<MergeDoc<BTreeMap<String, i32>>, _> = from_str(yaml);
    assert!(
        result.is_err(),
        "duplicate keys in merges must error by default"
    );
}

#[test]
fn merge_respects_first_wins_policy() {
    let yaml = r#"
base1: &B1 { a: 1, b: 2 }
base2: &B2 { b: 20, c: 3 }
target:
  <<: [*B1, *B2]
"#;

    let mut options = Options::default();
    options.duplicate_keys = DuplicateKeyPolicy::FirstWins;

    let doc: MergeDoc<BTreeMap<String, i32>> =
        from_str_with_options(yaml, options).expect("merge must honor FirstWins");
    assert_eq!(doc.target.get("b"), Some(&2));
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

    let mut options = Options::default();
    options.duplicate_keys = DuplicateKeyPolicy::LastWins;

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
