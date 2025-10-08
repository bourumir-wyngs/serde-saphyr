use std::collections::BTreeMap;

// Q5MG: Tab at beginning of line followed by a flow mapping
#[test]
fn yaml_q5mg_tab_followed_by_flow_mapping() {
    let y = "\t{}\n";
    let m: BTreeMap<String, String> = serde_saphyr::from_str(&y)
        .unwrap_or_else(|e| panic!("parser rejected leading tab before flow mapping (Q5MG): {e}"));
    assert!(m.is_empty(), "expected empty mapping");
}
