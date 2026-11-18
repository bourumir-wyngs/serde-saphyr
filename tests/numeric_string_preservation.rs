#![cfg(feature = "deserialize")]

use std::{borrow::Cow, collections::BTreeMap};

use rstest::rstest;
use serde::Deserialize;
use serde_json::{Value, json};
use serde_saphyr::{
    DefaultMessageFormatter, DuplicateKeyPolicy, Error, MessageFormatter, NonFiniteFloatPolicy,
    Options, UserMessageFormatter, from_reader_with_options, from_str_with_options,
};

fn preserve_both() -> Options {
    serde_saphyr::options! {
        leading_zero_numbers_as_strings: true,
        allow_numeric_underscores: false,
    }
}

fn assert_inferred(yaml: &str, expected: Value, options: Options) {
    let from_str: Value = from_str_with_options(yaml, options.clone()).unwrap();
    let from_reader: Value = from_reader_with_options(yaml.as_bytes(), options).unwrap();
    assert_eq!(from_str, expected, "string input: {yaml}");
    assert_eq!(from_reader, expected, "reader input: {yaml}");
}

#[track_caller]
fn assert_underscore_diagnostic(error: &Error) {
    assert!(matches!(
        error.without_snippet(),
        Error::NumericUnderscoresDisabled { .. }
    ));
    assert_eq!(
        DefaultMessageFormatter.format_message(error),
        "Underscores are disabled in configuration"
    );
    assert_eq!(
        UserMessageFormatter.format_message(error),
        "Underscores not allowed"
    );
    assert!(
        error
            .render()
            .contains("Underscores are disabled in configuration")
    );
    assert!(
        error
            .render_with_formatter(&UserMessageFormatter)
            .contains("Underscores not allowed")
    );
}

#[track_caller]
fn assert_invalid_scalar(error: &Error, expected_type: &str) {
    assert!(matches!(
        error.without_snippet(),
        Error::InvalidScalar { ty, .. } if *ty == expected_type
    ));
    let expected_message = format!("invalid {expected_type}");
    assert_eq!(
        DefaultMessageFormatter.format_message(error),
        expected_message
    );
    assert_eq!(UserMessageFormatter.format_message(error), expected_message);
}

#[track_caller]
fn assert_float_underscore_rejection(error: &Error, supports_underscores: bool) {
    if supports_underscores {
        assert_underscore_diagnostic(error);
    } else {
        // Without robotics conversions, the float syntax itself rejects underscores.
        assert_invalid_scalar(error, "floating point");
    }
}

