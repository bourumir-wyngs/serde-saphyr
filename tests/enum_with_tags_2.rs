use std::fmt::Debug;
use serde::de::DeserializeOwned;
use serde::Deserialize;

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub enum Value {
    Expression(String),
    Template(String),
    Pair(String, String),
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Context {
    value: Value,
}

#[test]
fn test_tagged_expression_scalar() {
    assert_eq!(
        serde_saphyr::<Context>(r#"value: !Expression "1 + 1""#),
        Context { value: Value::Expression("1 + 1".to_string()) }
    );
}

#[test]
fn test_tagged_pair_flow_seq() {
    assert_eq!(
        serde_saphyr::<Context>(r#"value: !Pair ["a", "b"]"#),
        Context { value: Value::Pair("a".to_string(), "b".to_string()) }
    );
}

#[test]
fn test_tagged_pair_block_seq() {
    assert_eq!(
        serde_saphyr::<Context>(
            r#"
value: !Pair
  - "a"
  - "b"
"#
        ),
        Context { value: Value::Pair("a".to_string(), "b".to_string()) }
    );
}

#[test]
fn test_tagged_pair_wrong_shape_scalar_should_error() {
    // arity>1 should *not* accept scalar
    let err = std::panic::catch_unwind(|| serde_saphyr::<Context>(r#"value: !Pair "a""#));
    assert!(err.is_err());
}

fn serde_saphyr<T: DeserializeOwned + Debug>(yaml: &str) -> T {
    match serde_saphyr::from_str::<T>(yaml) {
        Ok(value) => value,
        Err(err) => panic!("{}", err),
    }
}