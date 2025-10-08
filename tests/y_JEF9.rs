// JEF9: Trailing whitespace in streams with |+ block scalar keep
// We validate the simplest case: a sequence with a single kept-empty block scalar
// that contains two blank lines, resulting in "\n\n".

#[test]
fn yaml_jef9_trailing_whitespace_block_keep() {
    let y = r#"- |+


"#;
    let v: Vec<String> = serde_saphyr::from_str(y).expect("failed to parse JEF9 first variant");
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].as_str(), "\n\n");
}