#[test]
fn preservation_defaults_off() {
    let options = Options::default();
    assert!(!options.leading_zero_numbers_as_strings);
    assert!(options.allow_numeric_underscores);

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
    let options = serde_saphyr::options! { allow_numeric_underscores: false };
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

#[rstest]
#[case::positive("1_000e999")]
#[case::negative("-1_0e999")]
fn overflowing_underscored_exponents_stay_strings(
    #[case] scalar: &str,
    #[values(
        NonFiniteFloatPolicy::Default,
        NonFiniteFloatPolicy::Reject,
        NonFiniteFloatPolicy::PassThrough,
        NonFiniteFloatPolicy::AsString
    )]
    policy: NonFiniteFloatPolicy,
) {
    let options = serde_saphyr::options! {
        allow_numeric_underscores: false,
        non_finite_float_policy: policy,
        angle_conversions: cfg!(feature = "robotics"),
    };
    assert_inferred(scalar, json!(scalar), options.clone());
    assert_inferred(
        &format!("value: {scalar}\n"),
        json!({"value": scalar}),
        options.clone(),
    );

    // A numeric tag reports the underscore policy only if the enabled parser
    // accepts this spelling; ordinary float parsing rejects the syntax itself.
    let tagged = format!("!!float '{scalar}'");
    assert_float_underscore_rejection(
        &from_str_with_options::<Value>(&tagged, options.clone()).unwrap_err(),
        options.angle_conversions,
    );
    assert_float_underscore_rejection(
        &from_reader_with_options::<_, Value>(tagged.as_bytes(), options.clone()).unwrap_err(),
        options.angle_conversions,
    );
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
fn explicit_numeric_tags_obey_preservation_restrictions() {
    // Preservation cannot turn an invalid explicitly tagged number into a string.
    for yaml in [
        "!!int 0755",
        "!!float 0755",
        "!!float -0755",
        "!!float '0755'",
        "!!int 1_000",
        "!!int -1_000",
        "!!int 0xFF_FF",
        "!!int 1__000",
        "!!float 1_000.0",
    ] {
        assert!(from_str_with_options::<Value>(yaml, preserve_both()).is_err());
        assert!(from_reader_with_options::<_, Value>(yaml.as_bytes(), preserve_both()).is_err());
    }
}

#[test]
fn direct_numeric_and_string_targets_obey_preservation_restrictions() {
    assert!(from_str_with_options::<i64>("-1_000", preserve_both()).is_err());
    assert!(from_reader_with_options::<_, u64>("0xFF_FF".as_bytes(), preserve_both()).is_err());
    let text: String = from_str_with_options("0755", preserve_both()).unwrap();
    assert_eq!(text, "0755");
    assert!(from_str_with_options::<f64>("0755", preserve_both()).is_err());
    assert!(from_str_with_options::<u64>("0755", preserve_both()).is_err());
}

#[test]
fn preservation_takes_precedence_over_legacy_octal() {
    let options = serde_saphyr::options! {
        leading_zero_numbers_as_strings: true,
        allow_numeric_underscores: false,
        legacy_octal_numbers: true,
    };
    for yaml in ["0755", "-0755", "009", "0_10", "+0_10", "0x_FF"] {
        assert_inferred(yaml, json!(yaml), options.clone());
    }
    assert!(from_str_with_options::<Value>("!!int 0755", options.clone()).is_err());
    assert!(from_str_with_options::<Value>("!!int 0_10", options.clone()).is_err());
    assert!(from_str_with_options::<u64>("0755", options).is_err());
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

#[rstest]
#[case::decimal("1000", "1_000")]
#[case::hexadecimal("0xFF", "0xF_F")]
#[case::octal("0o755", "0o7_55")]
#[case::binary("0b1010", "0b10_10")]
fn underscored_mapping_keys_are_distinct_from_integers(
    #[case] integer: &str,
    #[case] text: &str,
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins,
        DuplicateKeyPolicy::PassThrough
    )]
    policy: DuplicateKeyPolicy,
) {
    let options = serde_saphyr::options! {
        allow_numeric_underscores: false,
        duplicate_keys: policy,
    };
    let expected = BTreeMap::from([
        (integer.to_owned(), "integer".to_owned()),
        (text.to_owned(), "text".to_owned()),
    ]);
    for yaml in [
        format!("{integer}: integer\n{text}: text\n"),
        format!("{text}: text\n{integer}: integer\n"),
    ] {
        let actual: BTreeMap<String, String> =
            from_str_with_options(&yaml, options.clone()).unwrap();
        assert_eq!(actual, expected);
        let actual: BTreeMap<String, String> =
            from_reader_with_options(yaml.as_bytes(), options.clone()).unwrap();
        assert_eq!(actual, expected);
        // Neither spelling order nor duplicate policy may hide an invalid integer key.
        assert!(from_str_with_options::<BTreeMap<i64, String>>(&yaml, options.clone()).is_err());
        assert!(
            from_reader_with_options::<_, BTreeMap<i64, String>>(yaml.as_bytes(), options.clone(),)
                .is_err()
        );
    }
}

#[rstest]
#[case::error(DuplicateKeyPolicy::Error, None)]
#[case::first(DuplicateKeyPolicy::FirstWins, Some("first"))]
#[case::last(DuplicateKeyPolicy::LastWins, Some("second"))]
fn repeated_underscored_string_keys_still_follow_duplicate_policy(
    #[case] policy: DuplicateKeyPolicy,
    #[case] winner: Option<&str>,
) {
    let yaml = "1_000: first\n1_000: second\n";
    let options = serde_saphyr::options! {
        allow_numeric_underscores: false,
        duplicate_keys: policy,
    };
    for result in [
        from_str_with_options::<BTreeMap<String, String>>(yaml, options.clone()),
        from_reader_with_options::<_, BTreeMap<String, String>>(yaml.as_bytes(), options),
    ] {
        if let Some(winner) = winner {
            assert_eq!(
                result.unwrap(),
                BTreeMap::from([("1_000".to_owned(), winner.to_owned())])
            );
        } else {
            assert!(matches!(
                result.unwrap_err().without_snippet(),
                serde_saphyr::Error::DuplicateMappingKey { .. }
            ));
        }
    }
}

