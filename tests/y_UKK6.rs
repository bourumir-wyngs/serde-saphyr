use serde_json::Value as J;
use std::collections::BTreeMap;

#[test]
fn y_ukk6_syntax_character_edge_cases() {
    // Case 1: "- :" — sequence with a single mapping that has a null key and null value.
    // JSON cannot represent non-string keys, so we deserialize into a Rust map keyed by Option<String>.
    let yaml1 = "- :\n";
    let v1: Vec<BTreeMap<Option<String>, Option<String>>> =
        serde_saphyr::from_str(yaml1).expect("UKK6 part 1 should parse into Option-keyed map");
    assert_eq!(v1.len(), 1);
    let m = &v1[0];
    assert_eq!(m.len(), 1);
    assert!(m.get(&None).is_some(), "expected a null key in the map, got: {:?}", m);
    assert_eq!(m.get(&None).unwrap(), &None);

    // Case 2: "::" -> expected object { ":": null }
    let yaml2 = "::\n";
    let v2: J = serde_saphyr::from_str(yaml2).expect("UKK6 part 2 should parse");
    let obj = v2.as_object().expect("expected mapping for '::'");
    assert_eq!(obj.len(), 1);
    assert!(obj.get(":").is_some(), "expected key ':' in mapping, got: {:?}", obj);
    assert!(obj.get(":").unwrap().is_null(), "expected null value for key ':'");

    // Case 3: "!" -> null
    let yaml3 = "!\n";
    let v3: J = serde_saphyr::from_str(yaml3).expect("UKK6 part 3 should parse");
    assert!(v3.is_null(), "expected null for '!' but got: {:?}", v3);
}
