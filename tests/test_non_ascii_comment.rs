use serde_json::json;

#[test]
fn test_non_ascii_comment_middle() {
    let yaml_str = "
a1:
  b: 1
# A \u{AC00}
a2:
  b: 2
";
    let yaml_value: serde_json::Value = serde_saphyr::from_str(yaml_str)
        .unwrap_or_else(|e| panic!("{}", e));

    let expected = json!({
        "a1": { "b": 1 },
        "a2": { "b": 2 }
    });

    assert_eq!(yaml_value, expected);
}

#[test]
fn test_non_ascii_comment_start() {
    // Non-ASCII character '가' as is, before the map
    let yaml_str = "
# A \u{AC00}
    a1:
        b: 1
    a2:
        b: 2
    ";

    let yaml_value: serde_json::Value = serde_saphyr::from_str(yaml_str)
        .unwrap_or_else(|e| panic!("{}", e));

    let expected = json!({
        "a1": { "b": 1 },
        "a2": { "b": 2 }
    });

    assert_eq!(yaml_value, expected);
}