#[rstest]
#[case::explicit_after_merge("<<: {1000: integer}\n1_000: text\n")]
#[case::explicit_before_merge("1_000: text\n<<: {1000: integer}\n")]
#[case::merged_underscore("<<: {1_000: text}\n1000: integer\n")]
#[case::merged_sources("<<: [{1000: integer}, {1_000: text}]\n")]
fn merges_keep_underscored_string_keys_distinct_and_reject_numeric_targets(
    #[case] yaml: &str,
    #[values(
        DuplicateKeyPolicy::Error,
        DuplicateKeyPolicy::FirstWins,
        DuplicateKeyPolicy::LastWins,
        DuplicateKeyPolicy::PassThrough
    )]
    policy: DuplicateKeyPolicy,
) {
    let options = serde_saphyr::options! {
        allow_numeric_underscores: false,
        duplicate_keys: policy,
    };
    let expected = BTreeMap::from([
        ("1000".to_owned(), "integer".to_owned()),
        ("1_000".to_owned(), "text".to_owned()),
    ]);
    let actual: BTreeMap<String, String> = from_str_with_options(yaml, options.clone()).unwrap();
    assert_eq!(actual, expected);
    let actual: BTreeMap<String, String> =
        from_reader_with_options(yaml.as_bytes(), options.clone()).unwrap();
    assert_eq!(actual, expected);
    assert!(from_str_with_options::<BTreeMap<i64, String>>(yaml, options.clone()).is_err());
    assert!(
        from_reader_with_options::<_, BTreeMap<i64, String>>(yaml.as_bytes(), options).is_err()
    );
}

#[rstest]
#[case::sequence("? [1000]\n: integer\n? [1_000]\n: text\n")]
#[case::nested_mapping("? [{1000: 1000}]\n: integer\n? [{1_000: 1_000}]\n: text\n")]
#[case::mapping_value("? [{id: 1000}]\n: integer\n? [{id: 1_000}]\n: text\n")]
fn composite_key_identity_obeys_underscore_restriction(#[case] yaml: &str) {
    // IgnoredAny still traverses keys and performs duplicate checking, including
    // numeric leaves inside nested mapping values.
    assert!(from_str_with_options::<serde::de::IgnoredAny>(yaml, preserve_both()).is_ok());
    assert!(
        from_reader_with_options::<_, serde::de::IgnoredAny>(yaml.as_bytes(), preserve_both())
            .is_ok()
    );
}

#[rstest]
#[case::explicit("? [1000]\n: integer\n? [1_000]\n: text\n")]
#[case::merged("<<: { [1000]: integer }\n? [1_000]\n: text\n")]
fn sequence_keys_keep_both_spellings_and_reject_integer_elements(#[case] yaml: &str) {
    let actual: BTreeMap<Vec<String>, String> =
        from_str_with_options(yaml, preserve_both()).unwrap();
    assert_eq!(
        actual,
        BTreeMap::from([
            (vec!["1000".to_owned()], "integer".to_owned()),
            (vec!["1_000".to_owned()], "text".to_owned()),
        ])
    );
    assert!(from_str_with_options::<BTreeMap<Vec<i64>, String>>(yaml, preserve_both()).is_err());
}

#[rstest]
#[case::plain("1_0")]
#[case::single_quoted("'1_0'")]
#[case::double_quoted("\"1_0\"")]
#[case::tagged("!!int 1_0")]
#[case::tagged_quoted("!!int '1_0'")]
#[case::verbatim_tag("!<tag:yaml.org,2002:int> 1_0")]
fn every_integer_target_rejects_disallowed_underscores(#[case] yaml: &str) {
    macro_rules! check {
        ($($ty:ty),+ $(,)?) => {$(
            assert_underscore_diagnostic(
                &from_str_with_options::<$ty>(yaml, preserve_both()).unwrap_err(),
            );
            assert_underscore_diagnostic(
                &from_reader_with_options::<_, $ty>(yaml.as_bytes(), preserve_both()).unwrap_err(),
            );
            let allowed: $ty = from_str_with_options(yaml, Options::default()).unwrap();
            assert_eq!(allowed, 10);
        )+};
    }
    check!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);
}

