use serde::Deserialize;

// N782: Invalid document markers in flow style (fail: true) — expect error
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Dummy(Vec<String>);

#[test]
fn yaml_n782_invalid_doc_markers_in_flow_should_fail() {
    let y = r#"[
--- ,
...
]
"#;
    let result: Result<Dummy, _> = serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "N782 should fail to parse due to invalid markers in flow sequence"
    );
}
