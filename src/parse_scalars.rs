use crate::de::{Error, Location};
use crate::scalar::{self, ResolvedScalar, ScalarError, ScalarKind, Schema};
use crate::tags::SfTag;
use granit_parser::ScalarStyle;
use std::str::FromStr;

/// Resolve a typed scalar under the selected scalar vocabulary.
/// Trimming belongs to the Serde adapter; the public resolver preserves text.
fn resolve_typed<'a>(
    text: &'a str,
    tag: &str,
    schema: Schema,
) -> Result<ResolvedScalar<'a>, ScalarError> {
    scalar::resolve(text.trim(), scalar::ScalarStyle::Plain, Some(tag), schema)
}

pub(crate) fn parse_bool(text: &str, strict: bool) -> Option<bool> {
    parse_bool_with_schema(
        text,
        Schema::Specific {
            strict_booleans: strict,
            legacy_octal_numbers: false,
            quote_all: false,
        },
    )
}

pub(crate) fn parse_bool_with_schema(text: &str, schema: Schema) -> Option<bool> {
    resolve_typed(text, "tag:yaml.org,2002:bool", schema)
        .and_then(|scalar| scalar.to_bool())
        .ok()
}

/// Parse the case-insensitive, whitespace-trimming YAML 1.1 boolean forms
/// historically accepted by the Serde APIs.
pub(crate) fn parse_yaml11_bool(s: &str) -> Result<bool, String> {
    parse_bool(s, false).ok_or_else(|| format!("invalid YAML 1.1 bool: `{s}`"))
}

pub(crate) fn parse_int_signed<T>(
    s: &str,
    ty: &'static str,
    location: Location,
    legacy_octal: bool,
) -> Result<T, Error>
where
    T: TryFrom<i128>,
{
    parse_int_signed_with_schema(
        s,
        ty,
        location,
        Schema::Specific {
            strict_booleans: false,
            legacy_octal_numbers: legacy_octal,
            quote_all: false,
        },
    )
}

pub(crate) fn parse_int_signed_with_schema<T>(
    s: &str,
    ty: &'static str,
    location: Location,
    schema: Schema,
) -> Result<T, Error>
where
    T: TryFrom<i128>,
{
    let invalid = || Error::InvalidScalar { ty, location };
    let value = resolve_typed(s, "tag:yaml.org,2002:int", schema)
        .and_then(|scalar| scalar.to_i128())
        .map_err(|_| invalid())?;
    T::try_from(value).map_err(|_| invalid())
}

#[cfg(test)]
pub(crate) fn parse_int_unsigned<T>(
    s: &str,
    ty: &'static str,
    location: Location,
    legacy_octal: bool,
) -> Result<T, Error>
where
    T: TryFrom<u128>,
{
    parse_int_unsigned_with_schema(
        s,
        ty,
        location,
        Schema::Specific {
            strict_booleans: false,
            legacy_octal_numbers: legacy_octal,
            quote_all: false,
        },
    )
}

pub(crate) fn parse_int_unsigned_with_schema<T>(
    s: &str,
    ty: &'static str,
    location: Location,
    schema: Schema,
) -> Result<T, Error>
where
    T: TryFrom<u128>,
{
    let invalid = || Error::InvalidScalar { ty, location };
    let value = resolve_typed(s, "tag:yaml.org,2002:int", schema)
        .and_then(|scalar| scalar.to_u128())
        .map_err(|_| invalid())?;
    T::try_from(value).map_err(|_| invalid())
}

fn parse_float<T: num_traits::Float + FromStr>(
    s: &str,
    location: Location,
    schema: Schema,
) -> Result<T, Error> {
    resolve_typed(s, "tag:yaml.org,2002:float", schema)
        .and_then(|scalar| scalar.convert_float())
        .map_err(|_| Error::InvalidScalar {
            ty: "floating point",
            location,
        })
}

pub(crate) fn parse_yaml12_float<T>(
    s: &str,
    location: Location,
    tag: SfTag,
    angle_conversions: bool,
) -> Result<T, Error>
where
    T: FromStr + num_traits::Float + FloatFromF64,
{
    parse_float_with_schema(
        s,
        location,
        tag,
        angle_conversions,
        Schema::Specific {
            strict_booleans: false,
            legacy_octal_numbers: false,
            quote_all: false,
        },
    )
}

#[cfg(feature = "robotics")]
pub(crate) use crate::robotics::FromF64 as FloatFromF64;
#[cfg(not(feature = "robotics"))]
pub(crate) trait FloatFromF64 {}
#[cfg(not(feature = "robotics"))]
impl<T> FloatFromF64 for T {}

