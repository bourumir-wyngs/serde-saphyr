use std::collections::HashMap;

// 6SLA: Allowed characters in quoted mapping key
#[test]
fn yaml_6sla_allowed_chars_in_quoted_mapping_key() {
    let y = "\"foo\\nbar:baz\\tx \\\\$%^&*()x\": 23\n'x\\ny:z\\tx $%^&*()x': 24\n";
    let m: HashMap<String, i32> = serde_saphyr::from_str(y).expect("failed to parse 6SLA");
    assert_eq!(m.get("foo\nbar:baz\tx \\$%^&*()x"), Some(&23));
    assert_eq!(m.get("x\\ny:z\\tx $%^&*()x"), Some(&24));
    assert_eq!(m.len(), 2);
}
