// QB6E: Wrong indented multiline quoted scalar
// According to the YAML test suite case QB6E, this document is invalid.
// The double-quoted scalar is split across multiple lines without proper
// indentation/continuation rules, so a compliant parser should reject it.

use serde_json::Value;

#[test]
fn yaml_qb6e_wrong_indented_multiline_quoted_scalar_should_fail() {
    let y = "---\nquoted: \"a\nb\nc\"\n";

    let result = serde_saphyr::from_str::<Value>(y);
    assert!(result.is_err(), "QB6E should fail to parse, but got: {:?}", result);
}
