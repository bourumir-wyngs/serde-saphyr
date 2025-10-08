use std::collections::HashMap;

// CT4Q: Single Pair Explicit Entry — sequence containing one mapping {"foo bar": "baz"}
#[test]
fn yaml_ct4q_single_pair_explicit_entry() {
    let y = "[\n? foo\n bar : baz\n]\n";
    let v: Vec<HashMap<String, String>> = serde_saphyr::from_str(y).expect("failed to parse CT4Q");
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].get("foo bar").map(String::as_str), Some("baz"));
}
