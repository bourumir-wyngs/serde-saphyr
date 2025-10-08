// 9MAG: Flow sequence with invalid comma at the beginning — marked fail: true
// Expect parsing to return an error (no panic).
#[test]
fn yaml_9mag_flow_sequence_invalid_leading_comma_should_fail() {
    let y = "---\n[ , a, b, c ]\n";
    let result: Result<Vec<String>, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "9MAG should fail due to leading comma in flow sequence");
}