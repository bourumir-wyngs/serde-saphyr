use serde::Deserialize;
use std::collections::BTreeMap;

// V9D5: Spec Example 8.19. Compact Block Mappings
// Expect a sequence of:
// - { sun: yellow }
// - { { earth: blue }: { moon: white } }

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Item {
    // First element: simple mapping { sun: yellow }
    Simple(BTreeMap<String, String>),

    // Second element: complex mapping
    // { { earth: blue }: { moon: white } }
    Complex(BTreeMap<BTreeMap<String, String>, BTreeMap<String, String>>),
}

#[test]
fn yaml_v9d5_compact_block_mappings() {
    let y = r#"
    - sun: yellow
    - ? earth: blue
      : moon: white
    "#;

    let v: Vec<Item> = serde_saphyr::from_str(y).expect("failed to parse V9D5");

    // Still two sequence elements
    assert_eq!(v.len(), 2);

    // First element: { sun: yellow }
    let first = match &v[0] {
        Item::Simple(m) => m,
        other => panic!("expected first element to be Simple map, got {:?}", other),
    };
    assert_eq!(first.len(), 1);
    assert_eq!(first.get("sun").map(String::as_str), Some("yellow"));

    // Second element: { { earth: blue }: { moon: white } }
    let second = match &v[1] {
        Item::Complex(m) => m,
        other => panic!("expected second element to be Complex map, got {:?}", other),
    };
    assert_eq!(second.len(), 1);

    let (key_map, value_map) = second
        .iter()
        .next()
        .expect("complex map should have one entry");

    // Key is a mapping { earth: blue }
    assert_eq!(key_map.len(), 1);
    assert_eq!(key_map.get("earth").map(String::as_str), Some("blue"));

    // Value is a mapping { moon: white }
    assert_eq!(value_map.len(), 1);
    assert_eq!(value_map.get("moon").map(String::as_str), Some("white"));
}
