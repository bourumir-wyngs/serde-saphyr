//! Scalar resolution without a Serde visitor or a YAML value tree.
//!
//! [`resolve`] classifies decoded scalar text using an explicit [`Schema`]. The
//! result borrows the original text; classification does not depend on a machine
//! integer or floating-point range. Callers can use their own arbitrary-precision
//! or date types, or request the checked primitive conversions on [`ResolvedScalar`].
//!
//! These policies are separate from typed Serde deserialization and its
//! compatibility options. In particular, [`Schema::Yaml12`] accepts leading-zero decimal
//! integers, but not binary integers, underscores, or YAML 1.1 booleans.
//! No whitespace is trimmed, including in explicitly tagged quoted/block scalars.
//! Supply decoded content, without source quotes, tag syntax, or block indicators.
//!
//! The [YAML 1.2.2 schemas](https://yaml.org/spec/1.2.2/#chapter-10-recommended-schemas)
//! define the resolution used by [`Schema::Strings`] (Failsafe),
//! [`Schema::Json`] (JSON), and [`Schema::Yaml12`] (Core). The YAML 1.1 policy adds the
//! [scalar type conventions](https://yaml.org/type/), including timestamps and
//! base-60 numbers. It follows the numeric examples where the 1.1 draft's float
//! expression is inconsistent: fractional underscores are accepted, while a
//! number must contain a digit and at most one decimal point. Radix prefixes must
//! also be followed by at least one digit, not just underscores.
//!
//! ```
//! use serde_saphyr::scalar::{resolve, ScalarKind, ScalarStyle, Schema};
//!
//! let number = resolve("0x2a", ScalarStyle::Plain, None, Schema::Yaml12)?;
//! assert_eq!(number.kind(), ScalarKind::Integer);
//! assert_eq!(number.to_u128()?, 42);
//!
//! let text = resolve("a.infra", ScalarStyle::Plain, None, Schema::Yaml12)?;
//! assert_eq!(text.kind(), ScalarKind::String);
//! assert_eq!(text.text(), "a.infra");
//! # Ok::<(), serde_saphyr::scalar::ScalarError>(())
//! ```

use crate::parse_scalars::checked_digits_u128;
use std::{borrow::Cow, fmt, str::FromStr};

mod legacy;

/// The scalar vocabulary used for resolution and validation of supported tags.
///
/// Choose a policy explicitly; this type deliberately has no default. Only
/// scalar types are covered, not collection tags, merge keys, or document syntax.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Schema {
    /// All implicit scalars are strings. Only the string tag is supported.
    Strings,
    /// YAML 1.2 JSON schema: lowercase booleans/null and decimal numbers.
    /// Unmatched plain scalars are errors, including empty scalars. As specified
    /// by YAML 1.2.2, `1.` is accepted (unlike the JSON file format).
    Json,
    /// YAML 1.2 Core schema, including decimal, `0o` octal, `0x` hexadecimal,
    /// dotted infinities/NaN, and the prescribed boolean/null case variants.
    /// Dates and unmatched scalars are strings.
    Yaml12,
    /// YAML 1.1 scalar conventions: legacy booleans, binary/leading-zero octal,
    /// numeric underscores, base-60 numbers, and lexical timestamps.
    /// See the module documentation for treatment of draft inconsistencies.
    Yaml11,
}

/// Presentation style of the decoded scalar.
///
/// Only untagged plain scalars undergo implicit type resolution. An explicit
/// supported tag takes precedence over any style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScalarStyle {
    /// An unquoted scalar.
    Plain,
    /// A single-quoted scalar.
    SingleQuoted,
    /// A double-quoted scalar.
    DoubleQuoted,
    /// A literal block scalar (`|`).
    Literal,
    /// A folded block scalar (`>`).
    Folded,
}

