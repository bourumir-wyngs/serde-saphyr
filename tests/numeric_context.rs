#![cfg(feature = "deserialize")]

use std::{borrow::Cow, collections::BTreeMap};

use rstest::rstest;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use serde_saphyr::{
    DuplicateKeyPolicy, Error, NonFiniteFloatPolicy, from_reader_with_options,
    from_str_with_options,
};

#[rstest]
#[case::plain("010")]
#[case::quoted("'010'")]
#[case::double_quoted("\"010\"")]
#[case::tagged("!!int 010")]
#[case::tagged_quoted("!!int '010'")]
fn all_integer_targets_obey_leading_zero_restriction(
    #[case] yaml: &str,
    #[values(false, true)] legacy_octal_numbers: bool,
) {
    let options = serde_saphyr::options! {
        leading_zero_numbers_as_strings: true,
        legacy_octal_numbers: legacy_octal_numbers,
    };
    macro_rules! check {
        ($($ty:ty),+ $(,)?) => {$(
            for result in [
                from_str_with_options::<$ty>(yaml, options.clone()),
                from_reader_with_options::<_, $ty>(yaml.as_bytes(), options.clone()),
            ] {
                assert!(matches!(
                    result.unwrap_err().without_snippet(),
                    Error::InvalidScalar { .. }
                ));
            }
        )+};
    }
    check!(
        i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
    );
}

#[rstest]
#[case::plain("010")]
#[case::negative("-010")]
#[case::positive_zero("+00")]
#[case::negative_zero("-00")]
#[case::quoted("'010'")]
#[case::tagged("!!float 010")]
#[case::tagged_quoted("!!float '010'")]
/// `yaml` supplies the spelling and tag; `legacy_octal_numbers` and
/// `angle_conversions` exercise both parser settings. Returns after asserting rejection.
fn float_targets_obey_leading_zero_restriction(
    #[case] yaml: &str,
    #[values(false, true)] legacy_octal_numbers: bool,
    #[values(false, true)] angle_conversions: bool,
) {
    let options = serde_saphyr::options! {
        leading_zero_numbers_as_strings: true,
        legacy_octal_numbers: legacy_octal_numbers,
        angle_conversions: angle_conversions,
    };
    assert!(from_str_with_options::<f32>(yaml, options.clone()).is_err());
    assert!(from_str_with_options::<f64>(yaml, options.clone()).is_err());
    assert!(from_reader_with_options::<_, f64>(yaml.as_bytes(), options.clone()).is_err());
    if yaml.starts_with("!!float") {
        assert!(from_str_with_options::<Value>(yaml, options).is_err());
    }
}

#[rstest]
#[case::zero("0", 0.0)]
#[case::negative_zero("-0", -0.0)]
#[case::fraction("00.5", 0.5)]
#[case::exponent("01e2", 100.0)]
#[case::hexadecimal("0xF_F", 255.0)]
#[case::binary("0b1_000", 8.0)]
#[case::octal("0o10", 8.0)]
#[case::legacy_underscore("0_10", 8.0)]
fn allowed_spellings_use_shared_integer_and_float_helpers(
    #[case] scalar: &str,
    #[case] expected: f64,
) {
    let options = serde_saphyr::options! {
        leading_zero_numbers_as_strings: true,
        legacy_octal_numbers: true,
    };
    for yaml in [scalar.to_owned(), format!("!!float '{scalar}'")] {
        assert_eq!(
            from_str_with_options::<f64>(&yaml, options.clone()).unwrap(),
            expected
        );
        assert_eq!(
            from_reader_with_options::<_, f32>(yaml.as_bytes(), options.clone()).unwrap(),
            expected as f32
        );
    }
}

