#![cfg(all(feature = "serialize", feature = "deserialize"))]
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub enum Value {
    Expression(String),
    Template(String),
    Pair(String, u32),
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Context {
    value: Value,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum TaggedScalar {
    Relative(i32),
    Unsigned(u32),
    Boolean(bool),
    Text(String),
}

#[test]
fn local_tags_do_not_become_core_string_tags() {
    for (yaml, expected) in [
        ("!relative 5", TaggedScalar::Relative(5)),
        ("!relative -3", TaggedScalar::Relative(-3)),
        ("!relative 0", TaggedScalar::Relative(0)),
        ("!unsigned 7", TaggedScalar::Unsigned(7)),
        ("!boolean true", TaggedScalar::Boolean(true)),
        ("!text 5", TaggedScalar::Text("5".to_owned())),
    ] {
        let actual: TaggedScalar =
            serde_saphyr::from_str(yaml).expect("tagged scalar should deserialize");

        assert_eq!(actual, expected, "failed to deserialize {yaml:?}");
    }
}

#[test]
fn test_tagged_expression_scalar() {
    assert_eq!(
        serde_saphyr::<Context>(r#"value: !Expression "1 + 1""#),
        Context {
            value: Value::Expression("1 + 1".to_string())
        }
    );
}

#[test]
fn test_tagged_pair_flow_seq() {
    assert_eq!(
        serde_saphyr::<Context>(r#"value: !Pair [a, 12]"#),
        Context {
            value: Value::Pair("a".to_string(), 12)
        }
    );
}

#[test]
fn test_tagged_pair_block_seq() {
    assert_eq!(
        serde_saphyr::<Context>(
            r#"
value: !Pair
  - "a"
  - 12
"#
        ),
        Context {
            value: Value::Pair("a".to_string(), 12)
        }
    );
}

#[test]
fn test_tagged_pair_wrong_shape_scalar_should_error() {
    // arity>1 should *not* accept scalar
    let err = serde_saphyr::from_str::<Context>(r#"value: !Pair "a""#).unwrap_err();
    let err = err.without_snippet();
    assert!(matches!(
        err,
        serde_saphyr::Error::Unexpected {
            expected: "sequence start",
            ..
        }
    ));
}

#[track_caller]
fn serde_saphyr<T: DeserializeOwned + Debug>(yaml: &str) -> T {
    match serde_saphyr::from_str::<T>(yaml) {
        Ok(value) => value,
        Err(err) => panic!("{}", err),
    }
}
