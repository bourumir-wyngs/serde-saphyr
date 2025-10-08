use serde::Deserialize;
use std::collections::HashMap;

// CFD4: Empty implicit key in single pair flow sequences
// Input: sequence of one-element sequences; each inner element is a mapping with empty key and a value.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum One {
    Map(HashMap<String, String>),
}

#[test]
fn yaml_cfd4_empty_implicit_key_in_single_pair_flow_sequences() {
    let y = "- [ : empty key ]\n- [: another empty key]\n";
    let v: Vec<Vec<One>> = serde_saphyr::from_str(y).expect("failed to parse CFD4");
    assert_eq!(v.len(), 2);

    // First
    match &v[0][0] {
        One::Map(m) => {
            assert_eq!(m.len(), 1);
            assert_eq!(m.values().next().map(String::as_str), Some("empty key"));
        }
    }

    // Second
    match &v[1][0] {
        One::Map(m) => {
            assert_eq!(m.len(), 1);
            assert_eq!(m.values().next().map(String::as_str), Some("another empty key"));
        }
    }
}