#[rstest]
#[case::underscores("1_000", false, false)]
#[case::uppercase_hexadecimal("0X10", true, true)]
#[case::uppercase_binary("0B10", true, true)]
#[case::uppercase_octal("0O10", true, true)]
/// `scalar` supplies the spelling; `allow_underscores` and `lowercase_prefixes`
/// select its restriction; `angle_conversions` exercises both float parsers.
/// Returns after checking that both float widths preserve the policy diagnostic.
fn float_conversion_cannot_bypass_integer_syntax_restrictions(
    #[case] scalar: &str,
    #[case] allow_underscores: bool,
    #[case] lowercase_prefixes: bool,
    #[values(false, true)] angle_conversions: bool,
) {
    let options = serde_saphyr::options! {
        allow_numeric_underscores: allow_underscores,
        lowercase_radix_prefixes_only: lowercase_prefixes,
        angle_conversions: angle_conversions,
    };
    for yaml in [scalar.to_owned(), format!("!!float '{scalar}'")] {
        for error in [
            from_str_with_options::<f32>(&yaml, options.clone()).unwrap_err(),
            from_str_with_options::<f64>(&yaml, options.clone()).unwrap_err(),
            from_reader_with_options::<_, f64>(yaml.as_bytes(), options.clone()).unwrap_err(),
        ] {
            if allow_underscores {
                assert!(matches!(
                    error.without_snippet(),
                    Error::UppercaseRadixPrefixDisabled { .. }
                ));
            } else {
                assert!(matches!(
                    error.without_snippet(),
                    Error::NumericUnderscoresDisabled { .. }
                ));
            }
        }
    }
}

#[rstest]
#[case::sequence("? [010]\n: text\n? [8]\n: integer\n")]
#[case::nested_mapping("? [{010: 010}]\n: text\n? [{8: 8}]\n: integer\n")]
#[case::mapping_value("? [{id: 010}]\n: text\n? [{id: 8}]\n: integer\n")]
fn composite_key_identity_obeys_leading_zero_restriction(#[case] yaml: &str) {
    let options = serde_saphyr::options! {
        leading_zero_numbers_as_strings: true,
        legacy_octal_numbers: true,
    };
    assert!(from_str_with_options::<serde::de::IgnoredAny>(yaml, options.clone()).is_ok());
    assert!(from_reader_with_options::<_, serde::de::IgnoredAny>(yaml.as_bytes(), options).is_ok());
}

#[rstest]
#[case::explicit("? [010]\n: text\n? [8]\n: integer\n")]
#[case::reversed("? [8]\n: integer\n? [010]\n: text\n")]
#[case::merged_integer("<<: { [8]: integer }\n? [010]\n: text\n")]
#[case::merged_text("<<: { [010]: text }\n? [8]\n: integer\n")]
#[case::merged_sources("<<: [{ [010]: text }, { [8]: integer }]\n")]
fn leading_zero_sequence_keys_stay_distinct_and_reject_numeric_elements(
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
        leading_zero_numbers_as_strings: true,
        legacy_octal_numbers: true,
        duplicate_keys: policy,
    };
    let expected = BTreeMap::from([
        (vec!["010".to_owned()], "text".to_owned()),
        (vec!["8".to_owned()], "integer".to_owned()),
    ]);
    for result in [
        from_str_with_options::<BTreeMap<Vec<String>, String>>(yaml, options.clone()),
        from_reader_with_options::<_, BTreeMap<Vec<String>, String>>(
            yaml.as_bytes(),
            options.clone(),
        ),
    ] {
        assert_eq!(result.unwrap(), expected);
    }
    assert!(from_str_with_options::<BTreeMap<Vec<i64>, String>>(yaml, options.clone()).is_err());
    assert!(
        from_reader_with_options::<_, BTreeMap<Vec<i64>, String>>(yaml.as_bytes(), options)
            .is_err()
    );
}

