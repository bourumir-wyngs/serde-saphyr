// QB6E: Wrong indented multiline quoted scalar — this YAML is expected to fail to parse.
// We attempt to parse into a simple struct; if the parser correctly rejects it, the test passes.
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Doc {
    quoted: String,
}

#[test]
fn yaml_qb6e_wrong_indented_multiline_quoted_scalar_should_fail() {
    let y = r#"---
quoted: "a
b
c""#;

    let r: Result<Doc, _> = serde_saphyr::from_str(y);
    assert!(r.is_err(), "Parser accepted malformed multiline double-quoted scalar; per test-suite this should fail. If this keeps passing, mark as #[ignore] and note parser limitation.");
}
