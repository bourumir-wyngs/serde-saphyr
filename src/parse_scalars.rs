#[cfg(feature = "deserialize")]
use crate::de::{Error, Location};
#[cfg(feature = "deserialize")]
use crate::tags::SfTag;
#[cfg(feature = "deserialize")]
use granit_parser::ScalarStyle;
#[cfg(feature = "deserialize")]
use std::str::FromStr;

/// Parse a YAML 1.1 boolean from a &str (handles the "Norway problem").
///
/// Accepted TRUE literals (case-insensitive): "y", "yes", "true", "on"
/// Accepted FALSE literals (case-insensitive): "n", "no", "false", "off"
///
/// Returns:
/// - Ok(true/false) on success
/// - Err(...) if the input is not a YAML 1.1 boolean literal
pub(crate) fn parse_yaml11_bool(s: &str) -> Result<bool, String> {
    let t = s.trim();
    if t.eq_ignore_ascii_case("true")
        || t.eq_ignore_ascii_case("yes")
        || t.eq_ignore_ascii_case("y")
        || t.eq_ignore_ascii_case("on")
    {
        Ok(true)
    } else if t.eq_ignore_ascii_case("false")
        || t.eq_ignore_ascii_case("no")
        || t.eq_ignore_ascii_case("n")
        || t.eq_ignore_ascii_case("off")
    {
        Ok(false)
    } else {
        Err(format!("invalid YAML 1.1 bool: `{s}`"))
    }
}

#[cfg(feature = "deserialize")]
fn parse_digits_u128(digits: &str, radix: u32) -> Option<u128> {
    let mut val: u128 = 0;
    let mut saw = false;
    let bytes = digits.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'_' => {
                let prev_ok = i > 0 && bytes[i - 1] != b'_';
                let next_ok = i + 1 < bytes.len() && bytes[i + 1] != b'_';
                if !prev_ok || !next_ok {
                    return None;
                }
            }
            b'0'..=b'9' => {
                let d = u32::from(b - b'0');
                if d >= radix {
                    return None;
                }
                val = val.checked_mul(u128::from(radix))?;
                val = val.checked_add(u128::from(d))?;
                saw = true;
            }
            b'a'..=b'f' if radix > 10 => {
                let d = 10 + u32::from(b - b'a');
                if d >= radix {
                    return None;
                }
                val = val.checked_mul(u128::from(radix))?;
                val = val.checked_add(u128::from(d))?;
                saw = true;
            }
            b'A'..=b'F' if radix > 10 => {
                let d = 10 + u32::from(b - b'A');
                if d >= radix {
                    return None;
                }
                val = val.checked_mul(u128::from(radix))?;
                val = val.checked_add(u128::from(d))?;
                saw = true;
            }
            _ => return None,
        }
    }
    if saw { Some(val) } else { None }
}

#[cfg(feature = "deserialize")]
fn parse_decimal_unsigned_u128(digits: &str) -> Option<u128> {
    let mut val: u128 = 0;
    let mut saw = false;
    let bytes = digits.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'_' => {
                let prev_ok = i > 0 && bytes[i - 1] != b'_';
                let next_ok = i + 1 < bytes.len() && bytes[i + 1] != b'_';
                if !prev_ok || !next_ok {
                    return None;
                }
            }
            b'0'..=b'9' => {
                let d = u128::from(b - b'0');
                val = val.checked_mul(10)?;
                val = val.checked_add(d)?;
                saw = true;
            }
            _ => return None,
        }
    }
    if saw { Some(val) } else { None }
}

