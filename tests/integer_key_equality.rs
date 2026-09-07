#![cfg(feature = "deserialize")]

use std::collections::BTreeMap;

use serde_saphyr::{
    DuplicateKeyPolicy, Error, from_reader, from_reader_with_options, from_str,
    from_str_with_options,
};

const EQUIVALENT_INTEGERS: &[(&str, &str)] = &[
    ("0xB", "11"),
    ("0XB", "+11"),
    ("0o13", "0b1011"),
    ("1_000", "1000"),
    ("-0xB", "-11"),
    ("-0", "+0"),
    ("0b0", "0"),
    ("0x8000000000000000", "9223372036854775808"),
    (
        "-0x80000000000000000000000000000000",
        "-170141183460469231731687303715884105728",
    ),
    (
        "0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
        "340282366920938463463374607431768211455",
    ),
];

#[test]
fn equivalent_integer_keys_are_duplicates_for_string_targets() {
    for &(first, second) in EQUIVALENT_INTEGERS {
        let yaml = format!("{first}: first\n{second}: second\n");
        let error = from_str::<BTreeMap<String, String>>(&yaml)
            .expect_err("integer key identity must use its parsed value");
        assert!(
            matches!(error.without_snippet(), Error::DuplicateMappingKey { .. }),
            "expected duplicate keys for {first} and {second}: {error}"
        );
    }
}

#[test]
fn duplicate_policy_keeps_the_winning_integer_key_spelling() {
    for &(first, second) in EQUIVALENT_INTEGERS {
        let yaml = format!("{first}: first\n{second}: second\n");
        for (policy, spelling, value) in [
            (DuplicateKeyPolicy::FirstWins, first, "first"),
            (DuplicateKeyPolicy::LastWins, second, "second"),
        ] {
            let options = serde_saphyr::options! { duplicate_keys: policy };
            let actual: BTreeMap<String, String> = from_str_with_options(&yaml, options)
                .expect("configured policy must resolve equivalent integer keys");
            assert_eq!(
                actual,
                BTreeMap::from([(spelling.to_owned(), value.to_owned())]),
                "wrong winner for {first} and {second} under {policy:?}"
            );
        }
    }
}

#[test]
fn equivalent_integer_keys_are_duplicates_for_numeric_targets() {
    let error = from_str::<BTreeMap<i32, String>>("0xB: first\n11: second\n")
        .expect_err("numeric targets must also receive duplicate-key checking");
    assert!(matches!(
        error.without_snippet(),
        Error::DuplicateMappingKey { .. }
    ));
}

#[test]
fn integer_values_deserialized_as_strings_preserve_their_spelling() {
    for &(first, second) in EQUIVALENT_INTEGERS {
        for spelling in [first, second] {
            assert_eq!(from_str::<String>(spelling).unwrap(), spelling);
            assert_eq!(from_str::<&str>(spelling).unwrap(), spelling);
        }
    }
}

#[test]
fn quoted_and_explicit_string_keys_remain_strings() {
    for string_key in ["'11'", "\"11\"", "!!str 11"] {
        let yaml = format!("0xB: integer\n{string_key}: string\n");
        let actual: BTreeMap<String, String> = from_str(&yaml).unwrap();
        assert_eq!(
            actual,
            BTreeMap::from([
                ("0xB".to_owned(), "integer".to_owned()),
                ("11".to_owned(), "string".to_owned()),
            ])
        );

        // The YAML keys differ in type even when their original text matches.
        // After String conversion, the target map itself keeps the later value.
        let yaml = format!("11: integer\n{string_key}: string\n");
        let actual: BTreeMap<String, String> = from_str(&yaml).unwrap();
        assert_eq!(
            actual,
            BTreeMap::from([("11".to_owned(), "string".to_owned())])
        );
    }
}

#[test]
fn explicit_integer_tags_use_integer_key_equality() {
    for integer_key in ["!!int 0xB", "!!int '0xB'", "!<tag:yaml.org,2002:int> 0xB"] {
        let yaml = format!("{integer_key}: first\n11: second\n");
        let error = from_str::<BTreeMap<i32, String>>(&yaml)
            .expect_err("explicit and implicit integers must have the same key identity");
        assert!(matches!(
            error.without_snippet(),
            Error::DuplicateMappingKey { .. }
        ));
    }
}

#[test]
fn distinct_large_integers_do_not_lose_precision() {
    let yaml = "9007199254740992: first\n9007199254740993: second\n";
    let actual: BTreeMap<u64, String> = from_str(yaml).unwrap();
    assert_eq!(actual.len(), 2);
    assert_eq!(actual[&9007199254740992], "first");
    assert_eq!(actual[&9007199254740993], "second");
}

