use std::collections::HashMap;

#[test]
fn yaml_2jqs_block_mapping_with_missing_keys_should_error_on_duplicate_empty_key() {
    // Two entries with empty keys; mapping into HashMap should produce a duplicate-key error
    let yaml = ": a\n: b\n";

    let result = serde_saphyr::from_str::<HashMap<String, String>>(yaml);
    assert!(
        result.is_err(),
        "Expected duplicate-key error for empty keys, but got: {:?}",
        result
    );
}
