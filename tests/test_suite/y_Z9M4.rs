// Z9M4: Global Tag Prefix via %TAG directive and a shorthand tag on a scalar.
// The value should deserialize as a plain string ignoring the tag semantics.
// YAML:
// %TAG !e! tag:example.com,2000:app/
// ---
// - !e!foo "bar"

#[test]
fn yaml_z9m4_global_tag_prefix() -> anyhow::Result<()> {
    let y = r#"%TAG !e! tag:example.com,2000:app/
---
- !e!foo "bar"
"#;

    let v: Vec<String> = serde_saphyr::from_str(y)?;
    assert_eq!(v, vec!["bar".to_string()]);
    Ok(())
}
