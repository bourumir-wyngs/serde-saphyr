#[cfg(feature = "deserialize")]
use crate::de::{Error, Location};
#[cfg(feature = "deserialize")]
use crate::tags::SfTag;
#[cfg(feature = "deserialize")]
use granit_parser::ScalarStyle;
#[cfg(feature = "deserialize")]
use std::str::FromStr;

/// Trim ASCII padding accepted during scalar conversion.
///
/// Besides YAML spaces and tabs, accept line breaks for quoted and block scalars.
/// Other Unicode whitespace is scalar content and must be preserved.
#[inline]
pub(crate) fn yaml_trim(s: &str) -> &str {
    s.trim_matches([' ', '\t', '\r', '\n'])
}

/// Parse a YAML 1.2 Core boolean spelling.
///
/// Preserve the ASCII padding accepted during scalar conversion.
#[cfg(feature = "deserialize")]
#[inline]
pub(crate) fn parse_yaml12_bool(s: &str) -> Option<bool> {
    match yaml_trim(s) {
        "true" | "True" | "TRUE" => Some(true),
        "false" | "False" | "FALSE" => Some(false),
        _ => None,
    }
}

/// Parse a YAML 1.1 boolean from a &str (handles the "Norway problem").
///
/// Accepted TRUE literals (case-insensitive): "y", "yes", "true", "on"
/// Accepted FALSE literals (case-insensitive): "n", "no", "false", "off"
///
/// Returns:
/// - Some(true/false) on success
/// - None if the input is not a YAML 1.1 boolean literal
///
/// This is also a scalar-classification probe; callers supply error context.
pub(crate) fn parse_yaml11_bool(s: &str) -> Option<bool> {
    let t = yaml_trim(s);
    if t.eq_ignore_ascii_case("true")
        || t.eq_ignore_ascii_case("yes")
        || t.eq_ignore_ascii_case("y")
        || t.eq_ignore_ascii_case("on")
    {
        Some(true)
    } else if t.eq_ignore_ascii_case("false")
        || t.eq_ignore_ascii_case("no")
        || t.eq_ignore_ascii_case("n")
        || t.eq_ignore_ascii_case("off")
    {
        Some(false)
    } else {
        None
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

/// Numeric syntax shared by scalar parsing and mapping-key fingerprints.
#[cfg(feature = "deserialize")]
#[derive(Clone, Copy)]
pub(crate) struct NumericSyntax {
    /// Accept legacy octal numbers and separators after radix prefixes.
    pub(crate) legacy_octal: bool,
    /// Accept otherwise valid underscore digit separators.
    pub(crate) allow_underscores: bool,
    /// Reject uppercase radix prefixes in otherwise valid integers.
    pub(crate) lowercase_radix_prefixes_only: bool,
    /// Keep redundant-zero decimal integer spellings out of all numeric parsers.
    pub(crate) leading_zero_numbers_as_strings: bool,
    /// Enable robotics expressions and unit conversions for floating-point values.
    #[cfg(feature = "robotics")]
    pub(crate) angle_conversions: bool,
}

#[cfg(feature = "deserialize")]
impl Default for NumericSyntax {
    fn default() -> Self {
        Self {
            legacy_octal: false,
            allow_underscores: true,
            lowercase_radix_prefixes_only: false,
            leading_zero_numbers_as_strings: false,
            #[cfg(feature = "robotics")]
            angle_conversions: false,
        }
    }
}

#[cfg(feature = "deserialize")]
impl NumericSyntax {
    /// Whether trimmed `s` must stay text after 128-bit integer inference fails.
    /// Radix, legacy-octal, and digit-separated integer spellings must not become
    /// lossy floats. Plain decimals retain their float fallback; fractional,
    /// exponential, and robotics expression syntax remains eligible too.
    pub(crate) fn blocks_implicit_float_fallback(self, s: &str) -> bool {
        let unsigned = s.strip_prefix(['+', '-']).unwrap_or(s);
        self.is_legacy_octal(s)
            || radix_and_digits(false, unsigned).0 != 10
            // Without legacy octal, leading-zero spellings are not accepted
            // integers and may still use robotics decimal parsing.
            || (!unsigned.starts_with('0')
                && unsigned.contains('_')
                && unsigned.bytes().all(|b| b.is_ascii_digit() || b == b'_'))
    }

    /// Whether a trimmed spelling is reserved for the legacy octal integer parser.
    /// Such spellings must never fall back to decimal interpretation on overflow.
    fn is_legacy_octal(self, s: &str) -> bool {
        let unsigned = s.strip_prefix(['+', '-']).unwrap_or(s);
        self.legacy_octal
            && unsigned.starts_with('0')
            && unsigned.bytes().all(|b| b.is_ascii_digit() || b == b'_')
    }
}

/// Enforce the underscore policy after a successful numeric conversion.
///
/// - `value`: successfully parsed numeric value, including target-range validation.
/// - `s`: scalar text, including any sign or radix prefix.
/// - `location`: source location for a rejected scalar.
/// - `allow_underscores`: whether numeric syntax may contain digit separators.
///
/// Returns `value`, or [`Error::NumericUnderscoresDisabled`] if underscores are
/// disabled and present. Callers retain parsing errors without invoking this check.
#[cfg(feature = "deserialize")]
#[inline]
fn check_numeric_underscores<T>(
    value: T,
    s: &str,
    location: Location,
    allow_underscores: bool,
) -> Result<T, Error> {
    if !allow_underscores && s.contains('_') {
        return Err(Error::NumericUnderscoresDisabled { location });
    }
    Ok(value)
}

/// Enforce integer syntax policies after a successful numeric conversion.
///
/// - `value`: parsed integer after target-range validation.
/// - `unsigned`: trimmed scalar text with its optional sign removed.
/// - `location`: source location for a rejected scalar.
/// - `syntax`: configured underscore and radix-prefix restrictions.
///
/// Returns `value`, or the configuration error for a disallowed spelling.
#[cfg(feature = "deserialize")]
#[inline]
fn check_integer_syntax<T>(
    value: T,
    unsigned: &str,
    location: Location,
    syntax: NumericSyntax,
) -> Result<T, Error> {
    let value = check_numeric_underscores(value, unsigned, location, syntax.allow_underscores)?;
    if syntax.lowercase_radix_prefixes_only
        && matches!(unsigned.as_bytes(), [b'0', b'X' | b'O' | b'B', ..])
    {
        return Err(Error::UppercaseRadixPrefixDisabled { location });
    }
    Ok(value)
}

/// Parse a signed integer using the configured numeric syntax.
///
/// - `s`: scalar text, including any sign or radix prefix.
/// - `ty`: numeric target name used in diagnostics.
/// - `location`: source location for a rejected scalar.
/// - `syntax`: accepted legacy octal, underscore, and radix-prefix syntax.
///
/// Returns the integer, or an error for invalid syntax or target overflow.
#[cfg(feature = "deserialize")]
pub(crate) fn parse_int_signed<T>(
    s: &str,
    ty: &'static str,
    location: Location,
    syntax: NumericSyntax,
) -> Result<T, Error>
where
    T: TryFrom<i128>,
{
    let invalid = || Error::InvalidScalar { ty, location };

    let t = yaml_trim(s);
    if syntax.leading_zero_numbers_as_strings && leading_zero_decimal(t) {
        return Err(invalid());
    }
    let (neg, rest) = match t.strip_prefix('+') {
        Some(r) => (false, r),
        None => match t.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, t),
        },
    };

    let (radix, digits) = radix_and_digits(syntax.legacy_octal, rest);
    if radix == 10 {
        // Keep zero-prefixed integer spellings opt-in through legacy octal parsing.
        if digits.starts_with('0') && digits != "0" {
            return Err(invalid());
        }
        let val_i128 = parse_decimal_signed_i128(digits, neg).ok_or_else(invalid)?;
        let value = T::try_from(val_i128).map_err(|_| invalid())?;
        return check_integer_syntax(value, rest, location, syntax);
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
    let value = T::try_from(val_i128).map_err(|_| invalid())?;
    check_integer_syntax(value, rest, location, syntax)
}

/// Parse an unsigned integer with the same arguments and syntax as [`parse_int_signed`].
/// Returns an error for negative inputs, invalid syntax, or target overflow.
#[cfg(feature = "deserialize")]
pub(crate) fn parse_int_unsigned<T>(
    s: &str,
    ty: &'static str,
    location: Location,
    syntax: NumericSyntax,
) -> Result<T, Error>
where
    T: TryFrom<u128>,
{
    let invalid = || Error::InvalidScalar { ty, location };

    let t = yaml_trim(s);
    if syntax.leading_zero_numbers_as_strings && leading_zero_decimal(t) {
        return Err(invalid());
    }
    if t.starts_with('-') {
        return Err(invalid());
    }
    let rest = t.strip_prefix('+').unwrap_or(t);
    let (radix, digits) = radix_and_digits(syntax.legacy_octal, rest);

    if radix == 10 {
        // Keep zero-prefixed integer spellings opt-in through legacy octal parsing.
        if digits.starts_with('0') && digits != "0" {
            return Err(invalid());
        }
        let val_u128 = parse_decimal_unsigned_u128(digits).ok_or_else(invalid)?;
        let value = T::try_from(val_u128).map_err(|_| invalid())?;
        return check_integer_syntax(value, rest, location, syntax);
    }

    let mag = parse_digits_u128(digits, radix).ok_or_else(invalid)?;
    let value = T::try_from(mag).map_err(|_| invalid())?;
    check_integer_syntax(value, rest, location, syntax)
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

/// Parse supported non-finite float spellings, including legacy signed NaN.
#[cfg(feature = "deserialize")]
fn parse_yaml12_non_finite_float<T: num_traits::Float>(t: &str) -> Option<T> {
    let (negative, unsigned) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    match unsigned {
        ".nan" | ".NaN" | ".NAN" => Some(T::nan()),
        ".inf" | ".Inf" | ".INF" if negative => Some(T::neg_infinity()),
        ".inf" | ".Inf" | ".INF" => Some(T::infinity()),
        _ => None,
    }
}

/// Preserve the sign of a signed NaN spelling only for an explicit float tag.
/// `t` must already be trimmed; other spellings and tags use the existing parser.
#[cfg(feature = "deserialize")]
fn parse_tagged_signed_nan<T: num_traits::Float>(t: &str, tag: SfTag) -> Option<T> {
    if tag != SfTag::Float || !is_signed_nan(t) {
        return None;
    }
    let sign = if t.starts_with('-') {
        -T::one()
    } else {
        T::one()
    };
    Some(T::nan().copysign(sign))
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

/// Interpret integer spellings before converting them to a floating-point value.
///
/// - `s`: trimmed scalar text, including its optional sign and radix prefix.
/// - `location`: source location for syntax-policy errors.
/// - `syntax`: the same numeric spelling rules used for integer values and keys.
///
/// Returns a converted integer, `None` for non-integer syntax, or a policy error.
#[cfg(feature = "deserialize")]
fn parse_integer_as_float<T: num_traits::Float>(
    s: &str,
    location: Location,
    syntax: NumericSyntax,
) -> Result<Option<T>, Error> {
    let invalid = || Error::InvalidScalar {
        ty: "floating point",
        location,
    };
    let parsed = if s.starts_with('-') {
        parse_int_signed::<i128>(s, "i128", location, syntax).and_then(|value| {
            // Integer identity normalizes zero, but floating-point conversion must
            // retain its sign, including alternate-radix spellings such as -0x0.
            if value == 0 {
                Ok(-T::zero())
            } else {
                T::from(value).ok_or_else(invalid)
            }
        })
    } else {
        parse_int_unsigned::<u128>(s, "u128", location, syntax)
            .and_then(|value| T::from(value).ok_or_else(invalid))
    };
    match parsed {
        Ok(value) => Ok(Some(value)),
        Err(Error::InvalidScalar { .. }) => {
            if syntax.is_legacy_octal(s) {
                // Invalid or overflowing legacy octal must not be retried as a
                // decimal float. Fractional/exponential forms still use floats.
                Err(invalid())
            } else {
                Ok(None)
            }
        }
        Err(error) => Err(error),
    }
}

/// Parse scalar `s` as a float, reporting errors at `location`.
///
/// - `s`: scalar text, including optional sign and radix prefix.
/// - `location`: source location for errors.
/// - `tag`: preserves explicit NaN signs and selects robotics unit conversion.
/// - `syntax`: shared numeric spelling policies and optional robotics expressions.
///
/// Returns the float or a numeric syntax/range/expression error. Spellings accepted
/// by the shared integer parser use its interpretation before float conversion.
/// Other spellings may fall back to decimal float parsing: with default options,
/// `010` becomes `10.0`, although integer targets reject it.
#[cfg(feature = "deserialize")]
pub(crate) fn parse_yaml12_float<T>(
    s: &str,
    location: Location,
    tag: SfTag,
    syntax: NumericSyntax,
) -> Result<T, Error>
where
    T: FromStr + num_traits::Float,
{
    let t = yaml_trim(s);
    if syntax.leading_zero_numbers_as_strings && leading_zero_decimal(t) {
        return Err(Error::InvalidScalar {
            ty: "floating point",
            location,
        });
    }
    if let Some(value) = parse_tagged_signed_nan(t, tag) {
        return Ok(value);
    }
    #[cfg(feature = "robotics")]
    if syntax.angle_conversions {
        let value = match parse_integer_as_float::<f64>(t, location, syntax)? {
            Some(value) => crate::robotics::convert_bare_angle(value, tag),
            None => crate::robotics::parse_yaml12_float_angle_converting::<f64>(s, location, tag)?,
        };
        let converted = T::from(value).ok_or(Error::InvalidScalar {
            ty: "floating point",
            location,
        })?;
        // Preserve non-finite literals and expression results, but reject
        // overflow introduced by narrowing a finite result (for example to f32).
        if value.is_finite() && !converted.is_finite() {
            return Err(Error::InvalidScalar {
                ty: "floating point",
                location,
            });
        }
        return check_numeric_underscores(converted, s, location, syntax.allow_underscores);
    }
    if let Some(value) = parse_yaml12_non_finite_float(t) {
        return Ok(value);
    }
    if let Some(value) = parse_integer_as_float(t, location, syntax)? {
        // u128::MAX rounds to infinity in f32; this remains a target-range error.
        if T::is_finite(value) {
            return Ok(value);
        }
        return Err(Error::InvalidScalar {
            ty: "floating point",
            location,
        });
    }
    let value = parse_yaml12_finite_float_fallback(t, location)?;
    check_numeric_underscores(value, s, location, syntax.allow_underscores)
}

/// Parse the same float syntax as [`parse_yaml12_float`], retaining numeric overflow.
///
/// - `s`: scalar text.
/// - `location`: source location for errors.
/// - `tag`: NaN sign and robotics unit semantics, as in [`parse_yaml12_float`].
/// - `syntax`: shared numeric spelling policies.
///
/// Returns a float, including infinity for a numeral such as `1e999`, or a syntax
/// error. Typeless visitors apply `NonFiniteFloatPolicy` after this classification;
/// typed targets still reject overflow. Alphabetic Rust spellings stay strings.
#[cfg(feature = "deserialize")]
pub(crate) fn try_parse_float_incl_overflow(
    s: &str,
    location: Location,
    tag: SfTag,
    syntax: NumericSyntax,
) -> Result<f64, Error> {
    let error = match parse_yaml12_float::<f64>(s, location, tag, syntax) {
        Ok(v) => return Ok(v),
        // Configuration rejections must never reach the permissive fallback.
        Err(
            error @ (Error::NumericUnderscoresDisabled { .. }
            | Error::UppercaseRadixPrefixDisabled { .. }),
        ) => return Err(error),
        Err(error) => error,
    };

    let t = yaml_trim(s);
    if (syntax.leading_zero_numbers_as_strings && leading_zero_decimal(t))
        || syntax.is_legacy_octal(t)
    {
        return Err(error);
    }
    let unsigned = t.strip_prefix(['+', '-']).unwrap_or(t);
    if !unsigned.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        return Err(error);
    }

    match t.parse::<f64>() {
        Ok(v) if v.is_infinite() => {
            check_numeric_underscores(v, s, location, syntax.allow_underscores)
        }
        _ => Err(error),
    }
}

/// Return whether trimmed scalar text is a signed NaN spelling that YAML Core treats as a string.
#[cfg(feature = "deserialize")]
#[inline]
pub(crate) fn is_signed_nan(t: &str) -> bool {
    t.strip_prefix(['+', '-'])
        .is_some_and(|unsigned| matches!(unsigned, ".nan" | ".NaN" | ".NAN"))
}

/// Check whether a string target requires quoting in `no_schema` mode.
///
/// - `s`: scalar text before trimming.
/// - `style`: scalar style; only plain scalars require this check.
/// - `strict_booleans`: whether to recognize only YAML 1.2 boolean spellings.
/// - `syntax`: shared numeric spelling policies, including legacy octal and robotics.
///
/// Returns `true` for ambiguous boolean, numeric, or null-like plain scalars.
#[cfg(feature = "deserialize")]
pub(crate) fn maybe_not_string(
    s: &str,
    style: &ScalarStyle,
    strict_booleans: bool,
    syntax: NumericSyntax,
) -> bool {
    let location = Location::UNKNOWN;
    style.is_plain()
        && !is_signed_nan(yaml_trim(s))
        && (try_parse_float_incl_overflow(s, location, SfTag::None, syntax).is_ok()
            || maybe_bool(s, strict_booleans)
            || scalar_is_nullish(s, style))
}

/// Check if a scalar looks like a YAML boolean, respecting `strict_booleans`.
#[cfg(feature = "deserialize")]
#[inline]
fn maybe_bool(s: &str, strict: bool) -> bool {
    if strict {
        parse_yaml12_bool(s).is_some()
    } else {
        parse_yaml11_bool(s).is_some()
    }
}

/// True if a scalar's text and style are YAML "null-like".
/// Callers must also respect tags such as `!!str`, `!`, and `!!null`.
///
/// Arguments:
/// - `value`: scalar text.
/// - `style`: YAML scalar style; only plain form participates.
///
/// Returns:
/// - `true` for empty, `~`, `null`, `Null`, or `NULL`; `false` otherwise.
///
/// Used by:
/// - Option, unit, typeless scalar handling, and other cases where absence is tolerated.
#[cfg(feature = "deserialize")]
#[inline]
pub(crate) fn scalar_is_nullish(value: &str, style: &ScalarStyle) -> bool {
    if !style.is_plain() {
        return false;
    }
    is_null_spelling(value)
}

/// Returns `true` for the YAML 1.2 core schema null spellings: empty, `~`, `null`,
/// `Null`, or `NULL`.
#[cfg(feature = "deserialize")]
#[inline]
fn is_null_spelling(value: &str) -> bool {
    matches!(value, "" | "~" | "null" | "Null" | "NULL")
}

#[cfg(feature = "deserialize")]
#[inline]
/// Resolve null while honoring explicit scalar types and string-forcing tags.
/// Non-null typed scalars must reach their deserializer even when their text looks null-like.
/// An explicit `!!null` still requires a null spelling, in any style: `!!null foo` is not null.
pub(crate) fn scalar_is_null(tag: &SfTag, value: &str, style: &ScalarStyle) -> bool {
    (*tag == SfTag::Null && is_null_spelling(value))
        || (!tag.is_core()
            && !tag.forces_string()
            && !matches!(tag, SfTag::Binary | SfTag::TimeStamp)
            && !tag.is_robotics()
            && scalar_is_nullish(value, style))
}

#[cfg(feature = "deserialize")]
/// Returns `true` if the string represents a decimal integer with a redundant leading zero,
/// such as `0127`, `+0127`, or `-0127`.
/// Floating-point spellings, underscores, and explicit radices (`0x`, `0o`, `0b`)
/// are excluded.
pub(crate) fn leading_zero_decimal(t: &str) -> bool {
    let s = yaml_trim(t);

    // Handle optional sign
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);

    digits.len() > 1 && digits.starts_with('0') && digits.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(all(test, feature = "deserialize"))]
mod tests {
    use super::*;
    use rstest::rstest;

    fn legacy_numeric_syntax() -> NumericSyntax {
        NumericSyntax {
            legacy_octal: true,
            ..NumericSyntax::default()
        }
    }

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
            assert!(scalar_is_null(&SfTag::None, value, &ScalarStyle::plain()));
            assert!(scalar_is_null(&SfTag::Null, value, &ScalarStyle::plain()));
            assert!(
                !scalar_is_null(&SfTag::Binary, value, &ScalarStyle::plain()),
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
    fn yaml11_bool_rejects_invalid_literal() {
        assert!(parse_yaml11_bool("maybe").is_none());
    }

    #[test]
    fn parse_int_signed_supports_alternate_radices_and_underscores() {
        let loc = sample_location();
        let value: i64 = parse_int_signed("0x7_fF", "i64", loc, NumericSyntax::default()).unwrap();
        assert_eq!(value, 0x7ff);

        let value: i32 =
            parse_int_signed("0b1010_1010", "i32", loc, NumericSyntax::default()).unwrap();
        assert_eq!(value, 0b1010_1010);
    }

    #[test]
    fn parse_int_signed_supports_i128_min_in_alternate_radices() {
        let loc = sample_location();
        let hex: i128 = parse_int_signed(
            "-0x80000000000000000000000000000000",
            "i128",
            loc,
            NumericSyntax::default(),
        )
        .unwrap();
        let binary_min = format!("-0b1{}", "0".repeat(127));
        let binary: i128 =
            parse_int_signed(&binary_min, "i128", loc, NumericSyntax::default()).unwrap();

        assert_eq!(hex, i128::MIN);
        assert_eq!(binary, i128::MIN);
        assert!(
            parse_int_signed::<i128>(
                "0x80000000000000000000000000000000",
                "i128",
                loc,
                NumericSyntax::default()
            )
            .is_err()
        );
    }

    #[test]
    fn parse_int_signed_rejects_invalid_underscores() {
        let loc = sample_location();
        // Leading underscore
        assert!(parse_int_signed::<i32>("_1", "i32", loc, NumericSyntax::default()).is_err());
        // Trailing underscore
        assert!(parse_int_signed::<i32>("1000_", "i32", loc, NumericSyntax::default()).is_err());
        // Double underscore
        assert!(parse_int_signed::<i32>("1__0", "i32", loc, NumericSyntax::default()).is_err());
        // Valid underscores
        assert!(parse_int_signed::<i32>("1000_1000", "i32", loc, NumericSyntax::default()).is_ok());
    }

    #[test]
    fn parse_int_signed_honors_legacy_octal_prefixes() {
        let loc = sample_location();
        let value: i32 = parse_int_signed("00077", "i32", loc, legacy_numeric_syntax()).unwrap();
        assert_eq!(value, 0o77);
    }

    #[test]
    fn parse_int_signed_honors_legacy_prefix_underscores() {
        let loc = sample_location();

        let octal: i32 = parse_int_signed("0_10", "i32", loc, legacy_numeric_syntax()).unwrap();
        let plus_octal: i32 =
            parse_int_signed("+0_10", "i32", loc, legacy_numeric_syntax()).unwrap();
        let negative_octal: i32 =
            parse_int_signed("-0_10", "i32", loc, legacy_numeric_syntax()).unwrap();
        let hex: i32 = parse_int_signed("0x_10", "i32", loc, legacy_numeric_syntax()).unwrap();
        let explicit_octal: i32 =
            parse_int_signed("0o_10", "i32", loc, legacy_numeric_syntax()).unwrap();
        let binary: i32 = parse_int_signed("0b_10", "i32", loc, legacy_numeric_syntax()).unwrap();

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

        assert!(parse_int_signed::<i32>("0_10", "i32", loc, NumericSyntax::default()).is_err());
        assert!(parse_int_signed::<i32>("0x_10", "i32", loc, NumericSyntax::default()).is_err());
    }

    #[test]
    fn parse_int_signed_preserves_error_location() {
        let loc = sample_location();
        let err =
            parse_int_signed::<i64>("0x8000000000000000", "i64", loc, NumericSyntax::default())
                .unwrap_err();
        match err {
            Error::InvalidScalar { location, .. } => assert_eq!(location, loc),
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn parse_int_unsigned_rejects_negative_inputs() {
        let loc = sample_location();
        let err =
            parse_int_unsigned::<u32>("-5", "u32", loc, NumericSyntax::default()).unwrap_err();
        match err {
            Error::InvalidScalar { location, .. } => assert_eq!(location, loc),
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn parse_int_unsigned_honors_legacy_prefix_underscores() {
        let loc = sample_location();

        let octal: u32 = parse_int_unsigned("0_10", "u32", loc, legacy_numeric_syntax()).unwrap();
        let plus_octal: u32 =
            parse_int_unsigned("+0_10", "u32", loc, legacy_numeric_syntax()).unwrap();
        let hex: u32 = parse_int_unsigned("0x_10", "u32", loc, legacy_numeric_syntax()).unwrap();
        let explicit_octal: u32 =
            parse_int_unsigned("0o_10", "u32", loc, legacy_numeric_syntax()).unwrap();
        let binary: u32 = parse_int_unsigned("0b_10", "u32", loc, legacy_numeric_syntax()).unwrap();

        assert_eq!(octal, 0o10);
        assert_eq!(plus_octal, 0o10);
        assert_eq!(hex, 0x10);
        assert_eq!(explicit_octal, 0o10);
        assert_eq!(binary, 0b10);
    }

    #[test]
    fn parse_yaml12_floats_handle_nan_and_infinity_forms() {
        let loc = sample_location();

        let nan: f64 =
            parse_yaml12_float(" .NaN ", loc, SfTag::None, NumericSyntax::default()).unwrap();
        assert!(nan.is_nan());

        let inf: f64 =
            parse_yaml12_float("+.INF", loc, SfTag::None, NumericSyntax::default()).unwrap();
        assert!(inf.is_infinite() && inf.is_sign_positive());

        let neg_inf: f64 =
            parse_yaml12_float("-.Inf", loc, SfTag::None, NumericSyntax::default()).unwrap();
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
        assert!(
            parse_yaml12_float::<f64>(input, loc(), SfTag::None, NumericSyntax::default()).is_err()
        );
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
            parse_yaml12_float::<f32>("1.5", loc(), SfTag::None, NumericSyntax::default()).unwrap(),
            1.5f32
        );
        assert_eq!(
            parse_yaml12_float::<f32>("-123.456", loc(), SfTag::None, NumericSyntax::default())
                .unwrap(),
            -123.456f32
        );
    }

    #[test]
    fn test_zero_values() {
        assert_eq!(
            parse_yaml12_float::<f32>("0", loc(), SfTag::None, NumericSyntax::default()).unwrap(),
            0.0f32
        );
        assert_eq!(
            parse_yaml12_float::<f32>("-0", loc(), SfTag::None, NumericSyntax::default()).unwrap(),
            -0.0f32
        );
    }

    #[test]
    fn test_nan_and_infinity() {
        let nan: f32 =
            parse_yaml12_float(".nan", loc(), SfTag::None, NumericSyntax::default()).unwrap();
        assert!(nan.is_nan());

        let inf: f64 =
            parse_yaml12_float(".inf", loc(), SfTag::None, NumericSyntax::default()).unwrap();
        assert!(inf.is_infinite() && inf.is_sign_positive());

        let ninf: f32 =
            parse_yaml12_float("-.Inf", loc(), SfTag::None, NumericSyntax::default()).unwrap();
        assert!(ninf.is_infinite() && ninf.is_sign_negative());
    }

    #[test]
    fn test_subnormal_preserved() {
        // Smallest positive subnormal f32
        let smallest = f32::from_bits(1) as f64;
        let val: f32 = parse_yaml12_float(
            &format!("{}", smallest),
            loc(),
            SfTag::None,
            NumericSyntax::default(),
        )
        .unwrap();
        assert_eq!(val, f32::from_bits(1));
    }

    #[test]
    fn test_negative_zero_preserved() {
        let val: f32 =
            parse_yaml12_float("-0.0", loc(), SfTag::None, NumericSyntax::default()).unwrap();
        assert_eq!(val.to_bits(), (-0.0f32).to_bits());
    }
}
