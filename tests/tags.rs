use serde_saphyr as yaml;

#[test]
fn binary_tag_to_string_when_utf8() {
    let s: String = yaml::from_str("!!binary aGVsbG8=").expect("valid UTF-8 binary to string");
    assert_eq!(s, "hello");
}

#[test]
fn binary_tag_invalid_utf8_errors_for_string() {
    // 0xFF is invalid in UTF-8
    let err = yaml::from_str::<String>("!!binary /w==").expect_err("invalid UTF-8 should error");
    let msg = format!("{}", err);
    assert!(
        msg.contains("!!binary scalar is not valid UTF-8"),
        "unexpected error: {msg}"
    );
}

#[test]
fn tagged_int_cannot_parse_into_string() {
    let err =
        yaml::from_str::<String>("!!int 42").expect_err("!!int should not deserialize into String");
    let msg = format!("{}", err);
    assert!(
        msg.contains("cannot deserialize scalar tagged"),
        "unexpected error: {msg}"
    );
}

#[test]
fn tagged_bool_cannot_parse_into_string() {
    let err = yaml::from_str::<String>("!!bool false")
        .expect_err("!!int should not deserialize into String");
    let msg = format!("{}", err);
    assert!(
        msg.contains("cannot deserialize scalar tagged"),
        "unexpected error: {msg}"
    );
}

#[test]
fn tagged_bool_cannot_parse_into_int() {
    let _err =
        yaml::from_str::<i32>("!!bool false").expect_err("!!bool should not deserialize into int");
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