#[cfg(feature = "deserialize")]
impl From<granit_parser::ScalarStyle> for ScalarStyle {
    fn from(style: granit_parser::ScalarStyle) -> Self {
        match style {
            granit_parser::ScalarStyle::Plain => Self::Plain,
            granit_parser::ScalarStyle::SingleQuoted => Self::SingleQuoted,
            granit_parser::ScalarStyle::DoubleQuoted => Self::DoubleQuoted,
            granit_parser::ScalarStyle::Literal => Self::Literal,
            granit_parser::ScalarStyle::Folded => Self::Folded,
        }
    }
}

/// A scalar's lexical type, independent of representable numeric range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ScalarKind {
    /// Text, preserved exactly as supplied.
    String,
    /// A null literal.
    Null,
    /// A boolean literal.
    Boolean,
    /// An integer literal, possibly larger than `u128` or `i128`.
    Integer,
    /// A floating-point literal, possibly outside any primitive float's range.
    Float,
    /// YAML 1.1 timestamp syntax. This does **not** validate calendar dates,
    /// clock/offset ranges, or leap seconds, nor assign a timezone. Use [`ResolvedScalar::text`]
    /// with the caller's date/time type to perform those checks.
    Timestamp,
}

impl fmt::Display for ScalarKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::String => "string",
            Self::Null => "null",
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Timestamp => "timestamp",
        })
    }
}

/// Resolution or conversion failure, without Serde or source-location coupling.
///
/// The caller retains the input text/tag and can attach its own diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScalarError {
    /// A supported explicit tag has invalid content for this schema.
    InvalidValue {
        /// The type required by the tag.
        kind: ScalarKind,
    },
    /// The tag is unknown, is a collection tag, or is unsupported by this schema.
    UnsupportedTag,
    /// An implicit plain scalar does not match the JSON schema.
    UnresolvedPlainScalar,
    /// The requested conversion does not match the resolved type.
    TypeMismatch {
        /// The type required by the conversion.
        expected: ScalarKind,
        /// The scalar's resolved type.
        actual: ScalarKind,
    },
    /// The integer does not fit the requested type, an unsigned conversion has
    /// a minus sign (including `-0`), or a finite float overflows to infinity.
    OutOfRange,
}

impl fmt::Display for ScalarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue { kind } => write!(f, "invalid {kind} scalar for selected schema"),
            Self::UnsupportedTag => f.write_str("unsupported scalar tag for selected schema"),
            Self::UnresolvedPlainScalar => {
                f.write_str("plain scalar does not match the JSON schema")
            }
            Self::TypeMismatch { expected, actual } => {
                write!(f, "expected {expected}, found {actual}")
            }
            Self::OutOfRange => f.write_str("scalar is outside the requested numeric range"),
        }
    }
}

impl std::error::Error for ScalarError {}

/// Borrowed scalar classification with optional checked conversions.
///
/// Construction goes through [`resolve`], so the syntax is validated before any
/// conversion. Use [`text`](Self::text) to retain spelling, parse larger numbers,
/// validate timestamps, or construct application-specific values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedScalar<'a> {
    text: &'a str,
    kind: ScalarKind,
    schema: Schema,
}