#[test]
fn integer_equality_applies_inside_sequence_and_mapping_keys() {
    let sequence_keys = "? [0xB, -0]\n: first\n? [11, +0]\n: second\n";
    let error = from_str::<BTreeMap<Vec<String>, String>>(sequence_keys).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::DuplicateMappingKey { .. }
    ));

    let mapping_keys = "? {0xB: 0o13}\n: first\n? {11: 11}\n: second\n";
    let error = from_str::<BTreeMap<BTreeMap<String, String>, String>>(mapping_keys).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::DuplicateMappingKey { .. }
    ));
}

#[test]
fn aliased_integer_keys_use_integer_equality() {
    let yaml = "? &integer 0xB\n: first\n? *integer\n: second\n11: third\n";
    let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins };
    let actual: BTreeMap<String, String> = from_str_with_options(yaml, options).unwrap();
    assert_eq!(
        actual,
        BTreeMap::from([("11".to_owned(), "third".to_owned())])
    );
}

#[test]
fn explicit_integer_keys_override_equivalent_merged_keys() {
    for policy in [
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins,
    ] {
        for yaml in [
            "<<: {0xB: merged}\n11: explicit\n",
            "11: explicit\n<<: {0xB: merged}\n",
        ] {
            let options = serde_saphyr::options! { duplicate_keys: policy };
            let actual: BTreeMap<String, String> = from_str_with_options(yaml, options).unwrap();
            assert_eq!(
                actual,
                BTreeMap::from([("11".to_owned(), "explicit".to_owned())])
            );
        }
    }
}

#[test]
fn equivalent_merged_integer_keys_keep_the_earlier_source_spelling() {
    let yaml = "<<: [{0xB: first}, {11: second}]\n";
    for policy in [
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins,
    ] {
        let options = serde_saphyr::options! { duplicate_keys: policy };
        let actual: BTreeMap<String, String> = from_str_with_options(yaml, options).unwrap();
        assert_eq!(
            actual,
            BTreeMap::from([("0xB".to_owned(), "first".to_owned())])
        );
    }
}

#[test]
fn integer_key_equality_honors_legacy_octal_configuration() {
    let yaml = "013: first\n11: second\n";
    let actual: BTreeMap<String, String> = from_str(yaml).unwrap();
    assert_eq!(actual.len(), 2);

    for legacy in ["013", "0_13", "0x_B", "+0o_13", "0b_1011"] {
        let yaml = format!("{legacy}: first\n11: second\n");
        let options = serde_saphyr::options! { legacy_octal_numbers: true };
        let error = from_str_with_options::<BTreeMap<String, String>>(&yaml, options).unwrap_err();
        assert!(matches!(
            error.without_snippet(),
            Error::DuplicateMappingKey { .. }
        ));
    }
}

#[test]
fn integers_outside_the_supported_range_keep_lexical_key_identity() {
    for (decimal, hexadecimal) in [
        (
            "340282366920938463463374607431768211456",
            "0x100000000000000000000000000000000",
        ),
        (
            "-170141183460469231731687303715884105729",
            "-0x80000000000000000000000000000001",
        ),
    ] {
        let yaml = format!("{decimal}: decimal\n{hexadecimal}: hexadecimal\n");
        let actual: BTreeMap<String, String> = from_str(&yaml).unwrap();
        assert_eq!(
            actual,
            BTreeMap::from([
                (decimal.to_owned(), "decimal".to_owned()),
                (hexadecimal.to_owned(), "hexadecimal".to_owned()),
            ])
        );
    }
}

#[test]
fn reader_integer_keys_follow_duplicate_policy_and_preserve_spelling() {
    let yaml = "0xB: first\n11: second\n";
    let error = from_reader::<_, BTreeMap<String, String>>(yaml.as_bytes()).unwrap_err();
    assert!(matches!(
        error.without_snippet(),
        Error::DuplicateMappingKey { .. }
    ));
    for (policy, spelling, value) in [
        (DuplicateKeyPolicy::FirstWins, "0xB", "first"),
        (DuplicateKeyPolicy::LastWins, "11", "second"),
    ] {
        let options = serde_saphyr::options! { duplicate_keys: policy };
        let actual: BTreeMap<String, String> =
            from_reader_with_options(yaml.as_bytes(), options).unwrap();
        assert_eq!(
            actual,
            BTreeMap::from([(spelling.to_owned(), value.to_owned())])
        );
    }
}

