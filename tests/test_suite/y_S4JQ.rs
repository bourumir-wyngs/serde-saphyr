use serde_json::Value;

// S4JQ: Non-Specific Tags
// Input:
// - "12"  => string
// - 12     => number
// - ! 12   => non-specific tag, treated as string per test-suite json
#[test]
fn yaml_s4jq_non_specific_tags_types() {
    // The non-specific tag `!` must force scalar to be treated as string.
    let y = "- \"12\"\n- 12\n- ! 12\n";
    let v: Value = serde_saphyr::from_str(y).unwrap();
    let a = v.as_array().expect("root not a sequence");
    assert_eq!(a.len(), 3);

    assert_eq!(a[0], Value::String("12".into()));
    assert!(
        a[1].is_number(),
        "second element should be a number, got {:?}",
        a[1]
    );
    assert_eq!(a[2], Value::String("12".into()));
}
