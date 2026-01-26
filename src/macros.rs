//! Public macros for constructing option structs without relying on struct literal syntax.
//!
//! These macros exist to keep call sites ergonomic while allowing the crate to evolve
//! its option structs over time (e.g., adding fields) without forcing breaking changes.

/// Construct [`crate::Options`] from `Default` and a list of field assignments.
///
/// Example:
///
/// ```rust
/// use serde_saphyr::options::DuplicateKeyPolicy;
///
/// let options = serde_saphyr::options! {
///     duplicate_keys: DuplicateKeyPolicy::LastWins,
///     strict_booleans: true,
/// };
/// ```
#[macro_export]
macro_rules! options {
    ( $( $field:ident : $value:expr ),* $(,)? ) => {{
        let mut opt = $crate::Options::default();
        $(
            #[allow(deprecated)]
            {
                opt.$field = $value;
            }
        )*
        opt
    }};
}

/// Construct [`crate::SerializerOptions`] from `Default` and a list of field assignments.
///
/// Example:
///
/// ```rust
/// let opts = serde_saphyr::serializer_options! {
///     indent_step: 4,
///     quote_all: true,
/// };
/// ```
#[macro_export]
macro_rules! serializer_options {
    ( $( $field:ident : $value:expr ),* $(,)? ) => {{
        let mut opt = $crate::SerializerOptions::default();
        $(
            #[allow(deprecated)]
            {
                opt.$field = $value;
            }
        )*
        opt
    }};
}

/// Compatibility alias for [`serializer_options!`].
///
/// The name is intentionally short for call sites.
#[macro_export]
macro_rules! ser_options {
    ( $( $field:ident : $value:expr ),* $(,)? ) => {{
        $crate::serializer_options! { $( $field : $value ),* }
    }};
}