pub(crate) fn parse_float_with_schema<T>(
    s: &str,
    location: Location,
    tag: SfTag,
    angle_conversions: bool,
    schema: Schema,
) -> Result<T, Error>
where
    T: FromStr + num_traits::Float + FloatFromF64,
{
    #[cfg(feature = "robotics")]
    if angle_conversions {
        return crate::robotics::parse_yaml12_float_angle_converting(s, location, tag);
    }
    #[cfg(not(feature = "robotics"))]
    let _ = (tag, angle_conversions);
    parse_float(s, location, schema)
}

/// Preserve typeless Serde's overflow handling separately from checked scalar
/// conversion: decimal/exponential overflow can become a non-finite value here.
/// Rust's undotted inf/nan spellings must still remain strings.
pub(crate) fn try_parse_float_incl_overflow(
    s: &str,
    location: Location,
    tag: SfTag,
    angle_conversions: bool,
    schema: Schema,
) -> Option<f64> {
    if let Ok(v) = parse_float_with_schema::<f64>(s, location, tag, angle_conversions, schema) {
        return Some(v);
    }

    // An overflow fallback must still match the selected scalar vocabulary.
    let resolved = resolve_typed(s, "tag:yaml.org,2002:float", schema).ok()?;
    if !matches!(schema, Schema::Specific { .. } | Schema::Legacy) {
        // The standard schemas include spellings (such as YAML 1.1 numeric
        // underscores and base-60 floats) that primitive parsing cannot read.
        // Checked conversion already distinguishes overflow from invalid syntax.
        return match resolved.to_f64() {
            Ok(value) => Some(value),
            Err(ScalarError::OutOfRange) => Some(if s.trim().starts_with('-') {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            }),
            Err(_) => None,
        };
    }

    let t = s.trim();
    let unsigned = t.strip_prefix(['+', '-']).unwrap_or(t);
    if !unsigned.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        return None;
    }

    match t.parse::<f64>() {
        Ok(v) if v.is_infinite() => Some(v),
        _ => None,
    }
}

/// If we are not using Rust struct as schema, check if we should not be quoting the value.
pub(crate) fn maybe_not_string(s: &str, style: &ScalarStyle, schema: Schema) -> bool {
    let location = Location::UNKNOWN;
    style == &ScalarStyle::Plain
        && (parse_float_with_schema::<f64>(s, location, SfTag::None, false, schema).is_ok()
            || parse_int_signed_with_schema::<i128>(s, "i128", location, schema).is_ok()
            || parse_bool_with_schema(s, schema).is_some()
            || scalar_is_nullish_with_schema(s, &ScalarStyle::Plain, schema))
}

/// Check null syntax without trimming, while respecting scalar style.
/// Explicit tag policy is applied separately by `scalar_is_null`.
#[inline]
pub(crate) fn scalar_is_nullish(value: &str, style: &ScalarStyle) -> bool {
    scalar_is_nullish_with_schema(
        value,
        style,
        Schema::Specific {
            strict_booleans: false,
            legacy_octal_numbers: false,
            quote_all: false,
        },
    )
}

#[inline]
pub(crate) fn scalar_is_nullish_with_schema(
    value: &str,
    style: &ScalarStyle,
    schema: Schema,
) -> bool {
    matches!(style, ScalarStyle::Plain)
        && scalar::resolve(value, scalar::ScalarStyle::Plain, None, schema)
            .is_ok_and(|scalar| scalar.kind() == ScalarKind::Null)
}

#[inline]
/// Resolve null while honoring explicit core types, binary tags, and string-forcing tags.
/// Non-null typed scalars must reach their deserializer even when their text looks null-like.
pub(crate) fn scalar_is_null(
    tag: &SfTag,
    value: &str,
    style: &ScalarStyle,
    schema: Schema,
) -> bool {
    (*tag == SfTag::Null
        && (matches!(schema, Schema::Specific { .. } | Schema::Legacy)
            || resolve_typed(value, "tag:yaml.org,2002:null", schema).is_ok()))
        || (!tag.is_core()
            && !tag.forces_string()
            && *tag != SfTag::Binary
            && scalar_is_nullish_with_schema(value, style, schema))
}

#[cfg(all(test, feature = "deserialize"))]
mod tests {
    use super::*;
    use rstest::rstest;

    fn sample_location() -> Location {
        Location {
            line: 42,
            column: 7,
            span: crate::location::Span::UNKNOWN,
            source_id: 0,
        }
    }

