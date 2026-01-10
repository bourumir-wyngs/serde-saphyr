use anyhow::Result;

#[test]
fn test_empty_array() -> Result<()> {
    let content = "\
key: [
]
";
    let value: serde_json::Value = serde_saphyr::from_str(content)?;
    assert_eq!(value["key"], serde_json::json!([]));
    Ok(())
}

#[test]
fn test_array_with_values() -> Result<()> {
    let content = "\
key: [
  1,
  2,
  3
]
";
    let value: serde_json::Value = serde_saphyr::from_str(content)?;
    assert_eq!(value["key"], serde_json::json!([1, 2, 3]));
    Ok(())
}

#[test]
fn test_array_with_values_compliant() -> Result<()> {
    let content = "\
key: [
  1,
  2,
  3
 ]
";
    let value: serde_json::Value = serde_saphyr::from_str(content)?;
    assert_eq!(value["key"], serde_json::json!([1, 2, 3]));
    Ok(())
}