#[rstest]
#[case::non_numeric("not_a_number")]
#[case::underscore_only("_")]
#[case::repeated_separator("1__000")]
#[case::trailing_separator("1000_")]
#[case::leading_separator("_1000")]
#[case::invalid_radix_digit("0xF_G")]
fn invalid_integer_syntax_keeps_parser_error(
    #[case] scalar: &str,
    #[values(true, false)] allow_numeric_underscores: bool,
) {
    let options = serde_saphyr::options! { allow_numeric_underscores: allow_numeric_underscores };
    let key_yaml = format!("{scalar}: value\n");
    let value_yaml = format!("value: {scalar}\n");
    macro_rules! check {
        ($($ty:ty),+ $(,)?) => {$(
            for error in [
                from_str_with_options::<$ty>(scalar, options.clone()).unwrap_err(),
                from_reader_with_options::<_, $ty>(scalar.as_bytes(), options.clone()).unwrap_err(),
                from_str_with_options::<BTreeMap<$ty, String>>(&key_yaml, options.clone()).unwrap_err(),
                from_str_with_options::<BTreeMap<String, $ty>>(&value_yaml, options.clone()).unwrap_err(),
            ] {
                assert_invalid_scalar(&error, stringify!($ty));
            }
        )+};
    }
    check!(i32, u32);
}

#[rstest]
fn invalid_integer_ranges_keep_parser_error(
    #[values(true, false)] allow_numeric_underscores: bool,
) {
    let options = serde_saphyr::options! { allow_numeric_underscores: allow_numeric_underscores };
    macro_rules! check {
        ($ty:ty, $scalar:literal) => {
            assert_invalid_scalar(
                &from_str_with_options::<$ty>($scalar, options.clone()).unwrap_err(),
                stringify!($ty),
            );
            assert_invalid_scalar(
                &from_reader_with_options::<_, $ty>($scalar.as_bytes(), options.clone())
                    .unwrap_err(),
                stringify!($ty),
            );
        };
    }
    check!(u8, "1_000");
    check!(i8, "1_000");
    check!(u32, "-1_0");
    check!(u32, "4_294_967_296");
    check!(u32, "0x1_00000000");
    check!(i32, "2_147_483_648");
    check!(i32, "-2_147_483_649");
    check!(i32, "0x8000_0000");
    check!(i128, "170141183460469231731687303715884105728_0");
    check!(u128, "340282366920938463463374607431768211456_0");
}

#[rstest]
#[case::non_numeric("foo_bar")]
#[case::underscore_only("_")]
#[case::repeated_separator("1__000.0")]
#[case::invalid_exponent("1e_2")]
#[case::underscored_float("1_000.0")]
#[case::overflowing_exponent("1_000e999")]
fn invalid_standard_float_syntax_keeps_parser_error(
    #[case] yaml: &str,
    #[values(true, false)] allow_numeric_underscores: bool,
) {
    // Keep the standard float parser active even in builds with robotics enabled.
    let options = serde_saphyr::options! {
        allow_numeric_underscores: allow_numeric_underscores,
        angle_conversions: false,
    };
    for error in [
        from_str_with_options::<f32>(yaml, options.clone()).unwrap_err(),
        from_str_with_options::<f64>(yaml, options.clone()).unwrap_err(),
        from_reader_with_options::<_, f64>(yaml.as_bytes(), options).unwrap_err(),
    ] {
        assert_invalid_scalar(&error, "floating point");
    }
}

#[rstest]
#[case::decimal("1_000", 1000)]
#[case::negative("-1_000", -1000)]
#[case::hexadecimal("0xF_F", 255)]
#[case::octal("0o7_55", 493)]
#[case::binary("0b10_10", 10)]
#[case::legacy_octal("0_10", 8)]
#[case::legacy_prefix("0x_FF", 255)]
fn numeric_mapping_keys_and_values_obey_underscore_restriction(
    #[case] scalar: &str,
    #[case] expected: i64,
) {
    let mut options = serde_saphyr::options! { legacy_octal_numbers: true };
    let key_yaml = format!("{scalar}: value\n");
    let value_yaml = format!("value: {scalar}\n");
    let keys: BTreeMap<i64, String> = from_str_with_options(&key_yaml, options.clone()).unwrap();
    let values: BTreeMap<String, i64> =
        from_str_with_options(&value_yaml, options.clone()).unwrap();
    assert_eq!(keys[&expected], "value");
    assert_eq!(values["value"], expected);
    options.allow_numeric_underscores = false;
    let key_error =
        from_str_with_options::<BTreeMap<i64, String>>(&key_yaml, options.clone()).unwrap_err();
    assert_underscore_diagnostic(&key_error);
    let location = key_error.location().unwrap();
    assert_eq!((location.line(), location.column()), (1, 1));
    assert_eq!(location.span().offset(), 0);

    let value_error =
        from_str_with_options::<BTreeMap<String, i64>>(&value_yaml, options).unwrap_err();
    assert_underscore_diagnostic(&value_error);
    let location = value_error.location().unwrap();
    assert_eq!((location.line(), location.column()), (1, 8));
    assert_eq!(location.span().offset(), 7);
}

