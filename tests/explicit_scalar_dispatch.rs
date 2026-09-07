#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[rstest]
#[case::integer("!!int 42", json!(42))]
#[case::negative_integer("!!int -42", json!(-42))]
#[case::minimum_signed_integer("!!int -9223372036854775808", json!(i64::MIN))]
#[case::maximum_unsigned_integer("!!int 18446744073709551615", json!(u64::MAX))]
#[case::quoted_integer("!!int \"42\"", json!(42))]
#[case::block_integer("!!int |-\n  42", json!(42))]
#[case::hexadecimal_integer("!!int 0x2a", json!(42))]
#[case::octal_integer("!!int 0o52", json!(42))]
#[case::binary_integer("!!int 0b101010", json!(42))]
#[case::float("!!float 4.25", json!(4.25))]
#[case::integer_looking_float("!!float 42", json!(42.0))]
#[case::quoted_float("!!float '4.25'", json!(4.25))]
#[case::block_float("!!float >-\n  42", json!(42.0))]
#[case::boolean("!!bool true", json!(true))]
#[case::quoted_boolean("!!bool 'false'", json!(false))]
#[case::block_boolean("!!bool |-\n  true", json!(true))]
#[case::legacy_boolean("!!bool yes", json!(true))]
fn explicit_scalar_tags_dispatch_to_json_types(#[case] scalar: &str, #[case] expected: Value) {
    let yaml = format!("answer: {scalar}\n");
    let value: Value = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(value["answer"], expected);
}

#[derive(Debug, Deserialize)]
struct Flattened<T> {
    #[serde(flatten)]
    values: BTreeMap<String, T>,
}

#[test]
fn flattened_integer_map_accepts_explicit_integer_tags() {
    let parsed: Flattened<u32> = serde_saphyr::from_str("answer: !!int 42\n").unwrap();
    assert_eq!(parsed.values, BTreeMap::from([("answer".to_owned(), 42)]));
}

#[test]
fn flattened_float_map_accepts_explicit_float_tags() {
    let parsed: Flattened<f64> = serde_saphyr::from_str("answer: !!float '42'\n").unwrap();
    assert_eq!(parsed.values, BTreeMap::from([("answer".to_owned(), 42.0)]));
}

#[test]
fn flattened_boolean_map_accepts_explicit_boolean_tags() {
    let parsed: Flattened<bool> = serde_saphyr::from_str("answer: !!bool 'true'\n").unwrap();
    assert_eq!(parsed.values, BTreeMap::from([("answer".to_owned(), true)]));
}

#[rstest]
#[case::canonical("!!bool true", true)]
#[case::quoted("!!bool 'false'", false)]
fn explicit_boolean_tags_respect_strict_mode(#[case] yaml: &str, #[case] expected: bool) {
    let options = serde_saphyr::options! { strict_booleans: true };
    let value: Value = serde_saphyr::from_str_with_options(yaml, options).unwrap();
    assert_eq!(value, Value::Bool(expected));
}

#[test]
fn strict_mode_rejects_explicit_legacy_boolean() {
    let options = serde_saphyr::options! { strict_booleans: true };
    let error = serde_saphyr::from_str_with_options::<Value>("!!bool yes", options).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        serde_saphyr::Error::InvalidBooleanStrict { .. }
    ));
}

#[rstest]
#[case::integer_fraction("!!int 1.5")]
#[case::integer_boolean("!!int true")]
#[case::integer_null("!!int null")]
#[case::integer_tilde("!!int ~")]
#[case::integer_missing("!!int")]
#[case::integer_empty("!!int ''")]
#[case::integer_out_of_range("!!int 18446744073709551616")]
#[case::float_boolean("!!float true")]
#[case::float_null("!!float null")]
#[case::float_text("!!float text")]
#[case::boolean_number("!!bool 42")]
#[case::boolean_null("!!bool null")]
#[case::boolean_text("!!bool text")]
fn invalid_explicit_scalars_do_not_fall_back_to_other_types(#[case] yaml: &str) {
    assert!(
        serde_saphyr::from_str::<Value>(yaml).is_err(),
        "invalid explicitly tagged scalar must fail: {yaml}"
    );
}
