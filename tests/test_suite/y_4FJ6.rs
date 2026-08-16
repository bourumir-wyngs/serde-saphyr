use std::collections::BTreeMap;

// 4FJ6: Nested implicit complex keys
//
// The document is a sequence containing a mapping. Its key is a sequence that
// itself contains a sequence with another implicit mapping as an element.
type InnerKey = Vec<Vec<String>>;
type InnerMap = BTreeMap<InnerKey, String>;
type OuterKey = (String, (InnerMap, String));
type Document = Vec<BTreeMap<OuterKey, i32>>;

#[test]
fn yaml_4fj6_nested_implicit_complex_keys() {
    let y = r#"---
[
  [ a, [ [[b,c]]: d, e]]: 23
]
"#;

    let actual: Document = serde_saphyr::from_str(y).expect("4FJ6 should parse");

    let inner_key = vec![vec!["b".to_owned(), "c".to_owned()]];
    let inner_map = BTreeMap::from([(inner_key, "d".to_owned())]);
    let outer_key = ("a".to_owned(), (inner_map, "e".to_owned()));
    let expected = vec![BTreeMap::from([(outer_key, 23)])];

    assert_eq!(actual, expected);
}
