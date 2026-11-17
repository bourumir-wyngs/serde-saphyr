#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde_json::{Value, json};
use serde_saphyr::NonFiniteFloatPolicy;
use std::collections::BTreeMap;

const POLICIES: [NonFiniteFloatPolicy; 4] = [
    NonFiniteFloatPolicy::Default,
    NonFiniteFloatPolicy::Reject,
    NonFiniteFloatPolicy::PassThrough,
    NonFiniteFloatPolicy::AsString,
];

fn infer(yaml: &str, policy: NonFiniteFloatPolicy) -> Value {
    let mut options = serde_saphyr::Options::default();
    options.non_finite_float_policy = policy;
    let from_str: Value = serde_saphyr::from_str_with_options(yaml, options.clone()).unwrap();
    let from_reader: Value =
        serde_saphyr::from_reader_with_options(yaml.as_bytes(), options).unwrap();
    assert_eq!(from_str, from_reader, "reader input: {yaml}");
    from_str
}

#[rstest]
fn signed_nan_infers_as_string(
    #[values(".nan", ".NaN", ".NAN")] nan: &str,
    #[values("+", "-")] sign: &str,
) {
    let scalar = format!("{sign}{nan}");
    for policy in POLICIES {
        assert_eq!(infer(&scalar, policy), json!(scalar), "{policy:?}");
        assert_eq!(
            infer(&format!("example: {scalar}\n"), policy),
            json!({ "example": scalar }),
            "{policy:?}"
        );
    }
}

#[rstest]
#[case(".nan", ".nan")]
#[case(".NaN", ".nan")]
#[case("+.inf", ".inf")]
#[case("-.Inf", "-.inf")]
fn unsigned_nan_and_signed_infinity_still_infer_as_floats(
    #[case] scalar: &str,
    #[case] canonical: &str,
) {
    assert_eq!(
        infer(scalar, NonFiniteFloatPolicy::AsString),
        json!(canonical)
    );
    assert_eq!(
        infer(scalar, NonFiniteFloatPolicy::PassThrough),
        Value::Null
    );
}

#[rstest]
fn typed_and_explicit_floats_keep_signed_nan(#[values("+.nan", "-.NaN", "-.NAN")] scalar: &str) {
    assert!(serde_saphyr::from_str::<f64>(scalar).unwrap().is_nan());
    assert!(serde_saphyr::from_str::<f32>(scalar).unwrap().is_nan());
    assert_eq!(
        infer(
            &format!("!!float {scalar}"),
            NonFiniteFloatPolicy::PassThrough
        ),
        Value::Null
    );
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum FloatOrText {
    Float(f64),
    Text(String),
}

#[derive(Debug, Deserialize)]
struct Flattened {
    #[serde(flatten)]
    values: BTreeMap<String, String>,
}

#[rstest]
fn buffered_values_preserve_signed_nan(#[values("+.nan", "-.NaN")] scalar: &str) {
    let options = serde_saphyr::options! {
        non_finite_float_policy: NonFiniteFloatPolicy::PassThrough,
    };
    let value: FloatOrText = serde_saphyr::from_str_with_options(scalar, options.clone()).unwrap();
    assert_eq!(value, FloatOrText::Text(scalar.to_owned()));
    let yaml = format!("example: {scalar}\n");
    let value: Flattened = serde_saphyr::from_str_with_options(&yaml, options).unwrap();
    assert_eq!(value.values["example"], scalar);
}

#[rstest]
#[case("'-.nan'")]
#[case("\"-.nan\"")]
#[case("!!str -.nan")]
#[case("! -.nan")]
fn explicit_strings_remain_strings(#[case] yaml: &str) {
    for policy in POLICIES {
        assert_eq!(infer(yaml, policy), json!("-.nan"), "{policy:?}");
    }
}
