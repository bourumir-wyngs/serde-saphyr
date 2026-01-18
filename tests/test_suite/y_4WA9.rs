use serde::Deserialize;

// 4WA9: Literal scalars inside a mapping within a sequence
#[derive(Debug, Deserialize, PartialEq)]
struct Item {
    aaa: String,
    bbb: String,
}

#[test]
fn yaml_4wa9_literal_scalars() {
    let y = "- aaa: |2\n    xxx\n  bbb: |\n    xxx\n";
    let v: Vec<Item> = serde_saphyr::from_str(y).expect("failed to parse 4WA9");
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].aaa, "xxx\n");
    assert_eq!(v[0].bbb, "xxx\n");
}
