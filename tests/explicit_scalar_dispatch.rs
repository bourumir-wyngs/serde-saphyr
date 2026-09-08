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
#[case::quoted_signed_hexadecimal("!!int ' -0x2a '", json!(-42))]
#[case::block_integer("!!int |-\n  42", json!(42))]
#[case::folded_integer_with_newline("!!int >\n  42\n", json!(42))]
#[case::hexadecimal_integer("!!int 0x2a", json!(42))]
#[case::octal_integer("!!int 0o52", json!(42))]
#[case::binary_integer("!!int 0b101010", json!(42))]
#[case::float("!!float 4.25", json!(4.25))]
#[case::integer_looking_float("!!float 42", json!(42.0))]
#[case::quoted_float("!!float '4.25'", json!(4.25))]
#[case::block_float("!!float >-\n  42", json!(42.0))]
#[case::literal_float_with_newline("!!float |\n  4.25\n", json!(4.25))]
#[case::quoted_float_exponent("!!float \"4.25e+2\"", json!(425.0))]
#[case::boolean("!!bool true", json!(true))]
#[case::quoted_boolean("!!bool 'false'", json!(false))]
#[case::block_boolean("!!bool |-\n  true", json!(true))]
#[case::legacy_boolean("!!bool yes", json!(true))]
fn explicit_scalar_tags_dispatch_to_json_types(#[case] scalar: &str, #[case] expected: Value) {
    let yaml = format!("answer: {scalar}\n");
    let value: Value = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(value["answer"], expected);
}

#[rstest]
#[case::verbatim_integer("!<tag:yaml.org,2002:int> '0x2a'", json!(42))]
#[case::verbatim_float("!<tag:yaml.org,2002:float> '42'", json!(42.0))]
#[case::named_integer("%TAG !number! tag:yaml.org,2002:\n--- !number!int '0x2a'", json!(42))]
#[case::named_float("%TAG !number! tag:yaml.org,2002:\n--- !number!float '42'", json!(42.0))]
fn explicit_numeric_tags_resolve_handles(#[case] yaml: &str, #[case] expected: Value) {
    let options = serde_saphyr::options! { reject_unsupported_tags: true };
    let value: Value = serde_saphyr::from_str_with_options(yaml, options).unwrap();
    assert_eq!(value, expected);
}

