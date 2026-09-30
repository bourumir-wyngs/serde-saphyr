#![cfg(all(feature = "serialize", feature = "deserialize"))]
use anyhow::Result;

#[test]
fn test_empty_array() -> Result<()> {
    let content = "\
key: [
]
";
    let value: serde_json::Value = serde_saphyr::from_str(content)?;
    assert_eq!(value["key"], serde_json::json!([]));
    let strict = serde_saphyr::from_str_with_options::<serde_json::Value>(
        content,
        serde_saphyr::options! { strict_indentation: true },
    );
    assert!(
        strict.is_err(),
        "strict mode accepted an unindented closing ]"
    );
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
    let strict = serde_saphyr::from_str_with_options::<serde_json::Value>(
        content,
        serde_saphyr::options! { strict_indentation: true },
    );
    assert!(
        strict.is_err(),
        "strict mode accepted an unindented closing ]"
    );
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
    let strict: serde_json::Value = serde_saphyr::from_str_with_options(
        content,
        serde_saphyr::options! { strict_indentation: true },
    )?;
    assert_eq!(strict, value);
    Ok(())
}