    #[test]
    fn null_resolution_preserves_explicit_binary_scalars() {
        for value in ["null", "Null", "NULL", "~", ""] {
            assert!(scalar_is_null(
                &SfTag::None,
                value,
                &ScalarStyle::Plain,
                Schema::Legacy
            ));
            assert!(scalar_is_null(
                &SfTag::Null,
                value,
                &ScalarStyle::Plain,
                Schema::Legacy
            ));
            assert!(
                !scalar_is_null(&SfTag::Binary, value, &ScalarStyle::Plain, Schema::Legacy),
                "explicit binary scalar must reach its deserializer: {value:?}"
            );
        }
    }

    #[test]
    fn yaml11_bool_accepts_all_literals_and_trims_whitespace() {
        let truthy = ["true", "Yes", " y ", "ON\n"];
        for value in truthy {
            assert!(parse_yaml11_bool(value).unwrap());
        }

        let falsy = ["false", "No", " n ", "OFF\t"];
        for value in falsy {
            assert!(!parse_yaml11_bool(value).unwrap());
        }
    }

    #[test]
    fn yaml11_bool_reports_error_for_invalid_literal() {
        let err = parse_yaml11_bool("maybe").unwrap_err();
        assert!(err.contains("invalid YAML 1.1 bool"));
    }

    #[test]
    fn parse_int_signed_supports_alternate_radices_and_underscores() {
        let loc = sample_location();
        let value: i64 = parse_int_signed("0x7_fF", "i64", loc, false).unwrap();
        assert_eq!(value, 0x7ff);

        let value: i32 = parse_int_signed("0b1010_1010", "i32", loc, false).unwrap();
        assert_eq!(value, 0b1010_1010);
    }

    #[test]
    fn parse_int_signed_supports_i128_min_in_alternate_radices() {
        let loc = sample_location();
        let hex: i128 =
            parse_int_signed("-0x80000000000000000000000000000000", "i128", loc, false).unwrap();
        let binary_min = format!("-0b1{}", "0".repeat(127));
        let binary: i128 = parse_int_signed(&binary_min, "i128", loc, false).unwrap();

        assert_eq!(hex, i128::MIN);
        assert_eq!(binary, i128::MIN);
        assert!(
            parse_int_signed::<i128>("0x80000000000000000000000000000000", "i128", loc, false)
                .is_err()
        );
    }

    #[test]
    fn parse_int_signed_rejects_invalid_underscores() {
        let loc = sample_location();
        // Leading underscore
        assert!(parse_int_signed::<i32>("_1", "i32", loc, false).is_err());
        // Trailing underscore
        assert!(parse_int_signed::<i32>("1000_", "i32", loc, false).is_err());
        // Double underscore
        assert!(parse_int_signed::<i32>("1__0", "i32", loc, false).is_err());
        // Valid underscores
        assert!(parse_int_signed::<i32>("1000_1000", "i32", loc, false).is_ok());
    }

    #[test]
    fn parse_int_signed_honors_legacy_octal_prefixes() {
        let loc = sample_location();
        let value: i32 = parse_int_signed("00077", "i32", loc, true).unwrap();
        assert_eq!(value, 0o77);
    }

    #[test]
    fn parse_int_signed_honors_legacy_prefix_underscores() {
        let loc = sample_location();

        let octal: i32 = parse_int_signed("0_10", "i32", loc, true).unwrap();
        let plus_octal: i32 = parse_int_signed("+0_10", "i32", loc, true).unwrap();
        let negative_octal: i32 = parse_int_signed("-0_10", "i32", loc, true).unwrap();
        let hex: i32 = parse_int_signed("0x_10", "i32", loc, true).unwrap();
        let explicit_octal: i32 = parse_int_signed("0o_10", "i32", loc, true).unwrap();
        let binary: i32 = parse_int_signed("0b_10", "i32", loc, true).unwrap();

        assert_eq!(octal, 0o10);
        assert_eq!(plus_octal, 0o10);
        assert_eq!(negative_octal, -0o10);
        assert_eq!(hex, 0x10);
        assert_eq!(explicit_octal, 0o10);
        assert_eq!(binary, 0b10);
    }

    #[test]
    fn parse_int_signed_keeps_prefix_underscores_opt_in() {
        let loc = sample_location();

        assert!(parse_int_signed::<i32>("0_10", "i32", loc, false).is_err());
        assert!(parse_int_signed::<i32>("0x_10", "i32", loc, false).is_err());
    }

