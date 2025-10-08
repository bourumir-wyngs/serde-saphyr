use std::collections::HashMap;

// JR7V: Question marks in scalars and keys
#[test]
fn yaml_jr7v_question_marks_in_scalars_and_keys() {
    let y = r#"- a?string
- another ? string
- key: value?
- [a?string]
- [another ? string]
- {key: value? }
- {key: value?}
- {key?: value }
"#;

    #[derive(Debug, serde::Deserialize)]
    #[serde(untagged)]
    enum Elem {
        S(String),
        L(Vec<String>),
        M(HashMap<String, String>),
    }

    let v: Vec<Elem> = serde_saphyr::from_str(y).expect("failed to parse JR7V");
    assert_eq!(v.len(), 8);

    // 1: "a?string"
    match &v[0] { Elem::S(s) => assert_eq!(s, "a?string"), _ => panic!("0 not string") }
    // 2: "another ? string"
    match &v[1] { Elem::S(s) => assert_eq!(s, "another ? string"), _ => panic!("1 not string") }
    // 3: { key: "value?" }
    match &v[2] { Elem::M(m) => assert_eq!(m.get("key").map(String::as_str), Some("value?")), _ => panic!("2 not map") }
    // 4: ["a?string"]
    match &v[3] { Elem::L(xs) => { assert_eq!(xs.len(), 1); assert_eq!(xs[0], "a?string"); }, _ => panic!("3 not list") }
    // 5: ["another ? string"]
    match &v[4] { Elem::L(xs) => { assert_eq!(xs.len(), 1); assert_eq!(xs[0], "another ? string"); }, _ => panic!("4 not list") }
    // 6: { key: "value?" }
    match &v[5] { Elem::M(m) => assert_eq!(m.get("key").map(String::as_str), Some("value?")), _ => panic!("5 not map") }
    // 7: { key: "value?" }
    match &v[6] { Elem::M(m) => assert_eq!(m.get("key").map(String::as_str), Some("value?")), _ => panic!("6 not map") }
    // 8: { "key?": "value" }
    match &v[7] { Elem::M(m) => assert_eq!(m.get("key?").map(String::as_str), Some("value")), _ => panic!("7 not map") }
}
