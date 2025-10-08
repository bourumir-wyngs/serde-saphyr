use std::collections::HashMap;

// AZW3: Lookahead test cases — mapping keys contain '"' and ']' characters
#[test]
fn yaml_azw3_lookahead_keys_with_special_chars() {
    let y = "- bla\"keks: foo\n- bla]keks: foo\n";
    let v: Vec<HashMap<String, String>> = serde_saphyr::from_str(y).expect("failed to parse AZW3");
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].get("bla\"keks").map(String::as_str), Some("foo"));
    assert_eq!(v[1].get("bla]keks").map(String::as_str), Some("foo"));
}