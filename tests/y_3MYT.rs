#[test]
fn yaml_3myt_plain_scalar_looking_like_key_comment_anchor_tag() {
    let yaml = "---\nk:#foo\n &a !t s\n";

    let s: String = serde_saphyr::from_str(yaml).expect("failed to parse 3MYT scalar");
    assert_eq!(s, "k:#foo &a !t s");
}