impl<'a> ResolvedScalar<'a> {
    /// Return the original decoded text without normalization or allocation.
    pub fn text(&self) -> &'a str {
        self.text
    }

    /// Return the lexical classification, regardless of conversion limits.
    pub fn kind(&self) -> ScalarKind {
        self.kind
    }

    /// Convert a boolean. Other scalar kinds return [`ScalarError::TypeMismatch`].
    pub fn to_bool(&self) -> Result<bool, ScalarError> {
        self.require(ScalarKind::Boolean)?;
        bool_value(self.text, self.schema).ok_or(ScalarError::InvalidValue { kind: self.kind })
    }

    /// Convert an integer in the inclusive range `i128::MIN..=i128::MAX`.
    ///
    /// All radices permitted by the selected schema are supported. Overflow is
    /// an error, never a float fallback. Use `TryFrom` on the result for narrower
    /// integer types; use [`text`](Self::text) for arbitrary-precision conversion.
    pub fn to_i128(&self) -> Result<i128, ScalarError> {
        self.require(ScalarKind::Integer)?;
        let magnitude = self.integer_magnitude()?;
        if self.text.starts_with('-') {
            if magnitude == (i128::MAX as u128) + 1 {
                return Ok(i128::MIN);
            }
            i128::try_from(magnitude)
                .map(|value| -value)
                .map_err(|_| ScalarError::OutOfRange)
        } else {
            i128::try_from(magnitude).map_err(|_| ScalarError::OutOfRange)
        }
    }

    /// Convert an integer in `0..=u128::MAX`.
    ///
    /// A minus sign, including on `-0`, returns [`ScalarError::OutOfRange`].
    pub fn to_u128(&self) -> Result<u128, ScalarError> {
        self.require(ScalarKind::Integer)?;
        if self.text.starts_with('-') {
            return Err(ScalarError::OutOfRange);
        }
        self.integer_magnitude()
    }

    /// Convert a float with overflow checking.
    ///
    /// Explicit dotted infinities and NaN are successful conversions. A finite
    /// literal such as `1e999` that becomes infinity returns [`ScalarError::OutOfRange`].
    /// IEEE rounding, loss of precision, and underflow to signed zero are allowed.
    /// YAML 1.1 base-60 components are accumulated at the target float's precision.
    /// Integers return [`ScalarError::TypeMismatch`], avoiding implicit precision loss.
    pub fn to_f64(&self) -> Result<f64, ScalarError> {
        self.convert_float()
    }

    /// Convert directly to `f32`, with the same policy as [`to_f64`](Self::to_f64).
    /// Overflow is checked against `f32` rather than `f64` range.
    pub fn to_f32(&self) -> Result<f32, ScalarError> {
        self.convert_float()
    }

    fn require(&self, expected: ScalarKind) -> Result<(), ScalarError> {
        if self.kind == expected {
            Ok(())
        } else {
            Err(ScalarError::TypeMismatch {
                expected,
                actual: self.kind,
            })
        }
    }

    fn integer_magnitude(&self) -> Result<u128, ScalarError> {
        let unsigned = self.text.strip_prefix(['+', '-']).unwrap_or(self.text);
        let parse = |digits: &str, radix| {
            checked_digits_u128(digits.bytes().filter(|&b| b != b'_'), radix)
                .ok_or(ScalarError::OutOfRange)
        };
        if self.schema == Schema::Yaml11 && unsigned.contains(':') {
            let mut value = 0u128;
            for part in unsigned.split(':') {
                value = value
                    .checked_mul(60)
                    .and_then(|value| value.checked_add(parse(part, 10).ok()?))
                    .ok_or(ScalarError::OutOfRange)?;
            }
            return Ok(value);
        }
        let (radix, digits) = if let Some(digits) = unsigned.strip_prefix("0x") {
            (16, digits)
        } else if let Some(digits) = unsigned.strip_prefix("0o") {
            (8, digits)
        } else if let Some(digits) = unsigned.strip_prefix("0b") {
            (2, digits)
        } else if self.schema == Schema::Yaml11 && unsigned.starts_with('0') {
            (8, unsigned)
        } else {
            (10, unsigned)
        };
        parse(digits, radix)
    }

    fn convert_float<T: num_traits::Float + FromStr>(&self) -> Result<T, ScalarError> {
        self.require(ScalarKind::Float)?;
        let unsigned = self.text.strip_prefix(['+', '-']).unwrap_or(self.text);
        // Syntax was validated by resolve; these comparisons cannot accept substrings.
        if unsigned.eq_ignore_ascii_case(".inf") {
            return Ok(if self.text.starts_with('-') {
                T::neg_infinity()
            } else {
                T::infinity()
            });
        }
        if unsigned.eq_ignore_ascii_case(".nan") {
            return Ok(T::nan());
        }
        let normalized = if self.text.contains('_') {
            Cow::Owned(self.text.replace('_', ""))
        } else {
            Cow::Borrowed(self.text)
        };
        let invalid = || ScalarError::InvalidValue {
            kind: ScalarKind::Float,
        };
        let value = if self.schema == Schema::Yaml11 && normalized.contains(':') {
            let unsigned = normalized.strip_prefix(['+', '-']).unwrap_or(&normalized);
            let sixty = T::from(60u8).ok_or(ScalarError::OutOfRange)?;
            let mut value = T::zero();
            for part in unsigned.split(':') {
                value = value * sixty + part.parse::<T>().map_err(|_| invalid())?;
            }
            if normalized.starts_with('-') {
                -value
            } else {
                value
            }
        } else {
            normalized.parse::<T>().map_err(|_| invalid())?
        };
        if value.is_finite() {
            Ok(value)
        } else {
            Err(ScalarError::OutOfRange)
        }
    }
}

