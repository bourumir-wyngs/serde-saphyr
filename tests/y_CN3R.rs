use std::collections::HashMap;

// CN3R: Various location of anchors in flow sequence — a sequence of mappings
#[test]
fn yaml_cn3r_various_anchor_locations_in_flow_sequence() {
    let y = "&flowseq [\n a: b,\n &c c: d,\n { &e e: f },\n &g { g: h }\n]\n";
    let v: Vec<HashMap<String, String>> = serde_saphyr::from_str(y).expect("failed to parse CN3R");
    assert_eq!(v.len(), 4);
    assert_eq!(v[0].get("a").map(String::as_str), Some("b"));
    assert_eq!(v[1].get("c").map(String::as_str), Some("d"));
    assert_eq!(v[2].get("e").map(String::as_str), Some("f"));
    assert_eq!(v[3].get("g").map(String::as_str), Some("h"));
}