#[test]
fn explicit_numeric_tags_survive_alias_and_merge_replay() {
    let yaml = "\
defaults: &defaults
  integer: &integer !!int '-0x2a'
  float: &float !!float |
    42
aliases: [*integer, *float]
merged:
  <<: *defaults
";
    let expected = json!({
        "defaults": { "integer": -42, "float": 42.0 },
        "aliases": [-42, 42.0],
        "merged": { "integer": -42, "float": 42.0 },
    });
    let from_str: Value = serde_saphyr::from_str(yaml).unwrap();
    let from_reader: Value = serde_saphyr::from_reader(yaml.as_bytes()).unwrap();
    assert_eq!(from_str, expected);
    assert_eq!(from_reader, expected);
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum NumberOrText {
    Integer(u64),
    Float(f64),
    Text(String),
}

#[rstest]
#[case::integer("!!int '42'", NumberOrText::Integer(42))]
#[case::float("!!float '42'", NumberOrText::Float(42.0))]
#[case::block_integer("!!int >\n  42\n", NumberOrText::Integer(42))]
#[case::block_float("!!float |\n  42\n", NumberOrText::Float(42.0))]
#[case::quoted_text("'42'", NumberOrText::Text("42".to_owned()))]
fn numeric_tags_select_the_matching_untagged_variant(
    #[case] yaml: &str,
    #[case] expected: NumberOrText,
) {
    assert_eq!(
        serde_saphyr::from_str::<NumberOrText>(yaml).unwrap(),
        expected
    );
}

#[rstest]
#[case::plain("!!float -0")]
#[case::quoted("!!float '-0'")]
#[case::block("!!float |\n  -0\n")]
fn explicitly_tagged_floats_preserve_negative_zero(#[case] yaml: &str) {
    let value: Value = serde_saphyr::from_str(yaml).unwrap();
    assert!(
        value.is_f64(),
        "the explicit float must stay a float: {value}"
    );
    assert_eq!(value.as_f64().unwrap().to_bits(), (-0.0_f64).to_bits());
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
#[case::negative_integer_out_of_range("!!int '-9223372036854775809'")]
#[case::quoted_integer_fraction("!!int '1.5'")]
#[case::block_integer_fraction("!!int |\n  1.5\n")]
#[case::integer_exponent("!!int '1e2'")]
#[case::float_boolean("!!float true")]
#[case::float_null("!!float null")]
#[case::float_text("!!float text")]
#[case::float_missing("!!float")]
#[case::float_empty("!!float ''")]
#[case::quoted_float_text("!!float 'text'")]
#[case::block_float_text("!!float >\n  text\n")]
#[case::boolean_number("!!bool 42")]
#[case::boolean_null("!!bool null")]
#[case::boolean_text("!!bool text")]
fn invalid_explicit_scalars_do_not_fall_back_to_other_types(#[case] yaml: &str) {
    assert!(
        serde_saphyr::from_str::<Value>(yaml).is_err(),
        "invalid explicitly tagged scalar must fail: {yaml}"
    );
}

#[rstest]
fn optional_explicit_scalars_reject_null_like_payloads(
    #[values("!!int", "!!float", "!!bool")] tag: &str,
    #[values("null", "~", "")] payload: &str,
) {
    let yaml = format!("{tag} {payload}");
    let typed = match tag {
        "!!int" => serde_saphyr::from_str::<Option<i32>>(&yaml).map(|_| ()),
        "!!float" => serde_saphyr::from_str::<Option<f64>>(&yaml).map(|_| ()),
        "!!bool" => serde_saphyr::from_str::<Option<bool>>(&yaml).map(|_| ()),
        _ => unreachable!(),
    };
    let generic = serde_saphyr::from_str::<Option<Value>>(&yaml);

    assert_eq!(
        (typed.is_err(), generic.is_err()),
        (true, true),
        "both typed and generic options must validate the explicit tag: {yaml}"
    );
}

#[test]
fn optional_explicit_scalars_preserve_values_and_real_null() {
    assert_eq!(
        serde_saphyr::from_str::<Option<i32>>("!!int 42").unwrap(),
        Some(42)
    );
    assert_eq!(
        serde_saphyr::from_str::<Option<f64>>("!!float 4.25").unwrap(),
        Some(4.25)
    );
    assert_eq!(
        serde_saphyr::from_str::<Option<bool>>("!!bool true").unwrap(),
        Some(true)
    );
    assert_eq!(
        serde_saphyr::from_str::<Option<Value>>("!!int 42").unwrap(),
        Some(json!(42))
    );

    for yaml in ["null", "~", "---\n", "!!null null", "!!null"] {
        assert_eq!(serde_saphyr::from_str::<Option<i32>>(yaml).unwrap(), None);
        assert_eq!(serde_saphyr::from_str::<Option<f64>>(yaml).unwrap(), None);
        assert_eq!(serde_saphyr::from_str::<Option<bool>>(yaml).unwrap(), None);
        assert_eq!(serde_saphyr::from_str::<Option<Value>>(yaml).unwrap(), None);
    }
}

#[rstest]
#[case::integer_null("!!int null")]
#[case::float_tilde("!!float ~")]
#[case::boolean_missing("!!bool")]
fn optional_mapping_values_validate_explicit_tags(#[case] scalar: &str) {
    let yaml = format!("answer: {scalar}\n");
    let ordinary = serde_saphyr::from_str::<BTreeMap<String, Option<Value>>>(&yaml);
    let flattened = serde_saphyr::from_str::<Flattened<Option<Value>>>(&yaml);

    let yaml = format!("1: {scalar}\n");
    let options = serde_saphyr::options! {
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::LastWins,
    };
    let buffered =
        serde_saphyr::from_str_with_options::<BTreeMap<u32, Option<Value>>>(&yaml, options);

    assert_eq!(
        (ordinary.is_err(), flattened.is_err(), buffered.is_err()),
        (true, true, true),
        "mapping values must validate the explicit tag: {scalar}"
    );
}

#[rstest]
#[case::integer_null("!!int null")]
#[case::float_tilde("!!float ~")]
#[case::boolean_missing("!!bool")]
fn document_streams_reject_invalid_explicit_scalars(#[case] scalar: &str) {
    let yaml = format!("--- {scalar}\n--- !!int 42\n");
    let multiple = serde_saphyr::from_multiple::<Option<Value>>(&yaml);
    let mut reader = yaml.as_bytes();
    let streamed =
        serde_saphyr::read::<_, Option<Value>>(&mut reader).collect::<Result<Vec<_>, _>>();

    assert_eq!(
        (multiple.is_err(), streamed.is_err()),
        (true, true),
        "document streams must validate the explicit tag: {scalar}"
    );
}

#[test]
fn invalid_explicit_scalar_does_not_become_an_empty_optional_container() {
    let yaml = "!!int null";
    assert!(serde_saphyr::from_str::<Option<Vec<i32>>>(yaml).is_err());
    assert!(serde_saphyr::from_str::<Option<BTreeMap<String, i32>>>(yaml).is_err());
    assert!(serde_saphyr::from_str::<Option<()>>(yaml).is_err());
}

#[test]
fn invalid_explicit_scalar_does_not_become_a_unit_variant_payload() {
    #[derive(Deserialize)]
    enum UnitVariant {
        Variant,
    }

    assert!(serde_saphyr::from_str::<UnitVariant>("{Variant: !!int null}").is_err());
}

#[test]
fn invalid_explicit_scalar_does_not_become_an_empty_merge() {
    let yaml = "<<: !!int null\nkept: 1\n";
    assert!(serde_saphyr::from_str::<BTreeMap<String, i32>>(yaml).is_err());
}
