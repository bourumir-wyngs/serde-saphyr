use std::collections::HashMap;

// LP6E: Whitespace After Scalars in Flow
// YAML content is a sequence of:
//  - [a, b , c ]
//  - { "a"  : b , c : 'd' , e   : "f" }
//  - [ ] (empty sequence)
#[test]
fn yaml_lp6e_whitespace_after_scalars_in_flow() {
    let y = r#"- [a, b , c ]
- { "a"  : b
   , c : 'd' ,
   e   : "f"
  }
- [      ]
"#;

    #[derive(Debug, serde::Deserialize)]
    #[serde(untagged)]
    enum Elem {
        L(Vec<String>),
        M(HashMap<String, String>),
    }

    let v: Vec<Elem> = serde_saphyr::from_str(y).expect("failed to parse LP6E");
    assert_eq!(v.len(), 3);

    match &v[0] { Elem::L(xs) => assert_eq!(xs, &vec!["a".to_string(), "b".to_string(), "c".to_string()]), _ => panic!("first not list") }
    match &v[1] { Elem::M(m) => {
        assert_eq!(m.len(), 3);
        assert_eq!(m.get("a").map(String::as_str), Some("b"));
        assert_eq!(m.get("c").map(String::as_str), Some("d"));
        assert_eq!(m.get("e").map(String::as_str), Some("f"));
    }, _ => panic!("second not map") }
    match &v[2] { Elem::L(xs) => assert!(xs.is_empty()), _ => panic!("third not list") }
}
