use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, PartialEq)]
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

#[test]
fn test_comment_like_string_quoted() -> anyhow::Result<()> {
    let t = CommentLikes {
        s1: Some("# like comment".to_string()),
        s2: None,
    };
    let yaml = serde_saphyr::to_string(&t)?;
    // Accepts either single or double quotes (Go's yaml.v3 prefers single)
    assert!(
        yaml.contains("\"# like comment\"") || yaml.contains("'# like comment'"),
        "String starting from # must be quoted: {yaml}"
    );
    Ok(())
}