/// Resolve one decoded scalar, borrowing its text and allocating no value tree.
///
/// `resolved_tag` must be an expanded URI, such as `tag:yaml.org,2002:int`,
/// **not** source syntax such as `!!int`, `!<...>`, or an unexpanded `%TAG` handle.
/// With `granit_parser`, use the parsed tag's `to_string()` and convert the style
/// with `.into()`. `None` or `Some("?")` requests implicit resolution by style;
/// `Some("!")` forces a string. Explicit supported tags override every style.
///
/// Supported tags are `str`, `null`, `bool`, `int`, and `float` in the YAML
/// namespace, plus `timestamp` under [`Schema::Yaml11`]. [`Schema::Strings`] supports only
/// `str`. Unknown or unsupported tags return [`ScalarError::UnsupportedTag`],
/// allowing the caller to handle custom types without silently reinterpreting them.
/// Explicit payloads are never trimmed. Tagged floats also accept canonical
/// forms omitted from implicit resolution: [`Schema::Json`] accepts `.inf`, `-.inf`,
/// and `.nan`; [`Schema::Yaml11`] accepts decimal integers such as `0` and `42` as floats.
///
/// Classification consumes the whole text and is independent of numeric range.
/// A valid but oversized number still resolves; only its conversion can overflow.
pub fn resolve<'a>(
    text: &'a str,
    style: ScalarStyle,
    resolved_tag: Option<&str>,
    schema: Schema,
) -> Result<ResolvedScalar<'a>, ScalarError> {
    let kind = match resolved_tag {
        Some("!") | Some("tag:yaml.org,2002:str") => ScalarKind::String,
        None | Some("?") if style != ScalarStyle::Plain || schema == Schema::Strings => {
            ScalarKind::String
        }
        None | Some("?") => implicit_kind(text, schema)?,
        Some(tag) => {
            if schema == Schema::Strings {
                return Err(ScalarError::UnsupportedTag);
            }
            let kind = match tag.strip_prefix(crate::tag::YAML_TAG_NAMESPACE) {
                Some("null") => ScalarKind::Null,
                Some("bool") => ScalarKind::Boolean,
                Some("int") => ScalarKind::Integer,
                Some("float") => ScalarKind::Float,
                Some("timestamp") if schema == Schema::Yaml11 => ScalarKind::Timestamp,
                _ => return Err(ScalarError::UnsupportedTag),
            };
            let valid = match kind {
                ScalarKind::Null => is_null(text, schema),
                ScalarKind::Boolean => bool_value(text, schema).is_some(),
                ScalarKind::Integer => is_integer(text, schema),
                ScalarKind::Float => is_explicit_float(text, schema),
                ScalarKind::Timestamp => legacy::is_timestamp(text),
                ScalarKind::String => true,
            };
            if !valid {
                return Err(ScalarError::InvalidValue { kind });
            }
            kind
        }
    };
    Ok(ResolvedScalar { text, kind, schema })
}

