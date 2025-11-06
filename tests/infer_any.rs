use serde_json::Value;

#[test]
fn infer_numbers_and_bools_into_json_value() {
    // unquoted numbers become JSON numbers
    let v: Value = serde_saphyr::from_str("30").expect("parse 30");
    assert!(v.is_number());
    assert_eq!(v, Value::from(30));

    // floats become numbers
    let v: Value = serde_saphyr::from_str("1.5").expect("parse float");
    assert!(v.is_number());

    // YAML 1.1 booleans
    let v: Value = serde_saphyr::from_str("Yes").expect("parse yes");
    assert_eq!(v, Value::Bool(true));
    let v: Value = serde_saphyr::from_str("off").expect("parse off");
    assert_eq!(v, Value::Bool(false));
}

#[test]
fn quoted_scalars_remain_strings_in_json_value() {
    let v: Value = serde_saphyr::from_str("\"30\"").expect("parse quoted 30");
    assert_eq!(v, Value::String("30".to_owned()));

    let v: Value = serde_saphyr::from_str("\"null\"").expect("parse quoted null");
    assert_eq!(v, Value::String("null".to_owned()));
}

#[test]
fn nullish_literals_remain_strings_in_json_value() {
    let v: Value = serde_saphyr::from_str("~").expect("parse ~");
    assert_eq!(v, Value::String("~".to_owned()));

    let v: Value = serde_saphyr::from_str("null").expect("parse null");
    assert_eq!(v, Value::Null);
}

#[test]
fn binary_decodes_to_string_when_valid_utf8() {
    let v: Value = serde_saphyr::from_str("!!binary aGVsbG8=").expect("parse !!binary hello");
    assert_eq!(v, Value::String("hello".to_owned()));
}

#[test]
fn tagged_null_yields_json_null() {
    let v: Value = serde_saphyr::from_str("!!null").expect("parse !!null");
    assert!(v.is_null(), "expected JSON null, got: {:?}", v);
}
