use super::yaml_suite_support::{assert_invalid_case, assert_json_case};
use serde::Deserialize;
use serde_saphyr::Error;

#[derive(Debug, Deserialize)]
struct Doc {
    foo: String,
    bar: i64,
}

#[test]
fn yaml_y79y_block_scalar_with_tab() {
    let y = "foo: |\n\t\nbar: 1\n";
    let r: Result<Doc, Error> = serde_saphyr::from_str(y);
    assert!(r.is_err(), "Tabs cannot be used to indent block scalars");
}

#[test]
fn yaml_y79y_quoted_scalar_with_tab() {
    let y = "foo: \"\\t\\n\"\nbar: 1\n";
    let v: Doc = serde_saphyr::from_str(y).unwrap();
    assert_eq!(v.foo, "\t\n");
    assert_eq!(v.bar, 1);
}

#[test]
fn yaml_y79y_000_tab_as_block_scalar_indentation() {
    assert_invalid_case("foo: |\n\t\nbar: 1\n");
}

#[test]
fn yaml_y79y_001_tab_after_block_scalar_indentation() {
    assert_json_case(
        "foo: |\n \t\nbar: 1\n",
        "{\n  \"foo\": \"\\t\\n\",\n  \"bar\": 1\n}\n",
    );
}

#[test]
fn yaml_y79y_002_tab_on_empty_flow_sequence_line() {
    assert_json_case("- [\n\t\n foo\n ]\n", "[\n  [\n    \"foo\"\n  ]\n]\n");
}

#[test]
fn yaml_y79y_003_tab_as_flow_sequence_indentation() {
    assert_invalid_case("- [\n\tfoo,\n foo\n ]\n");
}

#[test]
fn yaml_y79y_004_tab_after_block_sequence_indicator() {
    assert_invalid_case("-\t-\n");
}

#[test]
fn yaml_y79y_005_tab_before_nested_block_sequence_indicator() {
    assert_invalid_case("- \t-\n");
}

#[test]
fn yaml_y79y_006_tab_after_explicit_mapping_key_indicator() {
    assert_invalid_case("?\t-\n");
}

#[test]
fn yaml_y79y_007_tab_after_explicit_mapping_value_indicator() {
    assert_invalid_case("? -\n:\t-\n");
}

#[test]
fn yaml_y79y_008_tab_after_mapping_key_indicator() {
    assert_invalid_case("?\tkey:\n");
}

#[test]
fn yaml_y79y_009_tab_after_mapping_value_indicator() {
    assert_invalid_case("? key:\n:\tkey:\n");
}

#[test]
fn yaml_y79y_010_tab_between_sequence_indicator_and_scalar() {
    assert_json_case("-\t-1\n", "[\n  -1\n]\n");
}
