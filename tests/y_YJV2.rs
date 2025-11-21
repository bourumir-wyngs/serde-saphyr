use serde::Deserialize;

// YJV2: Dash in flow sequence
// The YAML test suite marks this as a failure. We'll assert that parsing fails.

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Dummy;

#[test]
fn yaml_yjv2_dash_in_flow_sequence_should_fail() {
    let y = r#"[-]
"#;

    let result: Result<Vec<String>, _> = serde_saphyr::from_str(y);
    assert!(result.is_err(), "YJV2 should be invalid YAML for this parser; it unexpectedly parsed as: {:?}", result);
}
