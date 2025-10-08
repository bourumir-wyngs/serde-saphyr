use std::collections::HashMap;

// J7PZ: Spec Example 2.26. Ordered Mappings (!!omap)
// We target the underlying representation: a sequence of one-pair mappings.
#[test]
fn yaml_j7pz_ordered_mappings_omap() {
    let y = r#"--- !!omap
- Mark McGwire: 65
- Sammy Sosa: 63
- Ken Griffy: 58
"#;
    // Our deserializer generally ignores unknown tags and parses the content structure.
    // Expect a Vec of HashMaps with single entries each.
    let v: Vec<HashMap<String, i32>> = serde_saphyr::from_str(y).expect("failed to parse J7PZ");
    assert_eq!(v.len(), 3);
    assert_eq!(v[0].get("Mark McGwire"), Some(&65));
    assert_eq!(v[1].get("Sammy Sosa"), Some(&63));
    assert_eq!(v[2].get("Ken Griffy"), Some(&58));
}
