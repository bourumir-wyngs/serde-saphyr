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
    ( $( $tt:tt )* ) => {{
        let mut opt = $crate::SerializerOptions::default();
        $crate::__serde_saphyr_serializer_options_apply!(opt, $( $tt )*);
        opt
    }};
}

/// Construct `Some([`crate::Budget`])` from `Default` and a list of field assignments.
///
/// This macro returns `Some(Budget)` (instead of just `Budget`) so it can be embedded
/// directly inside [`crate::options!`] as the value for `Options::budget`.
///
/// Example:
///
/// ```rust
/// let options = serde_saphyr::options! {
///     budget: serde_saphyr::budget! {
///         max_nodes: 30,
///     },
/// };
/// ```
#[macro_export]
macro_rules! budget {
    ( $( $field:ident : $value:expr ),* $(,)? ) => {{
        let mut b = $crate::Budget::default();
        $(
            #[allow(deprecated)]
            {
                b.$field = $value;
            }
        )*
        Some(b)
    }};
}

/// Implementation detail for [`serializer_options!`].
///
/// This is `#[macro_export]` so that `$crate::...` can resolve it from expansions in
/// downstream crates.
#[doc(hidden)]
#[macro_export]
macro_rules! __serde_saphyr_serializer_options_apply {
    // End.
    ($opt:ident,) => {};
    ($opt:ident) => {};

    // Special-case indent_step when the value is a literal: enforce at compile time.
    ($opt:ident, indent_step : $value:literal $(, $($rest:tt)*)? ) => {{
        const _: () = {
            // Keep the check aligned with the YAML emitter's constraints.
            // Valid range: 1..=65535.
            if !($value > 0 && $value < 65536) {
                panic!("`indent_step` must be in the range 1..=65535");
            }
        };
        #[allow(deprecated)]
        {
            $opt.indent_step = $value;
        }
        $( $crate::__serde_saphyr_serializer_options_apply!($opt, $($rest)*); )?
    }};

    // indent_step for non-const expressions: allow compilation; runtime validation will apply.
    ($opt:ident, indent_step : $value:expr $(, $($rest:tt)*)? ) => {{
        #[allow(deprecated)]
        {
            $opt.indent_step = $value;
        }
        $( $crate::__serde_saphyr_serializer_options_apply!($opt, $($rest)*); )?
    }};

    // Generic field assignment.
    ($opt:ident, $field:ident : $value:expr $(, $($rest:tt)*)? ) => {{
        #[allow(deprecated)]
        {
            $opt.$field = $value;
        }
        $( $crate::__serde_saphyr_serializer_options_apply!($opt, $($rest)*); )?
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
