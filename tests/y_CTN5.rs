// CTN5: Flow sequence with invalid extra comma — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_ctn5_flow_sequence_with_extra_comma_should_fail() {
    let y = "---\n[ a, b, c, , ]\n";
    let result: Result<Vec<String>, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "CTN5 should fail due to extra comma in flow sequence");
}
