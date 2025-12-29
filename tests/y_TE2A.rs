use serde::Deserialize;

// TE2A: Spec Example 8.16. Block Mappings
// YAML:
// block mapping:
//  key: value
// Expected: mapping with key "block mapping" -> nested map { key: "value" }

#[derive(Debug, Deserialize)]
struct Inner {
    key: String,
}

#[derive(Debug, Deserialize)]
struct Root {
    #[serde(rename = "block mapping")]
    block_mapping: Inner,
}

#[test]
fn y_te2a_block_mappings() {
    let y = "block mapping:\n key: value\n";
    let r: Root = serde_saphyr::from_str(y).expect("failed to parse TE2A block mapping");
    assert_eq!(r.block_mapping.key, "value");
}
