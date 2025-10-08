use serde::Deserialize;

// P2EQ: Invalid sequence item on same line as previous item (fail: true) — expect error
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Dummy(Vec<std::collections::HashMap<String, String>>);

#[test]
fn yaml_p2eq_invalid_sequence_item_same_line_should_fail() {
    let y = r#"---
- { y: z }- invalid
"#;
    let result: Result<Dummy, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "P2EQ should fail to parse due to invalid sequence item on the same line");
}
