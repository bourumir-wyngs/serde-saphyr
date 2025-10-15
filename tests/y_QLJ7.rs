use std::collections::BTreeMap;

// QLJ7: Tag shorthand used in documents but only defined in the first.
// Expectation from the YAML test suite: this should fail because later documents use !prefix!
// without re-declaring the %TAG directive (tag handles are per-document).
// We attempt to parse the stream into Vec<BTreeMap<String, String>> using from_multiple and
// assert that an error is produced. If our parser accepts this input, we will mark the test as
// #[ignore] in a follow-up edit and explain the limitation.
#[test]
fn yaml_qlj7_tag_shorthand_only_defined_in_first_doc_should_fail() {
    let y = "%TAG !prefix! tag:example.com,2011:\n--- !prefix!A\na: b\n--- !prefix!B\nc: d\n--- !prefix!C\ne: f\n";

    let r: Result<Vec<BTreeMap<String, String>>, _> = serde_saphyr::from_multiple(y);
    assert!(r.is_err(), "Parser accepted tag handle usage in later docs without %TAG redefinition; per test-suite this should fail. If this keeps passing, mark as #[ignore] and note parser limitation.");
}
