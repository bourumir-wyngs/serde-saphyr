use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub enum Value {
    Expression(String),
    Template(String),
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Context {
    value: Value,
}

#[test]
fn test_tagged_expression() {
    assert_eq!(
        serde_saphyr::<Context>(r#"value: !Expression "1 + 1""#),
        Context {
            value: Value::Expression("1 + 1".to_string())
        }
    );
}
#[test]
fn test_tagged_template() {
    assert_eq!(
        serde_saphyr::<Context>(r#"value: !Template "{{ a }}""#),
        Context {
            value: Value::Template("{{ a }}".to_string())
        }
    );
}

// As described in the readme, these do work, but require a new line:
#[test]
fn test_expression_context() {
    assert_eq!(
        serde_saphyr::<Context>("value:\n  Expression: \"1 + 1\""),
        Context {
            value: Value::Expression("1 + 1".to_string())
        }
    );
}
#[test]
fn test_template_context() {
    assert_eq!(
        serde_saphyr::<Context>("value:\n  Template: \"{{ a }}\""),
        Context {
            value: Value::Template("{{ a }}".to_string())
        }
    );
}

fn serde_saphyr<T: DeserializeOwned + Debug>(yaml: &str) -> T {
    match serde_saphyr::from_str::<T>(yaml) {
        Ok(value) => value,
        Err(err) => {
            let report = err.to_string();
            panic!("{report}");
        }
    }
}
