// P2AD: Block Scalar Header variants
#[test]
fn yaml_p2ad_block_scalar_header_variants() {
    let y = r#"- | # Empty header
 literal
- >1 # Indentation indicator
  folded
- |+ # Chomping indicator
 keep

- >1- # Both indicators
  strip
"#;
    let v: Vec<String> = serde_saphyr::from_str(y).expect("failed to parse P2AD");
    assert_eq!(v.len(), 4);
    assert_eq!(v[0].as_str(), "literal\n");
    assert_eq!(v[1].as_str(), " folded\n");
    assert_eq!(v[2].as_str(), "keep\n\n");
    assert_eq!(v[3].as_str(), " strip");
}