#[rstest]
#[case::plain("1_000.0")]
#[case::quoted("'1_000.0'")]
#[case::tagged("!!float 1_000.0")]
#[case::tagged_quoted("!!float '1_000.0'")]
#[case::exponent("1e1_0")]
#[case::fraction("1.0_0")]
fn float_targets_reject_disallowed_underscores(#[case] yaml: &str) {
    let mut options = preserve_both();
    options.angle_conversions = cfg!(feature = "robotics");
    assert_float_underscore_rejection(
        &from_str_with_options::<f32>(yaml, options.clone()).unwrap_err(),
        options.angle_conversions,
    );
    assert_float_underscore_rejection(
        &from_str_with_options::<f64>(yaml, options.clone()).unwrap_err(),
        options.angle_conversions,
    );
    assert_float_underscore_rejection(
        &from_reader_with_options::<_, f64>(yaml.as_bytes(), options.clone()).unwrap_err(),
        options.angle_conversions,
    );
}

#[rstest]
#[case::integer("!!int 1_000", true)]
#[case::signed_integer("!!int -1_000", true)]
#[case::float("!!float 1_000.0", false)]
#[case::quoted_float("!!float '1_000.0'", false)]
#[case::verbatim_float_tag("!<tag:yaml.org,2002:float> 1_000.0", false)]
fn explicit_numeric_tags_preserve_parser_diagnostic(#[case] yaml: &str, #[case] integer_tag: bool) {
    let mut options = preserve_both();
    options.angle_conversions = cfg!(feature = "robotics");
    for error in [
        from_str_with_options::<Value>(yaml, options.clone()).unwrap_err(),
        from_reader_with_options::<_, Value>(yaml.as_bytes(), options.clone()).unwrap_err(),
    ] {
        if integer_tag {
            assert_underscore_diagnostic(&error);
        } else {
            assert_float_underscore_rejection(&error, options.angle_conversions);
        }
    }
}

#[test]
fn float_value_underscore_diagnostic_preserves_source_location() {
    let yaml = "value:\n  1_000.0\n";
    let mut options = preserve_both();
    options.angle_conversions = cfg!(feature = "robotics");
    for error in [
        from_str_with_options::<BTreeMap<String, f64>>(yaml, options.clone()).unwrap_err(),
        from_reader_with_options::<_, BTreeMap<String, f64>>(yaml.as_bytes(), options.clone())
            .unwrap_err(),
    ] {
        assert_float_underscore_rejection(&error, options.angle_conversions);
        let location = error.location().unwrap();
        assert_eq!((location.line(), location.column()), (2, 3));
        assert_eq!(location.span().offset(), 9);
    }
}

#[rstest]
#[case::decimal("1_000")]
#[case::hexadecimal("0xF_F")]
#[case::signed("-1_000")]
fn no_schema_accepts_disallowed_underscore_numbers_as_strings(#[case] scalar: &str) {
    let options = serde_saphyr::options! {
        allow_numeric_underscores: false,
        no_schema: true,
    };
    assert_eq!(
        from_str_with_options::<String>(scalar, options.clone()).unwrap(),
        scalar
    );
    assert_eq!(
        from_str_with_options::<&str>(scalar, options.clone()).unwrap(),
        scalar
    );
    assert_eq!(
        from_str_with_options::<Cow<'_, str>>(scalar, options.clone()).unwrap(),
        scalar
    );
    let yaml = format!("{scalar}: {scalar}\n");
    let mapping: BTreeMap<&str, &str> = from_str_with_options(&yaml, options).unwrap();
    assert_eq!(mapping, BTreeMap::from([(scalar, scalar)]));
}

