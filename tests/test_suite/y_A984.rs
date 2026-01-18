use std::collections::HashMap;

// A984: Multiline Scalar in Mapping — folded values
#[test]
fn yaml_a984_multiline_scalar_in_mapping() {
    let y = "a: b\n c\nd:\n e\n  f\n";
    let m: HashMap<String, String> = serde_saphyr::from_str(y).expect("failed to parse A984");
    assert_eq!(m.get("a").map(String::as_str), Some("b c"));
    assert_eq!(m.get("d").map(String::as_str), Some("e f"));
}
