// ZYU8: Directive variants
use super::yaml_suite_support::assert_json_case;

#[test]
fn yaml_zyu8_directive_variant_yaml11_null_document() {
    assert_json_case("%YAML1.1\n---\n", "null\n");
}

#[test]
fn yaml_zyu8_punctuation_directive_null_document() {
    assert_json_case("%***\n---\n", "null\n");
}

#[test]
fn yaml_zyu8_large_minor_version_null_document() {
    assert_json_case("%YAML 1.12345\n---\n", "null\n");
}

#[test]
fn yaml_zyu8_directive_variant_spaced_yaml11_null_document() -> anyhow::Result<()> {
    // Additional control using the conventional YAML 1.1 directive form.
    let y = "%YAML 1.1\n---\n";
    let v: Option<i32> = serde_saphyr::from_str(y)?;
    assert!(v.is_none(), "Expected null document to deserialize to None");
    Ok(())
}
