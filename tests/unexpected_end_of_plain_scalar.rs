
#[test]
fn test_unexpected_end_of_plain_scalar() -> anyhow::Result<()>{
    let yaml = r#"
hello:
  world: this is a string
    --- still a string
"#;

    let parsed_yaml: serde_json::Value = serde_saphyr::from_str(yaml)?;
    assert_eq!(parsed_yaml, serde_json::json!({"hello":{"world":"this is a string --- still a string"}}));

    Ok(())
}