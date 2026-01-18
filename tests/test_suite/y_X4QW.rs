// X4QW: Comment without whitespace after block scalar indicator (invalid)
// Suite marks this as fail: true. Our test should ensure parser returns an error.

#[test]
fn yaml_x4qw_comment_immediately_after_folded_indicator_should_error() {
    let y = r#"block: ># comment
  scalar
"#;

    assert!(serde_saphyr::from_str::<std::collections::BTreeMap<String, String>>(y).is_err());
}
