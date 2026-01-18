// XLQ9: Multiline scalar that looks like a YAML directive
// Expect a single top-level plain scalar: "scalar %YAML 1.2"

#[test]
fn yaml_xlq9_plain_scalar_with_directive_lookalike() {
    let y = r#"---
scalar
%YAML 1.2
"#;

    let s: String = serde_saphyr::from_str(y).expect("XLQ9 should parse top-level plain scalar");
    assert_eq!(s, "scalar %YAML 1.2");
}
