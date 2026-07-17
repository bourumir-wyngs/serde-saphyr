use std::collections::{HashSet, VecDeque};
use std::{borrow::Cow, fmt};

use granit_parser::ScalarStyle;
use serde_core::de::{self, Deserializer as _, IntoDeserializer, Visitor};

use super::base64::decode_base64_yaml;
use super::cfg::Cfg;
use super::commented_deser;
use super::error::{Error, MissingFieldLocationGuard, TransformReason};
use super::events::{Ev, Events, ReplayEvents, attach_alias_locations_if_missing, eof_with_loc};
use super::key_nodes::{
    KeyFingerprint, KeyNode, PendingEntry, apply_duplicate_key_policy_to_entries, capture_node,
    capture_simple_tagged_node_as_map_events, is_empty_mapping_key_fingerprint, is_merge_key,
    is_one_entry_nullish_mapping_key_fingerprint, one_entry_map_spans,
    pending_entries_from_live_events, simple_tagged_enum_name,
    validate_no_merge_keys_in_node_events,
};
use super::options::{DuplicateKeyPolicy, MergeKeyPolicy};
#[cfg(any(feature = "garde", feature = "validator"))]
use super::path_map::PathRecorder;
#[cfg(feature = "properties")]
use super::properties::interpolate_compose_style;
use super::properties_redaction::{
    ScalarRedactionCtx, ScalarRedactionGuard, with_interp_redaction_scope,
};
use super::spanned_deser;
use super::tags::SfTag;
use crate::anchor_store::{self, AnchorKind};
use crate::location::Location;
#[cfg(any(feature = "garde", feature = "validator"))]
use crate::location::Locations;
use crate::parse_scalars::{
    leading_zero_decimal, maybe_not_string, parse_int_signed, parse_int_unsigned,
    parse_yaml11_bool, parse_yaml12_float, scalar_is_nullish, scalar_is_nullish_for_option,
    try_parse_float_incl_overflow,
};

struct TupleLenExpected {
    len: usize,
}

impl fmt::Display for TupleLenExpected {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a tuple of size {}", self.len)
    }
}

impl de::Expected for TupleLenExpected {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

struct TupleLenVisitor<V> {
    inner: V,
    len: usize,
}

impl<'de, V> Visitor<'de> for TupleLenVisitor<V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.expecting(formatter)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let mut consumed = 0usize;
        let value = self.inner.visit_seq(CountingSeqAccess {
            inner: &mut seq,
            consumed: &mut consumed,
        })?;

        if seq.next_element::<de::IgnoredAny>()?.is_some() {
            return Err(<A::Error as de::Error>::invalid_length(
                consumed + 1,
                &TupleLenExpected { len: self.len },
            ));
        }

        Ok(value)
    }
}

struct CountingSeqAccess<'a, A> {
    inner: &'a mut A,
    consumed: &'a mut usize,
}

impl<'de, A> de::SeqAccess<'de> for CountingSeqAccess<'_, A>
where
    A: de::SeqAccess<'de>,
{
    type Error = A::Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        let value = self.inner.next_element_seed(seed)?;
        if value.is_some() {
            *self.consumed += 1;
        }
        Ok(value)
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

fn skip_one_node_from_events(ev: &mut dyn Events<'_>) -> Result<(), Error> {
    let mut depth;
    match ev.next()? {
        Some(Ev::Scalar { .. }) => return Ok(()),
        Some(Ev::SeqStart { .. } | Ev::MapStart { .. }) => depth = 1usize,
        Some(Ev::SeqEnd { location } | Ev::MapEnd { location }) => {
            return Err(Error::UnexpectedContainerEndWhileSkippingNode { location });
        }
        Some(Ev::Taken { location }) => {
            return Err(Error::unexpected("consumed event").with_location(location));
        }
        None => return Err(eof_with_loc(ev)),
    }

    while depth != 0 {
        match ev.next()? {
            Some(Ev::SeqStart { .. } | Ev::MapStart { .. }) => depth += 1,
            Some(Ev::SeqEnd { .. } | Ev::MapEnd { .. }) => depth -= 1,
            Some(Ev::Scalar { .. }) => {}
            Some(Ev::Taken { location }) => {
                return Err(Error::unexpected("consumed event").with_location(location));
            }
            None => return Err(eof_with_loc(ev)),
        }
    }

    Ok(())
}

fn drain_remaining_sequence(ev: &mut dyn Events<'_>) -> Result<(), Error> {
    loop {
        match ev.peek()? {
            Some(Ev::SeqEnd { .. }) => {
                let _ = ev.next()?;
                return Ok(());
            }
            Some(_) => skip_one_node_from_events(ev)?,
            None => return Err(eof_with_loc(ev)),
        }
    }
}

/// The streaming Serde deserializer.
///
/// ## Important: this deserializer *borrows* and is only available in a closure
///
/// `YamlDeserializer` borrows from the YAML input processing state, so you generally
/// cannot construct and return it as a standalone value.
///
/// Instead, obtain it through the `with_deserializer_from_*` helpers, which provide a
/// [`crate::Deserializer`] (an alias for `YamlDeserializer`) **inside a closure**.
///
/// This is useful when you want to wrap the deserializer (for example, to collect
/// ignored fields or to add error context) while still deserializing into your target type.
///
/// ## Example
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize)]
/// struct Config {
///     host: String,
///     port: u16,
/// }
///
/// let yaml = "host: localhost\nport: 8080\n";
///
/// let cfg: Config = serde_saphyr::with_deserializer_from_str(yaml,
///     |de: serde_saphyr::Deserializer| {
///         Config::deserialize(de)
/// })?;
///
/// assert_eq!(cfg.port, 8080);
/// # Ok::<(), serde_saphyr::Error>(())
/// ```
///
/// This type is *stateless* with respect to ownership: it borrows the underlying input
/// state (`'e`) and forwards Serde requests into it, translating YAML shapes into Serde calls.
// Where do values come from: From an `Events` stream (typically [`LiveEvents`])
// that yields simplified YAML events.
// Where do values go: Into a Serde `Visitor` provided by the caller's
// `T: Deserialize`, which drives how we walk the event stream and construct `T`.
pub struct YamlDeserializer<'de, 'e> {
    pub(super) ev: &'e mut dyn Events<'de>,
    pub(super) cfg: Cfg,
    /// True when deserializing a map key.
    in_key: bool,
    /// True when Serde entered through `deserialize_struct`.
    ///
    /// Derived struct visitors reject repeated fields themselves, so `LastWins`
    /// needs duplicate fields collapsed before they reach Serde.
    struct_mode: bool,
    /// True when the recorded key node was exactly an empty mapping (`MapStart` followed by `MapEnd`).
    key_empty_map_node: bool,
    /// Comments that the parent mapping associated with this value.
    pub(super) pending_comments: Vec<Cow<'de, str>>,
    /// Same-line comments after the parent container separator (`key:` or `-`).
    pub(super) pending_value_separator_comments: Vec<Cow<'de, str>>,
    /// Comments that appeared above the value node itself.
    pub(super) pending_value_comments: Vec<Cow<'de, str>>,

    #[cfg(any(feature = "garde", feature = "validator"))]
    pub(super) garde: Option<&'e mut PathRecorder>,
}

#[derive(Clone)]
struct ScalarView<'de> {
    raw: Cow<'de, str>,
    effective: Cow<'de, str>,
    tag: SfTag,
    style: ScalarStyle,
    location: Location,
    interpolated: bool,
}

type EffectiveScalar<'de> = (Cow<'de, str>, SfTag, ScalarStyle, Location);

impl ScalarView<'_> {
    fn redaction_ctx(&self) -> Option<ScalarRedactionCtx> {
        self.interpolated.then(|| ScalarRedactionCtx {
            raw: self.raw.as_ref().to_owned(),
            effective: self.effective.as_ref().to_owned(),
        })
    }
}

fn with_scalar_redaction<T>(
    ctx: Option<ScalarRedactionCtx>,
    f: impl FnOnce() -> Result<T, Error>,
) -> Result<T, Error> {
    let _guard = ctx.map(ScalarRedactionGuard::new);
    f()
}

/// Runs one nested deserialize boundary inside its own subtree redaction scope while also
/// seeding that scope with the immediate scalar context when available.
///
/// Called from sequence/map seed boundaries and enum newtype payload accessors so any error
/// raised after child deserialization can still redact interpolated values seen within that
/// subtree.
fn with_subtree_redaction<T>(
    ctx: Option<ScalarRedactionCtx>,
    f: impl FnOnce() -> Result<T, Error>,
) -> Result<T, Error> {
    with_interp_redaction_scope(|| with_scalar_redaction(ctx, f))
}

#[cfg(feature = "properties")]
pub(crate) fn with_root_redaction<'de, 'e, T>(
    mut de: YamlDeserializer<'de, 'e>,
    f: impl FnOnce(YamlDeserializer<'de, 'e>) -> Result<T, Error>,
) -> Result<T, Error> {
    let redaction_ctx = de.peek_scalar_redaction_ctx()?;
    with_subtree_redaction(redaction_ctx, || f(de))
}

#[cfg(not(feature = "properties"))]
pub(crate) fn with_root_redaction<'de, 'e, T>(
    de: YamlDeserializer<'de, 'e>,
    f: impl FnOnce(YamlDeserializer<'de, 'e>) -> Result<T, Error>,
) -> Result<T, Error> {
    f(de)
}

/// Reject an explicit YAML core tag that does not match a typed Serde scalar request.
///
/// Untagged values and application-specific tags retain their existing parsing behavior.
fn validate_core_scalar_tag(
    tag: SfTag,
    expected: SfTag,
    ty: &'static str,
    location: Location,
) -> Result<(), Error> {
    if tag.is_core() && tag != expected {
        return Err(Error::InvalidScalar { ty, location });
    }
    Ok(())
}

struct EnumScalarId<'de> {
    raw: Cow<'de, str>,
    effective: Cow<'de, str>,
    interpolated: bool,
    location: Location,
}

impl<'de> EnumScalarId<'de> {
    fn from_view(view: ScalarView<'de>) -> Self {
        Self {
            raw: view.raw,
            effective: view.effective,
            interpolated: view.interpolated,
            location: view.location,
        }
    }

    fn redaction_ctx(&self) -> Option<ScalarRedactionCtx> {
        self.interpolated.then(|| ScalarRedactionCtx {
            raw: self.raw.as_ref().to_owned(),
            effective: self.effective.as_ref().to_owned(),
        })
    }
}

impl<'de> IntoDeserializer<'de, Error> for EnumScalarId<'de> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl<'de> de::Deserializer<'de> for EnumScalarId<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_identifier(visitor)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let ctx = self.redaction_ctx();
        let location = self.location;
        let effective = self.effective;
        with_scalar_redaction(ctx, move || match effective {
            Cow::Borrowed(value) => visitor.visit_borrowed_str(value),
            Cow::Owned(value) => visitor.visit_string(value),
        })
        .map_err(|err| err.with_location(location))
    }

    serde_core::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
        option unit unit_struct newtype_struct seq tuple tuple_struct map struct enum
        ignored_any
    }
}

impl<'de, 'e> YamlDeserializer<'de, 'e> {
    /// Construct a new streaming deserializer over an `Events` source.
    ///
    /// Arguments:
    /// - `ev`: the event source (e.g., `LiveEvents` or `ReplayEvents`).
    /// - `cfg`: small by-copy configuration affecting parsing policies.
    ///
    /// Returns:
    /// - `Deser` ready to be handed to Serde.
    ///
    /// Called by:
    /// - Top-level entry points and recursively for nested values.
    pub(crate) fn new(ev: &'e mut dyn Events<'de>, cfg: Cfg) -> Self {
        Self {
            ev,
            cfg,
            in_key: false,
            struct_mode: false,
            key_empty_map_node: false,
            pending_comments: Vec::new(),
            pending_value_separator_comments: Vec::new(),
            pending_value_comments: Vec::new(),

            #[cfg(any(feature = "garde", feature = "validator"))]
            garde: None,
        }
    }

    fn quoting_required_for_scalar(&self, view: &ScalarView<'de>) -> Error {
        Error::quoting_required(view.raw.as_ref(), view.interpolated).with_location(view.location)
    }