#[cfg(feature = "deserialize")]
fn parse_decimal_signed_i128(digits: &str, neg: bool) -> Option<i128> {
    if neg {
        // Accumulate as negative to allow i128::MIN
        let mut val: i128 = 0;
        let mut saw = false;
        let bytes = digits.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            match b {
                b'_' => {
                    let prev_ok = i > 0 && bytes[i - 1] != b'_';
                    let next_ok = i + 1 < bytes.len() && bytes[i + 1] != b'_';
                    if !prev_ok || !next_ok {
                        return None;
                    }
                }
                b'0'..=b'9' => {
                    let d = i128::from(b - b'0');
                    val = val.checked_mul(10)?;
                    val = val.checked_sub(d)?;
                    saw = true;
                }
                _ => return None,
            }
        }
        if saw { Some(val) } else { None }
    } else {
        let mut val: i128 = 0;
        let mut saw = false;
        let bytes = digits.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            match b {
                b'_' => {
                    let prev_ok = i > 0 && bytes[i - 1] != b'_';
                    let next_ok = i + 1 < bytes.len() && bytes[i + 1] != b'_';
                    if !prev_ok || !next_ok {
                        return None;
                    }
                }
                b'0'..=b'9' => {
                    let d = i128::from(b - b'0');
                    val = val.checked_mul(10)?;
                    val = val.checked_add(d)?;
                    saw = true;
                }
                _ => return None,
            }
        }
        if saw { Some(val) } else { None }
    }
}

#[cfg(feature = "deserialize")]
pub(crate) fn parse_int_signed<T>(
    s: &str,
    ty: &'static str,
    location: Location,
    legacy_octal: bool,
) -> Result<T, Error>
where
    T: TryFrom<i128>,
{
    let invalid = || Error::InvalidScalar { ty, location };

    let t = s.trim();
    let (neg, rest) = match t.strip_prefix('+') {
        Some(r) => (false, r),
        None => match t.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, t),
        },
    };

    let (radix, digits) = radix_and_digits(legacy_octal, rest);
    if radix == 10 {
        // Yaml 1.2 forbids decimal integer literals starting with zero.
        if digits.starts_with('0') && digits != "0" {
            return Err(invalid());
        }
        let val_i128 = parse_decimal_signed_i128(digits, neg).ok_or_else(invalid)?;
        return T::try_from(val_i128).map_err(|_| invalid());
    }

    let mag = parse_digits_u128(digits, radix).ok_or_else(invalid)?;
    let val_i128: i128 = if neg {
        if mag == (i128::MAX as u128) + 1 {
            i128::MIN
        } else {
            let mag_i128: i128 = mag.try_into().map_err(|_| invalid())?;
            mag_i128.checked_neg().ok_or_else(invalid)?
        }
    } else {
        mag.try_into().map_err(|_| invalid())?
    };
    T::try_from(val_i128).map_err(|_| invalid())
}

#[cfg(feature = "deserialize")]
pub(crate) fn parse_int_unsigned<T>(
    s: &str,
    ty: &'static str,
    location: Location,
    legacy_octal: bool,
) -> Result<T, Error>
where
    T: TryFrom<u128>,
{
    let invalid = || Error::InvalidScalar { ty, location };

    let t = s.trim();
    if t.starts_with('-') {
        return Err(invalid());
    }
    let rest = t.strip_prefix('+').unwrap_or(t);
    let (radix, digits) = radix_and_digits(legacy_octal, rest);

    if radix == 10 {
        // Yaml 1.2 forbids decimal integer literals starting with zero.
        if digits.starts_with('0') && digits != "0" {
            return Err(invalid());
        }
        let val_u128 = parse_decimal_unsigned_u128(digits).ok_or_else(invalid)?;
        return T::try_from(val_u128).map_err(|_| invalid());
    }

    let mag = parse_digits_u128(digits, radix).ok_or_else(invalid)?;
    T::try_from(mag).map_err(|_| invalid())
}

