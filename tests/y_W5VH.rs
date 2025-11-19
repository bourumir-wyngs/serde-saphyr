use serde::Deserialize;

// W5VH: Allowed characters in alias
// YAML uses an anchor/alias name containing characters like ':', '@', '!', '*', '$', '"', '<', '>' and ':'
// The expected mapping is { a: "scalar a", b: "scalar a" }

#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    a: String,
    b: String,
}

#[test]
fn yaml_w5vh_alias_with_special_characters() {
    let y = r#"a: &:@*!$"<foo>: scalar a
b: *:@*!$"<foo>:
"#;

    let v: Doc = serde_saphyr::from_str(y).expect("failed to parse W5VH");
    assert_eq!(v.a, "scalar a");
    assert_eq!(v.b, "scalar a");
}

