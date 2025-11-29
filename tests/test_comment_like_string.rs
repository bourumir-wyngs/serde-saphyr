use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq)]
struct CommentLikes {
    s1: Option<String>,
    s2: Option<String>,
}

#[test]
fn test_comment_like_string() -> anyhow::Result<()> {
    let yaml = r##"
        s1: #a
        s2: "#a"
    "##;

    let r: CommentLikes = serde_saphyr::from_str(&yaml)?;
    assert_eq!(r.s1, None);
    assert_eq!(r.s2, Some("#a".to_string()));
    Ok(())
}
