use crate::sf_serde::{Error, Location};
use std::str::FromStr;
use saphyr_parser::ScalarStyle;

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
        Err(format!("invalid YAML 1.1 bool: `{}`", s))
    }
}

fn parse_digits_u128(digits: &str, radix: u32) -> Option<u128> {
    let mut val: u128 = 0;
    let mut saw = false;
    for b in digits.as_bytes() {
        match *b {
            b'_' => continue,
            b'0'..=b'9' => {
                let d = (b - b'0') as u32;
                if d >= radix {
                    return None;
                }
                val = val.checked_mul(radix as u128)?;
                val = val.checked_add(d as u128)?;
                saw = true;
            }
            b'a'..=b'f' if radix > 10 => {
                let d = 10 + (b - b'a') as u32;
                if d >= radix {
                    return None;
                }
                val = val.checked_mul(radix as u128)?;
                val = val.checked_add(d as u128)?;
                saw = true;
            }
            b'A'..=b'F' if radix > 10 => {
                let d = 10 + (b - b'A') as u32;
                if d >= radix {
                    return None;
                }
                val = val.checked_mul(radix as u128)?;
                val = val.checked_add(d as u128)?;
                saw = true;
            }
            _ => return None,
        }
    }
    if saw { Some(val) } else { None }
}

fn parse_decimal_unsigned_u128(digits: &str) -> Option<u128> {
    let mut val: u128 = 0;
    let mut saw = false;
    for b in digits.as_bytes() {
        match *b {
            b'_' => continue,
            b'0'..=b'9' => {
                let d = (b - b'0') as u128;
                val = val.checked_mul(10)?;
                val = val.checked_add(d)?;
                saw = true;
            }
            _ => return None,
        }
    }
    if saw { Some(val) } else { None }
}

