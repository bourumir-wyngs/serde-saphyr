#![cfg(feature = "deserialize")]

use std::fmt::Debug;

use rstest::rstest;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use serde_saphyr::{Options, from_reader_with_options, from_str_with_options};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum NumberOrText {
    Num(u64),
    Text(String),
}

#[derive(Debug, Deserialize, PartialEq)]
struct Id {
    id: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct FlattenedId {
    #[serde(flatten)]
    fields: Id,
}

/// Deserialize `yaml` using `options` and compare with `expected`.
/// Returns nothing; checks both string and reader entry points.
#[track_caller]
fn assert_decoded<T: DeserializeOwned + Debug + PartialEq>(
    yaml: &str,
    expected: T,
    options: Options,
) {
    assert_eq!(
        from_str_with_options::<T>(yaml, options.clone()).unwrap(),
        expected,
        "string input: {yaml}"
    );
    assert_eq!(
        from_reader_with_options::<_, T>(yaml.as_bytes(), options).unwrap(),
        expected,
        "reader input: {yaml}"
    );
}

#[track_caller]
fn assert_preserved(scalar: &str, options: Options) {
    assert_decoded(scalar, json!(scalar), options.clone());
    assert_decoded(
        scalar,
        NumberOrText::Text(scalar.to_owned()),
        options.clone(),
    );
    assert_decoded(
        &format!("id: {scalar}\n"),
        FlattenedId {
            fields: Id {
                id: scalar.to_owned(),
            },
        },
        options,
    );
}

#[rstest]
#[case::below_i128_min("-0x80000000000000000000000000000001", false)]
#[case::hex_129_bits("0x100000000000000000000000000000000", false)]
#[case::hex_160_bits("0xffffffffffffffffffffffffffffffffffffffff", false)]
#[case::underscored_over_u128("340_282_366_920_938_463_463_374_607_431_768_211_456", false)]
#[case::legacy_octal_over_u128("077777777777777777777777777777777777777777777777777", true)]
fn integer_spellings_beyond_128_bits_remain_text(
    #[case] scalar: &str,
    #[case] legacy_octal_numbers: bool,
) {
    assert_preserved(
        scalar,
        serde_saphyr::options! { legacy_octal_numbers: legacy_octal_numbers },
    );
}

#[track_caller]
fn assert_rejected<T: DeserializeOwned>(yaml: &str, options: Options) {
    assert!(
        from_str_with_options::<T>(yaml, options.clone()).is_err(),
        "string input must reject an unsupported 128-bit integer: {yaml}"
    );
    assert!(
        from_reader_with_options::<_, T>(yaml.as_bytes(), options).is_err(),
        "reader input must reject an unsupported 128-bit integer: {yaml}"
    );
}

#[track_caller]
fn assert_unsupported_integer(scalar: &str, options: Options) {
    assert_rejected::<Value>(scalar, options.clone());
    assert_rejected::<NumberOrText>(scalar, options.clone());
    assert_rejected::<FlattenedId>(&format!("id: {scalar}\n"), options);
}

#[rstest]
#[case::hex_65_bits("0x10000000000000000", false)]
#[case::hex_identifier("0x550e8400e29b41d4a716446655440000", false)]
#[case::uppercase_signed("+0X10000000000000000", false)]
#[case::below_i64_min("-0x8000000000000001", false)]
#[case::u128_max("0xffffffffffffffffffffffffffffffff", false)]
#[case::i128_min("-0x80000000000000000000000000000000", false)]
#[case::binary_65_bits(concat!("0b1", "0000000000000000", "0000000000000000", "0000000000000000", "0000000000000000"), false)]
#[case::octal_65_bits("0o2000000000000000000000", false)]
#[case::underscored_decimal("18_446_744_073_709_551_616", false)]
#[case::negative_underscored_decimal("-9_223_372_036_854_775_809", false)]
#[case::legacy_octal("0637536023236517573056667", true)]
#[case::decimal_above_u64("18446744073709551616", false)]
#[case::decimal_below_i64("-9223372036854775809", false)]
#[case::decimal_u128_max("340282366920938463463374607431768211455", false)]
fn consumers_without_128_bit_support_reject_wide_integers(
    #[case] scalar: &str,
    #[case] legacy_octal_numbers: bool,
) {
    // These consumers cannot represent the inferred integer. They must receive
    // an error rather than a lossy float or a string selected by the 64-bit limit.
    assert_unsupported_integer(
        scalar,
        serde_saphyr::options! { legacy_octal_numbers: legacy_octal_numbers },
    );
}

#[cfg(feature = "robotics")]
#[test]
fn angle_conversions_preserve_integer_range_handling() {
    let options = serde_saphyr::options! {
        legacy_octal_numbers: true,
        angle_conversions: true,
    };
    for scalar in [
        "0x10000000000000000",
        "18_446_744_073_709_551_616",
        "0637536023236517573056667",
    ] {
        assert_unsupported_integer(scalar, options.clone());
    }
    for scalar in [
        "0x100000000000000000000000000000000",
        "340_282_366_920_938_463_463_374_607_431_768_211_456",
        "077777777777777777777777777777777777777777777777777",
    ] {
        assert_preserved(scalar, options.clone());
    }
}

#[cfg(feature = "robotics")]
#[rstest]
#[case::fraction("1_000.5", 1000.5, true)]
#[case::exponent("1_0e2", 1000.0, true)]
#[case::expression("1_0 + 2", 12.0, true)]
#[case::leading_zero_decimal("0_10", 10.0, false)]
#[case::wide_leading_zero_decimal("0_18446744073709551616", 18446744073709551616.0, false)]
/// `scalar` supplies float syntax, `expected` its value, and `legacy_octal_numbers`
/// controls integer syntax. Returns after checking both input paths.
fn angle_conversions_still_infer_underscored_floats(
    #[case] scalar: &str,
    #[case] expected: f64,
    #[case] legacy_octal_numbers: bool,
) {
    assert_decoded(
        scalar,
        json!(expected),
        serde_saphyr::options! {
            legacy_octal_numbers: legacy_octal_numbers,
            angle_conversions: true,
        },
    );
}

#[rstest]
#[case::u64_max("0xffffffffffffffff", json!(u64::MAX))]
#[case::i64_min("-0x8000000000000000", json!(i64::MIN))]
#[case::underscored_u64_max("18_446_744_073_709_551_615", json!(u64::MAX))]
fn integer_spellings_in_visitor_range_remain_integers(
    #[case] scalar: &str,
    #[case] expected: Value,
) {
    assert_decoded(scalar, expected, Options::default());
}

#[rstest]
#[case::above_u128("340282366920938463463374607431768211456")]
#[case::below_i128("-170141183460469231731687303715884105729")]
fn plain_decimals_beyond_128_bits_keep_float_inference(#[case] scalar: &str) {
    let expected = scalar.parse::<f64>().unwrap();
    assert_decoded(scalar, json!(expected), Options::default());
}

#[rstest]
#[case::radix("0x550e8400e29b41d4a716446655440000", 0x550e8400e29b41d4a716446655440000_u128 as f64, false)]
#[case::negative_radix("-0x80000000000000000000000000000000", i128::MIN as f64, false)]
#[case::underscores("18_446_744_073_709_551_616", (u64::MAX as u128 + 1) as f64, false)]
#[case::legacy_octal("0637536023236517573056667", 0o637536023236517573056667_u128 as f64, true)]
/// `scalar` supplies the integer spelling, `expected` its float conversion, and
/// `legacy_octal_numbers` selects octal syntax. Returns after checking typed and tagged floats.
fn explicit_float_conversion_accepts_wide_integers(
    #[case] scalar: &str,
    #[case] expected: f64,
    #[case] legacy_octal_numbers: bool,
) {
    let options = serde_saphyr::options! { legacy_octal_numbers: legacy_octal_numbers };
    assert_decoded(scalar, expected, options.clone());
    assert_decoded(&format!("!!float {scalar}"), json!(expected), options);
}
