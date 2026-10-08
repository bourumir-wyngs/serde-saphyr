#![cfg(feature = "deserialize")]

// Preserve the established 1.3.0 behavior and upstream JEF9/02 expectation:
// an otherwise empty scalar with keep chomping yields "\n" when its final
// indentation has no terminating line break. JEF9/02 inherits ["\n"] from the
// preceding variant; we retain that behavior for compatibility.
// https://github.com/yaml/yaml-test-suite/blob/main/src/JEF9.yaml

use rstest::rstest;
use std::collections::BTreeMap;

#[rstest]
#[case::literal("|+")]
#[case::folded(">+")]
#[case::literal_explicit_indent("|2+")]
#[case::folded_explicit_indent(">2+")]
#[case::literal_chomp_first("|+2")]
#[case::folded_chomp_first(">+2")]
fn empty_block_scalar_keep_retains_newline_at_indented_eof(#[case] header: &str) {
    let yaml = format!("{header}\n  ");
    let from_str: String = serde_saphyr::from_str(&yaml).unwrap();
    let from_reader: String = serde_saphyr::from_reader(yaml.as_bytes()).unwrap();
    assert_eq!(from_str, "\n", "from_str: {yaml:?}");
    assert_eq!(from_reader, "\n", "from_reader: {yaml:?}");

    let yaml = format!("- {header}\n  ");
    let from_str: Vec<String> = serde_saphyr::from_str(&yaml).unwrap();
    let from_reader: Vec<String> = serde_saphyr::from_reader(yaml.as_bytes()).unwrap();
    assert_eq!(from_str, ["\n"], "from_str: {yaml:?}");
    assert_eq!(from_reader, ["\n"], "from_reader: {yaml:?}");

    let yaml = format!("value: {header}\n  ");
    let from_str: BTreeMap<String, String> = serde_saphyr::from_str(&yaml).unwrap();
    let from_reader: BTreeMap<String, String> = serde_saphyr::from_reader(yaml.as_bytes()).unwrap();
    assert_eq!(from_str["value"], "\n", "from_str: {yaml:?}");
    assert_eq!(from_reader["value"], "\n", "from_reader: {yaml:?}");
}

#[rstest]
#[case::literal("|+")]
#[case::folded(">+")]
fn empty_block_scalar_keep_preserves_completed_lines_at_eof(#[case] header: &str) {
    for (body, expected) in [("  \n", "\n"), ("  \n  ", "\n"), ("  \n  \n  ", "\n\n")] {
        let yaml = format!("{header}\n{body}");
        let from_str: String = serde_saphyr::from_str(&yaml).unwrap();
        let from_reader: String = serde_saphyr::from_reader(yaml.as_bytes()).unwrap();
        assert_eq!(from_str, expected, "from_str: {yaml:?}");
        assert_eq!(from_reader, expected, "from_reader: {yaml:?}");
    }
}
