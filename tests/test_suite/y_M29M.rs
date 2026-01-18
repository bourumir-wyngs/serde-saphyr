use serde::Deserialize;

// M29M: Literal Block Scalar in a mapping
#[derive(Debug, Deserialize)]
struct Root {
    a: String,
}

#[test]
fn yaml_m29m_literal_block_scalar_in_mapping() {
    let y = r#"a: |
 ab

 cd
 ef


...
"#;
    let v: Root = serde_saphyr::from_str(y).expect("failed to parse M29M");
    assert_eq!(v.a.as_str(), "ab\n\ncd\nef\n");
}
