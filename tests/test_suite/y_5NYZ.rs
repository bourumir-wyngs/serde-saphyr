use std::collections::HashMap;

// 5NYZ: Separated Comment — mapping key with inline comment
#[test]
fn yaml_5nyz_separated_comment() {
    let y = "key:    # Comment\n  value\n";
    let m: HashMap<String, String> = serde_saphyr::from_str(y).expect("failed to parse 5NYZ");
    assert_eq!(m.get("key").map(String::as_str), Some("value"));
}
