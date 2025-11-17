// TS54: Folded Block Scalar
// YAML '>' folding with blank lines preserved.
// Source: tests/yaml-test-suite/src/TS54.yaml

#[test]
fn yaml_ts54_folded_block_scalar() {
    // Replace the visible space glyph with an actual blank or space line as per instruction.
    // Using the canonical input from tests/yaml-test-suite/data/TS54/in.yaml
    let y = ">\n ab\n cd\n \n ef\n\n\n gh\n\n";

    // Expect folded content: spaces/newlines per TS54.json
    // Expected string: "ab cd\nef\n\ngh\n"
    let s: String = serde_saphyr::from_str(y).expect("failed to parse TS54");

    assert_eq!(s, "ab cd\nef\n\ngh\n");
}
