#![cfg(feature = "deserialize")]

use std::collections::BTreeMap;

use rstest::rstest;
use serde::Deserialize;
use serde_json::{Value, json};
use serde_saphyr::{
    DuplicateKeyPolicy, Error, Options, from_reader_with_options, from_str_with_options,
};

fn preserve_both() -> Options {
    serde_saphyr::options! {
        leading_zero_numbers_as_strings: true,
        underscored_numbers_as_strings: true,
    }
}

fn assert_inferred(yaml: &str, expected: Value, options: Options) {
    let from_str: Value = from_str_with_options(yaml, options.clone()).unwrap();
    let from_reader: Value = from_reader_with_options(yaml.as_bytes(), options).unwrap();
    assert_eq!(from_str, expected, "string input: {yaml}");
    assert_eq!(from_reader, expected, "reader input: {yaml}");
}

#[test]
fn preservation_defaults_off() {
    let options = Options::default();
    assert!(!options.leading_zero_numbers_as_strings);
    assert!(!options.underscored_numbers_as_strings);

    for (yaml, expected) in [
        ("0755", json!(755.0)),
        ("-0755", json!(-755.0)),
        ("007", json!(7.0)),
        ("1_000", json!(1000)),
        ("0xFF_FF", json!(65535)),
    ] {
        assert_inferred(yaml, expected, options.clone());
    }
}

#[test]
fn leading_zero_option_preserves_signed_digit_only_spellings() {
    let options = serde_saphyr::options! { leading_zero_numbers_as_strings: true };
    for yaml in ["0755", "+0755", "-0755", "007", "00", "+00", "-00", "009"] {
        assert_inferred(yaml, json!(yaml), options.clone());
    }

    // The flags are independent: underscore-separated integers still infer as numbers.
    assert_inferred("1_000", json!(1000), options.clone());
    assert_inferred("0xFF_FF", json!(65535), options);
}

#[test]
fn underscore_option_preserves_decimal_and_radix_spellings() {
    let options = serde_saphyr::options! { underscored_numbers_as_strings: true };
    for yaml in [
        "1_000",
        "+1_000",
        "-1_000",
        "0xFF_FF",
        "-0xFF_FF",
        "0o7_55",
        "0b1010_0101",
        "1_000.0",
        "1_0e2",
        "word_with_underscores",
    ] {
        assert_inferred(yaml, json!(yaml), options.clone());
    }

    // The leading-zero option remains disabled.
    assert_inferred("0755", json!(755.0), options);
}

#[test]
fn ordinary_numbers_and_other_scalar_types_keep_their_inference() {
    for (yaml, expected) in [
        ("0", json!(0)),
        ("+0", json!(0)),
        ("-0", json!(0)),
        ("42", json!(42)),
        ("-42", json!(-42)),
        ("0.5", json!(0.5)),
        ("00.5", json!(0.5)),
        ("-00.5", json!(-0.5)),
        ("0e3", json!(0.0)),
        ("01e2", json!(100.0)),
        ("+01e2", json!(100.0)),
        ("0xFF", json!(255)),
        ("0o755", json!(493)),
        ("0b1010", json!(10)),
        ("true", json!(true)),
        ("null", Value::Null),
    ] {
        assert_inferred(yaml, expected, preserve_both());
    }
}

#[test]
fn quoted_and_explicit_string_scalars_keep_their_text() {
    for yaml in ["'0755'", "\"0755\"", "!!str 0755", "! 0755"] {
        assert_inferred(yaml, json!("0755"), preserve_both());
    }
    for yaml in ["'1_000'", "\"1_000\"", "!!str 1_000", "! 1_000"] {
        assert_inferred(yaml, json!("1_000"), preserve_both());
    }
}

