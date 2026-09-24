//! Public macros for constructing options without relying on struct literal syntax.
//!
//! These macros exist to keep call sites ergonomic while allowing the crate to evolve
//! its options over time (e.g., adding fields) without forcing breaking changes.

/// Construct [`Schema::Specific`](crate::scalar::Schema::Specific) with selected options.
///
/// Omitted fields default to `false`: `strict_booleans`, `legacy_octal_numbers`,
/// and `quote_all`. Fields can appear in any order, and a trailing comma is allowed.
/// Expressions are evaluated once in the supplied order; repeated fields use
/// their last supplied value, as with [`options!`](crate::options!).
///
/// The variant is non-exhaustive. This macro obtains its defaults through
/// [`Schema::specific`](crate::scalar::Schema::specific) and then updates the
/// supplied fields, allowing new fields to be added without changing callers.
///
/// ```rust
/// use serde_saphyr::{scalar::Schema, specific};
///
/// let schema = specific! { strict_booleans: true };
/// assert!(matches!(schema, Schema::Specific {
///     strict_booleans: true,
///     legacy_octal_numbers: false,
///     quote_all: false,
///     ..
/// }));
/// assert_eq!(specific! {}, Schema::specific());
/// ```
#[macro_export]
macro_rules! specific {
    () => { $crate::scalar::Schema::specific() };
    ( $( $field:ident : $value:expr ),+ $(,)? ) => {{
        let mut schema = $crate::scalar::Schema::specific();
        $(
            match &mut schema {
                $crate::scalar::Schema::Specific { $field: field, .. } => {
                    *field = $value;
                }
                _ => ::core::panic!("Schema::specific() must return Specific"),
            }
        )+
        schema
    }};
}

/// Construct [`crate::Options`] from `Default` and a list of field assignments.
///
/// Example:
///
/// ```rust
/// # #[cfg(feature = "deserialize")]
/// # {
/// use serde_saphyr::options::DuplicateKeyPolicy;
///
/// let options = serde_saphyr::options! {
///     duplicate_keys: DuplicateKeyPolicy::LastWins,
///     schema: serde_saphyr::specific! {
///         strict_booleans: true,
///     },
/// };
/// # }
/// ```
#[cfg(feature = "deserialize")]
#[macro_export]
macro_rules! options {
    ( $( $tt:tt )* ) => {{
        let mut opt = $crate::Options::default();
        $crate::__serde_saphyr_options_apply!(opt, $( $tt )*);
        opt
    }};
}

#[cfg(not(feature = "deserialize"))]
#[macro_export]
macro_rules! options {
    ( $( $tt:tt )* ) => {
        compile_error!("serde-saphyr `options!` requires feature `deserialize`");
    };
}

/// Implementation detail for [`options!`].
///
/// This is `#[macro_export]` so that `$crate::...` can resolve it from expansions in
/// downstream crates.
#[doc(hidden)]
#[macro_export]
macro_rules! __serde_saphyr_options_apply {
    // End.
    ($opt:ident,) => {};
    ($opt:ident) => {};

    // Generic field assignment.
    ($opt:ident, $field:ident : $value:expr $(, $($rest:tt)*)? ) => {{
        $opt.$field = $value;
        $( $crate::__serde_saphyr_options_apply!($opt, $($rest)*); )?
    }};
}

/// Construct [`crate::SerializerOptions`] from `Default` and a list of field assignments.
///
/// Example:
///
/// ```rust
/// # #[cfg(feature = "serialize")]
/// # {
/// let opts = serde_saphyr::ser_options! {
///     indent_step: 4,
///     schema: serde_saphyr::specific! {
///         quote_all: true,
///     },
/// };
/// # }
/// ```
#[cfg(feature = "serialize")]
#[macro_export]
macro_rules! ser_options {
    ( $( $tt:tt )* ) => {{
        let mut opt = $crate::SerializerOptions::default();
        $crate::__serde_saphyr_serializer_options_apply!(opt, $( $tt )*);
        opt
    }};
}

#[cfg(not(feature = "serialize"))]
#[macro_export]
macro_rules! ser_options {
    ( $( $tt:tt )* ) => {
        compile_error!("serde-saphyr `ser_options!` requires feature `serialize`");
    };
}

