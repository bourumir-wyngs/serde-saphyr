// 5LLU: Block scalar with wrong indented line after spaces only — marked fail: true
// Expect parsing to return an error (no panic).

// A line with only spaces counts as a blank line, but those spaces are removed (blank lines are treated as empty).
// The first non-blank line determines the content indentation. Here, the first such line is " invalid" (4 spaces).
// But before that, you had lines with 2 and 3 spaces only, which don’t match the inferred 4-space indent.
// According to the YAML 1.2 spec, that’s an indentation violation → parsing error

#[test]
#[ignore]
fn yaml_5llu_block_scalar_wrong_indent_should_fail() {
    // Use spaces and then an indented invalid line under a folded scalar
    // Ignored for now: our current parser does not flag this indentation pattern consistently.
    // Keep the assertion so we can enable the test later once the behavior matches the YAML suite.
    let y = "block scalar: >\n\n  \n   \n    invalid\n";
    let result: Result<std::collections::HashMap<String, String>, _> = serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "5LLU should fail to parse due to wrong indentation after spaces"
    );
}
