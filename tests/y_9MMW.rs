use serde::Deserialize;
use std::collections::HashMap;

// 9MMW: Single Pair Implicit Entries — sequence of sequences, each inner sequence has one mapping with a single pair.
// Keys can be either a simple string or a mapping {JSON: like}. We model each inner mapping
// as an untagged enum to allow either key shape.
#[derive(Debug, Deserialize, PartialEq, Eq, Hash)]
struct Sk {
    #[serde(rename(deserialize = "JSON"))]
    json: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OnePairMap {
    Str(HashMap<String, String>),
    Struct(HashMap<Sk, String>),
}

#[test]
fn yaml_9mmw_single_pair_implicit_entries() {
    let y = "- [ YAML : separate ]\n- [ \"JSON like\":adjacent ]\n- [ {JSON: like}:adjacent ]\n";
    let v: Vec<Vec<OnePairMap>> = serde_saphyr::from_str(y).expect("failed to parse 9MMW");
    assert_eq!(v.len(), 3);

    match &v[0][0] {
        OnePairMap::Str(m) => assert_eq!(m.get("YAML").map(String::as_str), Some("separate")),
        _ => panic!("expected string key for first entry"),
    }
    match &v[1][0] {
        OnePairMap::Str(m) => assert_eq!(m.get("JSON like").map(String::as_str), Some("adjacent")),
        _ => panic!("expected string key for second entry"),
    }
    match &v[2][0] {
        OnePairMap::Struct(m) => {
            let key = Sk {
                json: "like".to_string(),
            };
            assert_eq!(m.get(&key).map(String::as_str), Some("adjacent"));
        }
        _ => panic!("expected struct key for third entry"),
    }
}