#[cfg(feature = "deserialize")]
fn radix_and_digits(legacy_octal: bool, rest: &str) -> (u32, &str) {
    let (radix, digits) =
        if let Some(r) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
            (16u32, normalize_prefixed_digits(legacy_octal, r))
        } else if let Some(r) = rest.strip_prefix("0o").or_else(|| rest.strip_prefix("0O")) {
            (8u32, normalize_prefixed_digits(legacy_octal, r))
        } else if let Some(r) = rest.strip_prefix("0b").or_else(|| rest.strip_prefix("0B")) {
            (2u32, normalize_prefixed_digits(legacy_octal, r))
        } else if legacy_octal && rest.starts_with('0') {
            if rest == "0" {
                // 0 is 0 and not empty string
                (8u32, "0")
            } else {
                (8u32, normalize_prefixed_digits(legacy_octal, &rest[1..]))
            }
        } else {
            (10u32, rest)
        };
    (radix, digits)
}

#[cfg(feature = "deserialize")]
fn normalize_prefixed_digits(legacy_octal: bool, digits: &str) -> &str {
    if legacy_octal {
        digits.strip_prefix('_').unwrap_or(digits)
    } else {
        digits
    }
}

#[cfg(feature = "deserialize")]
fn parse_yaml12_finite_float_fallback<T>(t: &str, location: Location) -> Result<T, Error>
where
    T: FromStr,
    T: num_traits::Float,
{
    let value = t.parse::<T>().map_err(|_| Error::InvalidScalar {
        ty: "floating point",
        location,
    })?;

    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::InvalidScalar {
            ty: "floating point",
            location,
        })
    }
}

#[cfg(all(feature = "deserialize", feature = "robotics"))]
pub(crate) fn parse_yaml12_float<T>(
    s: &str,
    location: Location,
    tag: SfTag,
    angle_conversions: bool,
) -> Result<T, Error>
where
    T: FromStr + crate::robotics::FromF64,
    T: num_traits::Float,
{
    if angle_conversions {
        return crate::robotics::parse_yaml12_float_angle_converting(s, location, tag);
    }
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    match lower.as_str() {
        ".nan" | "+.nan" | "-.nan" => Ok(T::nan()),
        ".inf" | "+.inf" => Ok(T::infinity()),
        "-.inf" => Ok(T::neg_infinity()),
        _ => parse_yaml12_finite_float_fallback(t, location),
    }
}

#[cfg(all(feature = "deserialize", not(feature = "robotics")))]
pub(crate) fn parse_yaml12_float<T>(
    s: &str,
    location: Location,
    _tag: SfTag,
    _angle_conversions: bool,
) -> Result<T, Error>
where
    T: FromStr,
    T: num_traits::Float,
{
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    match lower.as_str() {
        ".nan" | "+.nan" | "-.nan" => Ok(T::nan()),
        ".inf" | "+.inf" => Ok(T::infinity()),
        "-.inf" => Ok(T::neg_infinity()),
        _ => parse_yaml12_finite_float_fallback(t, location),
    }
}

