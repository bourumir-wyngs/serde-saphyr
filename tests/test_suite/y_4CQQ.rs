use serde::Deserialize;

// 4CQQ: Multi-line Flow Scalars
#[derive(Debug, Deserialize, PartialEq)]
struct Doc {
    plain: String,
    quoted: String,
}

#[test]
fn yaml_4cqq_multi_line_flow_scalars() {
    let y = r#"
plain:
  This unquoted scalar
  spans many lines.

quoted: "So does this
  quoted scalar.\n"
"#;
    let d: Doc = serde_saphyr::from_str(y).expect("failed to parse 4CQQ");
    assert_eq!(d.plain, "This unquoted scalar spans many lines.");
    assert_eq!(d.quoted, "So does this quoted scalar.\n");
}
