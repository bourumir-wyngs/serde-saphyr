#![cfg(all(feature = "serialize", feature = "deserialize"))]
use rstest::rstest;
use serde_saphyr as yaml;
use serde_saphyr::Error;
use std::collections::BTreeMap;
use std::fmt::Debug;

#[test]
fn binary_tag_to_string_when_utf8() {
    let s: String = yaml::from_str("!!binary aGVsbG8=").expect("valid UTF-8 binary to string");
    assert_eq!(s, "hello");
}

#[test]
fn binary_tag_invalid_utf8_errors_for_string() {
    // 0xFF is invalid in UTF-8
    let err = yaml::from_str::<String>("!!binary /w==").expect_err("invalid UTF-8 should error");
    assert!(matches!(err.without_snippet(), Error::BinaryNotUtf8 { .. }));
}

#[test]
fn tagged_int_cannot_parse_into_string() {
    let err =
        yaml::from_str::<String>("!!int 42").expect_err("!!int should not deserialize into String");
    assert!(matches!(
        err.without_snippet(),
        Error::TaggedScalarCannotDeserializeIntoString { .. }
    ));
}

#[test]
fn tagged_bool_cannot_parse_into_string() {
    let err = yaml::from_str::<String>("!!bool false")
        .expect_err("!!int should not deserialize into String");
    assert!(matches!(
        err.without_snippet(),
        Error::TaggedScalarCannotDeserializeIntoString { .. }
    ));
}

#[test]
fn tagged_int_cannot_parse_into_boolean() {
    let err = yaml::from_str::<bool>("!!int true")
        .expect_err("!!int should not deserialize into bool even when its text is boolean");
    assert!(matches!(
        err.without_snippet(),
        Error::InvalidScalar { ty: "boolean", .. }
    ));
}

#[track_caller]
fn assert_tagged_string_cannot_parse_into_integer<T>()
where
    T: serde::de::DeserializeOwned + Debug,
{
    let err = yaml::from_str::<T>("!!str 42")
        .expect_err("!!str should not deserialize into an integer even when its text is numeric");
    assert!(matches!(err.without_snippet(), Error::InvalidScalar { .. }));
}

#[test]
fn tagged_string_cannot_parse_into_any_integer_type() {
    assert_tagged_string_cannot_parse_into_integer::<i8>();
    assert_tagged_string_cannot_parse_into_integer::<i16>();
    assert_tagged_string_cannot_parse_into_integer::<i32>();
    assert_tagged_string_cannot_parse_into_integer::<i64>();
    assert_tagged_string_cannot_parse_into_integer::<i128>();
    assert_tagged_string_cannot_parse_into_integer::<u8>();
    assert_tagged_string_cannot_parse_into_integer::<u16>();
    assert_tagged_string_cannot_parse_into_integer::<u32>();
    assert_tagged_string_cannot_parse_into_integer::<u64>();
    assert_tagged_string_cannot_parse_into_integer::<u128>();
}

#[rstest]
#[case::string("!!str 1.5")]
#[case::quoted_string("!!str '1.5'")]
#[case::block_string("!!str |-\n  1.5\n")]
#[case::verbatim_string("!<tag:yaml.org,2002:str> 1.5")]
#[case::integer("!!int 42")]
#[case::boolean("!!bool 1.5")]
#[case::null("!!null 1.5")]
fn incompatible_core_tags_cannot_parse_into_floats(#[case] yaml: &str) {
    let f32_error = yaml::from_str::<f32>(yaml).unwrap_err();
    let f64_error = yaml::from_str::<f64>(yaml).unwrap_err();
    for error in [f32_error, f64_error] {
        assert!(
            matches!(
                error.without_snippet(),
                Error::InvalidScalar {
                    ty: "floating point",
                    ..
                }
            ),
            "yaml: {yaml}, error: {error}"
        );
    }
}

#[rstest]
#[case::plain("1.5", 1.5)]
#[case::quoted("'1.5'", 1.5)]
#[case::plain_integer("42", 42.0)]
#[case::tagged_float("!!float 1.5", 1.5)]
#[case::integer_looking_float("!!float 42", 42.0)]
#[case::quoted_float("!!float '1.5'", 1.5)]
#[case::block_float("!!float >-\n  1.5\n", 1.5)]
#[case::custom_tag("!measurement 1.5", 1.5)]
fn compatible_scalars_can_parse_into_floats(#[case] yaml: &str, #[case] expected: f64) {
    assert_eq!(f64::from(yaml::from_str::<f32>(yaml).unwrap()), expected);
    assert_eq!(yaml::from_str::<f64>(yaml).unwrap(), expected);
}

#[test]
fn tagged_bool_can_parse_into_boolean() {
    let bool = yaml::from_str::<bool>("!!bool true").expect("!!bool should deserialize into bool");
    assert!(bool);
}

#[test]
fn tagged_int_can_parse_into_int() {
    let v: i32 = yaml::from_str::<i32>("!!int 42").expect("!!ing should deeserialize into int");
    assert_eq!(42, v);
}

#[test]
fn tagged_null_is_none_for_option_string() {
    let v: Option<String> = yaml::from_str("!!null").expect("parse !!null");
    assert!(v.is_none());
}

#[test]
fn value_tag_is_accepted_and_ignored() {
    let documents = [
        "!!value '='",
        "!<tag:yaml.org,2002:value> '='",
        "%TAG !v! tag:yaml.org,2002:\n--- !v!value '='",
    ];

    for document in documents {
        let value: String = yaml::from_str(document).expect("!!value must be string-compatible");
        assert_eq!(value, "=");
    }

    let mapping: BTreeMap<String, String> =
        yaml::from_str("!!value =: library.dll\nversion: 1.2\n")
            .expect("!!value mapping key must remain ordinary data");
    assert_eq!(mapping.get("=").map(String::as_str), Some("library.dll"));
    assert_eq!(mapping.get("version").map(String::as_str), Some("1.2"));
}
