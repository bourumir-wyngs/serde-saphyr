use std::collections::HashMap;

// NHX8: Empty Lines at End of Document — mapping with empty key ("") and empty value (null)

// ":" alone is not a valid YAML mapping entry with an empty key.

#[test]
fn yaml_nhx8_empty_lines_at_end_of_document() {
    // Empty key must be quoted as "" to be a valid empty string.
    // Empty value after ":" is YAML null.
    let y = r#""":


"#;

    let v: HashMap<String, Option<String>> =
        serde_saphyr::from_str(y).expect("failed to parse NHX8");

    assert_eq!(v.len(), 1);
    assert!(v.contains_key(""));
    assert_eq!(v.get("").unwrap().as_ref(), None);
}