#[test]
fn explicit_numeric_tags_take_precedence() {
    for (yaml, expected) in [
        ("!!int 1_000", json!(1000)),
        ("!!int -1_000", json!(-1000)),
        ("!!int 0xFF_FF", json!(65535)),
        ("!!float 0755", json!(755.0)),
        ("!!float -0755", json!(-755.0)),
        ("!!float '0755'", json!(755.0)),
    ] {
        assert_inferred(yaml, expected, preserve_both());
    }

    // Preservation cannot turn an invalid explicitly tagged number into a string.
    for yaml in ["!!int 0755", "!!int 1__000", "!!float 1_000.0"] {
        assert!(from_str_with_options::<Value>(yaml, preserve_both()).is_err());
        assert!(from_reader_with_options::<_, Value>(yaml.as_bytes(), preserve_both()).is_err());
    }
}

#[test]
fn direct_numeric_and_string_targets_keep_their_behavior() {
    let integer: i64 = from_str_with_options("-1_000", preserve_both()).unwrap();
    let radix: u64 = from_reader_with_options("0xFF_FF".as_bytes(), preserve_both()).unwrap();
    let float: f64 = from_str_with_options("0755", preserve_both()).unwrap();
    let text: String = from_str_with_options("0755", preserve_both()).unwrap();
    assert_eq!(integer, -1000);
    assert_eq!(radix, 65535);
    assert_eq!(float, 755.0);
    assert_eq!(text, "0755");
    assert!(from_str_with_options::<u64>("0755", preserve_both()).is_err());
}

#[test]
fn preservation_takes_precedence_over_implicit_legacy_octal() {
    let options = serde_saphyr::options! {
        leading_zero_numbers_as_strings: true,
        underscored_numbers_as_strings: true,
        legacy_octal_numbers: true,
    };
    for yaml in ["0755", "-0755", "009", "0_10", "+0_10", "0x_FF"] {
        assert_inferred(yaml, json!(yaml), options.clone());
    }
    assert_inferred("!!int 0755", json!(493), options.clone());
    assert_inferred("!!int 0_10", json!(8), options.clone());
    let typed: u64 = from_str_with_options("0755", options).unwrap();
    assert_eq!(typed, 493);
}

#[test]
fn leading_zero_option_does_not_capture_legacy_underscore_forms() {
    let options = serde_saphyr::options! {
        leading_zero_numbers_as_strings: true,
        legacy_octal_numbers: true,
    };
    assert_inferred("0_10", json!(8), options.clone());
    assert_inferred("0x_FF", json!(255), options);
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum NumberOrText {
    Number(f64),
    Text(String),
}

#[derive(Debug, Deserialize)]
struct FlattenedStrings {
    #[serde(flatten)]
    values: BTreeMap<String, String>,
}

#[test]
fn serde_buffering_observes_preservation_for_untagged_and_flattened_values() {
    for yaml in ["0755", "1_000"] {
        let default: NumberOrText = from_str_with_options(yaml, Options::default()).unwrap();
        assert!(matches!(default, NumberOrText::Number(_)));
        let preserved: NumberOrText = from_str_with_options(yaml, preserve_both()).unwrap();
        assert_eq!(preserved, NumberOrText::Text(yaml.to_owned()));
    }

    let yaml = "mode: 0755\ncount: 1_000\n";
    assert!(from_str_with_options::<FlattenedStrings>(yaml, Options::default()).is_err());
    let flattened: FlattenedStrings = from_str_with_options(yaml, preserve_both()).unwrap();
    assert_eq!(
        flattened.values,
        BTreeMap::from([
            ("mode".to_owned(), "0755".to_owned()),
            ("count".to_owned(), "1_000".to_owned()),
        ])
    );
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum BorrowedText<'a> {
    Text(#[serde(borrow)] &'a str),
}

#[test]
fn preserved_plain_strings_can_be_borrowed_through_deserialize_any() {
    for yaml in ["0755", "1_000"] {
        let BorrowedText::Text(text) =
            from_str_with_options::<BorrowedText<'_>>(yaml, preserve_both()).unwrap();
        assert_eq!(text, yaml);
        assert_eq!(text.as_ptr(), yaml.as_ptr());
    }
}

#[test]
fn aliases_and_merged_values_keep_numeric_spelling() {
    let yaml = "defaults: &defaults { mode: &mode 0755, count: 1_000 }\n\
                copy: *mode\n\
                target: { <<: *defaults }\n";
    let defaults = json!({"mode": "0755", "count": "1_000"});
    assert_inferred(
        yaml,
        json!({"defaults": defaults.clone(), "copy": "0755", "target": defaults}),
        preserve_both(),
    );
}

fn assert_distinct_key_spellings(keys: (&str, &str), options: Options) {
    for (first, second) in [keys, (keys.1, keys.0)] {
        let yaml = format!("{first}: first\n{second}: second\n");
        let expected = BTreeMap::from([
            (first.to_owned(), "first".to_owned()),
            (second.to_owned(), "second".to_owned()),
        ]);
        let actual: BTreeMap<String, String> =
            from_str_with_options(&yaml, options.clone()).unwrap();
        assert_eq!(actual, expected);
        assert_inferred(&yaml, json!(expected), options.clone());
    }
}

fn assert_duplicate_key(yaml: &str, options: Options) {
    for error in [
        from_str_with_options::<serde::de::IgnoredAny>(yaml, options.clone()).unwrap_err(),
        from_reader_with_options::<_, serde::de::IgnoredAny>(yaml.as_bytes(), options).unwrap_err(),
    ] {
        assert!(matches!(
            error.without_snippet(),
            Error::DuplicateMappingKey { .. }
        ));
    }
}

#[rstest]
#[case::decimal(("1000", "1_000"))]
#[case::positive(("+1000", "+1_000"))]
#[case::negative(("-1000", "-1_000"))]
#[case::hexadecimal(("0xFF", "0xF_F"))]
#[case::octal(("0o755", "0o7_55"))]
#[case::binary(("0b1010", "0b10_10"))]
fn underscored_mapping_keys_stay_distinct(
    #[case] keys: (&str, &str),
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins
    )]
    policy: DuplicateKeyPolicy,
) {
    let options = serde_saphyr::options! {
        underscored_numbers_as_strings: true,
        duplicate_keys: policy,
    };
    assert_distinct_key_spellings(keys, options);
}

