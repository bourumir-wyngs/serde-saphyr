use serde::Deserialize;

// Y2GN: Anchor with colon in the middle of the anchor name.
// YAML snippet defines a simple mapping { key: "value" } with an anchor on the value.
// No alias is used, so the resulting mapping is straightforward.

#[derive(Debug, Deserialize)]
struct Doc {
    key: String,
}

#[test]
fn yaml_y2gn_anchor_with_colon_in_name() {
    let y = r#"---
key: &an:chor value
"#;

    let v: Doc = serde_saphyr::from_str(y).expect("failed to parse Y2GN");
    assert_eq!(v.key, "value");
}
