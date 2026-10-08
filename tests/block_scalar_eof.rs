#![cfg(feature = "deserialize")]

use rstest::rstest;
use std::collections::BTreeMap;

#[rstest]
#[case::literal("|+")]
#[case::folded(">+")]
#[case::literal_explicit_indent("|2+")]
#[case::folded_explicit_indent(">2+")]
#[case::literal_chomp_first("|+2")]
#[case::folded_chomp_first(">+2")]
fn empty_block_scalar_ignores_unterminated_indentation_at_eof(#[case] header: &str) {
    // Indentation without a terminating line break is not an empty content line.
    let yaml = format!("{header}\n  ");
    let from_str: String = serde_saphyr::from_str(&yaml).unwrap();
    let from_reader: String = serde_saphyr::from_reader(yaml.as_bytes()).unwrap();
    assert_eq!(from_str, "", "from_str: {yaml:?}");
    assert_eq!(from_reader, "", "from_reader: {yaml:?}");

    let yaml = format!("- {header}\n  ");
    let from_str: Vec<String> = serde_saphyr::from_str(&yaml).unwrap();
    let from_reader: Vec<String> = serde_saphyr::from_reader(yaml.as_bytes()).unwrap();
    assert_eq!(from_str, [""], "from_str: {yaml:?}");
    assert_eq!(from_reader, [""], "from_reader: {yaml:?}");

    let yaml = format!("value: {header}\n  ");
    let from_str: BTreeMap<String, String> = serde_saphyr::from_str(&yaml).unwrap();
    let from_reader: BTreeMap<String, String> = serde_saphyr::from_reader(yaml.as_bytes()).unwrap();
    assert_eq!(from_str["value"], "", "from_str: {yaml:?}");
    assert_eq!(from_reader["value"], "", "from_reader: {yaml:?}");
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
