use std::collections::HashMap;

// NKF9: Empty keys in block and flow mapping across four documents.
// We parse all documents and assert contents using Option for empty key and value.
#[test]
fn yaml_nkf9_empty_keys_in_block_and_flow_mapping() {
    let y = r#"---
key: value
: empty key
---
{
 key: value, : empty key
}
---
# empty key and value
:
---
# empty key and value
{ : }
"#;

    type Map = HashMap<Option<String>, Option<String>>;
    let docs: Vec<Map> = serde_saphyr::from_multiple(y).expect("failed to parse NKF9");
    assert_eq!(docs.len(), 4);

    // Doc 1: block mapping with one normal and one empty key
    let d1 = &docs[0];
    assert_eq!(d1.get(&Some("key".to_string())).cloned().flatten().as_deref(), Some("value"));
    assert_eq!(d1.get(&None).cloned().flatten().as_deref(), Some("empty key"));

    // Doc 2: flow mapping with same contents
    let d2 = &docs[1];
    assert_eq!(d2.get(&Some("key".to_string())).cloned().flatten().as_deref(), Some("value"));
    assert_eq!(d2.get(&None).cloned().flatten().as_deref(), Some("empty key"));

    // Doc 3: empty key and empty value
    let d3 = &docs[2];
    assert_eq!(d3.len(), 1);
    assert_eq!(d3.get(&None).and_then(|v| v.as_ref().map(String::as_str)), None);

    // Doc 4: empty key and empty value in flow mapping
    let d4 = &docs[3];
    assert_eq!(d4.len(), 1);
    assert_eq!(d4.get(&None).and_then(|v| v.as_ref().map(String::as_str)), None);
}
