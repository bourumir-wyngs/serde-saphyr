#[cfg(feature = "properties")]
use std::cell::RefCell;

/// Property interpolation can resolve `${NAME}` placeholders into concrete values before the
/// target type finishes deserializing. That is normally desirable for successful parsing, but it
/// becomes a problem when later error paths echo the already-resolved text back to the caller.
/// For example, a custom `Deserialize` implementation may read one or more interpolated child
/// scalars first and only then return `de::Error::custom(...)`, `invalid_value(...)`,
/// `unknown_variant(...)`, or a similar Serde error. Without extra tracking, those later errors
/// can end up containing the secret resolved property value instead of the original `${...}`
/// source text.
///
/// This module implements the interpolation redaction layer that prevents such leaks. While a
/// deserialization operation is running, `src/de.rs`, `src/lib.rs`, and
/// `src/de/with_deserializer.rs` open subtree-local scopes and record every interpolated scalar as
/// a `{ raw, effective }` pair. When Serde eventually formats a dynamic error in `src/de_error.rs`,
/// the redaction helpers in this module scan the message and replace any remembered resolved value
/// with its original interpolation form, or with a generic redacted form when needed.
///
/// In other words, redaction here means: keep deserialization behavior unchanged, but scrub
/// resolved property values back out of error messages before they leave the library. The logic is
/// isolated in this module because it is specific to the `properties` feature; in non-`properties`
/// builds the same API remains available as lightweight no-ops so the surrounding deserializer code
/// does not need separate call paths.
///
/// Stores one interpolated scalar's original `${...}` text together with the resolved value that
/// must be redacted back out of later error messages.
///
/// Constructed from scalar deserialization code in `src/de.rs` and then recorded into the current
/// subtree scope by [`ScalarRedactionGuard`].
#[cfg_attr(not(feature = "properties"), allow(dead_code))]
#[derive(Clone, Debug)]
pub(crate) struct ScalarRedactionCtx {
    pub(crate) raw: String,
    pub(crate) effective: String,
}

#[cfg(feature = "properties")]
/// Accumulates every interpolated scalar seen while one deserialize subtree is running.
///
/// The active scope is filled by [`ScalarRedactionGuard`] each time a child interpolated
/// scalar is entered, and later read by the redaction helpers below when Serde formats a
/// custom/dynamic error after child deserialization has already completed.
#[derive(Default)]
struct InterpRedactionScope {
    pairs: Vec<ScalarRedactionCtx>,
}

#[cfg(feature = "properties")]
thread_local! {
    static INTERP_REDACTION: RefCell<Vec<InterpRedactionScope>> = const { RefCell::new(Vec::new()) };
}

/// Registers one interpolated scalar with the current subtree redaction scope.
///
/// Created by `with_scalar_redaction` in `src/de.rs` around scalar and subtree
/// deserialization boundaries so later container-level errors can still redact the resolved
/// value back to the original interpolation expression.
#[cfg(feature = "properties")]
pub(crate) struct ScalarRedactionGuard {
    ctx: Option<ScalarRedactionCtx>,
}

/// Registers one interpolated scalar with the current subtree redaction scope.
///
/// In non-`properties` builds there is no interpolation state to track, so this becomes a
/// no-op guard with the same API for callers in `src/de.rs`.
#[cfg(not(feature = "properties"))]
pub(crate) struct ScalarRedactionGuard;

/// Opens one subtree-local interpolation redaction scope for the duration of a deserialize
/// operation.
///
/// Constructed by [`with_interp_redaction_scope`], which is called from public entry points,
/// validation wrappers, helper closures, and nested deserialize boundaries.
#[cfg(feature = "properties")]
pub(crate) struct InterpRedactionScopeGuard;

#[cfg(feature = "properties")]
impl InterpRedactionScopeGuard {
    /// Pushes a fresh scope onto the thread-local redaction stack.
    ///
    /// Called only by [`with_interp_redaction_scope`] before executing the wrapped operation.
    pub(crate) fn new() -> Self {
        INTERP_REDACTION.with(|cell| {
            cell.borrow_mut().push(InterpRedactionScope::default());
        });
        Self
    }
}

#[cfg(feature = "properties")]
impl ScalarRedactionGuard {
    /// Records the current interpolated scalar in the active subtree scope, avoiding
    /// duplicate `{ raw, effective }` pairs.
    ///
    /// Called by `with_scalar_redaction` in `src/de.rs` whenever deserialization enters a
    /// scalar or subtree that may later need error-message redaction.
    pub(crate) fn new(ctx: ScalarRedactionCtx) -> Self {
        INTERP_REDACTION.with(|cell| {
            let mut stack = cell.borrow_mut();
            for scope in stack.iter_mut() {
                if !scope
                    .pairs
                    .iter()
                    .any(|pair| pair.raw == ctx.raw && pair.effective == ctx.effective)
                {
                    scope.pairs.push(ctx.clone());
                }
            }
        });
        Self { ctx: Some(ctx) }
    }
}

#[cfg(not(feature = "properties"))]
impl ScalarRedactionGuard {
    /// Keeps the redaction-guard call sites in `src/de.rs` uniform when property
    /// interpolation support is compiled out.
    pub(crate) fn new(_: ScalarRedactionCtx) -> Self {
        Self
    }
}

#[cfg(feature = "properties")]
impl Drop for ScalarRedactionGuard {
    fn drop(&mut self) {
        let _ = self.ctx.take();
    }
}

