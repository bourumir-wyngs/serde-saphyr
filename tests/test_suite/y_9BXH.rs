use std::collections::HashMap;

// 9BXH: Multiline doublequoted flow mapping key without value
// Expect a sequence of two maps; each has a key (possibly multi-line quoted)
// with a null value, and another pair a: b.
#[test]
fn yaml_9bxh_multiline_doublequoted_flow_mapping_key_without_value() {
    let y = "---\n- { \"single line\", a: b}\n- { \"multi\n  line\", a: b}\n";
    let v: Vec<HashMap<String, Option<String>>> =
        serde_saphyr::from_str(y).expect("failed to parse 9BXH");
    assert_eq!(v.len(), 2);
    // First map
    let m0 = &v[0];
    assert!(m0.contains_key("single line"));
    assert_eq!(m0.get("single line").and_then(|x| x.clone()), None);
    assert_eq!(m0.get("a").cloned().flatten().as_deref(), Some("b"));
    // Second map
    let m1 = &v[1];
    assert!(m1.contains_key("multi line"));
    assert_eq!(m1.get("multi line").and_then(|x| x.clone()), None);
    assert_eq!(m1.get("a").cloned().flatten().as_deref(), Some("b"));
}
