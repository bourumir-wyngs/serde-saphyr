// PRH3: Single quoted lines with folding
#[test]
fn yaml_prh3_single_quoted_lines() {
    let y = r#"' 1st non-empty

  2nd non-empty 
	3rd non-empty '
"#;
    let s: String = serde_saphyr::from_str(y).expect("failed to parse PRH3");
    assert_eq!(s, " 1st non-empty\n2nd non-empty 3rd non-empty ");
}
