use serde_saphyr::Value;

#[test]
fn test_value_index_returns_null() {
    let value: Value = serde_saphyr::from_str("{a: {b: 1}}" ).unwrap();
    assert_eq!(value["a"]["c"], Value::Null(None));
    assert_eq!(value["x"][0], Value::Null(None));
}
