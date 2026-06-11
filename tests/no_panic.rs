#![cfg(all(feature = "serialize", feature = "deserialize"))]
use rstest::rstest;
use serde::Deserialize;

#[derive(Deserialize)]
#[allow(dead_code)]
// Parsing target, less important as we mostly expect just error
struct Mura {
    x: String,
    key: String,
    value: String,
}

#[test]
fn test_yaml_malformed() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct TestStruct {
        x: String,
    }

    let yaml_input = "\n    x {\n        ";
    let result: Result<TestStruct, _> = serde_saphyr::from_str(yaml_input);
    assert!(
        result.is_err(),
        "Parsing invalid YAML should fail with an error, not succeed."
    );
}

#[rstest]
#[case::lexer_errors(">\n@ !")]
#[case::unmatched_brackets("{key: [value1, value2")]
#[case::invalid_escape_sequence(r#"key: "Invalid\xEscape""#)]
#[case::invalid_boolean_tagged("key: !!bool truue")]
#[case::incomplete_quoting("key: \"unterminated string")]
#[case::invalid_anchor_reference("key: *undefined_anchor")]
#[case::cyclic_references("&a [ *a ]")]
#[case::unexpected_eof("{key: value")]
fn test_invalid_yaml_errors_without_panic(#[case] yaml_input: &str) {
    let result: Result<Mura, _> = serde_saphyr::from_str(yaml_input);
    assert!(
        result.is_err(),
        "expected error for input `{yaml_input}`, got Ok"
    );
}

#[test]
fn test_deeply_nested_structures() {
    let yaml_input = format!("{}{}", "[".repeat(10_000), "]".repeat(10_000));
    let result: Result<Mura, _> = serde_saphyr::from_str(&yaml_input);
    assert!(
        result.is_err(),
        "Deeply nested structures should gracefully return an error."
    );
}

#[test]
fn test_empty_input() {
    let result: Result<Mura, _> = serde_saphyr::from_str("");
    assert!(result.is_err(), "Empty struct not enough");
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
         ] # Indentation must be nested in
    "#;

    let parsed: Data = serde_saphyr::from_str(yaml_input).expect("Failed to parse YAML");

    assert_eq!(
        parsed,
        Data {
            multiline_array: vec!["item".to_string()]
        }
    );
}
