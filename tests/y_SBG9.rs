use std::collections::BTreeMap;

// SBG9: Flow Sequence in Flow Mapping
// Note: original test calls for let y = "{a: [b, c], [d, e]: f}\n"
// but we only support variable members in array via variable enums we
// specify by using variant name as key value. Here we just cannot do.
// Also, serde_json::Value does not hold this structure. Hence
// testing only as much as we still can do.
#[test]
fn flow_sequence_in_flow_mapping() {
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Clone)]
    struct Target {
        r: BTreeMap<Vec<String>, String>,
    }

    let y = "r: {[d, e]: f}\n";
    let fm: Target = serde_saphyr::from_str(y).unwrap();

    // Assert the mapping has a single entry: key ["d", "e"] -> value "f"
    assert_eq!(fm.r.len(), 1);
    let key = vec!["d".to_string(), "e".to_string()];
    assert_eq!(fm.r.get(&key).map(|s| s.as_str()), Some("f"));
}

#[test]
fn flow_sequence_in_flow_value() {
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Clone)]
    struct Target {
        r: BTreeMap<String, Vec<String>>,
    }
    let y = "r: {a: [b, c]}\n";

    let fm: Target = serde_saphyr::from_str(y).unwrap();

    // Assert the mapping has a single entry: key "a" -> value ["b", "c"]
    assert_eq!(fm.r.len(), 1);
    assert_eq!(fm.r.get("a").cloned(), Some(vec!["b".to_string(), "c".to_string()]));
}

