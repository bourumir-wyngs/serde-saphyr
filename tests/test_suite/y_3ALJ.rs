use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Item {
    Seq(Vec<String>),
    Str(String),
}

#[test]
fn yaml_3alj_block_sequence_in_block_sequence() {
    let yaml = "- - s1_i1\n  - s1_i2\n- s2\n";

    let v: Vec<Item> = serde_saphyr::from_str(yaml).expect("failed to parse 3ALJ");
    assert_eq!(v.len(), 2);
    match &v[0] {
        Item::Seq(inner) => assert_eq!(inner, &vec!["s1_i1".to_string(), "s1_i2".to_string()]),
        _ => panic!("first element should be a sequence"),
    }
    assert_eq!(v[1], Item::Str("s2".to_string()));
}
