#![cfg(feature = "deserialize")]

use rstest::rstest;
use serde::Deserialize;
use serde::de::{self, Visitor};
use std::fmt;

#[derive(Debug, PartialEq)]
enum Scalar {
    I64(i64),
    U64(u64),
    I128(i128),
    U128(u128),
    F64(f64),
    String(String),
}

impl<'de> Deserialize<'de> for Scalar {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ScalarVisitor;

        impl Visitor<'_> for ScalarVisitor {
            type Value = Scalar;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a number or string")
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Scalar, E> {
                Ok(Scalar::I64(value))
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Scalar, E> {
                Ok(Scalar::U64(value))
            }

            fn visit_i128<E: de::Error>(self, value: i128) -> Result<Scalar, E> {
                Ok(Scalar::I128(value))
            }

            fn visit_u128<E: de::Error>(self, value: u128) -> Result<Scalar, E> {
                Ok(Scalar::U128(value))
            }

            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Scalar, E> {
                Ok(Scalar::F64(value))
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Scalar, E> {
                Ok(Scalar::String(value.to_owned()))
            }
        }

        deserializer.deserialize_any(ScalarVisitor)
    }
}

// Use exact IEEE-754 powers of two for float expectations: powi has unspecified
// precision, which Miri exercises by perturbing its results.
#[rstest]
#[case::zero("0", Scalar::U64(0))]
#[case::negative("-42", Scalar::I64(-42))]
#[case::u64_max("18446744073709551615", Scalar::U64(u64::MAX))]
#[case::above_u64("18446744073709551616", Scalar::U128(1u128 << 64))]
#[case::above_u64_with_low_bit("18446744073709551617", Scalar::U128((1u128 << 64) + 1))]
#[case::positive_sign("+18446744073709551617", Scalar::U128((1u128 << 64) + 1))]
#[case::u128_max("340282366920938463463374607431768211455", Scalar::U128(u128::MAX))]
#[case::above_u128("340282366920938463463374607431768211456", Scalar::F64(f64::from_bits(0x47f0_0000_0000_0000)))]
#[case::i64_min("-9223372036854775808", Scalar::I64(i64::MIN))]
#[case::below_i64("-9223372036854775809", Scalar::I128(i128::from(i64::MIN) - 1))]
#[case::i128_min("-170141183460469231731687303715884105728", Scalar::I128(i128::MIN))]
#[case::below_i128("-170141183460469231731687303715884105729", Scalar::F64(f64::from_bits(0xc7e0_0000_0000_0000)))]
#[case::leading_zero("018446744073709551617", Scalar::F64(f64::from_bits(0x43f0_0000_0000_0000)))]
#[case::hexadecimal("0x10000000000000001", Scalar::U128((1u128 << 64) + 1))]
#[case::hex_identifier(
    "0x550e8400e29b41d4a716446655440000",
    Scalar::U128(0x550e8400e29b41d4a716446655440000)
)]
#[case::hexadecimal_u128_max("0xffffffffffffffffffffffffffffffff", Scalar::U128(u128::MAX))]
#[case::signed_hexadecimal("-0x8000000000000001", Scalar::I128(i128::from(i64::MIN) - 1))]
#[case::hexadecimal_i128_min("-0x80000000000000000000000000000000", Scalar::I128(i128::MIN))]
#[case::octal("0o2000000000000000000001", Scalar::U128((1u128 << 64) + 1))]
#[case::binary("0b10000000000000000000000000000000000000000000000000000000000000001", Scalar::U128((1u128 << 64) + 1))]
#[case::digit_separators("18_446_744_073_709_551_617", Scalar::U128((1u128 << 64) + 1))]
#[case::uppercase_prefix("0X10000000000000001", Scalar::U128((1u128 << 64) + 1))]
#[case::fraction("1.5", Scalar::F64(1.5))]
#[case::exponent("1e20", Scalar::F64(1e20))]
#[case::quoted("'18446744073709551617'", Scalar::String("18446744073709551617".into()))]
#[case::string_tag("!!str 18446744073709551617", Scalar::String("18446744073709551617".into()))]
fn deserialize_any_preserves_integer_precision(#[case] yaml: &str, #[case] expected: Scalar) {
    assert_inferred(yaml, expected, serde_saphyr::Options::default());
}