#[rstest]
#[case::positive(("493", "0755"))]
#[case::negative(("-493", "-0755"))]
#[case::zero(("0", "+00"))]
fn leading_zero_mapping_keys_override_legacy_octal(
    #[case] keys: (&str, &str),
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins
    )]
    policy: DuplicateKeyPolicy,
) {
    let options = serde_saphyr::options! {
        leading_zero_numbers_as_strings: true,
        legacy_octal_numbers: true,
        duplicate_keys: policy,
    };
    assert_distinct_key_spellings(keys, options);
}

#[rstest]
#[case::leading_zero_only("1_000: first\n1000: second\n", true)]
#[case::underscore_only("0755: first\n493: second\n", false)]
fn key_preservation_flags_are_independent(#[case] yaml: &str, #[case] leading_zero: bool) {
    let options = serde_saphyr::options! {
        leading_zero_numbers_as_strings: leading_zero,
        underscored_numbers_as_strings: !leading_zero,
        legacy_octal_numbers: true,
    };
    assert_duplicate_key(yaml, options);
}

#[rstest]
#[case::underscore("!!int 1_000: first\n1000: second\n")]
#[case::quoted_underscore("!!int '1_000': first\n1000: second\n")]
#[case::leading_zero("!!int 0755: first\n493: second\n")]
#[case::quoted_leading_zero("!!int '0755': first\n493: second\n")]
fn explicit_integer_keys_keep_numeric_equality(#[case] yaml: &str) {
    let mut options = preserve_both();
    options.legacy_octal_numbers = true;
    assert_duplicate_key(yaml, options);
}

