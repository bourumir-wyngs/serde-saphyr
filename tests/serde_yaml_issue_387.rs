#![cfg(feature = "deserialize")]

// Regression coverage for https://github.com/dtolnay/serde-yaml/issues/387.
use std::collections::BTreeMap;

#[test]
fn colon_hyphen_plain_scalar_deserializes() {
    // YAML 1.2 allows `:`, `?`, and `-` to start a plain scalar when the next
    // character is safe. Here the initial `:` is followed by `-`.
    let cases = [
        "filter: [:- .gitignore]\n",
        "filter:\n  - :- .gitignore\n",
        "filter: [\":- .gitignore\"]\n",
        "filter:\n  - \":- .gitignore\"\n",
    ];
    let expected = BTreeMap::from([("filter".to_owned(), vec![":- .gitignore".to_owned()])]);

    for yaml in cases {
        let actual: BTreeMap<String, Vec<String>> =
            serde_saphyr::from_str(yaml).expect("colon-hyphen scalar should deserialize");
        assert_eq!(actual, expected, "input: {yaml:?}");
    }
}