/// Construct `Some([``crate::Budget``])` from `Default` and a list of field assignments.
///
/// This macro returns `Some(Budget)` (instead of just `Budget`) so it can be embedded
/// directly inside [`crate::options!`] as the value for `Options::budget`.
///
/// Example:
///
/// ```rust
/// # #[cfg(feature = "deserialize")]
/// # {
/// let options = serde_saphyr::options! {
///     budget: serde_saphyr::budget! {
///         max_nodes: 30,
///     },
/// };
/// # }
/// ```
#[cfg(feature = "deserialize")]
#[macro_export]
macro_rules! budget {
    ( $( $field:ident : $value:expr ),* $(,)? ) => {{
        let mut b = $crate::Budget::default();
        $(
            b.$field = $value;
        )*
        Some(b)
    }};
}

#[cfg(not(feature = "deserialize"))]
#[macro_export]
macro_rules! budget {
    ( $( $field:ident : $value:expr ),* $(,)? ) => {
        compile_error!("serde-saphyr `budget!` requires feature `deserialize`");
    };
}

/// Construct [`crate::options::AliasLimits`] from `Default` and a list of field assignments.
///
/// This macro returns `AliasLimits` directly so it can be embedded inside
/// [`crate::options!`] as the value for `Options::alias_limits`.
///
/// Example:
///
/// ```rust
/// # #[cfg(feature = "deserialize")]
/// # {
/// let options = serde_saphyr::options! {
///     alias_limits: serde_saphyr::alias_limits! {
///         max_replay_stack_depth: 16,
///     },
/// };
/// # let _ = options;
/// # }
/// ```
#[cfg(feature = "deserialize")]
#[macro_export]
macro_rules! alias_limits {
    ( $( $field:ident : $value:expr ),* $(,)? ) => {{
        let mut limits = $crate::options::AliasLimits::default();
        $(
            limits.$field = $value;
        )*
        limits
    }};
}

#[cfg(not(feature = "deserialize"))]
#[macro_export]
macro_rules! alias_limits {
    ( $( $field:ident : $value:expr ),* $(,)? ) => {
        compile_error!("serde-saphyr `alias_limits!` requires feature `deserialize`");
    };
}

/// Construct [`crate::RenderOptions`] from defaults and a list of field assignments.
///
/// This macro exists to keep call sites ergonomic while allowing `RenderOptions` to evolve
/// over time (e.g., adding fields) without forcing downstream crates to use struct literal
/// syntax.
///
/// Defaults:
/// - `formatter`: the built-in developer formatter (`DefaultMessageFormatter`)
/// - `snippets`: [`crate::SnippetMode::Auto`]
///
/// Example:
///
/// ```rust
/// # #[cfg(feature = "deserialize")]
/// # {
/// use serde_saphyr::{SnippetMode, UserMessageFormatter};
///
/// let user = UserMessageFormatter;
/// let opts = serde_saphyr::render_options! {
///     formatter: &user,
/// };
/// # }
/// ```
#[cfg(feature = "deserialize")]
#[macro_export]
macro_rules! render_options {
    ( $( $field:ident : $value:expr ),* $(,)? ) => {{
        let mut opt = $crate::RenderOptions::default();
        $(
            {
                opt.$field = $value;
            }
        )*
        opt
    }};
}

#[cfg(not(feature = "deserialize"))]
#[macro_export]
macro_rules! render_options {
    ( $( $tt:tt )* ) => {
        compile_error!("serde-saphyr `render_options!` requires feature `deserialize`");
    };
}

/// Implementation detail for [`ser_options!`].
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
            // Keep the limit aligned with SerializerOptions' runtime validation.
            if !($value > 0 && $value <= 64) {
                panic!("`indent_step` must be in the range 1..=64");
            }
        };
        $opt.indent_step = $value;
        $( $crate::__serde_saphyr_serializer_options_apply!($opt, $($rest)*); )?
    }};

    // indent_step for non-const expressions: allow compilation; runtime validation will apply.
    ($opt:ident, indent_step : $value:expr $(, $($rest:tt)*)? ) => {{
        $opt.indent_step = $value;
        $( $crate::__serde_saphyr_serializer_options_apply!($opt, $($rest)*); )?
    }};

    // Generic field assignment.
    ($opt:ident, $field:ident : $value:expr $(, $($rest:tt)*)? ) => {{
        $opt.$field = $value;
        $( $crate::__serde_saphyr_serializer_options_apply!($opt, $($rest)*); )?
    }};
}

// NOTE: `serializer_options!` intentionally removed; `ser_options!` is the canonical macro.