fn assert_inferred(yaml: &str, expected: Scalar, options: serde_saphyr::Options) {
    let from_str: Scalar = serde_saphyr::from_str_with_options(yaml, options.clone()).unwrap();
    let from_reader: Scalar =
        serde_saphyr::from_reader_with_options(yaml.as_bytes(), options).unwrap();
    assert_eq!(from_str, expected);
    assert_eq!(from_reader, expected);
}

#[rstest]
#[case::unsigned("02000000000000000000001", Scalar::U128((1u128 << 64) + 1))]
#[case::signed("-01000000000000000000001", Scalar::I128(i128::from(i64::MIN) - 1))]
#[case::prefixed_separator("0x_10000000000000001", Scalar::U128((1u128 << 64) + 1))]
#[case::wide_legacy_octal("0637536023236517573056667", Scalar::U128(0o637536023236517573056667))]
fn legacy_integer_spellings_preserve_precision(#[case] yaml: &str, #[case] expected: Scalar) {
    assert_inferred(
        yaml,
        expected,
        serde_saphyr::options! { legacy_octal_numbers: true },
    );
}

#[test]
fn string_preservation_options_take_precedence_over_large_integer_inference() {
    let options = serde_saphyr::options! {
        legacy_octal_numbers: true,
        leading_zero_numbers_as_strings: true,
        allow_numeric_underscores: false,
        lowercase_radix_prefixes_only: true,
    };
    for yaml in [
        "02000000000000000000001",
        "-01000000000000000000001",
        "18_446_744_073_709_551_617",
        "0x1_0000_0000_0000_0001",
        "+0X10000000000000000",
    ] {
        assert_inferred(yaml, Scalar::String(yaml.into()), options.clone());
    }
}

#[cfg(feature = "robotics")]
#[test]
fn integer_inference_precedes_robotics_float_parsing() {
    let options = serde_saphyr::options! {
        legacy_octal_numbers: true,
        angle_conversions: true,
    };
    for yaml in [
        "0x10000000000000001",
        "18_446_744_073_709_551_617",
        "02000000000000000000001",
    ] {
        assert_inferred(yaml, Scalar::U128((1u128 << 64) + 1), options.clone());
    }
}

#[test]
fn large_integers_survive_sequence_and_alias_replay() {
    let yaml = "[&large 18446744073709551617, *large, -9223372036854775809]";
    let expected = vec![
        Scalar::U128((1u128 << 64) + 1),
        Scalar::U128((1u128 << 64) + 1),
        Scalar::I128(i128::from(i64::MIN) - 1),
    ];
    assert_eq!(
        serde_saphyr::from_str::<Vec<Scalar>>(yaml).unwrap(),
        expected
    );
}

#[test]
fn consumers_without_large_integer_support_reject_instead_of_rounding() {
    for yaml in [
        "18446744073709551617",
        "-9223372036854775809",
        "0x10000000000000001",
        "-0x8000000000000001",
        "18_446_744_073_709_551_617",
    ] {
        assert!(serde_saphyr::from_str::<serde_json::Value>(yaml).is_err());
    }
}

#[cfg(feature = "figment")]
#[test]
fn figment_value_preserves_large_integers() {
    use figment::value::Value;

    // Issue #286: the untyped path used to round away the low bit through f64.
    let yaml = "18446744073709551617";
    let n = (1u128 << 64) + 1;
    assert_eq!(serde_saphyr::from_str::<u128>(yaml).unwrap(), n);
    assert_eq!(
        serde_saphyr::from_str::<Value>(yaml).unwrap(),
        Value::from(n)
    );

    let signed = i128::from(i64::MIN) - 1;
    assert_eq!(
        serde_saphyr::from_str::<Value>("-9223372036854775809").unwrap(),
        Value::from(signed),
    );
}