#[rstest]
#[case::plain(("1_000", "1_000"))]
#[case::quoted(("1_000", "'1_000'"))]
#[case::string_tag(("1_000", "!!str 1_000"))]
#[case::non_specific_tag(("1_000", "! 1_000"))]
#[case::leading_zero(("0755", "!!str 0755"))]
#[case::alias(("&key 1_000", "*key"))]
fn preserved_keys_still_detect_string_duplicates(
    #[case] keys: (&str, &str),
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins
    )]
    policy: DuplicateKeyPolicy,
) {
    let yaml = format!("{} : first\n{} : second\n", keys.0, keys.1);
    let mut options = preserve_both();
    options.legacy_octal_numbers = true;
    options.duplicate_keys = policy;
    if matches!(policy, DuplicateKeyPolicy::Error) {
        assert_duplicate_key(&yaml, options);
    } else {
        let spelling = if keys.0 == "0755" { "0755" } else { "1_000" };
        let value = if matches!(policy, DuplicateKeyPolicy::FirstWins) {
            "first"
        } else {
            "second"
        };
        assert_inferred(&yaml, json!({spelling: value}), options);
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(untagged)]
enum PreservedKey {
    Integer(i64),
    Text(String),
}

#[rstest]
#[case::underscore(("1_000", 1000))]
#[case::leading_zero(("0755", 493))]
fn explicit_integer_and_preserved_string_keys_stay_distinct(
    #[case] spelling: (&str, i64),
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins
    )]
    policy: DuplicateKeyPolicy,
) {
    let mut options = preserve_both();
    options.legacy_octal_numbers = true;
    options.duplicate_keys = policy;
    let yaml = format!("!!int {}: integer\n{}: string\n", spelling.0, spelling.0);
    let actual: BTreeMap<PreservedKey, String> = from_str_with_options(&yaml, options).unwrap();
    assert_eq!(
        actual,
        BTreeMap::from([
            (PreservedKey::Integer(spelling.1), "integer".to_owned()),
            (
                PreservedKey::Text(spelling.0.to_owned()),
                "string".to_owned()
            ),
        ])
    );
}

#[rstest]
#[case::underscore(("1000", "1_000"))]
#[case::leading_zero(("493", "0755"))]
fn preservation_applies_inside_composite_keys(
    #[case] keys: (&str, &str),
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins
    )]
    policy: DuplicateKeyPolicy,
) {
    let mut options = preserve_both();
    options.legacy_octal_numbers = true;
    options.duplicate_keys = policy;
    let yaml = format!("? [{}]\n: integer\n? [{}]\n: string\n", keys.0, keys.1);
    let actual: BTreeMap<Vec<String>, String> =
        from_str_with_options(&yaml, options.clone()).unwrap();
    let expected = BTreeMap::from([
        (vec![keys.0.to_owned()], "integer".to_owned()),
        (vec![keys.1.to_owned()], "string".to_owned()),
    ]);
    assert_eq!(actual, expected);

    // Values within mapping-valued keys participate in the outer key's identity.
    let yaml = format!(
        "? {{ids: [{}]}}\n: integer\n? {{ids: [{}]}}\n: string\n",
        keys.0, keys.1
    );
    type Mapping = BTreeMap<BTreeMap<String, Vec<String>>, String>;
    let actual: Mapping = from_str_with_options(&yaml, options).unwrap();
    let expected = expected
        .into_iter()
        .map(|(key, value)| (BTreeMap::from([("ids".to_owned(), key)]), value))
        .collect::<Mapping>();
    assert_eq!(actual, expected);
}

#[rstest]
#[case::explicit("<<: {1000: integer}\n1_000: string\n")]
#[case::merged("<<: {1_000: string}\n1000: integer\n")]
#[case::sequence("<<: [{1000: integer}, {1_000: string}]\n")]
#[case::alias("? &key 1_000\n: string\n<<: {1000: integer, *key : overridden}\n")]
fn preservation_applies_to_merge_key_identity(
    #[case] yaml: &str,
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins,
        DuplicateKeyPolicy::PassThrough
    )]
    policy: DuplicateKeyPolicy,
) {
    let mut options = preserve_both();
    options.duplicate_keys = policy;
    assert_inferred(yaml, json!({"1000": "integer", "1_000": "string"}), options);
}

