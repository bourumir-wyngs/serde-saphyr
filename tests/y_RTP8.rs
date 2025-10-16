use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SuiteCase {
    yaml: String,
}

#[test]
fn y_RTP8() {
    let yaml = r#"%YAML 1.2
---
Document
... # Suffix
"#;

    let v: String = serde_saphyr::from_str(yaml).expect("parse inner YAML with directive and markers");
    assert_eq!(v, "Document");
}
