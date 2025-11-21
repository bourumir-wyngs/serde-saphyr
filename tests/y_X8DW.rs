use std::collections::HashMap;

// X8DW: Explicit key and value separated by a comment line
// YAML builds a simple mapping: { key: value }
#[test]
fn yaml_x8dw_explicit_key_with_intervening_comment() {
    let y = r#"---
? key
# comment
: value
"#;

    let m: HashMap<String, String> = serde_saphyr::from_str(y)
        .expect("X8DW should parse: explicit key with comment between key and value");

    assert_eq!(m.len(), 1);
    assert_eq!(m.get("key"), Some(&"value".to_string()));
}
