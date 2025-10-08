// AVM7: Empty Stream — expect no documents when using from_multiple
#[test]
fn yaml_avm7_empty_stream() {
    let y = "";
    let docs: Vec<String> = serde_saphyr::from_multiple(y).expect("failed to parse AVM7");
    assert!(docs.is_empty(), "Expected no documents for empty stream");
}
