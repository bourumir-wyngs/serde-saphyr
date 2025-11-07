use serde_json::Value;

// S3PD: Implicit Block Mapping Entries
// Expect mapping with entries:
// - "plain key": "in-line value"
// - "" (empty key): null (empty value -> JSON null). Empty key must be quoted is serde-saphyr.
// - "quoted key": ["entry"]
#[test]
fn yaml_s3pd_implicit_block_mapping_entries() {
    let y = "plain key: in-line value\n\"\": \n\"quoted key\":\n- entry\n";

    let v: Value = serde_saphyr::from_str(y).unwrap();
    let m = v.as_object().expect("root is not mapping");

    assert_eq!(m.get("plain key").unwrap(), &Value::String("in-line value".into()));

    // Empty string key with empty value should deserialize to JSON null.
    assert!(m.contains_key(""));
    assert_eq!(m.get("").unwrap(), &Value::Null);

    let q = m.get("quoted key").expect("missing quoted key").as_array().unwrap();
    assert_eq!(q, &vec![Value::String("entry".into())]);
}
