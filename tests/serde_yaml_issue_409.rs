#![cfg(feature = "deserialize")]

// Regression coverage for https://github.com/dtolnay/serde-yaml/issues/409.
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
enum MixedTagging {
    Tagged(String),
    #[serde(untagged)]
    Untagged(usize),
}

#[test]
fn tagged_variant_remains_deserializable_with_untagged_variant() {
    let yaml = "- !Tagged example\n- 42\n";

    let actual: Vec<MixedTagging> = serde_saphyr::from_str(yaml)
        .expect("mixed tagged and untagged variants should deserialize");

    assert_eq!(
        actual,
        vec![
            MixedTagging::Tagged("example".to_owned()),
            MixedTagging::Untagged(42),
        ]
    );
}
