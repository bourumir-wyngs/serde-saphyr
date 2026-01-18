use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    sequence: Vec<String>,
    mapping: std::collections::BTreeMap<String, String>,
}

// S9E8: Block Structure Indicators
#[test]
fn yaml_s9e8_block_structure_indicators() {
    let y = "sequence:\n- one\n- two\nmapping:\n  ? sky\n  : blue\n  sea : green\n";

    let d: Doc = serde_saphyr::from_str(y).unwrap();
    assert_eq!(d.sequence, vec![String::from("one"), String::from("two")]);
    assert_eq!(d.mapping.get("sky").map(|s| s.as_str()), Some("blue"));
    assert_eq!(d.mapping.get("sea").map(|s| s.as_str()), Some("green"));
}