#[rstest]
#[case::positive("010")]
#[case::negative("-010")]
#[case::zero("+00")]
fn no_schema_accepts_preserved_leading_zero_strings(#[case] scalar: &str) {
    let options = serde_saphyr::options! {
        leading_zero_numbers_as_strings: true,
        legacy_octal_numbers: true,
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

#[rstest]
#[case::legacy_octal("0_10")]
#[case::legacy_hex_separator("0x_FF")]
#[case::large_unsigned("0xffffffffffffffffffffffffffffffff")]
fn no_schema_recognizes_all_supported_integer_spellings(#[case] scalar: &str) {
    let options = serde_saphyr::options! {
        legacy_octal_numbers: true,
        no_schema: true,
    };
    assert!(from_str_with_options::<u128>(scalar, options.clone()).is_ok());
    for error in [
        from_str_with_options::<String>(scalar, options.clone()).unwrap_err(),
        from_str_with_options::<&str>(scalar, options.clone()).unwrap_err(),
        from_str_with_options::<Cow<'_, str>>(scalar, options.clone()).unwrap_err(),
        from_reader_with_options::<_, String>(scalar.as_bytes(), options).unwrap_err(),
    ] {
        assert!(matches!(
            error.without_snippet(),
            Error::QuotingRequired { value, .. } if value == scalar
        ));
    }
}

#[test]
fn no_schema_recognizes_overflowing_float_literals() {
    let options = serde_saphyr::options! { no_schema: true };
    for error in [
        from_str_with_options::<String>("1e999", options.clone()).unwrap_err(),
        from_reader_with_options::<_, String>(b"1e999".as_slice(), options).unwrap_err(),
    ] {
        assert!(matches!(
            error.without_snippet(),
            Error::QuotingRequired { value, .. } if value == "1e999"
        ));
    }
}

#[rstest]
fn overflowing_leading_zero_integers_stay_strings(
    #[values(
        NonFiniteFloatPolicy::Default,
        NonFiniteFloatPolicy::Reject,
        NonFiniteFloatPolicy::PassThrough,
        NonFiniteFloatPolicy::AsString
    )]
    policy: NonFiniteFloatPolicy,
) {
    let scalar = format!("0{}", "9".repeat(400));
    let options = serde_saphyr::options! {
        leading_zero_numbers_as_strings: true,
        non_finite_float_policy: policy,
    };
    for result in [
        from_str_with_options::<Value>(&scalar, options.clone()),
        from_reader_with_options::<_, Value>(scalar.as_bytes(), options.clone()),
    ] {
        assert_eq!(result.unwrap(), Value::String(scalar.clone()));
    }
}

#[rstest]
#[case::decimal("-0")]
#[case::hexadecimal("-0x0")]
#[case::octal("-0o0")]
#[case::binary("-0b0")]
fn integer_to_float_conversion_preserves_negative_zero(#[case] scalar: &str) {
    let value: f64 = from_str_with_options(scalar, serde_saphyr::options! {}).unwrap();
    assert_eq!(value.to_bits(), (-0.0_f64).to_bits());
}

#[cfg(feature = "robotics")]
#[test]
fn robotics_unit_conversion_follows_shared_integer_parsing() {
    let options = serde_saphyr::options! { angle_conversions: true };
    for scalar in ["!degrees 0xB", "!degrees 1_1"] {
        let value: f64 = from_str_with_options(scalar, options.clone()).unwrap();
        assert_eq!(value, 11.0 * (std::f64::consts::PI / 180.0));
    }
}

#[cfg(feature = "robotics")]
#[rstest]
#[case::radix("0xffffffffffffffffffffffffffffffff")]
#[case::tagged_radix("!!float 0xffffffffffffffffffffffffffffffff")]
#[case::quoted_radix("'0xffffffffffffffffffffffffffffffff'")]
#[case::decimal("340282366920938463463374607431768211455")]
#[case::positive_exponent("1e39")]
#[case::negative_exponent("-1e39")]
#[case::expression("1e38 * 10")]
#[case::radians("!radians 0xffffffffffffffffffffffffffffffff")]
#[case::degrees("!degrees 1e41")]
fn robotics_float_conversion_rejects_f32_overflow(#[case] scalar: &str) {
    let options = serde_saphyr::options! { angle_conversions: true };
    let value: f64 = from_str_with_options(scalar, options.clone()).unwrap();
    assert!(value.is_finite());
    assert!((value as f32).is_infinite());

    for error in [
        from_str_with_options::<f32>(scalar, options.clone()).unwrap_err(),
        from_reader_with_options::<_, f32>(scalar.as_bytes(), options).unwrap_err(),
    ] {
        assert!(matches!(
            error.without_snippet(),
            Error::InvalidScalar {
                ty: "floating point",
                ..
            }
        ));
    }
}

#[cfg(feature = "robotics")]
#[test]
fn robotics_float_range_check_follows_unit_conversion() {
    let scalar = "!degrees 0xffffffffffffffffffffffffffffffff";
    let options = serde_saphyr::options! { angle_conversions: true };
    let expected: f64 = from_str_with_options(scalar, options.clone()).unwrap();
    for value in [
        from_str_with_options::<f32>(scalar, options.clone()).unwrap(),
        from_reader_with_options::<_, f32>(scalar.as_bytes(), options).unwrap(),
    ] {
        assert!(value.is_finite());
        assert_eq!(value, expected as f32);
    }
}