#[cfg(feature = "robotics")]
#[rstest]
#[case::plain("1_000.0", 1000.0)]
#[case::explicit_float("!!float 1_000.0", 1000.0)]
#[case::expression("1_0 + 2", 12.0)]
#[case::degrees("!degrees 1_80", std::f64::consts::PI)]
#[case::radians("!radians 1_0", 10.0)]
#[case::time("!time 1_0", 10.0)]
fn robotics_cannot_override_underscore_restriction(#[case] yaml: &str, #[case] expected: f64) {
    let mut options = serde_saphyr::options! { angle_conversions: true };
    let typed: f64 = from_str_with_options(yaml, options.clone()).unwrap();
    assert_eq!(typed, expected);
    options.allow_numeric_underscores = false;
    assert_underscore_diagnostic(&from_str_with_options::<f32>(yaml, options.clone()).unwrap_err());
    assert_underscore_diagnostic(&from_str_with_options::<f64>(yaml, options.clone()).unwrap_err());
    if yaml.starts_with('!') {
        assert_underscore_diagnostic(&from_str_with_options::<Value>(yaml, options).unwrap_err());
    } else {
        assert_inferred(yaml, json!(yaml), options);
    }
}

#[cfg(feature = "robotics")]
#[rstest]
#[case::non_numeric("foo_bar")]
#[case::underscore_only("_")]
#[case::invalid_separator("1__000.0")]
#[case::invalid_operand("1_000 + foo")]
#[case::tagged_non_numeric("!degrees foo_bar")]
fn invalid_robotics_syntax_keeps_parser_error(#[case] yaml: &str) {
    let mut options = serde_saphyr::options! { angle_conversions: true };
    let original = from_str_with_options::<f64>(yaml, options.clone()).unwrap_err();
    options.allow_numeric_underscores = false;
    for error in [
        from_str_with_options::<f64>(yaml, options.clone()).unwrap_err(),
        from_reader_with_options::<_, f64>(yaml.as_bytes(), options.clone()).unwrap_err(),
    ] {
        assert!(!matches!(
            error.without_snippet(),
            Error::NumericUnderscoresDisabled { .. }
        ));
        assert_eq!(
            std::mem::discriminant(error.without_snippet()),
            std::mem::discriminant(original.without_snippet()),
        );
        assert_eq!(
            DefaultMessageFormatter.format_message(&error),
            DefaultMessageFormatter.format_message(&original),
        );
        assert_eq!(
            UserMessageFormatter.format_message(&error),
            UserMessageFormatter.format_message(&original),
        );
    }
}

#[cfg(feature = "properties")]
#[rstest]
#[case::plain("${NUMBER}")]
#[case::tagged("!!int ${NUMBER}")]
fn interpolated_numbers_obey_underscore_restriction(#[case] yaml: &str) {
    let mut options =
        serde_saphyr::options! {}.with_properties(std::collections::HashMap::from([(
            "NUMBER".to_owned(),
            "1_000".to_owned(),
        )]));
    assert_eq!(
        from_str_with_options::<u64>(yaml, options.clone()).unwrap(),
        1000
    );
    options.allow_numeric_underscores = false;
    assert!(from_str_with_options::<u64>(yaml, options.clone()).is_err());
    if yaml.starts_with("!!int") {
        assert!(from_str_with_options::<Value>(yaml, options).is_err());
    } else {
        assert_inferred(yaml, json!("1_000"), options);
    }
}

#[cfg(feature = "serde_derived_types")]
#[test]
fn serialized_options_round_trip_and_missing_fields_use_defaults() {
    let serialized = serde_json::to_value(preserve_both()).unwrap();
    assert_eq!(serialized["allow_numeric_underscores"], json!(false));
    assert!(serialized.get("underscored_numbers_as_strings").is_none());
    let configured: Options = serde_json::from_value(serialized.clone()).unwrap();
    assert!(configured.leading_zero_numbers_as_strings);
    assert!(!configured.allow_numeric_underscores);

    let mut older = serialized;
    let fields = older.as_object_mut().unwrap();
    fields.remove("leading_zero_numbers_as_strings");
    fields.remove("allow_numeric_underscores");
    let configured: Options = serde_json::from_value(older).unwrap();
    assert!(!configured.leading_zero_numbers_as_strings);
    assert!(configured.allow_numeric_underscores);
}