#[test]
fn duplicate_integer_error_preserves_original_spelling_and_location() {
    let yaml = "root:\n  11: first\n  0xB: second\n";
    type Document = BTreeMap<String, BTreeMap<String, String>>;
    for error in [
        from_str::<Document>(yaml).unwrap_err(),
        from_reader::<_, Document>(yaml.as_bytes()).unwrap_err(),
    ] {
        assert!(
            matches!(
                error.without_snippet(),
                Error::DuplicateMappingKey { key: Some(key), location, .. }
                    if key == "0xB" && location.line() == 3 && location.column() == 3
            ),
            "unexpected duplicate diagnostic: {error}"
        );
    }
}

#[test]
fn legacy_octal_equality_applies_inside_composite_keys() {
    let yaml = "? [{013: 013}]\n: first\n? [{11: 11}]\n: second\n";
    type Mapping = BTreeMap<Vec<BTreeMap<String, String>>, String>;
    for policy in [DuplicateKeyPolicy::Error, DuplicateKeyPolicy::LastWins] {
        let options = serde_saphyr::options! {
            duplicate_keys: policy,
            legacy_octal_numbers: true,
        };
        let result = from_str_with_options::<Mapping>(yaml, options);
        if matches!(policy, DuplicateKeyPolicy::Error) {
            assert!(matches!(
                result.unwrap_err().without_snippet(),
                Error::DuplicateMappingKey { .. }
            ));
        } else {
            assert_eq!(
                result.unwrap(),
                BTreeMap::from([(
                    vec![BTreeMap::from([("11".to_owned(), "11".to_owned())])],
                    "second".to_owned(),
                )])
            );
        }
    }
}

#[test]
fn legacy_octal_equality_applies_to_merge_precedence() {
    for policy in [DuplicateKeyPolicy::Error, DuplicateKeyPolicy::LastWins] {
        for (yaml, spelling, value) in [
            ("<<: [{013: first}, {11: second}]\n", "013", "first"),
            ("<<: {013: merged}\n11: explicit\n", "11", "explicit"),
        ] {
            let options = serde_saphyr::options! {
                duplicate_keys: policy,
                legacy_octal_numbers: true,
            };
            let actual: BTreeMap<String, String> = from_str_with_options(yaml, options).unwrap();
            assert_eq!(
                actual,
                BTreeMap::from([(spelling.to_owned(), value.to_owned())])
            );
        }
    }
}

#[test]
fn non_specific_and_custom_tagged_keys_keep_lexical_identity() {
    for tag in ["!", "!custom"] {
        let yaml = format!("{tag} 0xB: first\n{tag} 11: second\n");
        let actual: BTreeMap<String, String> = from_str(&yaml).unwrap();
        assert_eq!(
            actual,
            BTreeMap::from([
                ("0xB".to_owned(), "first".to_owned()),
                ("11".to_owned(), "second".to_owned()),
            ])
        );
    }
}

#[test]
fn last_wins_handles_string_duplicates_around_integer_duplicates() {
    let yaml = "ordinary: first\n0xB: first integer\n11: last integer\nordinary: last\n";
    let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins };
    let actual: BTreeMap<String, String> = from_str_with_options(yaml, options).unwrap();
    assert_eq!(
        actual,
        BTreeMap::from([
            ("ordinary".to_owned(), "last".to_owned()),
            ("11".to_owned(), "last integer".to_owned()),
        ])
    );
}

#[test]
fn last_wins_keeps_merge_precedence_across_the_first_integer_key() {
    for yaml in [
        "prior: explicit\n\
         <<: {prior: old, shared: first, 0xB: old}\n\
         11: integer\n\
         <<: {prior: later, shared: second, later: added}\n",
        "prior: explicit\n\
         11: integer\n\
         <<: {prior: old, shared: first, 0xB: old}\n\
         <<: {prior: later, shared: second, later: added}\n",
    ] {
        let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins };
        let actual: BTreeMap<String, String> = from_str_with_options(yaml, options).unwrap();
        assert_eq!(
            actual,
            BTreeMap::from([
                ("prior".to_owned(), "explicit".to_owned()),
                ("shared".to_owned(), "first".to_owned()),
                ("11".to_owned(), "integer".to_owned()),
                ("later".to_owned(), "added".to_owned()),
            ])
        );
    }
}

#[test]
fn last_wins_preserves_trailing_comments_in_ordinary_string_key_maps() {
    use serde_saphyr::Commented;

    let yaml = "a: 1 # first\na: 2 # second\nb: 3 # third\n";
    let options = serde_saphyr::options! { duplicate_keys: DuplicateKeyPolicy::LastWins };
    let actual: BTreeMap<String, Commented<i32>> = from_str_with_options(yaml, options).unwrap();
    assert_eq!(
        actual,
        BTreeMap::from([
            ("a".to_owned(), Commented(2, "second".to_owned())),
            ("b".to_owned(), Commented(3, "third".to_owned())),
        ])
    );
}