#[rstest]
#[case::explicit("<<: {493: integer}\n0755: string\n")]
#[case::sequence("<<: [{493: integer}, {0755: string}]\n")]
fn leading_zero_preservation_overrides_legacy_octal_in_merges(#[case] yaml: &str) {
    let options = serde_saphyr::options! {
        leading_zero_numbers_as_strings: true,
        legacy_octal_numbers: true,
    };
    assert_inferred(yaml, json!({"493": "integer", "0755": "string"}), options);
}

#[rstest]
fn typed_numeric_keys_can_coalesce_distinct_preserved_keys(
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins
    )]
    policy: DuplicateKeyPolicy,
) {
    let mut options = preserve_both();
    options.duplicate_keys = policy;
    // Duplicate policies compare YAML keys before typed numeric conversion.
    let actual: BTreeMap<i64, String> =
        from_str_with_options("1000: first\n1_000: second\n", options).unwrap();
    assert_eq!(actual, BTreeMap::from([(1000, "second".to_owned())]));
}

#[rstest]
#[case::hexadecimal(("0X1F", 31))]
#[case::positive_octal(("+0O17", 15))]
#[case::negative_binary(("-0B101", -5))]
fn uppercase_radix_inference_keeps_its_distinct_key_policy(#[case] scalar: (&str, i64)) {
    assert_inferred(scalar.0, json!(scalar.0), preserve_both());
    let typed: i64 = from_str_with_options(scalar.0, preserve_both()).unwrap();
    assert_eq!(typed, scalar.1);
    let yaml = format!("{}: first\n{}: second\n", scalar.0, scalar.1);
    assert_duplicate_key(&yaml, preserve_both());
}

#[rstest]
#[case::positive(("18446744073709551616", "0x10000000000000000", 1_i128 << 64))]
#[case::negative(("-18446744073709551616", "-0x10000000000000000", -(1_i128 << 64)))]
fn integer_key_identity_keeps_a_wider_range_than_inference(#[case] scalar: (&str, &str, i128)) {
    // These powers of two are exact f64 values but outside the inferred integer range.
    assert_inferred(scalar.0, json!(scalar.2 as f64), preserve_both());
    let typed: i128 = from_str_with_options(scalar.0, preserve_both()).unwrap();
    assert_eq!(typed, scalar.2);
    let yaml = format!("{}: first\n{}: second\n", scalar.0, scalar.1);
    assert_duplicate_key(&yaml, preserve_both());
}

#[rstest]
fn integer_key_normalization_does_not_allow_negative_unsigned_zero(
    #[values("-0", "-0x0", "-0b0")] scalar: &str,
) {
    assert_inferred(scalar, json!(0), preserve_both());
    let signed: i128 = from_str_with_options(scalar, preserve_both()).unwrap();
    assert_eq!(signed, 0);
    assert!(from_str_with_options::<u128>(scalar, preserve_both()).is_err());
    let yaml = format!("{scalar}: first\n0: second\n");
    assert_duplicate_key(&yaml, preserve_both());
}

#[cfg(feature = "robotics")]
#[test]
fn underscore_preservation_precedes_implicit_robotics_float_parsing() {
    let mut options = preserve_both();
    options.angle_conversions = true;
    assert_inferred("1_000.0", json!("1_000.0"), options.clone());
    assert_inferred("!!float 1_000.0", json!(1000.0), options.clone());
    let typed: f64 = from_str_with_options("1_000.0", options).unwrap();
    assert_eq!(typed, 1000.0);
}

#[cfg(feature = "serde_derived_types")]
#[test]
fn serialized_options_round_trip_and_older_configurations_default_to_false() {
    let serialized = serde_json::to_value(preserve_both()).unwrap();
    let configured: Options = serde_json::from_value(serialized.clone()).unwrap();
    assert!(configured.leading_zero_numbers_as_strings);
    assert!(configured.underscored_numbers_as_strings);

    let mut older = serialized;
    let fields = older.as_object_mut().unwrap();
    fields.remove("leading_zero_numbers_as_strings");
    fields.remove("underscored_numbers_as_strings");
    let configured: Options = serde_json::from_value(older).unwrap();
    assert!(!configured.leading_zero_numbers_as_strings);
    assert!(!configured.underscored_numbers_as_strings);
}
