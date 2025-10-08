use serde::Deserialize;

// L94M: Tags in Explicit Mapping
// YAML uses explicit keys with tags (!!str for key, !!int/!!str for values).
// Expected JSON: { "a": 47, "c": "d" }
#[derive(Debug, Deserialize, PartialEq)]
struct Root { a: i32, c: String }

#[test]
fn yaml_l94m_tags_in_explicit_mapping() {
    let y = r#"? !!str a
: !!int 47
? c
: !!str d
"#;
    let v: Root = serde_saphyr::from_str(y).expect("failed to parse L94M");
    assert_eq!(v, Root { a: 47, c: "d".to_string() });
}