fn implicit_kind(text: &str, schema: Schema) -> Result<ScalarKind, ScalarError> {
    Ok(if is_null(text, schema) {
        ScalarKind::Null
    } else if bool_value(text, schema).is_some() {
        ScalarKind::Boolean
    } else if is_integer(text, schema) {
        ScalarKind::Integer
    } else if is_float(text, schema) {
        ScalarKind::Float
    } else if schema == Schema::Yaml11 && legacy::is_timestamp(text) {
        ScalarKind::Timestamp
    } else if schema == Schema::Json {
        return Err(ScalarError::UnresolvedPlainScalar);
    } else {
        ScalarKind::String
    })
}

fn is_null(text: &str, schema: Schema) -> bool {
    match schema {
        Schema::Strings => false,
        Schema::Json => text == "null",
        Schema::Yaml12 | Schema::Yaml11 => matches!(text, "" | "~" | "null" | "Null" | "NULL"),
    }
}

fn bool_value(text: &str, schema: Schema) -> Option<bool> {
    match schema {
        Schema::Strings => None,
        Schema::Yaml11 => legacy::parse_bool(text),
        Schema::Json => match text {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        Schema::Yaml12 => match text {
            "true" | "True" | "TRUE" => Some(true),
            "false" | "False" | "FALSE" => Some(false),
            _ => None,
        },
    }
}

fn digits(text: &str, radix: u32) -> bool {
    !text.is_empty() && text.bytes().all(|b| char::from(b).is_digit(radix))
}

fn is_integer(text: &str, schema: Schema) -> bool {
    match schema {
        Schema::Strings => false,
        Schema::Yaml11 => legacy::is_integer(text),
        Schema::Json => json_integer(text.strip_prefix('-').unwrap_or(text)),
        Schema::Yaml12 => {
            if let Some(rest) = text.strip_prefix("0x") {
                digits(rest, 16)
            } else if let Some(rest) = text.strip_prefix("0o") {
                digits(rest, 8)
            } else {
                digits(text.strip_prefix(['+', '-']).unwrap_or(text), 10)
            }
        }
    }
}

fn json_integer(unsigned: &str) -> bool {
    digits(unsigned, 10) && (unsigned == "0" || !unsigned.starts_with('0'))
}

fn is_explicit_float(text: &str, schema: Schema) -> bool {
    if is_float(text, schema) {
        return true;
    }
    match schema {
        // YAML 1.2.2 section 10.2.1.4 includes these explicit canonical values,
        // although JSON's implicit resolution table deliberately omits them.
        Schema::Json => matches!(text, ".inf" | "-.inf" | ".nan"),
        Schema::Yaml11 => {
            let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
            unsigned.as_bytes().first().is_some_and(u8::is_ascii_digit)
                && unsigned.bytes().all(|b| b.is_ascii_digit() || b == b'_')
        }
        Schema::Strings | Schema::Yaml12 => false,
    }
}

fn is_float(text: &str, schema: Schema) -> bool {
    if schema == Schema::Yaml11 {
        return legacy::is_float(text);
    }
    if schema == Schema::Strings {
        return false;
    }
    let unsigned = if schema == Schema::Yaml12 {
        let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
        if matches!(unsigned, ".inf" | ".Inf" | ".INF") || matches!(text, ".nan" | ".NaN" | ".NAN")
        {
            return true;
        }
        unsigned
    } else {
        text.strip_prefix('-').unwrap_or(text)
    };
    let mantissa = if let Some((mantissa, exponent)) = unsigned.split_once(['e', 'E']) {
        if !digits(exponent.strip_prefix(['+', '-']).unwrap_or(exponent), 10) {
            return false;
        }
        mantissa
    } else {
        unsigned
    };
    let (whole, fraction) = mantissa
        .split_once('.')
        .map_or((mantissa, None), |(w, f)| (w, Some(f)));
    if fraction.is_some_and(|fraction| !fraction.is_empty() && !digits(fraction, 10)) {
        return false;
    }
    if schema == Schema::Json {
        json_integer(whole)
    } else {
        digits(whole, 10) || (whole.is_empty() && fraction.is_some_and(|f| digits(f, 10)))
    }
}
