// TS54: Folded Block Scalar
// YAML '>' folding with blank lines preserved.
// Source: tests/yaml-test-suite/src/TS54.yaml

#[test]
fn yaml_ts54_folded_block_scalar() {
    let y = ">\n ab\n cd\n \n ef\n\n\n gh\n\n";

    // Expect folded content: spaces/newlines per TS54.json
    // Expected string: "ab cd\nef\n\ngh\n"
    let s: String = serde_saphyr::from_str(y).expect("failed to parse TS54");

    assert_eq!(s, "ab cd\nef\n\ngh\n");
}
