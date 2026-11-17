#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Assert inference for `yaml` against `expected` using the selected `options`.
/// Returns nothing; tests both string and reader entry points.
fn assert_inferred(yaml: &str, expected: Value, options: serde_saphyr::Options) {
    let from_str: Value = serde_saphyr::from_str_with_options(yaml, options.clone()).unwrap();
    let from_reader: Value =
        serde_saphyr::from_reader_with_options(yaml.as_bytes(), options).unwrap();
    assert_eq!(from_str, expected, "string input: {yaml}");
    assert_eq!(from_reader, expected, "reader input: {yaml}");
}

#[rstest]
#[case("0X1F", 31)]
#[case("0O17", 15)]
#[case("0B101", 5)]
fn uppercase_prefix_inference_follows_shared_integer_policy(
    #[case] scalar: &str,
    #[case] value: i64,
    #[values("", "+", "-")] sign: &str,
) {
    let scalar = format!("{sign}{scalar}");
    let value = if sign == "-" { -value } else { value };
    let restricted = serde_saphyr::options! { lowercase_radix_prefixes_only: true };
    assert_inferred(&scalar, json!(scalar), restricted.clone());
    assert_inferred(
        &format!("example: {scalar}\n"),
        json!({ "example": scalar }),
        restricted,
    );
    assert_inferred(&scalar, json!(value), serde_saphyr::Options::default());
    assert_inferred(
        &format!("example: {scalar}\n"),
        json!({ "example": value }),
        serde_saphyr::Options::default(),
    );
}

#[rstest]
#[case("0x1F", 31)]
#[case("+0x1F", 31)]
#[case("-0x1F", -31)]
#[case("0o17", 15)]
#[case("+0o17", 15)]
#[case("-0o17", -15)]
#[case("0b101", 5)]
#[case("+0b101", 5)]
#[case("-0b101", -5)]
fn lowercase_prefixes_still_infer_as_integers(#[case] scalar: &str, #[case] expected: i64) {
    for lowercase_only in [false, true] {
        assert_inferred(
            scalar,
            json!(expected),
            serde_saphyr::options! { lowercase_radix_prefixes_only: lowercase_only },
        );
    }
}

#[rstest]
#[case("0X1F", 31)]
#[case("+0X1F", 31)]
#[case("-0X1F", -31)]
#[case("0O17", 15)]
#[case("+0O17", 15)]
#[case("-0O17", -15)]
#[case("0B101", 5)]
#[case("+0B101", 5)]
#[case("-0B101", -5)]
fn typed_and_explicit_integers_keep_uppercase_prefixes(
    #[case] scalar: &str,
    #[case] expected: i64,
) {
    assert_eq!(serde_saphyr::from_str::<i64>(scalar).unwrap(), expected);
    if expected >= 0 {
        assert_eq!(
            serde_saphyr::from_str::<u64>(scalar).unwrap(),
            expected as u64
        );
    }
    assert_inferred(
        &format!("!!int {scalar}"),
        json!(expected),
        serde_saphyr::Options::default(),
    );
    assert_inferred(
        &format!("!!int '{scalar}'"),
        json!(expected),
        serde_saphyr::Options::default(),
    );
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum NumberOrText {
    Number(i64),
    Text(String),
}

#[derive(Debug, Deserialize)]
struct Flattened {
    #[serde(flatten)]
    values: BTreeMap<String, String>,
}

#[rstest]
fn buffered_values_preserve_uppercase_prefixes(#[values("0X1F", "+0O17", "-0B101")] scalar: &str) {
    let options = serde_saphyr::options! { lowercase_radix_prefixes_only: true };
    let value: NumberOrText = serde_saphyr::from_str_with_options(scalar, options.clone()).unwrap();
    assert_eq!(value, NumberOrText::Text(scalar.to_owned()));
    let yaml = format!("example: {scalar}\n");
    let value: Flattened = serde_saphyr::from_str_with_options(&yaml, options).unwrap();
    assert_eq!(value.values["example"], scalar);
}

#[test]
fn inference_preserves_borrowed_strings() {
    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(untagged)]
    enum BorrowedNumberOrText<'a> {
        Number(i64),
        Text(#[serde(borrow)] &'a str),
    }

    let value: BorrowedNumberOrText<'_> = serde_saphyr::from_str_with_options(
        "-0X1F",
        serde_saphyr::options! { lowercase_radix_prefixes_only: true },
    )
    .unwrap();
    assert_eq!(value, BorrowedNumberOrText::Text("-0X1F"));
}

#[rstest]
#[case("'0X1F'")]
#[case("\"0X1F\"")]
#[case("!!str 0X1F")]
#[case("! 0X1F")]
fn explicit_strings_remain_strings(#[case] yaml: &str) {
    for lowercase_only in [false, true] {
        assert_inferred(
            yaml,
            json!("0X1F"),
            serde_saphyr::options! { lowercase_radix_prefixes_only: lowercase_only },
        );
    }
}

#[test]
fn legacy_octal_does_not_override_uppercase_prefix_restriction() {
    let options = serde_saphyr::options! {
        legacy_octal_numbers: true,
        lowercase_radix_prefixes_only: true,
    };
    let value: Value =
        serde_saphyr::from_str_with_options("[0X1F, +0O17, -0B101]", options).unwrap();
    assert_eq!(value, json!(["0X1F", "+0O17", "-0B101"]));
}