#[cfg(feature = "deserialize")]
/// Like [`parse_yaml12_float`], but a decimal/exponential literal that overflows `f64` to
/// infinity (e.g. `1e999`, `9e400`) is treated as a successful non-finite parse instead of
/// an error.
///
/// This is used only by `deserialize_any`'s typeless path (e.g. `serde_json::Value`), where
/// non-finite floats are rejected by default, or represented as canonical strings when
/// `reject_non_finite_typeless_float` is disabled, rather than causing an "invalid floating
/// point" parse error. Elsewhere, overflowing literals continue to be rejected as invalid
/// floats via [`parse_yaml12_float`], so this function must not replace it as the general
/// entry point.
///
/// Deliberately narrower than a bare `str::parse::<f64>()`: Rust's parser also accepts
/// alphabetic spellings (`inf`, `infinity`, `nan`) that YAML/serde-saphyr correctly keep as
/// plain strings, so only numeral-shaped literals (optional sign, then a leading digit) are
/// considered here.
pub(crate) fn try_parse_float_incl_overflow(
    s: &str,
    location: Location,
    tag: SfTag,
    angle_conversions: bool,
) -> Option<f64> {
    if let Ok(v) = parse_yaml12_float::<f64>(s, location, tag, angle_conversions) {
        return Some(v);
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

#[cfg(feature = "deserialize")]
/// If we are not using Rust struct as schema, check if we should not be quoting the value.
pub(crate) fn maybe_not_string(s: &str, style: &ScalarStyle, strict_booleans: bool) -> bool {
    let location = Location::UNKNOWN;
    style == &ScalarStyle::Plain
        && (parse_yaml12_float::<f64>(s, location, SfTag::None, false).is_ok()
            || parse_int_signed::<i128>(s, "i128", location, false).is_ok()
            || maybe_bool(s, strict_booleans)
            || scalar_is_nullish(s, &ScalarStyle::Plain))
}

/// Check if a scalar looks like a YAML boolean, respecting `strict_booleans`.
#[cfg(feature = "deserialize")]
#[inline]
fn maybe_bool(s: &str, strict: bool) -> bool {
    if strict {
        s.trim().eq_ignore_ascii_case("true") || s.trim().eq_ignore_ascii_case("false")
    } else {
        parse_yaml11_bool(s).is_ok()
    }
}

/// True if a scalar is a YAML "null-like" value in non-`Option` contexts.
///
/// Arguments:
/// - `value`: scalar text.
/// - `style`: YAML scalar style; only plain form participates.
///
/// Returns:
/// - `true` for empty, `~`, or case-insensitive `null`; `false` otherwise.
///
/// Used by:
/// - Unit handling and some edge cases where absence is tolerated.
#[cfg(feature = "deserialize")]
#[inline]
pub(crate) fn scalar_is_nullish(value: &str, style: &ScalarStyle) -> bool {
    if !matches!(style, ScalarStyle::Plain) {
        return false;
    }
    value.is_empty() || value == "~" || value.eq_ignore_ascii_case("null")
}

#[cfg(feature = "deserialize")]
#[inline]
pub(crate) fn scalar_document_is_empty_or_null(
    tag: &SfTag,
    value: &str,
    style: &ScalarStyle,
) -> bool {
    *tag == SfTag::Null || (*tag != SfTag::String && scalar_is_nullish(value, style))
}

/// True if a scalar should be turned into `None` for `Option<T>`.
///
/// Arguments:
/// - `value`: scalar text.
/// - `style`: scalar style.
///
/// Returns:
/// - `true` for empty unquoted or plain `~`/`null`; `false` otherwise.
///
/// Used by:
/// - `deserialize_option` only (does not affect other types).
#[cfg(feature = "deserialize")]
#[inline]
pub(crate) fn scalar_is_nullish_for_option(value: &str, style: &ScalarStyle) -> bool {
    // For Option: treat empty unquoted scalar as null, and plain "~"/"null" as null.
    (value.is_empty() && !matches!(style, ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted)) || // empty_unquoted
    (matches!(style, ScalarStyle::Plain) && (value == "~" || value.eq_ignore_ascii_case("null"))) // plain_nullish
}

#[cfg(feature = "deserialize")]
/// Returns `true` if the string represents a decimal number with a redundant leading zero,
/// such as `0127`, `+0127`, or `-0127`.
/// Explicit radices (`0x`, `0o`, `0b`) are excluded.
/// A `true` result means this token should be avoided as an integer.
pub(crate) fn leading_zero_decimal(t: &str) -> bool {
    let s = t.trim();

    // Handle optional sign
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);

    // Must start with 0 but not just "0"
    if let Some(rest) = digits.strip_prefix('0') {
        if let Some(next) = rest.chars().next() {
            // If next char denotes radix, then allow
            !matches!(next, 'x' | 'X' | 'o' | 'O' | 'b' | 'B')
        } else {
            false // "0", "+0", "-0"
        }
    } else {
        false
    }
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
