use serde::Deserialize;

// ZH7C: Anchors in Mapping
// YAML:
//   &a a: b
//   c: &d d
// Anchors should not affect the data model; resulting map is { a: "b", c: "d" }.

#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    a: String,
    c: String,
}

#[test]
fn yaml_zh7c_anchors_in_mapping() {
    let y = r#"&a a: b
c: &d d
"#;

    let v: Doc = serde_saphyr::from_str(y).expect("failed to parse ZH7C");
    assert_eq!(v.a, "b");
    assert_eq!(v.c, "d");
}
