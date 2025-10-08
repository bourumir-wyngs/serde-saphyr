use serde::Deserialize;

// LHL4: Invalid tag -> expect parse error
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Dummy(String);

#[test]
fn yaml_lhl4_invalid_tag_should_fail() {
    let y = r#"---
!invalid{}tag scalar
"#;
    let result: Result<Dummy, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "LHL4 should fail to parse due to invalid tag");
}