#[cfg(feature = "robotics")]
#[rstest]
#[case::positive_infinity(".inf", f32::INFINITY)]
#[case::negative_infinity("-.inf", f32::NEG_INFINITY)]
#[case::nan(".nan", f32::NAN)]
#[case::division_by_zero("1/0", f32::INFINITY)]
#[case::nan_expression("nan + 1", f32::NAN)]
fn robotics_float_conversion_preserves_non_finite_values(
    #[case] scalar: &str,
    #[case] expected: f32,
) {
    let options = serde_saphyr::options! { angle_conversions: true };
    for value in [
        from_str_with_options::<f32>(scalar, options.clone()).unwrap(),
        from_reader_with_options::<_, f32>(scalar.as_bytes(), options).unwrap(),
    ] {
        if expected.is_nan() {
            assert!(value.is_nan());
        } else {
            assert_eq!(value, expected);
        }
    }
}

/// Store the bits so a float visitor can be used inside an ordered composite key.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FloatKey(u64);

impl<'de> Deserialize<'de> for FloatKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        f64::deserialize(deserializer).map(|value| Self(value.to_bits()))
    }
}

#[rstest]
#[case::plain("010")]
#[case::tagged("!!float 010")]
fn float_targets_interpret_legacy_octal_like_integers(#[case] yaml: &str) {
    let options = serde_saphyr::options! { legacy_octal_numbers: true };
    assert_eq!(
        from_str_with_options::<f64>(yaml, options.clone()).unwrap(),
        8.0
    );
    assert_eq!(
        from_reader_with_options::<_, f32>(yaml.as_bytes(), options).unwrap(),
        8.0
    );
}

#[rstest]
#[case::invalid_digit("09".to_owned())]
#[case::invalid_underscored_digit("0_9".to_owned())]
#[case::integer_overflow(format!("0{}", "7".repeat(43)))]
#[case::float_overflow(format!("0{}", "7".repeat(310)))]
fn invalid_legacy_octal_cannot_fall_back_to_decimal_float(
    #[case] scalar: String,
    #[values("", "-")] sign: &str,
) {
    let scalar = format!("{sign}{scalar}");
    let options = serde_saphyr::options! {
        legacy_octal_numbers: true,
        angle_conversions: cfg!(feature = "robotics"),
    };
    for result in [
        from_str_with_options::<Value>(&scalar, options.clone()),
        from_reader_with_options::<_, Value>(scalar.as_bytes(), options.clone()),
    ] {
        assert_eq!(result.unwrap(), Value::String(scalar.clone()));
    }
    assert!(from_str_with_options::<f32>(&scalar, options.clone()).is_err());
    assert!(from_str_with_options::<f64>(&scalar, options.clone()).is_err());
    let tagged = format!("!!float {scalar}");
    assert!(from_str_with_options::<Value>(&tagged, options).is_err());
}

#[rstest]
#[case::first(DuplicateKeyPolicy::FirstWins, "first")]
#[case::last(DuplicateKeyPolicy::LastWins, "second")]
fn floating_composite_keys_keep_the_fingerprinted_integer_value(
    #[case] policy: DuplicateKeyPolicy,
    #[case] expected: &str,
) {
    let options = serde_saphyr::options! {
        legacy_octal_numbers: true,
        duplicate_keys: policy,
    };
    let yaml = "? [010]\n: first\n? [8]\n: second\n";
    let expected = BTreeMap::from([(vec![FloatKey(8.0_f64.to_bits())], expected.to_owned())]);
    for result in [
        from_str_with_options::<BTreeMap<Vec<FloatKey>, String>>(yaml, options.clone()),
        from_reader_with_options::<_, BTreeMap<Vec<FloatKey>, String>>(
            yaml.as_bytes(),
            options.clone(),
        ),
    ] {
        assert_eq!(result.unwrap(), expected);
    }
}

#[rstest]
#[case::explicit_octal("<<: { [8]: merged }\n? [010]\n: explicit\n")]
#[case::merged_octal("<<: { [010]: merged }\n? [8]\n: explicit\n")]
fn floating_composite_keys_preserve_explicit_merge_precedence(
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
        legacy_octal_numbers: true,
        duplicate_keys: policy,
    };
    let expected = BTreeMap::from([(vec![FloatKey(8.0_f64.to_bits())], "explicit".to_owned())]);
    for result in [
        from_str_with_options::<BTreeMap<Vec<FloatKey>, String>>(yaml, options.clone()),
        from_reader_with_options::<_, BTreeMap<Vec<FloatKey>, String>>(
            yaml.as_bytes(),
            options.clone(),
        ),
    ] {
        assert_eq!(result.unwrap(), expected);
    }
}