fn parse_decimal_signed_i128(digits: &str, neg: bool) -> Option<i128> {
    if neg {
        // Accumulate as negative to allow i128::MIN
        let mut val: i128 = 0;
        let mut saw = false;
        for b in digits.as_bytes() {
            match *b {
                b'_' => continue,
                b'0'..=b'9' => {
                    let d = (b - b'0') as i128;
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
        for b in digits.as_bytes() {
            match *b {
                b'_' => continue,
                b'0'..=b'9' => {
                    let d = (b - b'0') as i128;
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

pub(crate) fn parse_int_signed<T>(
    s: String,
    ty: &'static str,
    location: Location,
    legacy_octal: bool,
) -> Result<T, Error>
where
    T: TryFrom<i128>,
{
    let t = s.trim();
    let (neg, rest) = match t.strip_prefix('+') {
        Some(r) => (false, r),
        None => match t.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, t),
        },
    };

    let (radix, digits) = radix_and_digits(legacy_octal, &rest);
    if radix == 10 {
        let val_i128 = parse_decimal_signed_i128(digits, neg)
            .ok_or_else(|| Error::msg(format!("invalid {ty}")).with_location(location))?;
        return T::try_from(val_i128)
            .map_err(|_| Error::msg(format!("invalid {ty}")).with_location(location));
    }

    let mag = parse_digits_u128(digits, radix)
        .ok_or_else(|| Error::msg(format!("invalid {ty}")).with_location(location))?;
    let val_i128: i128 = if neg {
        let mag_i128: i128 = mag
            .try_into()
            .map_err(|_| Error::msg(format!("invalid {ty}")).with_location(location))?;
        mag_i128
            .checked_neg()
            .ok_or_else(|| Error::msg(format!("invalid {ty}")).with_location(location))?
    } else {
        mag.try_into()
            .map_err(|_| Error::msg(format!("invalid {ty}")).with_location(location))?
    };
    T::try_from(val_i128).map_err(|_| Error::msg(format!("invalid {ty}")).with_location(location))
}

pub(crate) fn parse_int_unsigned<T>(
    s: String,
    ty: &'static str,
    location: Location,
    legacy_octal: bool,
) -> Result<T, Error>
where
    T: TryFrom<u128>,
{
    let t = s.trim();
    if t.starts_with('-') {
        return Err(Error::msg(format!("invalid {ty}")).with_location(location));
    }
    let rest = t.strip_prefix('+').unwrap_or(t);
    let (radix, digits) = radix_and_digits(legacy_octal, &rest);

    if radix == 10 {
        let val_u128 = parse_decimal_unsigned_u128(digits)
            .ok_or_else(|| Error::msg(format!("invalid {ty}")).with_location(location))?;
        return T::try_from(val_u128)
            .map_err(|_| Error::msg(format!("invalid {ty}")).with_location(location));
    }

    let mag = parse_digits_u128(digits, radix)
        .ok_or_else(|| Error::msg(format!("invalid {ty}")).with_location(location))?;
    T::try_from(mag).map_err(|_| Error::msg(format!("invalid {ty}")).with_location(location))
}

fn radix_and_digits<'a>(legacy_octal: bool, rest: &str) -> (u32, &str) {
    let (radix, digits) =
        if let Some(r) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
            (16u32, r)
        } else if let Some(r) = rest.strip_prefix("0o").or_else(|| rest.strip_prefix("0O")) {
            (8u32, r)
        } else if let Some(r) = rest.strip_prefix("0b").or_else(|| rest.strip_prefix("0B")) {
            (2u32, r)
        } else if legacy_octal && rest.starts_with("00") {
            if rest == "00" {
                // 00 is 0 and not empty string
                (8u32, "0")
            } else {
                (8u32, &rest[2..])
            }
        } else {
            (10u32, rest)
        };
    (radix, digits)
}

pub(crate) fn parse_yaml12_float<T>(s: &str, location: Location) -> Result<T, Error>
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
        _ => t.parse::<T>().map_err(|_| {
            Error::msg(format!(
                "invalid floating point ({} value)",
                std::any::type_name::<T>()
            ))
            .with_location(location)
        }),
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
#[inline]
pub (crate) fn scalar_is_nullish(value: &str, style: &ScalarStyle) -> bool {
    if !matches!(style, ScalarStyle::Plain) {
        return false;
    }
    value.is_empty() || value == "~" || value.eq_ignore_ascii_case("null")
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
#[inline]
pub (crate) fn scalar_is_nullish_for_option(value: &str, style: &ScalarStyle) -> bool {
    // For Option: treat empty unquoted scalar as null, and plain "~"/"null" as null.
    let empty_unquoted =
        value.is_empty() && !matches!(style, ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted);
    let plain_nullish =
        matches!(style, ScalarStyle::Plain) && (value == "~" || value.eq_ignore_ascii_case("null"));
    empty_unquoted || plain_nullish
}


#[cfg(test)]
mod tests {
    use super::*;

    fn sample_location() -> Location {
        Location { row: 42, column: 7 }
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
        let value: i64 = parse_int_signed("0x7_fF".to_string(), "i64", loc, false).unwrap();
        assert_eq!(value, 0x7ff);

        let value: i32 = parse_int_signed("0b1010_1010".to_string(), "i32", loc, false).unwrap();
        assert_eq!(value, 0b1010_1010);
    }

    #[test]
    fn parse_int_signed_honors_legacy_octal_prefixes() {
        let loc = sample_location();
        let value: i32 = parse_int_signed("00077".to_string(), "i32", loc, true).unwrap();
        assert_eq!(value, 0o77);
    }

    #[test]
    fn parse_int_signed_preserves_error_location() {
        let loc = sample_location();
        let err = parse_int_signed::<i64>("0x8000000000000000".to_string(), "i64", loc, false)
            .unwrap_err();
        match err {
            Error::Message { location, .. } => assert_eq!(location, loc),
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn parse_int_unsigned_rejects_negative_inputs() {
        let loc = sample_location();
        let err = parse_int_unsigned::<u32>("-5".to_string(), "u32", loc, false).unwrap_err();
        match err {
            Error::Message { location, .. } => assert_eq!(location, loc),
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn parse_yaml12_floats_handle_nan_and_infinity_forms() {
        let loc = sample_location();

        let nan: f64 = parse_yaml12_float(" .NaN ", loc).unwrap();
        assert!(nan.is_nan());

        let inf: f64 = parse_yaml12_float("+.INF", loc).unwrap();
        assert!(inf.is_infinite() && inf.is_sign_positive());

        let neg_inf: f64 = parse_yaml12_float("-.Inf", loc).unwrap();
        assert!(neg_inf.is_infinite() && neg_inf.is_sign_negative());
    }

    fn loc() -> Location {
        // Replace with how you construct Location in your code
        Location { row: 1, column: 1 }
    }

    #[test]
    fn test_normal_values() {
        assert_eq!(parse_yaml12_float::<f32>("1.5", loc()).unwrap(), 1.5f32);
        assert_eq!(
            parse_yaml12_float::<f32>("-123.456", loc()).unwrap(),
            -123.456f32
        );
    }

    #[test]
    fn test_zero_values() {
        assert_eq!(parse_yaml12_float::<f32>("0", loc()).unwrap(), 0.0f32);
        assert_eq!(parse_yaml12_float::<f32>("-0", loc()).unwrap(), -0.0f32);
    }

    #[test]
    fn test_nan_and_infinity() {
        let nan: f32 = parse_yaml12_float(".nan", loc()).unwrap();
        assert!(nan.is_nan());

        let inf: f64 = parse_yaml12_float(".inf", loc()).unwrap();
        assert!(inf.is_infinite() && inf.is_sign_positive());

        let ninf: f32 = parse_yaml12_float("-.Inf", loc()).unwrap();
        assert!(ninf.is_infinite() && ninf.is_sign_negative());
    }

    #[test]
    fn test_subnormal_preserved() {
        // Smallest positive subnormal f32
        let smallest = f32::from_bits(1) as f64;
        let val: f32 = parse_yaml12_float(&format!("{}", smallest), loc()).unwrap();
        assert_eq!(val, f32::from_bits(1));
    }

    #[test]
    fn test_negative_zero_preserved() {
        let val: f32 = parse_yaml12_float("-0.0", loc()).unwrap();
        assert_eq!(val.to_bits(), (-0.0f32).to_bits());
    }
}
