use serde::Deserialize;
use serde_saphyr::{Deserializer, Value};

#[test]
fn null_key() {
    let yaml: serde_json::Value = serde_json::Value::deserialize(Deserializer::from_str(r#"null: "key_value""#)).unwrap();
    let json_str = serde_json::to_string(&yaml).unwrap();
    assert_eq!("{\"null\":\"key_value\"}", json_str);
}

#[test]
fn test_yaml_malformed() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct TestStruct {
        x: String
    }

    let yaml_input = "\n    x {\n        ";

    let result: Result<TestStruct, _> = serde_saphyr::from_str(yaml_input);
    println!("{result:?}");

    // Confirm parsing yields an error, and does not panic or succeed.
    assert!(result.is_err(), "Parsing invalid YAML should fail with an error, not succeed.");
}

#[test]
fn test_lexer_errors() {
    let yaml_input = ">\n@ !";
    let result: Result<serde_saphyr::Value, _> = serde_saphyr::from_str(yaml_input);

    // The YAML input is invalid, so expect an Err, but no panic
    assert!(result.is_err(), "Parsing invalid YAML should return an error, not panic.");
}

#[test]
fn test_unmatched_brackets() {
    let yaml_input = "{key: [value1, value2";
    let result: Result<serde_saphyr::Value, _> = serde_saphyr::from_str(yaml_input);
    assert!(result.is_err(), "Unmatched brackets should yield an error without panic.");
}

#[test]
fn test_invalid_escape_sequence() {
    let yaml_input = r#"key: "Invalid\xEscape""#;
    let result: Result<serde_saphyr::Value, _> = serde_saphyr::from_str(yaml_input);
    assert!(result.is_err(), "Invalid escape sequences should yield an error without panic.");
}

#[test]
fn test_invalid_boolean_tagged() {
    let yaml_input = "key: !!bool truue";
    let result: Result<serde_saphyr::Value, _> = serde_saphyr::from_str(yaml_input);
    assert!(result.is_err(), "Tagged invalid boolean should yield an error without panic.");
}

#[test]
fn test_deeply_nested_structures() {
    let yaml_input = format!("{}{}", "[".repeat(10_000), "]".repeat(10_000));
    let result: Result<serde_saphyr::Value, _> = serde_saphyr::from_str(&yaml_input);
    assert!(result.is_err(), "Deeply nested structures should gracefully return an error.");
}

#[test]
fn test_incomplete_quoting() {
    let yaml_input = "key: \"unterminated string";
    let result: Result<serde_saphyr::Value, _> = serde_saphyr::from_str(yaml_input);
    assert!(result.is_err(), "Incomplete quoting should yield an error.");
}

#[test]
fn test_invalid_anchor_reference() {
    let yaml_input = "key: *undefined_anchor";
    let result: Result<serde_saphyr::Value, _> = serde_saphyr::from_str(yaml_input);
    assert!(result.is_err(), "Undefined anchors should yield an error.");
}

#[test]
fn test_cyclic_references() {
    let yaml_input = "&a [ *a ]";
    let result: Result<serde_saphyr::Value, _> = serde_saphyr::from_str(yaml_input);
    assert!(result.is_err(), "Cyclic references should yield an error.");
}

#[test]
fn test_unexpected_eof() {
    let yaml_input = "{key: value";
    let result: Result<serde_saphyr::Value, _> = serde_saphyr::from_str(yaml_input);
    assert!(result.is_err(), "Unexpected EOF should yield an error.");
}

#[test]
fn test_empty_input() {
    assert_eq!(serde_saphyr::from_str::<Value>("").unwrap(), Value::Null(None));
}

#[test]
fn test_multiline_array() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Data {
        multiline_array: Vec<String>,
    }

    let yaml_input = r#"
        multiline_array: [
          'item'
        ]
    "#;

    let parsed: Data = serde_saphyr::from_str(yaml_input).expect("Failed to parse YAML");

    assert_eq!(
        parsed,
        Data {
            multiline_array: vec!["item".to_string()]
        }
    );
}
