use crate::location::Location;

/// Formats errors and their source locations for display.
///
/// This trait is intentionally not wired into the crate yet; it is a building block
/// for future configurable error rendering.
pub trait ErrorFormatter {
    /// Convert an error into a displayable string.
    fn format_error(&self, err: &(dyn std::error::Error + 'static)) -> String;

    /// Format a [`Location`] (line/column/span) into a displayable string.
    fn format_location(&self, location: &Location) -> String;
}

/// The default developer-oriented formatter matching the crate's current plain-text style.
///
/// This is intentionally simple: errors are rendered via their `Display` implementation,
/// and locations are formatted as the suffix currently used by error `Display`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Developer;

impl ErrorFormatter for Developer {
    #[inline]
    fn format_error(&self, err: &(dyn std::error::Error + 'static)) -> String {
        err.to_string()
    }

    #[inline]
    fn format_location(&self, location: &Location) -> String {
        if location == &Location::UNKNOWN {
            String::new()
        } else {
            format!("at line {}, column {}", location.line(), location.column())
        }
    }
}
