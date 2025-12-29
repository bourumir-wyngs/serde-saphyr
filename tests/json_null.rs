#[test]
fn json_null() {
    let original: Option<String> = None;
    let serialized = serde_saphyr::to_string(&original).unwrap();
    let into_option: Option<String> = serde_saphyr::from_str(&serialized).unwrap();
    let into_json_value: serde_json::Value = serde_saphyr::from_str(&serialized).unwrap();

    assert_eq!(serialized, "null\n");
    assert_eq!(into_option, None);
    assert_eq!(into_json_value, serde_json::Value::Null);
}
