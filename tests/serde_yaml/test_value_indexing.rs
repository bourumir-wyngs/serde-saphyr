use serde_saphyr::Value;

#[test]
fn sequence_indexing() {
    let yaml = "- a\n- b";
    let value: Value = serde_saphyr::from_str(yaml).unwrap();

    assert_eq!(value[0], Value::from("a"));
    assert_eq!(value[2], Value::Null(None));
}

#[test]
fn mapping_indexing() {
    let yaml = "k: v";
    let value: Value = serde_saphyr::from_str(yaml).unwrap();

    assert_eq!(value["k"], Value::from("v"));
    assert_eq!(value["missing"], Value::Null(None));
    assert_eq!(value[0], Value::Null(None));
}

#[test]
fn nested_indexing() {
    let yaml = "a:\n  - b\n  - c";
    let value: Value = serde_saphyr::from_str(yaml).unwrap();

    assert_eq!(value["a"][0], Value::from("b"));
    assert_eq!(value["a"][5], Value::Null(None));
    assert_eq!(value["a"]["missing"], Value::Null(None));
    assert_eq!(value["missing"][0], Value::Null(None));
}