    fn interpolation_possible(&self, tag: SfTag, style: ScalarStyle) -> bool {
        if self.in_key || tag == SfTag::Binary || style != ScalarStyle::Plain {
            return false;
        }

        #[cfg(not(feature = "properties"))]
        {
            false
        }

        #[cfg(feature = "properties")]
        {
            self.ev.property_map().is_some()
        }
    }

    fn peek_scalar_redaction_ctx(&mut self) -> Result<Option<ScalarRedactionCtx>, Error> {
        let Some((tag, style)) = (match self.ev.peek()? {
            Some(Ev::Scalar { tag, style, .. }) => Some((*tag, *style)),
            _ => None,
        }) else {
            return Ok(None);
        };

        if !self.interpolation_possible(tag, style) {
            return Ok(None);
        }

        Ok(self
            .peek_scalar_view()?
            .and_then(|view| view.redaction_ctx()))
    }

    #[cfg(any(feature = "garde", feature = "validator"))]
    pub(crate) fn new_with_path_recorder(
        ev: &'e mut dyn Events<'de>,
        cfg: Cfg,
        garde: &'e mut PathRecorder,
    ) -> Self {
        Self {
            ev,
            cfg,
            in_key: false,
            struct_mode: false,
            key_empty_map_node: false,
            pending_comments: Vec::new(),
            pending_value_separator_comments: Vec::new(),
            pending_value_comments: Vec::new(),
            garde: Some(garde),
        }
    }

    /// Consume the next scalar event and return `(value, tag, location)`.
    ///
    /// Returns:
    /// - `Ok((String, Option<String>, Location))` on scalar,
    /// - `Err(Error)` otherwise.
    ///
    /// Called by:
    /// - Numeric/bool/char parsers and `take_string_scalar`.
    fn take_scalar_event(&mut self) -> Result<(String, SfTag, Location), Error> {
        let view = self.take_scalar_view()?;
        Ok((view.effective.into_owned(), view.tag, view.location))
    }

    /// Consume the next scalar event and return it without allocating a new `String` (if possible).
    ///
    /// This keeps the scalar text in its existing `Cow` container, which is cheap to clone
    /// and allows primitive parsers (bool/int/float/char) to work directly on `&str`.
    fn take_scalar_cow_event(&mut self) -> Result<(Cow<'de, str>, SfTag, Location), Error> {
        let view = self.take_scalar_view()?;
        Ok((view.effective, view.tag, view.location))
    }

    fn take_scalar_view(&mut self) -> Result<ScalarView<'de>, Error> {
        match self.ev.next()? {
            Some(Ev::Scalar {
                value,
                tag,
                style,
                location,
                ..
            }) => self.scalar_view_from_parts(value, tag, style, location),
            Some(other) => Err(Error::unexpected("string scalar").with_location(other.location())),
            None => Err(eof_with_loc(self.ev)),
        }
    }

    fn take_peeked_scalar_view(&mut self, view: ScalarView<'de>) -> Result<ScalarView<'de>, Error> {
        match self.ev.next()? {
            Some(Ev::Scalar { .. }) => Ok(view),
            Some(other) => Err(Error::unexpected("string scalar").with_location(other.location())),
            None => Err(eof_with_loc(self.ev)),
        }
    }

    /// Read a scalar as `String`, decoding `!!binary` into UTF-8 text if needed.
    ///
    /// Errors if the tag is incompatible with strings or if the binary payload
    /// is not valid UTF-8.
    fn take_string_scalar(&mut self) -> Result<String, Error> {
        let (value, tag, location) = self.take_scalar_event()?;

        // Special-case binary: decode base64 and require valid UTF-8.
        if tag == SfTag::Binary && !self.cfg.ignore_binary_tag_for_string {
            let data = decode_base64_yaml(&value).map_err(|err| err.with_location(location))?;
            let text = String::from_utf8(data).map_err(|_| Error::BinaryNotUtf8 { location })?;
            return Ok(text);
        }

        // For non-binary, ensure the tag allows string deserialization.
        if !(tag.can_parse_into_string()
            || self.cfg.ignore_binary_tag_for_string && tag == SfTag::Binary)
        {
            return Err(Error::TaggedScalarCannotDeserializeIntoString { location });
        }

        Ok(value)
    }

    /// Expect a sequence start and consume it, or error otherwise.
    fn expect_seq_start(&mut self) -> Result<(), Error> {
        match self.ev.next()? {
            Some(Ev::SeqStart { .. }) => Ok(()),
            Some(other) => Err(Error::unexpected("sequence start").with_location(other.location())),
            None => Err(eof_with_loc(self.ev)),
        }
    }

    /// Expect a mapping start and consume it, or error otherwise.
    fn expect_map_start(&mut self) -> Result<(), Error> {
        match self.ev.next()? {
            Some(Ev::MapStart { .. }) => Ok(()),
            Some(other) => Err(Error::unexpected("mapping start").with_location(other.location())),
            None => Err(eof_with_loc(self.ev)),
        }
    }

    fn peek_effective_scalar(&mut self) -> Result<Option<EffectiveScalar<'de>>, Error> {
        let Some(view) = self.peek_scalar_view()? else {
            return Ok(None);
        };
        Ok(Some((view.effective, view.tag, view.style, view.location)))
    }

    fn peek_scalar_view(&mut self) -> Result<Option<ScalarView<'de>>, Error> {
        let (value, tag, style, location) = match self.ev.peek()? {
            Some(Ev::Scalar {
                value,
                tag,
                style,
                location,
                ..
            }) => (value.clone(), *tag, *style, *location),
            _ => return Ok(None),
        };

        Ok(Some(
            self.scalar_view_from_parts(value, tag, style, location)?,
        ))
    }

    fn scalar_view_from_parts(
        &self,
        raw: Cow<'de, str>,
        tag: SfTag,
        style: ScalarStyle,
        location: Location,
    ) -> Result<ScalarView<'de>, Error> {
        let effective = if self.interpolation_possible(tag, style) {
            self.effective_scalar_value(raw.clone(), tag, style, location)?
        } else {
            raw.clone()
        };
        let interpolated = raw.as_ref() != effective.as_ref();
        Ok(ScalarView {
            raw,
            effective,
            tag,
            style,
            location,
            interpolated,
        })
    }

    fn effective_scalar_value(
        &self,
        value: Cow<'de, str>,
        tag: SfTag,
        style: ScalarStyle,
        location: Location,
    ) -> Result<Cow<'de, str>, Error> {
        if self.in_key || tag == SfTag::Binary || style != ScalarStyle::Plain {
            return Ok(value);
        }

        #[cfg(not(feature = "properties"))]
        {
            let _ = location;
            Ok(value)
        }

        #[cfg(feature = "properties")]
        {
            let Some(vars) = self.ev.property_map() else {
                return Ok(value);
            };
            let vars = vars.as_ref();
            let syntax = self.ev.property_syntax();

            match interpolate_compose_style(value, vars, syntax) {
                Ok(value) => Ok(value),
                Err(crate::properties::PropertyError::Unresolved(name)) => {
                    Err(Error::UnresolvedProperty { name, location })
                }
                Err(crate::properties::PropertyError::InvalidName(name)) => {
                    Err(Error::InvalidPropertyName { name, location })
                }
                Err(crate::properties::PropertyError::RequiredButUnset { name, message }) => {
                    Err(Error::PropertyRequiredButUnset {
                        name,
                        message,
                        location,
                    })
                }
                Err(crate::properties::PropertyError::RequiredButEmpty { name, message }) => {
                    Err(Error::PropertyRequiredButEmpty {
                        name,
                        message,
                        location,
                    })
                }
            }
        }
    }

    /// Peek at the next event's anchor id, if any (0 indicates no anchor).
    fn peek_anchor_id(&mut self) -> Result<Option<usize>, Error> {
        match self.ev.peek()? {
            Some(
                Ev::Scalar { anchor, .. }
                | Ev::SeqStart { anchor, .. }
                | Ev::MapStart { anchor, .. },
            ) => {
                if *anchor == 0 {
                    Ok(None)
                } else {
                    Ok(Some(*anchor))
                }
            }
            _ => Ok(None),
        }
    }
}

impl<'de> de::Deserializer<'de> for YamlDeserializer<'de, '_> {
    type Error = Error;

    /// Fallback entry point when the caller's type has no specific expectation.
    ///
    /// When does Serde call this?
    /// - When the caller (Serde) does not know the exact Rust type to deserialize yet and
    ///   wants the format to "do the best it can" from the data. This happens, for example,
    ///   inside some enum deserialization strategies, in erased/typeless positions (e.g. Value-like
    ///   seeds), or when visitor-based APIs defer the concrete type decision.
    /// - Even for structs/enums, Serde may call `deserialize_any` for individual field values
    ///   when the driving logic cannot or does not specify a concrete numeric/bool/char method.
    ///
    /// Can we force Serde to call the typed methods (`deserialize_u8`, `deserialize_bool`, ...)?
    /// - Not from within a format Deserializer. Serde chooses which method to call based on the
    ///   Rust type information it has via the caller’s `Deserialize`/`DeserializeSeed` logic.
    ///   Implementing the typed methods (which we do) ensures Serde will use them whenever it knows
    ///   the target type; otherwise, it falls back to `deserialize_any`.
    ///
    /// Can we learn the target field’s Rust type from here?
    /// - No. Serde does not expose type reflection to Deserializers. The only hint we get is which
    ///   method Serde chose to call. Field names are available in `deserialize_struct`, but not the
    ///   field types.
    ///
    /// Our policy:
    /// - For scalars, we heuristically interpret plain, untagged values as native YAML scalars
    ///   (null-like → bool → int → float) before falling back to string. Quoted scalars and scalars
    ///   with explicit non-string-friendly tags (or !!binary) are treated as strings.
    ///
    /// Flow: We inspect the next event; scalars are parsed with the heuristic above; containers
    /// delegate to `deserialize_seq`/`deserialize_map`.
    fn deserialize_any<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        // Serde's internal buffering for `untagged` or `flatten` uses internal private visitors.
        // We only want to convert tagged nodes into map events for these buffers to preserve enum variants.
        // General untyped visitors (like `serde_json::Value`) expect the tag to be discarded.
        let is_serde_internal_buffer = std::any::type_name::<V>().contains("::private::de::");
        if is_serde_internal_buffer
            && let Some(events) = capture_simple_tagged_node_as_map_events(self.ev)?
        {
            let mut replay = ReplayEvents::new(
                events,
                #[cfg(feature = "properties")]
                self.ev.property_map().cloned(),
                #[cfg(feature = "properties")]
                self.ev.property_syntax(),
            );
            return YamlDeserializer::new(&mut replay, self.cfg).deserialize_map(visitor);
        }

