use super::yaml_suite_support::assert_valid_events_with_options;
use serde_saphyr::DuplicateKeyPolicy;

#[test]
fn yaml_2jqs_block_mapping_with_missing_keys() {
    let yaml = ": a\n: b\n";
    let events = "+STR\n+DOC\n+MAP\n=VAL :\n=VAL :a\n=VAL :\n=VAL :b\n-MAP\n-DOC\n-STR\n";

    // YAML Test Suite records both pairs at the parser-event level; it does not
    // prescribe how a constructed mapping resolves duplicate keys. Use an
    // explicitly permissive construction policy, then compare its exact event stream.
    let options = serde_saphyr::options! {
        duplicate_keys: DuplicateKeyPolicy::FirstWins,
    };
    assert_valid_events_with_options(yaml, events, options);
}
