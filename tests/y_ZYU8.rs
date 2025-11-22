// ZYU8: Directive variants
// The suite notes these are valid per 1.2 productions but "not usefully valid" and are skipped.
// We'll implement the first case only: "%YAML1.1" followed by an empty document `---`.
// Expected JSON is `null`, so deserializing into Option<T> should yield None.

// In YAML 1.2, a directive name is any non‑space, non‑line‑break sequence of characters
// Serde-saphyr expects only alphabetic characters in a directive name, dot . triggers the error.
// serde-saphyr does not support lots of directives anyway.
#[ignore]
#[test]
fn yaml_zyu8_directive_variant_yaml11_null_document() -> anyhow::Result<()>{
    let y = "%YAML1.1\n---\n";
    let v: Option<i32> = serde_saphyr::from_str(y)?;
    assert!(v.is_none(), "Expected null document to deserialize to None");
    Ok(())
}

#[test]
fn yaml_zyu8_directive_variant_spaced_yaml11_null_document() -> anyhow::Result<()> {
    // With space is fine.
    let y = "%YAML 1.1\n---\n";
    let v: Option<i32> = serde_saphyr::from_str(y)?;
    assert!(v.is_none(), "Expected null document to deserialize to None");
    Ok(())
}
