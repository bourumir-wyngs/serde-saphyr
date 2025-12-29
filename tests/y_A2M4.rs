use serde::Deserialize;
use std::collections::HashMap;

// A2M4: Indentation Indicators — explicit key mapping to a sequence ["b", ["c", "d"]]
#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Elem {
    S(String),
    L(Vec<String>),
}

#[test]
fn yaml_a2m4_indentation_indicators() {
    let y = "? a\n: - b\n  -  - c\n     - d\n";
    let m: HashMap<String, Vec<Elem>> = serde_saphyr::from_str(y).expect("failed to parse A2M4");
    let v = m.get("a").expect("missing key 'a'");
    assert_eq!(v.len(), 2);
    match &v[0] {
        Elem::S(s) => assert_eq!(s, "b"),
        _ => panic!("first should be string 'b'"),
    }
    match &v[1] {
        Elem::L(inner) => assert_eq!(inner, &vec!["c".to_string(), "d".to_string()]),
        _ => panic!("second should be a list [c, d]"),
    }
}
