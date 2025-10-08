use std::collections::HashMap;

// 3UYS: Escaped slash in double quotes
// Expect: key "escaped slash" -> value "a/b"
#[test]
fn yaml_3uys_escaped_slash_in_double_quotes() {
    let y = "escaped slash: \"a\\/b\"\n";
    let map: HashMap<String, String> = serde_saphyr::from_str(y).expect("failed to parse 3UYS");
    assert_eq!(map.get("escaped slash").map(String::as_str), Some("a/b"));
    assert_eq!(map.len(), 1);
}
