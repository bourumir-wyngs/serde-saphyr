use serde_json::Value;

// S7BG: Colon followed by comma
// YAML: sequence with a single scalar value ":,"
#[test]
fn yaml_s7bg_colon_followed_by_comma_scalar() {
    let y = "---\n- :,\n";
    let v: Value = serde_saphyr::from_str(y).unwrap();
    let a = v.as_array().expect("root not a sequence");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0], Value::String(":,".into()));
}
