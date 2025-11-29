use serde_saphyr::Error;

// saphyr-parser 0.0.6 does not emit closing event.
#[test]
fn extra_bracket_should_err() {
    let y = "---\n[ a, b, c ] ]\n";
    let result: Error = serde_saphyr::from_str::<serde_json::Value>(y).expect_err("Expected error");
    assert!(result.to_string().contains("bracket"));
}
