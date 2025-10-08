use std::collections::HashMap;

// M7NX: Nested flow collections
// { a: [ b, c, { d: [e, f] } ] }
#[derive(Debug, serde::Deserialize)]
struct Root { a: Vec<Item> }

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum Item {
    S(String),
    M(HashMap<String, Vec<String>>),
}

#[test]
fn yaml_m7nx_nested_flow_collections() {
    let y = r#"---
{
 a: [
  b, c, {
   d: [e, f]
  }
 ]
}
"#;
    let v: Root = serde_saphyr::from_str(y).expect("failed to parse M7NX");
    assert_eq!(v.a.len(), 3);
    match &v.a[0] { Item::S(s) => assert_eq!(s, "b"), _ => panic!("a[0] not 'b'") }
    match &v.a[1] { Item::S(s) => assert_eq!(s, "c"), _ => panic!("a[1] not 'c'") }
    match &v.a[2] {
        Item::M(m) => {
            assert_eq!(m.len(), 1);
            assert_eq!(m.get("d").cloned(), Some(vec!["e".to_string(), "f".to_string()]));
        }
        _ => panic!("a[2] not a mapping {{d: [e, f]}}")
    }
}
