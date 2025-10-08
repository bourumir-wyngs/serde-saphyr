use std::collections::HashMap;

// JQ4R: Block sequence where second element is a mapping { two: three }
#[test]
fn yaml_jq4r_block_sequence() {
    let y = r#"block sequence:
  - one
  - two : three
"#;

    #[derive(Debug, serde::Deserialize)]
    struct Root {
        #[serde(rename = "block sequence")]
        block_sequence: Vec<Elem>,
    }

    #[derive(Debug, serde::Deserialize)]
    #[serde(untagged)]
    enum Elem {
        S(String),
        M(HashMap<String, String>),
    }

    let v: Root = serde_saphyr::from_str(y).expect("failed to parse JQ4R");
    assert_eq!(v.block_sequence.len(), 2);
    match &v.block_sequence[0] {
        Elem::S(s) => assert_eq!(s, "one"),
        _ => panic!("first element should be a string 'one'"),
    }
    match &v.block_sequence[1] {
        Elem::M(m) => {
            assert_eq!(m.len(), 1);
            assert_eq!(m.get("two").map(String::as_str), Some("three"));
        }
        _ => panic!("second element should be a mapping {{two: three}}"),
    }
}
