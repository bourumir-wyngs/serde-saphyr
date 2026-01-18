#[test]
fn yaml_5llu_block_scalar_wrong_indent_should_fail() {
    // It is ONLY an error for any of the leading empty lines to contain
    // MORE spaces than the first non-empty line. This YAML is ok
    let y = "block scalar: >\n\n  \n   \n    valid\n";
    let result: Result<std::collections::HashMap<String, String>, _> = serde_saphyr::from_str(y);
    assert!(result.is_ok());

    // It IS an error for any of the leading empty lines to contain
    // MORE spaces than the first non-empty line. This YAML is not ok
    let y = "block scalar: >\n\n                                  \n   \n    invalid\n";
    let result: Result<std::collections::HashMap<String, String>, _> = serde_saphyr::from_str(y);
    assert!(
        result.is_err(),
        "5LLU should fail to parse due to wrong indentation after spaces"
    );
}
