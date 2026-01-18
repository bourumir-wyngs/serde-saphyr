use std::collections::HashMap;

#[test]
fn yaml_2sxe_anchors_with_colon_in_name_parse_mapping() {
    let yaml = "&a: key: &a value\nfoo:\n  *a:\n";

    let m: HashMap<String, String> =
        serde_saphyr::from_str(yaml).expect("failed to parse 2SXE mapping");
    assert_eq!(m.get("key").map(String::as_str), Some("value"));
    assert_eq!(m.get("foo").map(String::as_str), Some("key"));
    assert_eq!(m.len(), 2);
}
