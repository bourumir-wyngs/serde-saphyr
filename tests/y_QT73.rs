use serde_json::Value;

#[test]
fn y_qt73() {
    // YAML under test is just a comment and a document-end marker ("...")
    // This represents an empty stream (no documents). from_str::<Value>()
    // should serialize into Null
    let yaml = r#"# comment
..."#;

    let res: Result<Value, _> = serde_saphyr::from_str(yaml);
    match res {
        Ok(val) => assert_eq!(val, Value::Null, "Empty stream should deserialize into JSON null"),
        Err(e) => panic!("Expected Ok(Value::Null) for empty stream, got error: {}", e),
    }
}