    #[test]
    fn parse_int_signed_preserves_error_location() {
        let loc = sample_location();
        let err = parse_int_signed::<i64>("0x8000000000000000", "i64", loc, false).unwrap_err();
        match err {
            Error::InvalidScalar { location, .. } => assert_eq!(location, loc),
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn parse_int_unsigned_rejects_negative_inputs() {
        let loc = sample_location();
        let err = parse_int_unsigned::<u32>("-5", "u32", loc, false).unwrap_err();
        match err {
            Error::InvalidScalar { location, .. } => assert_eq!(location, loc),
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn parse_int_unsigned_honors_legacy_prefix_underscores() {
        let loc = sample_location();

        let octal: u32 = parse_int_unsigned("0_10", "u32", loc, true).unwrap();
        let plus_octal: u32 = parse_int_unsigned("+0_10", "u32", loc, true).unwrap();
        let hex: u32 = parse_int_unsigned("0x_10", "u32", loc, true).unwrap();
        let explicit_octal: u32 = parse_int_unsigned("0o_10", "u32", loc, true).unwrap();
        let binary: u32 = parse_int_unsigned("0b_10", "u32", loc, true).unwrap();

        assert_eq!(octal, 0o10);
        assert_eq!(plus_octal, 0o10);
        assert_eq!(hex, 0x10);
        assert_eq!(explicit_octal, 0o10);
        assert_eq!(binary, 0b10);
    }

    #[test]
    fn parse_yaml12_floats_handle_nan_and_infinity_forms() {
        let loc = sample_location();

        let nan: f64 = parse_yaml12_float(" .NaN ", loc, SfTag::None, false).unwrap();
        assert!(nan.is_nan());

        let inf: f64 = parse_yaml12_float("+.INF", loc, SfTag::None, false).unwrap();
        assert!(inf.is_infinite() && inf.is_sign_positive());

        let neg_inf: f64 = parse_yaml12_float("-.Inf", loc, SfTag::None, false).unwrap();
        assert!(neg_inf.is_infinite() && neg_inf.is_sign_negative());
    }

    #[rstest]
    #[case::nan("nan")]
    #[case::capital_nan("NaN")]
    #[case::inf("inf")]
    #[case::plus_inf("+inf")]
    #[case::minus_inf("-inf")]
    #[case::infinity("Infinity")]
    #[case::plus_infinity("+Infinity")]
    #[case::minus_infinity("-Infinity")]
    fn parse_yaml12_float_rejects_rust_nonfinite_spellings(#[case] input: &str) {
        assert!(parse_yaml12_float::<f64>(input, loc(), SfTag::None, false).is_err());
    }

    fn loc() -> Location {
        // Replace with how you construct Location in your code
        Location {
            line: 1,
            column: 1,
            span: crate::location::Span::UNKNOWN,
            source_id: 0,
        }
    }

    #[test]
    fn test_normal_values() {
        assert_eq!(
            parse_yaml12_float::<f32>("1.5", loc(), SfTag::None, false).unwrap(),
            1.5f32
        );
        assert_eq!(
            parse_yaml12_float::<f32>("-123.456", loc(), SfTag::None, false).unwrap(),
            -123.456f32
        );
    }

    #[test]
    fn test_zero_values() {
        assert_eq!(
            parse_yaml12_float::<f32>("0", loc(), SfTag::None, false).unwrap(),
            0.0f32
        );
        assert_eq!(
            parse_yaml12_float::<f32>("-0", loc(), SfTag::None, false).unwrap(),
            -0.0f32
        );
    }

    #[test]
    fn test_nan_and_infinity() {
        let nan: f32 = parse_yaml12_float(".nan", loc(), SfTag::None, false).unwrap();
        assert!(nan.is_nan());

        let inf: f64 = parse_yaml12_float(".inf", loc(), SfTag::None, false).unwrap();
        assert!(inf.is_infinite() && inf.is_sign_positive());

        let ninf: f32 = parse_yaml12_float("-.Inf", loc(), SfTag::None, false).unwrap();
        assert!(ninf.is_infinite() && ninf.is_sign_negative());
    }

    #[test]
    fn test_subnormal_preserved() {
        // Smallest positive subnormal f32
        let smallest = f32::from_bits(1) as f64;
        let val: f32 =
            parse_yaml12_float(&format!("{}", smallest), loc(), SfTag::None, false).unwrap();
        assert_eq!(val, f32::from_bits(1));
    }

    #[test]
    fn test_negative_zero_preserved() {
        let val: f32 = parse_yaml12_float("-0.0", loc(), SfTag::None, false).unwrap();
        assert_eq!(val.to_bits(), (-0.0f32).to_bits());
    }
}