        if let Some((value, tag, style, location)) = self.peek_effective_scalar()? {
            // Tagged nulls map to unit/null regardless of style
            if tag == SfTag::Null {
                let _ = self.take_scalar_event()?; // consume
                return visitor.visit_unit();
            }
            let is_plain = matches!(style, ScalarStyle::Plain);
            // Treat all YAML null-like scalars (null, ~, empty) as null when typeless.
            if scalar_is_nullish(&value, &style) {
                let _ = self.ev.next()?; // consume
                return visitor.visit_unit();
            }
            if !is_plain
                || !tag.can_parse_into_string()
                || tag == SfTag::Binary
                || tag == SfTag::String
                || tag == SfTag::NonSpecific
            {
                // For string-ish scalars, rely on the parser's own zero-copy capability:
                // if the scalar is returned as `Cow::Borrowed`, we can pass it through.
                // Otherwise we fall back to owning.
                if tag == SfTag::Binary && !self.cfg.ignore_binary_tag_for_string {
                    return visitor.visit_string(self.take_string_scalar()?);
                }
                if !(tag.can_parse_into_string()
                    || self.cfg.ignore_binary_tag_for_string && tag == SfTag::Binary)
                {
                    return Err(Error::TaggedScalarCannotDeserializeIntoString { location });
                }

                let (cow, _tag2, _location) = self.take_scalar_cow_event()?;
                return match cow {
                    Cow::Borrowed(b) => visitor.visit_borrowed_str(b),
                    Cow::Owned(s) => visitor.visit_string(s),
                };
            }

            // Consume the scalar and attempt typed parses in order: bool -> int -> float.
            // Parse the effective value, but keep the raw source text for diagnostics.
            let view = self.take_scalar_view()?;
            let raw = view.raw.into_owned();
            let effective = view.effective.into_owned();
            let location = view.location;

            // Try booleans.
            if self.cfg.strict_booleans {
                let tt = effective.trim();
                if tt.eq_ignore_ascii_case("true") {
                    return visitor.visit_bool(true);
                } else if tt.eq_ignore_ascii_case("false") {
                    return visitor.visit_bool(false);
                }
                // otherwise not a bool in strict mode; continue to numbers/float/string
            } else if let Ok(b) = parse_yaml11_bool(&effective) {
                return visitor.visit_bool(b);
            }

            // Try integers: prefer signed if leading '-', else unsigned. Fallbacks use 64-bit.
            let t = effective.trim();
            if t.starts_with('-') && !leading_zero_decimal(t) {
                if let Ok(v) =
                    parse_int_signed::<i64>(t, "i64", location, self.cfg.legacy_octal_numbers)
                {
                    return visitor.visit_i64(v);
                }
            } else {
                if let Ok(v) =
                    parse_int_unsigned::<u64>(t, "u64", location, self.cfg.legacy_octal_numbers)
                {
                    return visitor.visit_u64(v);
                }
                // If unsigned failed, a signed parse might still succeed (e.g., overflow handling)
                if let Ok(v) =
                    parse_int_signed::<i64>(t, "i64", location, self.cfg.legacy_octal_numbers)
                {
                    return visitor.visit_i64(v);
                }
            }

            // Try float per YAML 1.2 forms, treating an f64-overflowing literal (e.g.
            // `1e999`) as non-finite too, so both shapes are handled consistently below.
            if let Some(v) = try_parse_float_incl_overflow(
                &effective,
                location,
                view.tag,
                self.cfg.angle_conversions,
            ) {
                // Typeless consumers such as serde_json::Value cannot represent non-finite
                // floats. By default, reject these scalars. When rejection is disabled,
                // deserialize_any returns a canonical string so these values do not become
                // null or fail later.
                if v.is_finite() {
                    return visitor.visit_f64(v);
                }
                if self.cfg.reject_non_finite_typeless_float {
                    return Err(Error::NonFiniteFloat {
                        value: raw,
                        location,
                    });
                }
                let canon = if v.is_nan() {
                    ".nan".to_string()
                } else if v.is_sign_negative() {
                    "-.inf".to_string()
                } else {
                    ".inf".to_string()
                };
                return visitor.visit_string(canon);
            }

            // Fallback: treat as string as-is.
            return visitor.visit_string(effective);
        }

