use serde::Deserialize;

// The YAML in tests/yaml-test-suite/src/2AUY.yaml under the `yaml:` key is a
// block sequence with explicit tags (!!str, !!int). It represents:
// ["a", "b", 42, "d"].
//
// We model this as a tuple struct so that Serde deserializes the sequence into
// positional fields of matching Rust types.
#[derive(Debug, Deserialize, PartialEq)]
struct TaggedSeq(String, String, i32, String);

#[test]
fn parse_tagged_sequence_with_explicit_scalar_tags() {
    let yaml = "- !!str a\n- b\n- !!int 42\n- d\n";

    let v: TaggedSeq = serde_saphyr::from_str(yaml).expect("failed to parse tagged sequence");

    assert_eq!(v, TaggedSeq("a".into(), "b".into(), 42, "d".into()));
}
