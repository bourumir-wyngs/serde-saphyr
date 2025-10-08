use std::collections::HashMap;

// NB6Z: Multiline plain value with tabs on empty lines
// Expectation from suite JSON: { "key": "value with\ntabs" }
// A TAB-only intermediate line is treated as an empty line between plain-scalar lines,
// which becomes a single newline in the folded result.
#[test]
fn yaml_nb6z_multiline_plain_with_tab_only_line() {
    let y = "key:\n  value\n  with\n  \t\n  tabs\n";
    let v: HashMap<String, String> = serde_saphyr::from_str(y).expect("failed to parse NB6Z");
    assert_eq!(v.get("key").map(String::as_str), Some("value with\ntabs"));
}