        match self.ev.peek()? {
            Some(Ev::SeqStart { .. }) => self.deserialize_seq(visitor),
            Some(Ev::MapStart { .. }) => self.deserialize_map(visitor),
            Some(Ev::SeqEnd { location }) => Err(Error::UnexpectedSequenceEnd {
                location: *location,
            }),
            Some(Ev::MapEnd { location }) => Err(Error::UnexpectedMappingEnd {
                location: *location,
            }),
            None => {
                // When deserializing typeless positions (for example
                // `serde_json::Value`) a completely empty document should be
                // treated as YAML null rather than an EOF error. Structured
                // entry points like `deserialize_map` still surface EOF
                // through their dedicated `expect_*` helpers.
                visitor.visit_unit()
            }
            Some(Ev::Taken { location }) => {
                Err(Error::unexpected("consumed event").with_location(*location))
            }
            Some(Ev::Scalar { location, .. }) => {
                Err(Error::unexpected("scalar").with_location(*location))
            }
        }
    }

    /// Parse a YAML 1.1 boolean literal into `bool`.
    ///
    /// Caller: Serde when target expects `bool`.
    /// Flow: scalar text → `Visitor::visit_bool`.
    fn deserialize_bool<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_event()?;
        validate_core_scalar_tag(tag, SfTag::Bool, "boolean", location)?;
        let s = s.as_ref();
        let t = s.trim();
        let b: bool = if self.cfg.strict_booleans {
            if t.eq_ignore_ascii_case("true") {
                true
            } else if t.eq_ignore_ascii_case("false") {
                false
            } else {
                return Err(Error::InvalidBooleanStrict { location });
            }
        } else {
            parse_yaml11_bool(s).map_err(|_| Error::InvalidScalar {
                ty: "boolean",
                location,
            })?
        };
        visitor.visit_bool(b)
    }

    /// Parse a signed 8-bit integer.
    fn deserialize_i8<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_event()?;
        validate_core_scalar_tag(tag, SfTag::Int, "i8", location)?;
        let v: i8 = parse_int_signed(s.as_ref(), "i8", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i8(v)
    }
    /// Parse a signed 16-bit integer.
    fn deserialize_i16<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_event()?;
        validate_core_scalar_tag(tag, SfTag::Int, "i16", location)?;
        let v: i16 = parse_int_signed(s.as_ref(), "i16", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i16(v)
    }
    /// Parse a signed 32-bit integer.
    fn deserialize_i32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_event()?;
        validate_core_scalar_tag(tag, SfTag::Int, "i32", location)?;
        let v: i32 = parse_int_signed(s.as_ref(), "i32", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i32(v)
    }
    /// Parse a signed 64-bit integer.
    fn deserialize_i64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_event()?;
        validate_core_scalar_tag(tag, SfTag::Int, "i64", location)?;
        let v: i64 = parse_int_signed(s.as_ref(), "i64", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i64(v)
    }
    /// Parse a signed 128-bit integer.
    fn deserialize_i128<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_event()?;
        validate_core_scalar_tag(tag, SfTag::Int, "i128", location)?;
        let v: i128 =
            parse_int_signed(s.as_ref(), "i128", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i128(v)
    }

    /// Parse an unsigned 8-bit integer.
    fn deserialize_u8<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_event()?;
        validate_core_scalar_tag(tag, SfTag::Int, "u8", location)?;
        let v: u8 = parse_int_unsigned(s.as_ref(), "u8", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u8(v)
    }
    /// Parse an unsigned 16-bit integer.
    fn deserialize_u16<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_event()?;
        validate_core_scalar_tag(tag, SfTag::Int, "u16", location)?;
        let v: u16 =
            parse_int_unsigned(s.as_ref(), "u16", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u16(v)
    }
    /// Parse an unsigned 32-bit integer.
    fn deserialize_u32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_event()?;
        validate_core_scalar_tag(tag, SfTag::Int, "u32", location)?;
        let v: u32 =
            parse_int_unsigned(s.as_ref(), "u32", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u32(v)
    }
    /// Parse an unsigned 64-bit integer.
    fn deserialize_u64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_event()?;
        validate_core_scalar_tag(tag, SfTag::Int, "u64", location)?;
        let v: u64 =
            parse_int_unsigned(s.as_ref(), "u64", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u64(v)
    }
    /// Parse an unsigned 128-bit integer.
    fn deserialize_u128<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_event()?;
        validate_core_scalar_tag(tag, SfTag::Int, "u128", location)?;
        let v: u128 =
            parse_int_unsigned(s.as_ref(), "u128", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u128(v)
    }

    /// Parse a 32-bit float (supports YAML 1.2 `+.inf`, `-.inf`, `.nan`).
    fn deserialize_f32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_event()?;
        let v: f32 = parse_yaml12_float(s.as_ref(), location, tag, self.cfg.angle_conversions)?;
        visitor.visit_f32(v)
    }
    /// Parse a 64-bit float (supports YAML 1.2 `+.inf`, `-.inf`, `.nan`).
    fn deserialize_f64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_event()?;
        let v: f64 = parse_yaml12_float(s.as_ref(), location, tag, self.cfg.angle_conversions)?;
        visitor.visit_f64(v)
    }

    /// Parse a single Unicode scalar value (`char`).
    ///
    /// Null semantics:
    /// - Tagged null or plain null-like scalars (empty, `~`, or case-insensitive `null`) are not valid `char`.
    ///   Quoted forms are treated as normal strings and validated for length 1.
    /// - In `no_schema` mode, plain scalars that look like non-strings (numbers, bools, etc.)
    ///   must be quoted; this check uses scalar style to avoid flagging quoted scalars.
    fn deserialize_char<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        // Mirror deserialize_string pre-checks to leverage tag/style and maybe_not_string.
        if let Some(view) = self.peek_scalar_view()?
            && view.tag != SfTag::String
        {
            // Reject YAML null for char (allow quoted values like "null").
            if view.tag == SfTag::Null || scalar_is_nullish(&view.effective, &view.style) {
                let (_value, _tag, location) = self.take_scalar_event()?;
                return Err(Error::InvalidCharNull { location });
            } else if self.cfg.no_schema
                && maybe_not_string(&view.effective, &view.style, self.cfg.strict_booleans)
            {
                // Require quoting for ambiguous plain scalars in no_schema mode.
                let view = self.take_scalar_view()?;
                return Err(self.quoting_required_for_scalar(&view));
            }
        }

        // Now consume the scalar and validate it contains exactly one Unicode scalar value.
        let (s, _tag, location) = self.take_scalar_cow_event()?;
        let mut it = s.as_ref().chars();
        match (it.next(), it.next()) {
            (Some(c), None) => visitor.visit_char(c),
            _ => Err(Error::InvalidCharNotSingleScalar { location }),
        }
    }

    /// Deserialize a borrowed string.
    ///
    /// When the scalar exists verbatim in the original input, this method uses
    /// `Visitor::visit_borrowed_str`.
    ///
    /// Borrowing is only possible for in-memory inputs (e.g. `from_str` / `from_slice`) and only
    /// when no transformation is required (no escape processing, folding, chomping/indent handling,
    /// or multi-line normalization).
    ///
    /// If borrowing is not possible, this method falls back to `Visitor::visit_string`.
    /// When the target type requires `&str`, that fallback produces a helpful error suggesting
    /// `String` or `Cow<str>`, with a [`TransformReason`] describing why borrowing was impossible.
    fn deserialize_str<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let view = match self.peek_scalar_view()? {
            Some(view) => {
                // Check for null - not valid for string deserialization. A deliberately
                // substituted ${...} that resolved to "" stays a string, not null.
                if (view.tag == SfTag::Null
                    || (!view.interpolated && scalar_is_nullish(&view.effective, &view.style)))
                    && view.tag != SfTag::String
                {
                    let loc = view.location;
                    let _ = self.ev.next()?;
                    return Err(Error::NullIntoString { location: loc });
                } else if self.cfg.no_schema
                    && maybe_not_string(&view.effective, &view.style, self.cfg.strict_booleans)
                    && view.tag != SfTag::String
                {
                    let view = self.take_scalar_view()?;
                    return Err(self.quoting_required_for_scalar(&view));
                }

                if view.tag == SfTag::Binary && !self.cfg.ignore_binary_tag_for_string {
                    let res: Result<V::Value, Self::Error> =
                        visitor.visit_string(self.take_string_scalar()?);
                    return match res {
                        Ok(v) => Ok(v),
                        Err(err) if err.to_string().contains("expected a borrowed string") => Err(
                            Error::cannot_borrow_transformed(TransformReason::ParserReturnedOwned)
                                .with_location(view.location),
                        ),
                        Err(err) => Err(err),
                    };
                }
                if !(view.tag.can_parse_into_string()
                    || self.cfg.ignore_binary_tag_for_string && view.tag == SfTag::Binary)
                {
                    return Err(Error::TaggedScalarCannotDeserializeIntoString {
                        location: view.location,
                    });
                }

                view
            }
            None => return Err(eof_with_loc(self.ev)),
        };

        let location = view.location;
        let redaction_ctx = view.redaction_ctx();
        let cannot_borrow_reason = if view.interpolated {
            TransformReason::VariableInterpolation
        } else if self.ev.input_for_borrowing().is_none() {
            TransformReason::InputNotBorrowable
        } else {
            TransformReason::ParserReturnedOwned
        };

        let view = self.take_peeked_scalar_view(view)?;
        if let Cow::Borrowed(b) = view.effective {
            return visitor.visit_borrowed_str(b);
        }

        let res: Result<V::Value, Self::Error> = with_scalar_redaction(redaction_ctx, || {
            visitor.visit_string(view.effective.into_owned())
        });
        match res {
            Ok(v) => Ok(v),
            Err(err) => {
                let msg = err.to_string();
                if msg.contains("expected a borrowed string") {
                    return Err(Error::cannot_borrow_transformed(cannot_borrow_reason)
                        .with_location(location));
                }
                Err(err)
            }
        }
    }

    /// Deserialize an owned string (with `!!binary` UTF-8 support).
    ///
    /// Null semantics:
    /// - Tagged null or plain null-like scalars (empty, `~`, or case-insensitive `null`) are not valid `String`.
    ///   Suggest using `Option<String>` for such YAML values.
    /// - Quoted "null" and quoted empty strings are treated as normal strings and allowed.
    ///
    /// **From/To:** scalar text (or base64-decoded bytes) → `Visitor::visit_string`.
    fn deserialize_string<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        // Reject YAML null when deserializing into String. Allow quoted forms.
        let view = if let Some(view) = self.peek_scalar_view()? {
            // If explicitly tagged as null, or plain null-like, this is not a valid String.
            // A ${...} that the user deliberately substituted to "" is a legitimate string.
            if (view.tag == SfTag::Null
                || (!view.interpolated && scalar_is_nullish(&view.effective, &view.style)))
                && view.tag != SfTag::String
            {
                // Consume the scalar to anchor the error at the correct location.
                let (_value, _tag, location) = self.take_scalar_event()?;
                return Err(Error::NullIntoString { location });
            } else if self.cfg.no_schema
                && maybe_not_string(&view.effective, &view.style, self.cfg.strict_booleans)
                && view.tag != SfTag::String
            {
                // Consume the scalar to anchor the error at the correct location.
                let view = self.take_scalar_view()?;
                return Err(self.quoting_required_for_scalar(&view));
            }
            if view.tag == SfTag::Binary && !self.cfg.ignore_binary_tag_for_string {
                return visitor.visit_string(self.take_string_scalar()?);
            }
            if !(view.tag.can_parse_into_string()
                || self.cfg.ignore_binary_tag_for_string && view.tag == SfTag::Binary)
            {
                return Err(Error::TaggedScalarCannotDeserializeIntoString {
                    location: view.location,
                });
            }

            view
        } else {
            // Let take_string_scalar handle the error if it's not a scalar
            return visitor.visit_string(self.take_string_scalar()?);
        };

        let redaction_ctx = view.redaction_ctx();
        let view = self.take_peeked_scalar_view(view)?;
        match view.effective {
            Cow::Borrowed(b) => {
                with_scalar_redaction(redaction_ctx, || visitor.visit_borrowed_str(b))
            }
            Cow::Owned(s) => with_scalar_redaction(redaction_ctx, || visitor.visit_string(s)),
        }
    }

    /// Deserialize bytes either from `!!binary` or from a sequence of integers (0..=255).
    ///
    /// **From/To:**
    /// - Tagged scalar → base64-decoded `Vec<u8>` into `Visitor::visit_byte_buf`.
    /// - Sequence of integers → packed into `Vec<u8>` and visited.
    fn deserialize_bytes<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        if let Some((_value, tag, _style, location)) = self.peek_effective_scalar()? {
            if tag == SfTag::Binary {
                let (value, data_location) = match self.ev.next()? {
                    Some(Ev::Scalar {
                        value, location, ..
                    }) => (value, location),
                    Some(other) => {
                        return Err(
                            Error::unexpected("binary scalar").with_location(other.location())
                        );
                    }
                    None => return Err(eof_with_loc(self.ev)),
                };
                let data =
                    decode_base64_yaml(&value).map_err(|err| err.with_location(data_location))?;
                return visitor.visit_byte_buf(data);
            }
            return Err(Error::BytesNotSupportedMissingBinaryTag { location });
        }

        match self.ev.peek()? {
            // Untagged → expect a sequence of YAML integers (0..=255) and pack into bytes
            Some(Ev::SeqStart { .. }) => {
                self.expect_seq_start()?;
                let mut out = Vec::new();
                loop {
                    match self.ev.peek()? {
                        Some(Ev::SeqEnd { .. }) => {
                            let _ = self.ev.next()?; // consume end
                            break;
                        }
                        Some(_) => {
                            // Deserialize each element as u8 using our own Deser
                            let b: u8 = <u8 as serde_core::Deserialize>::deserialize(
                                YamlDeserializer::new(&mut *self.ev, self.cfg),
                            )?;
                            out.push(b);
                        }
                        None => return Err(eof_with_loc(self.ev)),
                    }
                }
                visitor.visit_byte_buf(out)
            }

            // Anything else is unexpected here
            Some(other) => Err(
                Error::unexpected("scalar (!!binary) or sequence of 0..=255")
                    .with_location(other.location()),
            ),
            None => Err(eof_with_loc(self.ev)),
        }
    }

    /// Deserialize owned bytes; same semantics as `deserialize_bytes`.
    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_bytes(visitor)
    }

    /// Deserialize an `Option<T>`.
    ///
    /// **What is treated as `None`?** End-of-input, container end, or a scalar
    /// that is empty-unquoted / `~` / `null` in plain style.
    fn deserialize_option<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        // Only when Serde asks for Option<T> do we interpret YAML null-like scalars as None.
        // Special-case for map keys: treat an explicit empty key captured as an empty mapping node
        // as None when the target is Option<T>. This is scoped strictly to key position to avoid
        // conflating a literal empty mapping `{}` with null for non-Option targets.
        if self.in_key && self.key_empty_map_node {
            // Recorded key is an empty mapping: treat as None for Option<T> in key position
            match self.ev.next()? {
                Some(Ev::MapStart { .. }) => {}
                Some(other) => {
                    return Err(
                        Error::unexpected("empty mapping start").with_location(other.location())
                    );
                }
                None => return Err(eof_with_loc(self.ev)),
            }
            match self.ev.next()? {
                Some(Ev::MapEnd { .. }) => {}
                Some(other) => {
                    return Err(
                        Error::unexpected("empty mapping end").with_location(other.location())
                    );
                }
                None => return Err(eof_with_loc(self.ev)),
            }
            return visitor.visit_none();
        }

        if let Some((value, tag, style, _location)) = self.peek_effective_scalar()?
            && (tag == SfTag::Null || scalar_is_nullish_for_option(&value, &style))
        {
            let _ = self.ev.next()?; // consume the scalar
            return visitor.visit_none();
        }

        match self.ev.peek()? {
            // End of input → None
            None => visitor.visit_none(),

            // In flow/edge cases a missing value can manifest as an immediate container end → None
            Some(Ev::MapEnd { .. } | Ev::SeqEnd { .. }) => visitor.visit_none(),

            // Otherwise there is a value → Some(...)
            Some(_) => visitor.visit_some(self),
        }
    }

    /// Deserialize the unit type `()`.
    ///
    /// **What is “unit” here?** Rust's `()` indicates “no value”. In Serde it
    /// commonly appears in unit structs/variants or fields intentionally
    /// ignored.  
    /// **Accepted YAML forms:** end-of-input, container end, or a null-like
    /// scalar in plain style (`""`, `~`, `null`).
    fn deserialize_unit<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        if let Some((value, _tag, style, _location)) = self.peek_effective_scalar()?
            && scalar_is_nullish(&value, &style)
        {
            let _ = self.ev.next()?; // consume the scalar
            return visitor.visit_unit();
        }

        match self.ev.peek()? {
            // Accept absence as unit
            None => visitor.visit_unit(),
            // End of a container where a value was expected: treat as unit in this subset
            Some(Ev::MapEnd { .. } | Ev::SeqEnd { .. }) => visitor.visit_unit(),
            // Anything else isn't a unit value
            Some(other) => Err(Error::UnexpectedValueForUnit {
                location: other.location(),
            }),
        }
    }

    /// Deserialize a unit struct.
    ///
    /// **Delegation:** Struct unit forms are handled by allowing an **empty mapping**
    /// (`{}`) as the YAML representation, or by deferring to the same null-like
    /// forms accepted by `deserialize_unit`.  
    /// `Visitor` origin: Serde generates a visitor when
    /// deserializing the target unit struct type (via `derive(Deserialize)` or a
    /// manual impl). That visitor expects us to call `Visitor::visit_unit`.
    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.ev.peek()? {
            // Allow empty mapping `{}` as a unit struct
            Some(Ev::MapStart { .. }) => {
                let _ = self.ev.next()?; // consume MapStart
                match self.ev.peek()? {
                    Some(Ev::MapEnd { .. }) => {
                        let _ = self.ev.next()?; // consume MapEnd
                        visitor.visit_unit()
                    }
                    Some(other) => Err(Error::ExpectedEmptyMappingForUnitStruct {
                        location: other.location(),
                    }),
                    None => Err(eof_with_loc(self.ev)),
                }
            }
            // Otherwise, delegate to unit handling (null, ~, empty scalar, EOF, etc.)
            _ => self.deserialize_unit(visitor),
        }
    }

    /// Deserialize a newtype struct (`struct Wrapper(T);`) by delegating to its inner value.
    ///
    /// Why is this needed: Serde distinguishes *newtype structs* from their
    /// inner `T` so that attributes (like `#[serde(transparent)]`) and coherence
    /// rules are preserved. Even though YAML has no distinct “newtype” shape,
    /// Serde will invoke this method when the target is a newtype struct.  
    /// What do we do: Hand our own deserializer (`self`) to
    /// `Visitor::visit_newtype_struct`, which in turn will deserialize `T`
    /// using the same YAML event stream.
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        mut self,
        n: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match n {
            // Internal wrapper types use `__yaml_*` names (see `__yaml_rc_anchor`, etc.).
            "__yaml_spanned" => spanned_deser::deserialize_yaml_spanned(self, visitor),
            "__yaml_commented" => commented_deser::deserialize_yaml_commented(self, visitor),
            "__yaml_rc_anchor" => {
                let anchor = self.peek_anchor_id()?;
                anchor_store::with_anchor_context(AnchorKind::Rc, anchor, || {
                    visitor.visit_newtype_struct(self)
                })
            }
            "__yaml_arc_anchor" => {
                let anchor = self.peek_anchor_id()?;
                anchor_store::with_anchor_context(AnchorKind::Arc, anchor, || {
                    visitor.visit_newtype_struct(self)
                })
            }
            "__yaml_rc_recursive" => {
                let anchor = self.peek_anchor_id()?;
                anchor_store::with_anchor_context(AnchorKind::RcRecursive, anchor, || {
                    visitor.visit_newtype_struct(self)
                })
            }
            "__yaml_arc_recursive" => {
                let anchor = self.peek_anchor_id()?;
                anchor_store::with_anchor_context(AnchorKind::ArcRecursive, anchor, || {
                    visitor.visit_newtype_struct(self)
                })
            }
            "__yaml_rc_weak_anchor" => {
                let anchor = self.peek_anchor_id()?;
                anchor_store::with_anchor_context(AnchorKind::Rc, anchor, || {
                    visitor.visit_newtype_struct(self)
                })
            }
            "__yaml_arc_weak_anchor" => {
                let anchor = self.peek_anchor_id()?;
                anchor_store::with_anchor_context(AnchorKind::Arc, anchor, || {
                    visitor.visit_newtype_struct(self)
                })
            }
            "__yaml_rc_recursion" => {
                let anchor = self.peek_anchor_id()?;
                anchor_store::with_anchor_context(AnchorKind::RcRecursive, anchor, || {
                    visitor.visit_newtype_struct(self)
                })
            }
            "__yaml_arc_recursion" => {
                let anchor = self.peek_anchor_id()?;
                anchor_store::with_anchor_context(AnchorKind::ArcRecursive, anchor, || {
                    visitor.visit_newtype_struct(self)
                })
            }
            _ => visitor.visit_newtype_struct(self),
        }
    }

    /// Deserialize a YAML sequence into a Serde sequence.
    ///
    /// Flow: We provide a `SeqAccess` that repeatedly feeds nested
    /// `Deser` instances back into Serde for each element. Also supports a
    /// `!!binary` scalar as a byte *sequence* view when the caller expects a
    /// sequence of u8.
    fn deserialize_seq<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        if let Some((s, tag, style, _location)) = self.peek_effective_scalar()? {
            // Treat null-like scalar as an empty sequence.
            if tag == SfTag::Null || scalar_is_nullish(&s, &style) {
                let _ = self.ev.next()?; // consume the null-like scalar
                struct EmptySeq;
                impl<'de> de::SeqAccess<'de> for EmptySeq {
                    type Error = Error;
                    fn next_element_seed<T>(&mut self, _seed: T) -> Result<Option<T::Value>, Error>
                    where
                        T: de::DeserializeSeed<'de>,
                    {
                        Ok(None)
                    }
                }
                return visitor.visit_seq(EmptySeq);
            }
            if tag == SfTag::Binary {
                let (scalar, data_location) = match self.ev.next()? {
                    Some(Ev::Scalar {
                        value, location, ..
                    }) => (value, location),
                    Some(other) => {
                        return Err(
                            Error::unexpected("binary scalar").with_location(other.location())
                        );
                    }
                    None => return Err(eof_with_loc(self.ev)),
                };
                let data =
                    decode_base64_yaml(&scalar).map_err(|err| err.with_location(data_location))?;
                /// `SeqAccess` that iterates over bytes from a decoded `!!binary`.
                struct ByteSeq {
                    data: Vec<u8>,
                    idx: usize,
                }
                impl<'de> de::SeqAccess<'de> for ByteSeq {
                    type Error = Error;
                    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Error>
                    where
                        T: de::DeserializeSeed<'de>,
                    {
                        if self.idx >= self.data.len() {
                            return Ok(None);
                        }
                        let b = self.data[self.idx];
                        self.idx += 1;
                        let deser = serde_core::de::value::U8Deserializer::<Error>::new(b);
                        seed.deserialize(deser).map(Some)
                    }
                }
                return visitor.visit_seq(ByteSeq { data, idx: 0 });
            }
        }
        // Comments passed in from the parent value slot belong to the first item
        // of the sequence. If this sequence is reached through a nested alias,
        // alias replay exposes the anchored sequence start here, so the same
        // rule applies to comments written above the alias token.
        let mut seq_start_comments = std::mem::take(&mut self.pending_value_comments);
        seq_start_comments.extend(self.ev.take_leading_comments_for_next_node()?);
        let seq_location = self
            .ev
            .peek()?
            .map(|ev| ev.location())
            .unwrap_or_else(|| self.ev.last_location());
        let child_cfg = self.cfg.enter_container(seq_location)?;
        self.expect_seq_start()?;
        /// Streaming `SeqAccess` over the underlying `Events`.
        struct SA<'de, 'e> {
            ev: &'e mut dyn Events<'de>,
            cfg: Cfg,
            pending_first_element_comments: Vec<Cow<'de, str>>,

            #[cfg(any(feature = "garde", feature = "validator"))]
            garde: Option<&'e mut PathRecorder>,
            #[cfg(any(feature = "garde", feature = "validator"))]
            idx: usize,
        }
        impl<'de> de::SeqAccess<'de> for SA<'de, '_> {
            type Error = Error;
            /// Produce the next element by recursively deserializing from the same event source.
            fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Error>
            where
                T: de::DeserializeSeed<'de>,
            {
                let (is_end, defined_location) = {
                    let peeked = self.ev.peek()?;
                    match peeked {
                        Some(Ev::SeqEnd { .. }) => (true, Location::UNKNOWN),
                        Some(ev) => (false, ev.location()),
                        None => return Err(eof_with_loc(self.ev)),
                    }
                };

                if is_end {
                    return Ok(None);
                }

                // The peek borrow is now released, so it's safe to query other cursor state.
                let reference_location = self.ev.reference_location();
                let _missing_field_guard = MissingFieldLocationGuard::new(reference_location);
                let mut item_comments = std::mem::take(&mut self.pending_first_element_comments);
                item_comments.extend(self.ev.take_leading_comments_for_next_node()?);
                let value_separator_comments = self
                    .ev
                    .take_separator_comments_before_sequence_item_value()?;

                #[cfg(any(feature = "garde", feature = "validator"))]
                {
                    if let Some(garde_ref) = self.garde.as_mut() {
                        let recorder: &mut PathRecorder = garde_ref;

                        let prev = recorder.current.take();
                        let now = prev.clone().join(self.idx);
                        recorder.current = now.clone();
                        recorder.map.insert(
                            now,
                            Locations {
                                reference_location,
                                defined_location,
                            },
                        );

                        let mut de =
                            YamlDeserializer::new_with_path_recorder(self.ev, self.cfg, recorder);
                        de.pending_comments = item_comments.clone();
                        de.pending_value_separator_comments = value_separator_comments.clone();
                        let redaction_ctx = de.peek_scalar_redaction_ctx()?;
                        let res = with_subtree_redaction(redaction_ctx, || seed.deserialize(de))
                            .map(Some)
                            .map_err(|e| {
                                attach_alias_locations_if_missing(
                                    e,
                                    reference_location,
                                    defined_location,
                                )
                            });

                        recorder.current = prev;
                        self.idx += 1;
                        return res;
                    }
                }

                let mut de = YamlDeserializer::new(self.ev, self.cfg);
                de.pending_comments = item_comments;
                de.pending_value_separator_comments = value_separator_comments;
                let redaction_ctx = de.peek_scalar_redaction_ctx()?;
                with_subtree_redaction(redaction_ctx, || seed.deserialize(de))
                    .map(Some)
                    .map_err(|e| {
                        attach_alias_locations_if_missing(e, reference_location, defined_location)
                    })
            }
        }

        #[cfg(any(feature = "garde", feature = "validator"))]
        let garde = self.garde;

        let result = visitor.visit_seq(SA {
            ev: self.ev,
            cfg: child_cfg,
            pending_first_element_comments: seq_start_comments,

            #[cfg(any(feature = "garde", feature = "validator"))]
            garde,
            #[cfg(any(feature = "garde", feature = "validator"))]
            idx: 0,
        })?;
        drain_remaining_sequence(self.ev)?;
        Ok(result)
    }

    /// Deserialize a tuple; identical mechanics to sequences (fixed length checked by caller).
    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(TupleLenVisitor {
            inner: visitor,
            len,
        })
    }

    /// Deserialize a tuple struct; identical mechanics to sequences.
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(TupleLenVisitor {
            inner: visitor,
            len,
        })
    }

    /// Deserialize a YAML mapping into a Serde map/struct field stream.
    ///
    /// Flow: We expose a `MapAccess` implementation (`MA`) that:
    /// - Captures key/value nodes (able to replay them),
    /// - Applies duplicate-key policy,
    /// - Expands YAML merge keys (`<<`) in the correct precedence order.
    ///
    /// Caller: Serde field visitors for maps and for Rust structs
    /// (which Serde also requests via `deserialize_map`).
    fn deserialize_map<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        // Treat null-like scalar as an empty map/struct.
        if let Some((s, tag, style, location)) = self.peek_effective_scalar()?
            && (tag == SfTag::Null || scalar_is_nullish(&s, &style))
        {
            let _ = self.ev.next()?; // consume the null-like scalar
            struct EmptyMap {
                location: Location,
            }
            impl<'de> de::MapAccess<'de> for EmptyMap {
                type Error = Error;
                fn next_key_seed<K>(&mut self, _seed: K) -> Result<Option<K::Value>, Error>
                where
                    K: de::DeserializeSeed<'de>,
                {
                    Ok(None)
                }
                fn next_value_seed<Vv>(&mut self, _seed: Vv) -> Result<Vv::Value, Error>
                where
                    Vv: de::DeserializeSeed<'de>,
                {
                    Err(Error::ValueRequestedBeforeKey {
                        location: self.location,
                    })
                }
            }
            return visitor.visit_map(EmptyMap { location });
        }
        // Same-line separator comments on the parent field belong to this mapping node.
        // `Commented<Container>` consumes those comments before this point; a plain map
        // must not reattach them to the first child key.
        let _map_node_comments = std::mem::take(&mut self.pending_value_separator_comments);
        // Comments already inside the map, before the first key, remain first-key
        // comments. `Commented<Container>` defers these instead of capturing them.
        // If this map is reached through a nested alias, alias replay exposes the
        // anchored map start here; comments above the alias follow the same rule.
        let mut map_start_comments = std::mem::take(&mut self.pending_value_comments);
        map_start_comments.extend(self.ev.take_leading_comments_for_next_node()?);
        let map_location = self
            .ev
            .peek()?
            .map(|ev| ev.location())
            .unwrap_or_else(|| self.ev.last_location());
        let child_cfg = self.cfg.enter_container(map_location)?;
        self.expect_map_start()?;

        // Ensure "missing field" errors (which have no natural span) get attributed to the
        // current container.
        let _missing_field_guard = MissingFieldLocationGuard::new(self.ev.reference_location());

        #[cfg(any(feature = "garde", feature = "validator"))]
        if let Some(recorder) = self.garde.as_mut() {
            // Record the container itself, not just its leaf scalars, so that missing-field
            // errors can fall back to a parent structure.
            let path = recorder.current.clone();
            recorder.map.insert(
                path,
                Locations {
                    reference_location: self.ev.reference_location(),
                    defined_location: self.ev.last_location(),
                },
            );
        }

        fn collect_struct_last_wins_entries<'de>(
            ev: &mut dyn Events<'de>,
            mut first_key_comments: Vec<Cow<'de, str>>,
            duplicate_keys: DuplicateKeyPolicy,
            merge_keys: MergeKeyPolicy,
        ) -> Result<VecDeque<PendingEntry<'de>>, Error> {
            let mut explicit_entries = Vec::new();
            let mut merge_batches = Vec::new();

            loop {
                match ev.peek()? {
                    Some(Ev::MapEnd { .. }) => {
                        let _ = ev.next()?;
                        break;
                    }
                    Some(_) => {
                        let mut key_comments = std::mem::take(&mut first_key_comments);
                        key_comments.extend(ev.take_leading_comments_for_next_node()?);
                        let key = capture_node(ev)?;

                        if is_merge_key(&key) {
                            match merge_keys {
                                MergeKeyPolicy::Merge => {
                                    let _ = ev.peek()?;
                                    let merge_ref_loc = ev.reference_location();
                                    let entries = pending_entries_from_live_events(
                                        ev,
                                        merge_ref_loc,
                                        merge_keys,
                                        duplicate_keys,
                                    )?;
                                    if !entries.is_empty() {
                                        merge_batches.push(entries);
                                    }
                                    continue;
                                }
                                MergeKeyPolicy::AsOrdinary => {}
                                MergeKeyPolicy::Error => {
                                    return Err(Error::MergeKeyNotAllowed {
                                        location: key.location(),
                                    });
                                }
                            }
                        }

                        let field_comments = key_comments;
                        let value_separator_comments =
                            ev.take_separator_comments_before_mapping_value()?;
                        let value_comments = ev.take_leading_comments_for_next_node()?;
                        let reference_location = ev.reference_location();
                        let value = capture_node(ev)?;
                        explicit_entries.push(PendingEntry {
                            key,
                            value,
                            reference_location,
                            field_comments,
                            value_separator_comments,
                            value_comments,
                        });
                    }
                    None => return Err(eof_with_loc(ev)),
                }
            }

            let mut explicit_entries = apply_duplicate_key_policy_to_entries(
                explicit_entries,
                duplicate_keys,
                merge_keys,
            )?;
            let mut seen = HashSet::with_capacity(explicit_entries.len());
            for entry in &explicit_entries {
                seen.insert(entry.key.fingerprint().into_owned());
            }

            let mut merge_entries = Vec::new();
            for batch in merge_batches {
                for entry in batch {
                    let fingerprint = entry.key.fingerprint().into_owned();
                    if seen.insert(fingerprint) {
                        merge_entries.push(entry);
                    }
                }
            }

            explicit_entries.extend(merge_entries);
            Ok(explicit_entries.into_iter().collect())
        }

        /// Streaming `MapAccess` over the underlying `Events`.
        struct MA<'de, 'e> {
            ev: &'e mut dyn Events<'de>,
            cfg: Cfg,
            have_key: bool,

            // Persist a best-effort “current location” across `next_key_seed` returning to Serde.
            // This allows Serde-produced structural/type errors (e.g. `unknown_field`) to carry
            // a useful span even though they are raised outside of this deserializer’s call stack.
            fallback_guard: Option<MissingFieldLocationGuard>,

            #[cfg(any(feature = "garde", feature = "validator"))]
            garde: Option<&'e mut PathRecorder>,
            #[cfg(any(feature = "garde", feature = "validator"))]
            pending_path_segment: Option<String>,

            // For duplicate-key detection for arbitrary keys.
            seen: HashSet<KeyFingerprint<'de>>,
            pending: VecDeque<PendingEntry<'de>>,
            merge_stack: VecDeque<Vec<PendingEntry<'de>>>,
            flushing_merges: bool,
            live_done: bool,
            pending_value: Option<(Vec<Ev<'de>>, Location)>,
            pending_field_comments: Vec<Cow<'de, str>>,
            pending_value_separator_comments: Vec<Cow<'de, str>>,
            pending_value_comments: Vec<Cow<'de, str>>,
            pending_first_key_comments: Vec<Cow<'de, str>>,
        }

        impl<'de> MA<'de, '_> {
            /// Skip exactly one YAML node (scalar/sequence/mapping) in the live stream.
            ///
            /// Used by:
            /// - `DuplicateKeyPolicy::FirstWins` to discard a later value.
            fn skip_one_node(&mut self) -> Result<(), Error> {
                if self.cfg.merge_keys == MergeKeyPolicy::Error {
                    let node = capture_node(self.ev)?;
                    return validate_no_merge_keys_in_node_events(node.events());
                }

                let mut depth; // assigned later
                match self.ev.next()? {
                    Some(Ev::Scalar { .. }) => return Ok(()),
                    Some(Ev::SeqStart { .. } | Ev::MapStart { .. }) => depth = 1,
                    Some(Ev::SeqEnd { location } | Ev::MapEnd { location }) => {
                        return Err(Error::UnexpectedContainerEndWhileSkippingNode { location });
                    }
                    Some(Ev::Taken { location }) => {
                        return Err(Error::unexpected("consumed event").with_location(location));
                    }
                    None => return Err(eof_with_loc(self.ev)),
                }
                while depth != 0 {
                    match self.ev.next()? {
                        Some(Ev::SeqStart { .. } | Ev::MapStart { .. }) => depth += 1,
                        Some(Ev::SeqEnd { .. } | Ev::MapEnd { .. }) => depth -= 1,
                        Some(Ev::Scalar { .. }) => {}
                        Some(Ev::Taken { location }) => {
                            return Err(Error::unexpected("consumed event").with_location(location));
                        }
                        None => return Err(eof_with_loc(self.ev)),
                    }
                }
                Ok(())
            }

            fn validate_skipped_node(&self, node: &KeyNode<'_>) -> Result<(), Error> {
                if self.cfg.merge_keys == MergeKeyPolicy::Error {
                    validate_no_merge_keys_in_node_events(node.events())?;
                }
                Ok(())
            }

            /// Deserialize a recorded key using a temporary `ReplayEvents`.
            ///
            /// Arguments:
            /// - `seed`: Serde seed for the key type.
            /// - `events`: recorded node events for the key.
            fn deserialize_recorded_key<'de2, K>(
                &mut self,
                seed: K,
                events: Vec<Ev<'de2>>,
                kemn: bool,
            ) -> Result<K::Value, Error>
            where
                K: de::DeserializeSeed<'de2>,
            {
                let mut replay = ReplayEvents::new(
                    events,
                    #[cfg(feature = "properties")]
                    self.ev.property_map().cloned(),
                    #[cfg(feature = "properties")]
                    self.ev.property_syntax(),
                );

                // Get location from replay events for error reporting.
                let location = replay.reference_location();

                let de = YamlDeserializer::<'de2, '_> {
                    ev: &mut replay,
                    cfg: self.cfg,
                    in_key: true,
                    struct_mode: false,
                    key_empty_map_node: kemn,
                    pending_comments: Vec::new(),
                    pending_value_separator_comments: Vec::new(),
                    pending_value_comments: Vec::new(),

                    #[cfg(any(feature = "garde", feature = "validator"))]
                    garde: None,
                };
                seed.deserialize(de).map_err(|e| {
                    if e.location().is_none() {
                        e.with_location(location)
                    } else {
                        e
                    }
                })
            }

            /// Push a batch of entries to the front of the pending queue in order.
            fn enqueue_entries(&mut self, entries: Vec<PendingEntry<'de>>) {
                self.pending.reserve(entries.len());
                for entry in entries.into_iter().rev() {
                    self.pending.push_front(entry);
                }
            }

            /// Pop the next merge batch and enqueue its entries; return whether anything was queued.
            fn enqueue_next_merge_batch(&mut self) -> bool {
                while let Some(entries) = self.merge_stack.pop_front() {
                    if entries.is_empty() {
                        continue;
                    }
                    self.enqueue_entries(entries);
                    return true;
                }
                false
            }
        }

        impl<'de> de::MapAccess<'de> for MA<'de, '_> {
            type Error = Error;

            /// Produce the next key for the visitor, honoring duplicate policy and merges.
            fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
            where
                K: de::DeserializeSeed<'de>,
            {
                let mut seed = Some(seed);

                loop {
                    if let Some(entry) = self.pending.pop_front() {
                        let PendingEntry {
                            mut key,
                            mut value,
                            reference_location,
                            field_comments,
                            value_separator_comments,
                            value_comments,
                        } = entry;
                        let fingerprint = key.take_fingerprint();
                        let location = key.location();
                        let mut events = key.take_events();

                        let is_duplicate = self.seen.contains(&fingerprint);
                        if self.flushing_merges {
                            if is_duplicate {
                                self.validate_skipped_node(&value)?;
                                continue;
                            }
                        } else {
                            match self.cfg.dup_policy {
                                DuplicateKeyPolicy::Error => {
                                    if is_duplicate {
                                        let key = fingerprint
                                            .stringy_scalar_value()
                                            .map(|s| s.to_owned());
                                        return Err(Error::DuplicateMappingKey { key, location });
                                    }
                                }
                                DuplicateKeyPolicy::FirstWins => {
                                    if is_duplicate {
                                        self.validate_skipped_node(&value)?;
                                        continue;
                                    }
                                }
                                DuplicateKeyPolicy::LastWins => {}
                            }
                        }

                        let mut value_events = value.take_events();
                        // Special-case: explicit empty key captured as a one-entry mapping { null: V }
                        // In this case, we want key=None and the outer value to be V.
                        let mut kemn = is_empty_mapping_key_fingerprint(&fingerprint);
                        if !kemn
                            && is_one_entry_nullish_mapping_key_fingerprint(&fingerprint)
                            && let Some((_ks, _ke, vs, ve)) = one_entry_map_spans(&events)
                        {
                            // Zero-copy probe over recorded events to extract inner key/value spans.
                            value_events = events.drain(vs..ve).collect();
                            // Build empty map events using the first and last from original events.
                            let start = match events.first() {
                                Some(Ev::MapStart { anchor, location }) => Ev::MapStart {
                                    anchor: *anchor,
                                    location: *location,
                                },
                                Some(other) => other.clone(),
                                None => {
                                    return Err(
                                        Error::unexpected("mapping start").with_location(location)
                                    );
                                }
                            };
                            let end = match events.last() {
                                Some(Ev::MapEnd { location }) => Ev::MapEnd {
                                    location: *location,
                                },
                                Some(other) => other.clone(),
                                None => {
                                    return Err(
                                        Error::unexpected("mapping end").with_location(location)
                                    );
                                }
                            };
                            events = vec![start, end];
                            kemn = true;
                        }
                        let Some(key_seed) = seed.take() else {
                            return Err(Error::InternalSeedReusedForMapKey { location });
                        };

                        // Set the fallback location to the key span *before* deserializing the key.
                        // Serde can raise `unknown_field` (and similar structural errors) during key
                        // deserialization itself; if we set the guard only after deserialization,
                        // those errors will incorrectly fall back to the container start.
                        match &mut self.fallback_guard {
                            Some(guard) => guard.replace_location(location),
                            None => {
                                self.fallback_guard =
                                    Some(MissingFieldLocationGuard::new(location));
                            }
                        }

                        let key_value = self.deserialize_recorded_key(key_seed, events, kemn)?;
                        self.have_key = true;
                        self.pending_field_comments = field_comments;
                        self.pending_value_separator_comments = value_separator_comments;
                        self.pending_value_comments = value_comments;
                        self.pending_value = Some((value_events, reference_location));

                        #[cfg(any(feature = "garde", feature = "validator"))]
                        {
                            self.pending_path_segment =
                                fingerprint.stringy_scalar_value().map(|s| s.to_owned());
                        }

                        self.seen.insert(fingerprint);
                        return Ok(Some(key_value));
                    }

                    if self.live_done {
                        return Ok(None);
                    }

                    if self.flushing_merges {
                        if self.enqueue_next_merge_batch() {
                            continue;
                        }
                        self.flushing_merges = false;
                        self.live_done = true;
                        return Ok(None);
                    }

                    match self.ev.peek()? {
                        Some(Ev::MapEnd { .. }) => {
                            let _ = self.ev.next()?; // consume end
                            if self.merge_stack.is_empty() {
                                self.live_done = true;
                                return Ok(None);
                            }
                            self.flushing_merges = true;
                            if self.enqueue_next_merge_batch() {
                                continue;
                            }
                            self.flushing_merges = false;
                            self.live_done = true;
                            return Ok(None);
                        }
                        Some(_) => {
                            let mut key_comments =
                                std::mem::take(&mut self.pending_first_key_comments);
                            key_comments.extend(self.ev.take_leading_comments_for_next_node()?);
                            let mut key_node = capture_node(self.ev)?;
                            if is_merge_key(&key_node) {
                                match self.cfg.merge_keys {
                                    MergeKeyPolicy::Merge => {
                                        // Preserve where the merge value is *referenced* (use-site).
                                        // For alias merges (`<<: *m`), `key_node`/`value_node`
                                        // locations will point at the anchored mapping, but we want
                                        // `referenced` to point at the alias token.
                                        let _ = self.ev.peek()?;
                                        let merge_ref_loc = self.ev.reference_location();
                                        let entries = pending_entries_from_live_events(
                                            self.ev,
                                            merge_ref_loc,
                                            self.cfg.merge_keys,
                                            self.cfg.dup_policy,
                                        )?;
                                        if !entries.is_empty() {
                                            self.merge_stack.push_back(entries);
                                        }
                                        continue;
                                    }
                                    MergeKeyPolicy::AsOrdinary => {}
                                    MergeKeyPolicy::Error => {
                                        return Err(Error::MergeKeyNotAllowed {
                                            location: key_node.location(),
                                        });
                                    }
                                }
                            }

                            let fingerprint = key_node.fingerprint();
                            let is_duplicate = self.seen.contains(&fingerprint);
                            match self.cfg.dup_policy {
                                DuplicateKeyPolicy::Error => {
                                    if is_duplicate {
                                        let location = key_node.location();
                                        let key = key_node
                                            .fingerprint()
                                            .stringy_scalar_value()
                                            .map(|s| s.to_owned());
                                        return Err(Error::DuplicateMappingKey { key, location });
                                    }
                                }
                                DuplicateKeyPolicy::FirstWins => {
                                    if is_duplicate {
                                        self.skip_one_node()?;
                                        continue;
                                    }
                                }
                                DuplicateKeyPolicy::LastWins => {}
                            }

                            // Decide whether we need the slow recorded path (only for the tricky
                            // explicit-empty-key-as-one-entry-map-with-nullish-inner-key case).
                            let kemn_direct =
                                is_empty_mapping_key_fingerprint(fingerprint.as_ref());
                            let kemn_one_entry_nullish =
                                is_one_entry_nullish_mapping_key_fingerprint(fingerprint.as_ref());

                            if kemn_one_entry_nullish {
                                // Slow path needed: capture value and enqueue so pending branch can
                                // swap inner value to outer and treat key as None.
                                // IMPORTANT: preserve where the value is *referenced* (use-site).
                                // If the value is an alias (`*a`), `capture_node` will record events
                                // from the anchor definition, so `value_node.location()` would point
                                // at the definition-site. `Spanned<T>` wants the alias token location
                                // in `referenced`, so take it from `Events::reference_location()`.
                                let field_comments = key_comments;
                                let value_separator_comments =
                                    self.ev.take_separator_comments_before_mapping_value()?;
                                let value_comments =
                                    self.ev.take_leading_comments_for_next_node()?;
                                let reference_location = self.ev.reference_location();
                                let value_node = capture_node(self.ev)?;
                                self.enqueue_entries(vec![PendingEntry {
                                    key: key_node,
                                    value: value_node,
                                    reference_location,
                                    field_comments,
                                    value_separator_comments,
                                    value_comments,
                                }]);
                                continue;
                            }

                            // Fast path: deserialize key now from recorded events, do not buffer value.

                            let fingerprint = fingerprint.into_owned();
                            let location = key_node.location();
                            let events = key_node.take_events();
                            let Some(key_seed) = seed.take() else {
                                return Err(Error::InternalSeedReusedForMapKey { location });
                            };

                            // Same reasoning as the buffered path above: set key-span fallback
                            // before key deserialization so errors during key parsing are
                            // attributed to this key.
                            match &mut self.fallback_guard {
                                Some(guard) => guard.replace_location(location),
                                None => {
                                    self.fallback_guard =
                                        Some(MissingFieldLocationGuard::new(location));
                                }
                            }

                            let key_value =
                                self.deserialize_recorded_key(key_seed, events, kemn_direct)?;
                            self.have_key = true;
                            self.pending_field_comments = key_comments;
                            self.pending_value = None; // value will be read live

                            #[cfg(any(feature = "garde", feature = "validator"))]
                            {
                                self.pending_path_segment =
                                    fingerprint.stringy_scalar_value().map(|s| s.to_owned());
                            }

                            self.seen.insert(fingerprint);
                            return Ok(Some(key_value));
                        }
                        None => return Err(eof_with_loc(self.ev)),
                    }
                }
            }

            /// Provide the value corresponding to the most recently yielded key.
            fn next_value_seed<Vv>(&mut self, seed: Vv) -> Result<Vv::Value, Error>
            where
                Vv: de::DeserializeSeed<'de>,
            {
                if !self.have_key {
                    return Err(Error::ValueRequestedBeforeKey {
                        location: self.ev.last_location(),
                    });
                }
                self.have_key = false;

                #[cfg(any(feature = "garde", feature = "validator"))]
                let pending_segment = self.pending_path_segment.take();

                let field_comments = std::mem::take(&mut self.pending_field_comments);
                let mut value_separator_comments =
                    std::mem::take(&mut self.pending_value_separator_comments);
                let mut value_comments = std::mem::take(&mut self.pending_value_comments);

                if let Some(events) = self.pending_value.take() {
                    let (events, reference_location) = events;
                    let mut replay = ReplayEvents::with_reference(
                        events,
                        reference_location,
                        #[cfg(feature = "properties")]
                        self.ev.property_map().cloned(),
                        #[cfg(feature = "properties")]
                        self.ev.property_syntax(),
                    );

                    // Definition-site location: where the node is defined in the YAML.
                    // For aliases, this will point at the anchor definition.
                    let defined_location = replay
                        .peek()?
                        .map(|ev| ev.location())
                        .unwrap_or_else(|| replay.last_location());

                    #[cfg(any(feature = "garde", feature = "validator"))]
                    {
                        if let (Some(seg), Some(garde_ref)) = (pending_segment, self.garde.as_mut())
                        {
                            let recorder: &mut PathRecorder = garde_ref;

                            let prev = recorder.current.take();
                            let now = prev.clone().join(seg.as_str());
                            recorder.current = now.clone();
                            recorder.map.insert(
                                now,
                                Locations {
                                    reference_location,
                                    defined_location,
                                },
                            );

                            let mut de = YamlDeserializer::new_with_path_recorder(
                                &mut replay,
                                self.cfg,
                                recorder,
                            );
                            de.pending_comments = field_comments.clone();
                            de.pending_value_separator_comments = value_separator_comments.clone();
                            de.pending_value_comments = value_comments.clone();
                            let redaction_ctx = de.peek_scalar_redaction_ctx()?;
                            let res =
                                with_subtree_redaction(redaction_ctx, || seed.deserialize(de))
                                    .map_err(|e| {
                                        attach_alias_locations_if_missing(
                                            e,
                                            reference_location,
                                            defined_location,
                                        )
                                    });
                            recorder.current = prev;
                            return res;
                        }
                    }

                    let mut de = YamlDeserializer::new(&mut replay, self.cfg);
                    de.pending_comments = field_comments;
                    de.pending_value_separator_comments = value_separator_comments;
                    de.pending_value_comments = value_comments;
                    let redaction_ctx = de.peek_scalar_redaction_ctx()?;
                    with_subtree_redaction(redaction_ctx, || seed.deserialize(de)).map_err(|e| {
                        attach_alias_locations_if_missing(e, reference_location, defined_location)
                    })
                } else {
                    value_separator_comments
                        .extend(self.ev.take_separator_comments_before_mapping_value()?);
                    value_comments.extend(self.ev.take_leading_comments_for_next_node()?);

                    // Live stream: get both locations for potential alias error reporting.
                    let defined_location = self
                        .ev
                        .peek()?
                        .map(|ev: &Ev| ev.location())
                        .unwrap_or_else(|| self.ev.last_location());

                    let reference_location = self.ev.reference_location();

                    #[cfg(any(feature = "garde", feature = "validator"))]
                    {
                        if let (Some(seg), Some(garde_ref)) = (pending_segment, self.garde.as_mut())
                        {
                            let recorder: &mut PathRecorder = garde_ref;
                            let prev = recorder.current.take();
                            let now = prev.clone().join(seg.as_str());
                            recorder.current = now.clone();
                            recorder.map.insert(
                                now,
                                Locations {
                                    reference_location,
                                    defined_location,
                                },
                            );

                            let mut de = YamlDeserializer::new_with_path_recorder(
                                self.ev, self.cfg, recorder,
                            );
                            de.pending_comments = field_comments.clone();
                            de.pending_value_separator_comments = value_separator_comments.clone();
                            de.pending_value_comments = value_comments.clone();
                            let redaction_ctx = de.peek_scalar_redaction_ctx()?;
                            let res =
                                with_subtree_redaction(redaction_ctx, || seed.deserialize(de))
                                    .map_err(|e| {
                                        attach_alias_locations_if_missing(
                                            e,
                                            reference_location,
                                            defined_location,
                                        )
                                    });
                            recorder.current = prev;
                            return res;
                        }
                    }

                    let mut de = YamlDeserializer::new(self.ev, self.cfg);
                    de.pending_comments = field_comments;
                    de.pending_value_separator_comments = value_separator_comments;
                    de.pending_value_comments = value_comments;
                    let redaction_ctx = de.peek_scalar_redaction_ctx()?;
                    with_scalar_redaction(redaction_ctx, || seed.deserialize(de)).map_err(|e| {
                        attach_alias_locations_if_missing(e, reference_location, defined_location)
                    })
                }
            }
        }

        let (pending, pending_first_key_comments, live_done) =
            if self.struct_mode && matches!(self.cfg.dup_policy, DuplicateKeyPolicy::LastWins) {
                (
                    collect_struct_last_wins_entries(
                        self.ev,
                        map_start_comments,
                        self.cfg.dup_policy,
                        self.cfg.merge_keys,
                    )?,
                    Vec::new(),
                    true,
                )
            } else {
                (VecDeque::new(), map_start_comments, false)
            };

        #[cfg(any(feature = "garde", feature = "validator"))]
        let garde = self.garde;

        visitor.visit_map(MA {
            ev: self.ev,
            cfg: child_cfg,
            have_key: false,

            fallback_guard: None,

            #[cfg(any(feature = "garde", feature = "validator"))]
            garde,
            #[cfg(any(feature = "garde", feature = "validator"))]
            pending_path_segment: None,

            seen: HashSet::with_capacity(8),
            pending,
            merge_stack: VecDeque::new(),
            flushing_merges: false,
            live_done,
            pending_value: None,
            pending_field_comments: Vec::new(),
            pending_value_separator_comments: Vec::new(),
            pending_value_comments: Vec::new(),
            pending_first_key_comments,
        })
    }

    /// **Delegates struct deserialization** to the same machinery as mappings.
    ///
    /// `Visitor` origin: From Serde for the caller’s
    /// Rust struct type (usually generated by `#[derive(Deserialize)]`). That
    /// visitor expects a `MapAccess` yielding field names/values.  
    /// **Where does it go?** We call `visitor.visit_map(..)` via `deserialize_map`,
    /// which streams YAML mapping pairs as struct fields.
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        let mut de = self;
        de.struct_mode = true;
        de.deserialize_map(visitor)
    }

    /// Deserialize an externally-tagged enum in either `Variant` or `{ Variant: value }` form.
    ///
    /// `Visitor` origin: From Serde for the target enum type.
    /// Flow: We surface an `EnumAccess` (`EA`) that provides the variant
    /// name, and a `VariantAccess` (`VA`) that reads the payload (unit/newtype/tuple/struct).
    fn deserialize_enum<V: Visitor<'de>>(
        mut self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        enum Mode<'de, 'a> {
            Unit(EnumScalarId<'de>),
            Map(String, Location),
            /// Tag selects the variant, scalar value is the newtype payload.
            TaggedNewtype(EnumScalarId<'de>, Vec<Ev<'a>>),
        }

        let mut tagged_enum = None;

        let peeked_ev = self.ev.peek()?.cloned();
        let mode = match peeked_ev {
            Some(Ev::Scalar {
                tag,
                style,
                value: _,
                raw_tag,
                location,
                ..
            }) => {
                if let Some(tag_name) = simple_tagged_enum_name(&raw_tag, &tag) {
                    tagged_enum = Some((tag_name, location));
                }
                let Some(view) = self.peek_scalar_view()? else {
                    return Err(eof_with_loc(self.ev));
                };
                if self.cfg.no_schema
                    && tag != SfTag::String
                    && maybe_not_string(&view.effective, &style, self.cfg.strict_booleans)
                {
                    let view = self.take_scalar_view()?;
                    return Err(self.quoting_required_for_scalar(&view));
                }
                // If the tag matches a variant name, use tag as variant selector
                // and the scalar value as newtype payload.
                if let Some((ref tag_name, tag_loc)) = tagged_enum {
                    if _variants.contains(&tag_name.as_str()) {
                        let variant_name = EnumScalarId {
                            raw: Cow::Owned(tag_name.clone()),
                            effective: Cow::Owned(tag_name.clone()),
                            interpolated: false,
                            location: tag_loc,
                        };
                        // Consume the scalar and re-emit it without the tag for payload deserialization
                        let Some(ev) = self.ev.next()? else {
                            return Err(eof_with_loc(self.ev));
                        };
                        let replay = match ev {
                            Ev::Scalar {
                                value,
                                style,
                                location,
                                anchor,
                                ..
                            } => {
                                vec![Ev::Scalar {
                                    value,
                                    tag: SfTag::String,
                                    raw_tag: None,
                                    style,
                                    location,
                                    anchor,
                                }]
                            }
                            other => vec![other],
                        };
                        tagged_enum = None; // prevent mismatch check
                        Mode::TaggedNewtype(variant_name, replay)
                    } else {
                        let view = self.take_scalar_view()?;
                        Mode::Unit(EnumScalarId::from_view(view))
                    }
                } else {
                    let view = self.take_scalar_view()?;
                    Mode::Unit(EnumScalarId::from_view(view))
                }
            }
            Some(Ev::MapStart { .. }) => {
                self.expect_map_start()?;
                let mut key_de = YamlDeserializer::new(&mut *self.ev, self.cfg);
                key_de.in_key = true;
                if let Some(view) = key_de.peek_scalar_view()? {
                    if self.cfg.no_schema
                        && view.tag != SfTag::String
                        && maybe_not_string(&view.raw, &view.style, self.cfg.strict_booleans)
                    {
                        let view = key_de.take_scalar_view()?;
                        return Err(self.quoting_required_for_scalar(&view));
                    }
                    let view = key_de.take_scalar_view()?;
                    Mode::Map(view.raw.into_owned(), view.location)
                } else {
                    match self.ev.next()? {
                        Some(other) => {
                            return Err(Error::ExpectedStringKeyForExternallyTaggedEnum {
                                location: other.location(),
                            });
                        }
                        None => return Err(eof_with_loc(self.ev)),
                    }
                }
            }
            Some(Ev::SeqStart {
                tag,
                raw_tag,
                location,
                ..
            }) => {
                if let Some(tag_name) = simple_tagged_enum_name(&raw_tag, &tag)
                    && _variants.contains(&tag_name.as_str())
                {
                    // Consume the SeqStart, collect all events until SeqEnd, replay as untagged sequence
                    let Some(seq_start) = self.ev.next()? else {
                        return Err(eof_with_loc(self.ev));
                    };
                    let start_loc = seq_start.location();
                    let mut replay_events: Vec<Ev<'de>> = Vec::new();
                    // Re-emit SeqStart without tag
                    if let Ev::SeqStart {
                        anchor, location, ..
                    } = seq_start
                    {
                        replay_events.push(Ev::SeqStart {
                            anchor,
                            tag: SfTag::None,
                            raw_tag: None,
                            location,
                        });
                    }
                    let mut depth = 1usize;
                    while depth > 0 {
                        match self.ev.next()? {
                            Some(ev @ Ev::SeqStart { .. }) => {
                                depth += 1;
                                replay_events.push(ev);
                            }
                            Some(ev @ Ev::SeqEnd { .. }) => {
                                depth -= 1;
                                replay_events.push(ev);
                            }
                            Some(ev @ Ev::MapStart { .. }) => {
                                depth += 1;
                                replay_events.push(ev);
                            }
                            Some(ev @ Ev::MapEnd { .. }) => {
                                depth -= 1;
                                replay_events.push(ev);
                            }
                            Some(ev) => {
                                replay_events.push(ev);
                            }
                            None => return Err(eof_with_loc(self.ev)),
                        }
                    }
                    let replay = Box::new(ReplayEvents::new(
                        replay_events,
                        #[cfg(feature = "properties")]
                        self.ev.property_map().cloned(),
                        #[cfg(feature = "properties")]
                        self.ev.property_syntax(),
                    ));
                    return visitor.visit_enum(TaggedEA {
                        replay,
                        cfg: self.cfg,
                        variant: EnumScalarId {
                            raw: Cow::Owned(tag_name.clone()),
                            effective: Cow::Owned(tag_name),
                            interpolated: false,
                            location: start_loc,
                        },
                    });
                }
                return Err(Error::ExternallyTaggedEnumExpectedScalarOrMapping { location });
            }
            Some(Ev::SeqEnd { location }) => {
                return Err(Error::UnexpectedSequenceEnd { location });
            }
            Some(Ev::MapEnd { location }) => {
                return Err(Error::UnexpectedMappingEnd { location });
            }
            Some(Ev::Taken { location }) => {
                return Err(Error::unexpected("consumed event").with_location(location));
            }
            None => return Err(eof_with_loc(self.ev)),
        };

        if let Some((tag_name, location)) = tagged_enum
            && tag_name != _name
        {
            return Err(Error::TaggedEnumMismatch {
                tagged: tag_name,
                target: _name,
                location,
            });
        }

        struct EA<'de, 'e> {
            ev: &'e mut dyn Events<'de>,
            cfg: Cfg,
            variant: EnumScalarId<'de>,
            map_mode: bool,
        }

        impl<'de, 'e> de::EnumAccess<'de> for EA<'de, 'e> {
            type Error = Error;
            type Variant = VA<'de, 'e>;

            /// Provide the variant identifier to Serde and return a `VariantAccess`.
            fn variant_seed<Vv>(self, seed: Vv) -> Result<(Vv::Value, Self::Variant), Error>
            where
                Vv: de::DeserializeSeed<'de>,
            {
                let EA {
                    ev,
                    cfg,
                    variant,
                    map_mode,
                } = self;
                let v = seed.deserialize(variant.into_deserializer())?;
                Ok((v, VA { ev, cfg, map_mode }))
            }
        }

        struct VA<'de, 'e> {
            ev: &'e mut dyn Events<'de>,
            cfg: Cfg,
            map_mode: bool,
        }

        impl VA<'_, '_> {
            /// In map mode (`{ Variant: ... }`) ensure the closing `}` is present.
            fn expect_map_end(&mut self) -> Result<(), Error> {
                match self.ev.next()? {
                    Some(Ev::MapEnd { .. }) => Ok(()),
                    Some(other) => Err(Error::ExpectedMappingEndAfterEnumVariantValue {
                        location: other.location(),
                    }),
                    None => Err(eof_with_loc(self.ev)),
                }
            }
        }

        impl<'de> de::VariantAccess<'de> for VA<'de, '_> {
            type Error = Error;

            /// Handle unit variants: `Variant` or `{ Variant: null/~ }`.
            fn unit_variant(mut self) -> Result<(), Error> {
                if self.map_mode {
                    match self.ev.peek()? {
                        Some(Ev::MapEnd { .. }) => {
                            let _ = self.ev.next()?;
                            Ok(())
                        }
                        Some(Ev::Scalar {
                            value: s, style, ..
                        }) if scalar_is_nullish(s, style) => {
                            let _ = self.ev.next()?; // consume the null-like scalar
                            self.expect_map_end()
                        }
                        Some(other) => Err(Error::UnexpectedValueForUnitEnumVariant {
                            location: other.location(),
                        }),
                        None => Err(eof_with_loc(self.ev)),
                    }
                } else {
                    Ok(())
                }
            }

            /// Handle newtype variants by delegating into `Deser`.
            fn newtype_variant_seed<T>(mut self, seed: T) -> Result<T::Value, Error>
            where
                T: de::DeserializeSeed<'de>,
            {
                // Get locations for error reporting before deserializing.
                let defined_location = self
                    .ev
                    .peek()?
                    .map(|ev: &Ev| ev.location())
                    .unwrap_or_else(|| self.ev.last_location());
                let reference_location = self.ev.reference_location();

                let mut de = YamlDeserializer::new(self.ev, self.cfg);
                let redaction_ctx = de.peek_scalar_redaction_ctx()?;
                let value = with_subtree_redaction(redaction_ctx, || seed.deserialize(de))
                    .map_err(|e| {
                        attach_alias_locations_if_missing(e, reference_location, defined_location)
                    })?;
                if self.map_mode {
                    self.expect_map_end()?;
                }
                Ok(value)
            }

            /// Handle tuple variants via `deserialize_tuple`.
            fn tuple_variant<Vv>(mut self, len: usize, visitor: Vv) -> Result<Vv::Value, Error>
            where
                Vv: Visitor<'de>,
            {
                let result =
                    YamlDeserializer::new(self.ev, self.cfg).deserialize_tuple(len, visitor)?;
                if self.map_mode {
                    self.expect_map_end()?;
                }
                Ok(result)
            }

            /// Handle struct variants via `deserialize_struct`.
            fn struct_variant<Vv>(
                mut self,
                fields: &'static [&'static str],
                visitor: Vv,
            ) -> Result<Vv::Value, Error>
            where
                Vv: Visitor<'de>,
            {
                let result = YamlDeserializer::new(self.ev, self.cfg)
                    .deserialize_struct("", fields, visitor)?;
                if self.map_mode {
                    self.expect_map_end()?;
                }
                Ok(result)
            }
        }

        struct TaggedEA<'de> {
            replay: Box<ReplayEvents<'de>>,
            cfg: Cfg,
            variant: EnumScalarId<'de>,
        }

        impl<'de> de::EnumAccess<'de> for TaggedEA<'de> {
            type Error = Error;
            type Variant = TaggedVA<'de>;

            fn variant_seed<Vv>(self, seed: Vv) -> Result<(Vv::Value, Self::Variant), Error>
            where
                Vv: de::DeserializeSeed<'de>,
            {
                let v = seed.deserialize(self.variant.into_deserializer())?;
                Ok((
                    v,
                    TaggedVA {
                        replay: self.replay,
                        cfg: self.cfg,
                    },
                ))
            }
        }

        struct TaggedVA<'de> {
            replay: Box<ReplayEvents<'de>>,
            cfg: Cfg,
        }

        impl<'de> de::VariantAccess<'de> for TaggedVA<'de> {
            type Error = Error;

            fn unit_variant(self) -> Result<(), Error> {
                Ok(())
            }

            fn newtype_variant_seed<T>(mut self, seed: T) -> Result<T::Value, Error>
            where
                T: de::DeserializeSeed<'de>,
            {
                let mut de = YamlDeserializer::new(&mut *self.replay, self.cfg);
                let redaction_ctx = de.peek_scalar_redaction_ctx()?;
                with_subtree_redaction(redaction_ctx, || seed.deserialize(de))
            }

            fn tuple_variant<Vv>(mut self, len: usize, visitor: Vv) -> Result<Vv::Value, Error>
            where
                Vv: Visitor<'de>,
            {
                YamlDeserializer::new(&mut *self.replay, self.cfg).deserialize_tuple(len, visitor)
            }

            fn struct_variant<Vv>(
                mut self,
                fields: &'static [&'static str],
                visitor: Vv,
            ) -> Result<Vv::Value, Error>
            where
                Vv: Visitor<'de>,
            {
                YamlDeserializer::new(&mut *self.replay, self.cfg)
                    .deserialize_struct("", fields, visitor)
            }
        }

        let access = match mode {
            Mode::Unit(variant) => EA {
                ev: self.ev,
                cfg: self.cfg,
                variant,
                map_mode: false,
            },
            Mode::Map(variant, variant_location) => EA {
                ev: self.ev,
                cfg: self.cfg,
                variant: EnumScalarId {
                    raw: Cow::Owned(variant.clone()),
                    effective: Cow::Owned(variant),
                    interpolated: false,
                    location: variant_location,
                },
                map_mode: true,
            },
            Mode::TaggedNewtype(variant, replay_buf) => {
                let replay = Box::new(ReplayEvents::new(
                    replay_buf,
                    #[cfg(feature = "properties")]
                    self.ev.property_map().cloned(),
                    #[cfg(feature = "properties")]
                    self.ev.property_syntax(),
                ));
                // We need to use a replay source for the payload
                return visitor.visit_enum(TaggedEA {
                    replay,
                    cfg: self.cfg,
                    variant,
                });
            }
        };

        visitor.visit_enum(access)
    }

    /// Deserialize an identifier (e.g., struct field name); treated as string.
    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_str(visitor)
    }

    /// Deserialize a value that the caller intends to ignore.
    ///
    /// Note: We still produce a value via `deserialize_any`; true “ignore”
    /// requires `serde::de::IgnoredAny` at the call site.
    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        // Delegate to `any`—callers that truly want to ignore should request `IgnoredAny`.
        self.deserialize_any(visitor)
    }
}

#[cfg(all(test, feature = "properties"))]
mod tests {
    use super::super::options::{Options, PropertySyntax};
    use super::*;

    #[test]
    fn effective_scalar_value_without_property_map_returns_original_scalar() {
        let mut events = ReplayEvents::new(Vec::new(), None, PropertySyntax::Braced);
        let de = YamlDeserializer::new(&mut events, Cfg::from_options(&Options::default()));

        let value = de
            .effective_scalar_value(
                Cow::Borrowed("${MISSING}"),
                SfTag::None,
                ScalarStyle::Plain,
                Location::new(1, 1),
            )
            .expect("missing property map should not be treated as interpolation");

        assert_eq!(value, Cow::Borrowed("${MISSING}"));
    }
}
