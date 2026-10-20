#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
struct Field<T> {
    value: T,
}

#[derive(Debug, Deserialize)]
struct Skipped {}

#[derive(Debug, Deserialize)]
struct Flattened<T> {
    #[serde(flatten)]
    values: BTreeMap<String, T>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Protocol {
    Vast,
    OpenRtb,
}

#[test]
fn core_tags_preserve_typed_and_flattened_values() {
    let integer: Field<u32> = serde_saphyr::from_str("value: !!int 10").unwrap();
    let flattened: Flattened<u32> = serde_saphyr::from_str("value: !!int 10").unwrap();
    assert_eq!(integer.value, 10);
    assert_eq!(flattened.values["value"], 10);

    let boolean: Field<bool> = serde_saphyr::from_str("value: !!bool false").unwrap();
    let flattened: Flattened<bool> = serde_saphyr::from_str("value: !!bool false").unwrap();
    assert!(!boolean.value);
    assert!(!flattened.values["value"]);

    let float: Field<f64> = serde_saphyr::from_str("value: !!float 0.5").unwrap();
    let flattened: Flattened<f64> = serde_saphyr::from_str("value: !!float 0.5").unwrap();
    assert_eq!(float.value, 0.5);
    assert_eq!(flattened.values["value"], 0.5);
}

#[rstest]
fn semantic_tags_do_not_become_integers_or_booleans(
    #[values("!!timestamp", "!time", "!degrees", "!radians", "!!binary")] tag: &str,
    #[values(false, true)] angle_conversions: bool,
) {
    let options = serde_saphyr::options! { angle_conversions: angle_conversions };
    let integer = format!("{tag} 10");
    assert!(serde_saphyr::from_str_with_options::<i8>(&integer, options.clone()).is_err());
    assert!(serde_saphyr::from_str_with_options::<i16>(&integer, options.clone()).is_err());
    assert!(serde_saphyr::from_str_with_options::<i32>(&integer, options.clone()).is_err());
    assert!(serde_saphyr::from_str_with_options::<i64>(&integer, options.clone()).is_err());
    assert!(serde_saphyr::from_str_with_options::<i128>(&integer, options.clone()).is_err());
    assert!(serde_saphyr::from_str_with_options::<u8>(&integer, options.clone()).is_err());
    assert!(serde_saphyr::from_str_with_options::<u16>(&integer, options.clone()).is_err());
    assert!(serde_saphyr::from_str_with_options::<u32>(&integer, options.clone()).is_err());
    assert!(serde_saphyr::from_str_with_options::<u64>(&integer, options.clone()).is_err());
    assert!(serde_saphyr::from_str_with_options::<u128>(&integer, options.clone()).is_err());

    let boolean = format!("{tag} false");
    assert!(serde_saphyr::from_str_with_options::<bool>(&boolean, options).is_err());
}

#[rstest]
fn disabled_semantic_tags_fail_in_typed_skipped_and_flattened_fields(
    #[values("!!timestamp", "!time", "!degrees", "!radians")] tag: &str,
) {
    let yaml = format!("value: {tag} 10\n");
    assert!(serde_saphyr::from_str::<Field<u32>>(&yaml).is_err());
    assert!(serde_saphyr::from_str::<Field<f32>>(&yaml).is_err());
    assert!(serde_saphyr::from_str::<Field<f64>>(&yaml).is_err());
    assert!(serde_saphyr::from_str::<Skipped>(&yaml).is_err());
    assert!(serde_saphyr::from_str::<IgnoredAny>(&yaml).is_err());
    assert!(serde_saphyr::from_str::<Flattened<f64>>(&yaml).is_err());
    assert!(serde_saphyr::from_reader::<_, Field<u32>>(yaml.as_bytes()).is_err());
}

#[cfg(not(feature = "robotics"))]
#[rstest]
fn angle_option_requires_compiled_robotics_support(
    #[values("!time 10", "!degrees 180", "!radians 0.5")] yaml: &str,
) {
    let options = serde_saphyr::options! { angle_conversions: true };
    assert!(serde_saphyr::from_str_with_options::<f64>(yaml, options.clone()).is_err());
    assert!(serde_saphyr::from_str_with_options::<Value>(yaml, options).is_err());
}

#[rstest]
fn timestamp_is_not_a_numeric_robotics_time(
    #[values("!!timestamp 10", "!!timestamp 01:02:03", "!!timestamp 2026-10-20")] yaml: &str,
) {
    let options = serde_saphyr::options! { angle_conversions: true };
    assert!(serde_saphyr::from_str_with_options::<f32>(yaml, options.clone()).is_err());
    assert!(serde_saphyr::from_str_with_options::<f64>(yaml, options.clone()).is_err());
    assert!(serde_saphyr::from_str_with_options::<Value>(yaml, options).is_err());
}

#[rstest]
fn semantic_tags_do_not_become_null(
    #[values("!!timestamp", "!time", "!degrees", "!radians")] tag: &str,
    #[values("null", "~", "")] payload: &str,
) {
    let yaml = format!("{tag} {payload}");
    let options = serde_saphyr::options! { angle_conversions: true };
    assert!(serde_saphyr::from_str_with_options::<Option<f64>>(&yaml, options.clone()).is_err());
    assert!(serde_saphyr::from_str_with_options::<Option<Value>>(&yaml, options.clone()).is_err());
    assert!(serde_saphyr::from_str_with_options::<()>(&yaml, options).is_err());
}

#[rstest]
fn incompatible_tags_do_not_become_characters(
    #[values("!!int v", "!!bool v", "!!timestamp v", "!time v", "!degrees v", "!radians v")] yaml: &str,
) {
    assert!(serde_saphyr::from_str::<char>(yaml).is_err());
}

#[rstest]
#[case::plain("v")]
#[case::string_tag("!!str v")]
#[case::non_specific_tag("! v")]
fn string_scalars_remain_valid_characters(#[case] yaml: &str) {
    assert_eq!(serde_saphyr::from_str::<char>(yaml).unwrap(), 'v');
}

#[rstest]
fn enum_variant_names_validate_tags(
    #[values("!!int", "!!bool", "!!float", "!!timestamp", "!time", "!degrees", "!radians")] tag: &str,
) {
    let scalar = format!("{tag} vast");
    assert!(serde_saphyr::from_str::<Protocol>(&scalar).is_err());
    assert!(serde_saphyr::from_str::<IgnoredAny>(&scalar).is_err());

    let field = format!("value: {scalar}\n");
    assert!(serde_saphyr::from_str::<Field<Protocol>>(&field).is_err());
    assert!(serde_saphyr::from_str::<Skipped>(&field).is_err());
    assert!(serde_saphyr::from_str::<Flattened<Protocol>>(&field).is_err());

    let mapping = format!("{{{tag} vast: null}}");
    assert!(serde_saphyr::from_str::<Protocol>(&mapping).is_err());
}

#[rstest]
#[case::plain("vast")]
#[case::string_tag("!!str vast")]
#[case::non_specific_tag("! vast")]
#[case::variant_tag("!vast null")]
#[case::type_tag("!!Protocol vast")]
#[case::mapping("{vast: null}")]
#[case::mapping_string_tag("{!!str vast: null}")]
fn valid_enum_representations_remain_supported(#[case] yaml: &str) {
    assert_eq!(
        serde_saphyr::from_str::<Protocol>(yaml).unwrap(),
        Protocol::Vast
    );
}

#[test]
fn numeric_enum_uses_its_integer_representation() {
    #[derive(Debug, PartialEq, serde_repr::Deserialize_repr)]
    #[repr(u8)]
    enum Numeric {
        Ten = 10,
    }

    assert_eq!(
        serde_saphyr::from_str::<Numeric>("!!int 10").unwrap(),
        Numeric::Ten
    );
}

#[cfg(feature = "robotics")]
#[rstest]
#[case::degrees("!degrees 180", std::f64::consts::PI)]
#[case::radians("!radians 0.5", 0.5)]
#[case::time("!time 01:02:03", 3723.0)]
#[case::quoted_degrees("!degrees '180'", std::f64::consts::PI)]
#[case::time_alias("!<tag:yaml.org,2002:time> 01:02:03", 3723.0)]
fn robotics_tags_agree_across_typed_skipped_and_buffered_reads(
    #[case] scalar: &str,
    #[case] expected: f64,
) {
    let options = serde_saphyr::options! { angle_conversions: true };
    let typed: f64 = serde_saphyr::from_str_with_options(scalar, options.clone()).unwrap();
    assert!((typed - expected).abs() < 1e-12);
    let typed: f32 = serde_saphyr::from_str_with_options(scalar, options.clone()).unwrap();
    assert!((f64::from(typed) - expected).abs() < 1e-5);
    let generic: Value = serde_saphyr::from_str_with_options(scalar, options.clone()).unwrap();
    assert!((generic.as_f64().unwrap() - expected).abs() < 1e-12);

    let yaml = format!("value: {scalar}\n");
    let typed: Field<f64> =
        serde_saphyr::from_reader_with_options(yaml.as_bytes(), options.clone()).unwrap();
    assert!((typed.value - expected).abs() < 1e-12);
    let flattened: Flattened<f64> =
        serde_saphyr::from_str_with_options(&yaml, options.clone()).unwrap();
    assert!((flattened.values["value"] - expected).abs() < 1e-12);
    serde_saphyr::from_str_with_options::<Skipped>(&yaml, options.clone()).unwrap();
    serde_saphyr::from_str_with_options::<IgnoredAny>(&yaml, options).unwrap();
}

#[cfg(feature = "robotics")]
#[test]
fn robotics_tags_survive_alias_replay() {
    let options = serde_saphyr::options! { angle_conversions: true };
    let yaml = "original: &angle !degrees 180\ncopy: *angle\n";
    let flattened: Flattened<f64> =
        serde_saphyr::from_str_with_options(yaml, options.clone()).unwrap();
    for key in ["original", "copy"] {
        assert!((flattened.values[key] - std::f64::consts::PI).abs() < 1e-12);
    }
    serde_saphyr::from_reader_with_options::<_, Skipped>(yaml.as_bytes(), options).unwrap();
}

#[cfg(feature = "robotics")]
#[rstest]
#[case::infinity("!radians .inf", ".inf")]
#[case::nan("!degrees .nan", ".nan")]
fn robotics_tags_respect_typeless_non_finite_policy(
    #[case] scalar: &str,
    #[case] canonical: &str,
) {
    use serde_saphyr::NonFiniteFloatPolicy;

    let options = serde_saphyr::options! {
        angle_conversions: true,
        non_finite_float_policy: NonFiniteFloatPolicy::Reject,
    };
    let error = serde_saphyr::from_str_with_options::<Value>(scalar, options).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        serde_saphyr::Error::NonFiniteFloat { .. }
    ));

    let options = serde_saphyr::options! {
        angle_conversions: true,
        non_finite_float_policy: NonFiniteFloatPolicy::AsString,
    };
    let value: Value = serde_saphyr::from_str_with_options(scalar, options).unwrap();
    assert_eq!(value, Value::String(canonical.to_owned()));

    let options = serde_saphyr::options! {
        angle_conversions: true,
        non_finite_float_policy: NonFiniteFloatPolicy::PassThrough,
    };
    let yaml = format!("value: {scalar}\n");
    let value: Flattened<f64> = serde_saphyr::from_str_with_options(&yaml, options).unwrap();
    if canonical == ".nan" {
        assert!(value.values["value"].is_nan());
    } else {
        assert_eq!(value.values["value"], f64::INFINITY);
    }
}
