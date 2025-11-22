use serde::Deserialize;

// ZWK4: Key with anchor after missing explicit mapping value
// YAML:
// ---
// a: 1
// ? b
// &anchor c: 3
// Expected map: { a: 1, b: null, c: 3 }

#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    a: i32,
    b: Option<i32>,
    c: i32,
}

#[test]
fn yaml_zwk4_explicit_key_missing_value_and_anchor() {
    let y = r#"---
 a: 1
 ? b
 &anchor c: 3
"#;

    let v: Doc = serde_saphyr::from_str(y).expect("failed to parse ZWK4");
    assert_eq!(v.a, 1);
    assert_eq!(v.b, None);
    assert_eq!(v.c, 3);
}
