use serde_json::Value;

// T5N4: Literal scalar with actual tab and newline characters (suite used glyphs to visualize them).

#[test]
fn y_t5n4_literal_scalar_with_suite_glyphs() {
    let y = "--- |\n literal\n \ttext\n";
    let r: Result<Value, _> = serde_saphyr::from_str(y);
    assert!(
        r.is_ok(),
        "Parser failed to handle literal block with a tabbed line: {:?}",
        r
    );
}
