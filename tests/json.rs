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

#[test]
fn valid_json() {
    // Claimed to be "Valid JSON that would not parse as YAML"
    let yaml = "{\n\t\"abc\":\"xyz\"\n}";
    let value = serde_saphyr::from_str::<serde_json::Value>(yaml).unwrap();
    assert_eq!(value["abc"], "xyz");
}

#[test]
fn valid_json_simple_object() {
    let value = serde_saphyr::from_str::<serde_json::Value>("{\"a\":1}").unwrap();
    assert_eq!(value["a"], 1);
}

#[test]
fn valid_json_array_value() {
    let value = serde_saphyr::from_str::<serde_json::Value>("{\"a\":[1,2,3]}").unwrap();
    assert_eq!(value["a"][0], 1);
    assert_eq!(value["a"][1], 2);
    assert_eq!(value["a"][2], 3);
}

#[test]
fn valid_json_url_string() {
    let value =
        serde_saphyr::from_str::<serde_json::Value>("{\"a\":\"http://example.org/\"}").unwrap();
    assert_eq!(value["a"], "http://example.org/");
}

#[test]
fn valid_json_u2028_u2029_escapes() {
    let value =
        serde_saphyr::from_str::<serde_json::Value>("{\"a\":\"\\u2028\",\"b\":\"\\u2029\"}")
            .unwrap();
    assert_eq!(value["a"], "\u{2028}");
    assert_eq!(value["b"], "\u{2029}");
}

#[test]
fn valid_json_numeric_values() {
    let value = serde_saphyr::from_str::<serde_json::Value>("{\"a\":-1,\"b\":0.25,\"c\":1e2}")
        .unwrap();
    assert_eq!(value["a"], -1);
    assert_eq!(value["b"], 0.25);
    assert_eq!(value["c"], 100.0);
}

#[test]
fn valid_json_boolean_and_null_values() {
    let value = serde_saphyr::from_str::<serde_json::Value>("{\"a\":true,\"b\":false,\"c\":null}")
        .unwrap();
    assert_eq!(value["a"], true);
    assert_eq!(value["b"], false);
    assert_eq!(value["c"], serde_json::Value::Null);
}