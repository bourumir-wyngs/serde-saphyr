use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SuiteCase {
    yaml: String,
}

#[test]
fn y_RXY3() {
    let yaml = r#"---
'
...
'
"#;

    let res: Result<serde_json::Value, serde_saphyr::Error> = serde_saphyr::from_str(yaml);
    assert!(res.is_err(), "Expected parse error for invalid document-end marker inside single-quoted string");
}
