// U44R: Bad indentation in mapping (2)
// This YAML should be invalid due to mis-indented key2.

#[test]
fn yaml_u44r_bad_indentation_mapping() {
    let y = "map:\n  key1: \"quoted1\"\n   key2: \"bad indentation\"\n";
    let r: Result<serde_json::Value, _> = serde_saphyr::from_str(y);
    assert!(r.is_err(), "Parser unexpectedly accepted bad-indented mapping in U44R: {:?}", r);
}
