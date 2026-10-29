//! Custom value formatter.
//!
//! Select it with [`PropertySyntax::Custom`](crate::PropertySyntax::Custom). Maps and sequences
//! are parsed as regular YAML; every value scalar under them (map values and sequence items at
//! any depth, plain, quoted or block) is handed whole to the formatter, and the returned string
//! is what gets deserialized. Keys and `!!binary` scalars are never formatted.
//! A [`FormatterError`] is reported as
//! [`Error::PropertyFormatterFailed`](crate::Error::PropertyFormatterFailed) at the scalar's location.

use std::borrow::Cow;
use std::fmt;

/// Error returned by a [`FormatterFn`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatterError {
    reason: String,
}

impl FormatterError {
    /// Creates an error; `reason` is shown to the user next to the scalar's location.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    /// Why the formatter failed.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Display for FormatterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.reason)
    }
}

impl std::error::Error for FormatterError {}

/// Signature of a custom formatter: receives one scalar value, returns its replacement.
pub type FormatterFn = fn(Cow<'_, str>) -> Result<Cow<'_, str>, FormatterError>;

/// Identity formatter: leaves every value as written.
fn identity(input: Cow<'_, str>) -> Result<Cow<'_, str>, FormatterError> {
    Ok(input)
}

/// Wrapper around a [`FormatterFn`] so it can live inside [`PropertySyntax`](crate::PropertySyntax).
#[derive(Clone, Copy)]
pub struct PropertyCustomFormatter {
    fmt: FormatterFn,
}

impl PropertyCustomFormatter {
    /// Wraps `fmt`.
    pub const fn new(fmt: FormatterFn) -> Self {
        Self { fmt }
    }

    /// Formats one scalar value.
    pub fn format<'f>(&self, input: Cow<'f, str>) -> Result<Cow<'f, str>, FormatterError> {
        (self.fmt)(input)
    }
}

impl From<FormatterFn> for PropertyCustomFormatter {
    fn from(fmt: FormatterFn) -> Self {
        Self::new(fmt)
    }
}

impl Default for PropertyCustomFormatter {
    fn default() -> Self {
        Self::new(identity)
    }
}

/// Two formatters are equal when they wrap the same function.
/// Like any function-pointer comparison this is best-effort: the compiler may merge
/// identical functions or duplicate one across codegen units.
impl PartialEq for PropertyCustomFormatter {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::fn_addr_eq(self.fmt, other.fmt)
    }
}

impl Eq for PropertyCustomFormatter {}

impl fmt::Debug for PropertyCustomFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PropertyCustomFormatter")
            .field("fmt", &(self.fmt as *const ()))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{FormatterError, FormatterFn, PropertyCustomFormatter};
    use std::borrow::Cow;

    fn upper(input: Cow<'_, str>) -> Result<Cow<'_, str>, FormatterError> {
        Ok(Cow::Owned(input.to_uppercase()))
    }

    fn reject(_: Cow<'_, str>) -> Result<Cow<'_, str>, FormatterError> {
        Err(FormatterError::new("nope"))
    }

    #[test]
    fn default_formatter_is_identity() {
        let val = "test with $SOME va${RS} and st $ U$$F";
        let out = PropertyCustomFormatter::default()
            .format(Cow::Borrowed(val))
            .unwrap();
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out, val);
    }

    #[test]
    fn format_calls_wrapped_function() {
        let f = PropertyCustomFormatter::new(upper);
        assert_eq!(f.format(Cow::Borrowed("abc")).unwrap(), "ABC");
        let err = PropertyCustomFormatter::new(reject)
            .format(Cow::Borrowed("abc"))
            .unwrap_err();
        assert_eq!(err.reason(), "nope");
        assert_eq!(err.to_string(), "nope");
    }

    #[test]
    fn equality_follows_wrapped_function() {
        let a = PropertyCustomFormatter::new(upper);
        assert_eq!(a, PropertyCustomFormatter::from(upper as FormatterFn));
        assert_ne!(a, PropertyCustomFormatter::new(reject));
    }
}