#[cfg(feature = "properties")]
impl Drop for InterpRedactionScopeGuard {
    fn drop(&mut self) {
        INTERP_REDACTION.with(|cell| {
            let _ = cell.borrow_mut().pop();
        });
    }
}

#[cfg(feature = "properties")]
#[cold]
#[inline(never)]
/// Exposes the current subtree's recorded interpolation pairs to the redaction helpers below.
///
/// Called by `redact_custom_message`, `redact_dynamic_value`, `redact_dynamic_identifier`,
/// and validation issue sanitization in `src/de_error.rs`.
pub(crate) fn with_interp_redaction<T>(f: impl FnOnce(&[ScalarRedactionCtx]) -> T) -> T {
    INTERP_REDACTION.with(|cell| {
        let borrow = cell.borrow();
        let pairs = borrow
            .last()
            .map(|scope| scope.pairs.as_slice())
            .unwrap_or(&[]);
        f(pairs)
    })
}

#[cfg(feature = "properties")]
#[cold]
#[inline(never)]
/// Rewrites any resolved interpolated values found in `text` back to their original raw
/// `${...}` forms, or falls back to a generic message if no recorded value matches.
///
/// Called by the dynamic/custom Serde error redaction helpers and validation issue
/// sanitization when they need to scrub text against all pairs collected for the current subtree.
pub(crate) fn redact_with_ctxs(
    mut text: String,
    ctxs: &[ScalarRedactionCtx],
    fallback: &str,
) -> String {
    let mut pairs: Vec<&ScalarRedactionCtx> = ctxs
        .iter()
        .filter(|ctx| !ctx.effective.is_empty())
        .collect();

    pairs.sort_by_key(|ctx| std::cmp::Reverse(ctx.effective.len()));

    let mut replaced = false;
    for ctx in pairs {
        if text.contains(&ctx.effective) {
            text = text.replace(&ctx.effective, &ctx.raw);
            replaced = true;
        }
    }
    if replaced {
        text
    } else {
        fallback.to_owned()
    }
}

#[cfg(feature = "properties")]
#[cold]
#[inline(never)]
/// Redacts free-form Serde custom error messages using the current subtree scope.
///
/// Called from `Error::custom` so custom `Deserialize` implementations cannot leak resolved
/// property values after reading nested data.
pub(crate) fn redact_custom_message(text: String) -> String {
    with_interp_redaction(|ctxs| {
        if ctxs.is_empty() {
            text
        } else {
            redact_with_ctxs(text, ctxs, "invalid interpolated scalar value")
        }
    })
}

#[cfg(not(feature = "properties"))]
/// Preserves the core error formatting flow when interpolation redaction is disabled.
///
/// Called from `src/de_error.rs`; without the `properties` feature the original message is
/// returned unchanged.
pub(crate) fn redact_custom_message(text: String) -> String {
    text
}

#[cfg(feature = "properties")]
#[cold]
#[inline(never)]
/// Redacts dynamic Serde value/type strings using all interpolation pairs recorded for the
/// current subtree.
///
/// Called from dynamic Serde constructors such as `invalid_type` and `invalid_value`.
pub(crate) fn redact_dynamic_value(text: String, fallback: &str) -> String {
    with_interp_redaction(|ctxs| {
        if ctxs.is_empty() {
            text
        } else {
            redact_with_ctxs(text, ctxs, fallback)
        }
    })
}

#[cfg(not(feature = "properties"))]
/// Preserves dynamic Serde error text unchanged when interpolation redaction is disabled.
///
/// Called from `src/de_error.rs` for `invalid_type` and `invalid_value` in non-`properties`
/// builds.
pub(crate) fn redact_dynamic_value(text: String, _: &str) -> String {
    text
}

#[cfg(feature = "properties")]
#[cold]
#[inline(never)]
/// Redacts identifier-like dynamic strings, returning the raw interpolation token on exact
/// match or a generic fallback otherwise.
///
/// Called from `unknown_variant` and `unknown_field`, where Serde passes the unexpected
/// identifier separately from the rest of the message.
pub(crate) fn redact_dynamic_identifier(text: &str, fallback: &str) -> String {
    with_interp_redaction(|ctxs| {
        if ctxs.is_empty() {
            return text.to_owned();
        }
        for ctx in ctxs {
            if text == ctx.effective {
                return ctx.raw.clone();
            }
        }
        fallback.to_owned()
    })
}

#[cfg(not(feature = "properties"))]
/// Preserves identifier text unchanged when interpolation redaction is disabled.
///
/// Called from `src/de_error.rs` for `unknown_variant` and `unknown_field` in non-`properties`
/// builds.
pub(crate) fn redact_dynamic_identifier(text: &str, _: &str) -> String {
    text.to_owned()
}

#[cfg(feature = "properties")]
#[cold]
#[inline(never)]
/// Runs one deserialize/validation operation inside a fresh subtree-local interpolation
/// redaction scope.
///
/// Called by root entry points, validation helpers, `with_deserializer_*`, and nested
/// deserialize wrappers so any later errors in that operation can redact previously seen
/// interpolated values.
pub(crate) fn with_interp_redaction_scope<T>(f: impl FnOnce() -> T) -> T {
    let _guard = InterpRedactionScopeGuard::new();
    f()
}

#[cfg(not(feature = "properties"))]
/// Preserves the redaction-scope call sites when interpolation support is compiled out.
///
/// Called from the same deserialization entry points as the feature-enabled version, but it
/// simply executes the operation directly.
pub(crate) fn with_interp_redaction_scope<T>(f: impl FnOnce() -> T) -> T {
    f()
}