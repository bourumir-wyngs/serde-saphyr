use std::collections::HashMap;

// 6KGN: Anchor for empty node — both a and b become null
#[test]
fn yaml_6kgn_anchor_for_empty_node() {
    let y = "---\na: &anchor\nb: *anchor\n";
    let m: HashMap<String, Option<String>> = serde_saphyr::from_str(y).expect("failed to parse 6KGN");
    assert_eq!(m.get("a").and_then(|v| v.clone()), None);
    assert_eq!(m.get("b").and_then(|v| v.clone()), None);
    assert_eq!(m.len(), 2);
}
