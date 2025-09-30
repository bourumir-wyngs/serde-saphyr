//! Streaming Serde deserializer over saphyr-parser events (no Node AST).
//!
//! Supported:
//! - Scalars: string, bool (YAML 1.1 forms), integers, floats (incl. YAML 1.2 .nan / ±.inf), char.
//! - Bytes: `!!binary` (base64) or sequences of 0..=255.
//! - Arbitrarily nested sequences and mappings.
//! - Externally-tagged enums: `Variant` or `{ Variant: value }`.
//! - Anchors/aliases by recording slices and replaying them on alias.
//!
//! Hardening & policies:
//! - Alias replay limits: total replayed events, per-anchor expansion count, and replay stack depth.
//! - Duplicate key policy: Error (default), FirstWins (skip later pairs), or LastWins (let later override).
//!
//! Multiple documents:
//! - `from_str*` rejects multiple docs.
//! - `from_multiple*` collects non-empty docs; empty docs are skipped.

use crate::live_events::LiveEvents;
use std::collections::{HashSet};
use std::fmt;

use crate::base64::{decode_base64_yaml, is_binary_tag};
pub use crate::budget::{Budget, BudgetBreach, BudgetEnforcer};
use crate::parse_scalars::{
    parse_int_signed, parse_int_unsigned, parse_yaml11_bool, parse_yaml12_f32, parse_yaml12_f64,
};
use crate::tags::can_parse_into_string;
use saphyr_parser::{ScalarStyle, ScanError, Span};
use serde::de::{self, DeserializeOwned, Deserializer as _, IntoDeserializer, Visitor};

/// Row/column location within the source YAML document (1-indexed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    pub row: usize,
    pub column: usize,
}

impl Location {
    pub const UNKNOWN: Self = Self { row: 0, column: 0 };

    const fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }

    pub(crate) const fn unknown() -> Self {
        Self::UNKNOWN
    }

    fn is_known(&self) -> bool {
        self.row != 0 && self.column != 0
    }
}

/// Error type compatible with `serde::de::Error`.
#[derive(Debug)]
pub enum Error {
    Message {
        msg: String,
        location: Location,
    },
    Eof {
        location: Location,
    },
    Unexpected {
        expected: &'static str,
        location: Location,
    },
    UnknownAnchor {
        id: usize,
        location: Location,
    },
}

impl Error {
    pub(crate) fn msg<S: Into<String>>(s: S) -> Self {
        Error::Message {
            msg: s.into(),
            location: Location::unknown(),
        }
    }

    fn unexpected(what: &'static str) -> Self {
        Error::Unexpected {
            expected: what,
            location: Location::unknown(),
        }
    }

    fn eof() -> Self {
        Error::Eof {
            location: Location::unknown(),
        }
    }

    pub(crate) fn unknown_anchor(id: usize) -> Self {
        Error::UnknownAnchor {
            id,
            location: Location::unknown(),
        }
    }

    pub(crate) fn with_location(mut self, set_location: Location) -> Self {
        match &mut self {
            Error::Message { location, .. }
            | Error::Eof { location }
            | Error::Unexpected { location, .. }
            | Error::UnknownAnchor { location, .. } => {
                *location = set_location;
            }
        }
        self
    }

    pub fn location(&self) -> Option<Location> {
        match self {
            Error::Message { location, .. }
            | Error::Eof { location }
            | Error::Unexpected { location, .. }
            | Error::UnknownAnchor { location, .. } => {
                if location.is_known() {
                    Some(*location)
                } else {
                    None
                }
            }
        }
    }

    pub(crate) fn from_scan_error(err: ScanError) -> Self {
        let mark = err.marker();
        let location = Location::new(mark.line(), mark.col() + 1);
        Error::Message {
            msg: err.info().to_owned(),
            location,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Message { msg, location } => fmt_with_location(f, msg, location),
            Error::Eof { location } => fmt_with_location(f, "unexpected end of input", location),
            Error::Unexpected { expected, location } => fmt_with_location(
                f,
                &format!("unexpected event: expected {expected}"),
                location,
            ),
            Error::UnknownAnchor { id, location } => fmt_with_location(
                f,
                &format!("alias references unknown anchor id {id}"),
                location,
            ),
        }
    }
}
impl std::error::Error for Error {}
impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::msg(msg.to_string())
    }
}

fn fmt_with_location(f: &mut fmt::Formatter<'_>, msg: &str, location: &Location) -> fmt::Result {
    if location.is_known() {
        write!(
            f,
            "{msg} at line {}, column {}",
            location.row, location.column
        )
    } else {
        write!(f, "{msg}")
    }
}

pub(crate) fn location_from_span(span: &Span) -> Location {
    let start = &span.start;
    Location::new(start.line(), start.col() + 1)
}

// Re-export moved Options and related enums from the options module to preserve
// the public path serde_saphyr::sf_serde::*.
pub use crate::options::{AliasLimits, DuplicateKeyPolicy, Options};

/// Small immutable runtime configuration that `Deser` needs.
#[derive(Clone, Copy)]
struct Cfg {
    dup_policy: DuplicateKeyPolicy,
    legacy_octal_numbers: bool,
}

/// Our simplified owned event kind that we feed into Serde.
#[derive(Clone, Debug)]
pub(crate) enum Ev {
    Scalar {
        value: String,
        tag: Option<String>,
        style: ScalarStyle,
        location: Location,
    },
    SeqStart {
        location: Location,
    },
    SeqEnd {
        location: Location,
    },
    MapStart {
        location: Location,
    },
    MapEnd {
        location: Location,
    },
}

impl Ev {
    pub(crate) fn location(&self) -> Location {
        match self {
            Ev::Scalar { location, .. }
            | Ev::SeqStart { location }
            | Ev::SeqEnd { location }
            | Ev::MapStart { location }
            | Ev::MapEnd { location } => *location,
        }
    }
}

fn scalar_is_nullish(value: &str, style: ScalarStyle) -> bool {
    if !matches!(style, ScalarStyle::Plain) {
        return false;
    }
    value.is_empty() || value == "~" || value.eq_ignore_ascii_case("null")
}

fn scalar_is_nullish_for_option(value: &str, style: ScalarStyle) -> bool {
    // For Option: treat empty unquoted scalar as null, and plain "~"/"null" as null.
    let empty_unquoted = value.is_empty()
        && !matches!(style, ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted);
    let plain_nullish = matches!(style, ScalarStyle::Plain)
        && (value == "~" || value.eq_ignore_ascii_case("null"));
    empty_unquoted || plain_nullish
}

/// Canonical fingerprint of a YAML node for duplicate-key detection.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum KeyFingerprint {
    Scalar { value: String, tag: Option<String> },
    Sequence(Vec<KeyFingerprint>),
    Mapping(Vec<(KeyFingerprint, KeyFingerprint)>),
}

impl KeyFingerprint {
    fn stringy_scalar_value(&self) -> Option<&str> {
        match self {
            KeyFingerprint::Scalar { value, tag } => {
                if can_parse_into_string(tag.as_deref()) && !is_binary_tag(tag.as_deref()) {
                    Some(value.as_str())
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// A location-free representation of events for duplicate-key comparison.
/// Source of events with lookahead and alias-injection.
pub(crate) trait Events {
    fn next(&mut self) -> Result<Option<Ev>, Error>;
    fn peek(&mut self) -> Result<Option<Ev>, Error>;
    fn last_location(&self) -> Location;
}

/// Event source that replays a pre-recorded buffer.
struct ReplayEvents {
    buf: Vec<Ev>,
    idx: usize,
    last_location: Location,
}

impl ReplayEvents {
    fn new(buf: Vec<Ev>) -> Self {
        Self {
            buf,
            idx: 0,
            last_location: Location::unknown(),
        }
    }
}

impl Events for ReplayEvents {
    fn next(&mut self) -> Result<Option<Ev>, Error> {
        if self.idx >= self.buf.len() {
            return Ok(None);
        }
        let ev = self.buf[self.idx].clone();
        self.idx += 1;
        self.last_location = ev.location();
        Ok(Some(ev))
    }

    fn peek(&mut self) -> Result<Option<Ev>, Error> {
        if self.idx >= self.buf.len() {
            Ok(None)
        } else {
            Ok(Some(self.buf[self.idx].clone()))
        }
    }

    fn last_location(&self) -> Location {
        self.last_location
    }
}

/// The streaming Serde deserializer over `Events`.
struct Deser<'e> {
    ev: &'e mut dyn Events,
    cfg: Cfg,
}

impl<'e> Deser<'e> {
    fn new(ev: &'e mut dyn Events, cfg: Cfg) -> Self {
        Self { ev, cfg }
    }

    fn take_scalar_event(&mut self) -> Result<(String, Option<String>, Location), Error> {
        match self.ev.next()? {
            Some(Ev::Scalar {
                     value,
                     tag,
                     location,
                     ..
                 }) => Ok((value, tag, location)),
            Some(other) => Err(Error::unexpected("string scalar").with_location(other.location())),
            None => Err(Error::eof().with_location(self.ev.last_location())),
        }
    }
    fn take_scalar_with_tag(&mut self) -> Result<(String, Option<String>), Error> {
        let (value, tag, _) = self.take_scalar_event()?;
        Ok((value, tag))
    }
    fn take_scalar(&mut self) -> Result<String, Error> {
        self.take_scalar_with_tag().map(|(value, _)| value)
    }
    fn take_scalar_with_location(&mut self) -> Result<(String, Location), Error> {
        let (value, _, location) = self.take_scalar_event()?;
        Ok((value, location))
    }
    fn take_string_scalar(&mut self) -> Result<String, Error> {
        let (value, tag, location) = self.take_scalar_event()?;
        let tag_ref = tag.as_deref();
        if !can_parse_into_string(tag_ref) {
            if let Some(t) = tag_ref {
                return Err(Error::msg(format!(
                    "cannot deserialize scalar tagged {t} into string"
                ))
                    .with_location(location));
            }
        }

        if is_binary_tag(tag_ref) {
            let data = decode_base64_yaml(&value).map_err(|err| err.with_location(location))?;
            let text = std::str::from_utf8(&data).map_err(|_| {
                Error::msg("!!binary scalar is not valid UTF-8").with_location(location)
            })?;
            Ok(text.to_owned())
        } else {
            Ok(value)
        }
    }

    fn expect_seq_start(&mut self) -> Result<(), Error> {
        match self.ev.next()? {
            Some(Ev::SeqStart { .. }) => Ok(()),
            Some(other) => Err(Error::unexpected("sequence start").with_location(other.location())),
            None => Err(Error::eof().with_location(self.ev.last_location())),
        }
    }
    fn expect_map_start(&mut self) -> Result<(), Error> {
        match self.ev.next()? {
            Some(Ev::MapStart { .. }) => Ok(()),
            Some(other) => Err(Error::unexpected("mapping start").with_location(other.location())),
            None => Err(Error::eof().with_location(self.ev.last_location())),
        }
    }
}
impl<'de, 'e> de::Deserializer<'de> for Deser<'e> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.ev.peek()? {
            Some(Ev::Scalar { .. }) => visitor.visit_string(self.take_string_scalar()?),
            Some(Ev::SeqStart { .. }) => self.deserialize_seq(visitor),
            Some(Ev::MapStart { .. }) => self.deserialize_map(visitor),
            Some(Ev::SeqEnd { location }) => {
                Err(Error::msg("unexpected sequence end").with_location(location))
            }
            Some(Ev::MapEnd { location }) => {
                Err(Error::msg("unexpected mapping end").with_location(location))
            }
            None => Err(Error::eof().with_location(self.ev.last_location())),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let b: bool =
            parse_yaml11_bool(&s).map_err(|msg| Error::msg(msg).with_location(location))?;
        visitor.visit_bool(b)
    }

    fn deserialize_i8<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: i8 = parse_int_signed(s, "i8", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i8(v)
    }
    fn deserialize_i16<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: i16 = parse_int_signed(s, "i16", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i16(v)
    }
    fn deserialize_i32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: i32 = parse_int_signed(s, "i32", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i32(v)
    }
    fn deserialize_i64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: i64 = parse_int_signed(s, "i64", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i64(v)
    }
    fn deserialize_i128<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: i128 = parse_int_signed(s, "i128", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i128(v)
    }

    fn deserialize_u8<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: u8 = parse_int_unsigned(s, "u8", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u8(v)
    }
    fn deserialize_u16<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: u16 = parse_int_unsigned(s, "u16", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u16(v)
    }
    fn deserialize_u32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: u32 = parse_int_unsigned(s, "u32", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u32(v)
    }
    fn deserialize_u64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: u64 = parse_int_unsigned(s, "u64", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u64(v)
    }
    fn deserialize_u128<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: u128 = parse_int_unsigned(s, "u128", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u128(v)
    }

    fn deserialize_f32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: f32 = parse_yaml12_f32(&s, location)?;
        visitor.visit_f32(v)
    }
    fn deserialize_f64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: f64 = parse_yaml12_f64(&s, location)?;
        visitor.visit_f64(v)
    }

    fn deserialize_char<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        // Treat YAML null forms as invalid for `char`
        if s.is_empty() || s == "~" || s.eq_ignore_ascii_case("null") {
            return Err(Error::msg("invalid char: null not allowed").with_location(location));
        }
        let mut it = s.chars();
        match (it.next(), it.next()) {
            (Some(c), None) => visitor.visit_char(c),
            _ => Err(
                Error::msg("invalid char: expected a single Unicode scalar value")
                    .with_location(location),
            ),
        }
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_string(visitor)
    }

    fn deserialize_string<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_string(self.take_string_scalar()?)
    }

    fn deserialize_bytes<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.ev.peek()? {
            // Tagged binary scalar → base64-decode
            Some(Ev::Scalar { tag, .. }) if is_binary_tag(tag.as_deref()) => {
                let (value, data_location) = match self.ev.next()? {
                    Some(Ev::Scalar {
                             value, location, ..
                         }) => (value, location),
                    _ => unreachable!(),
                };
                let data =
                    decode_base64_yaml(&value).map_err(|err| err.with_location(data_location))?;
                visitor.visit_byte_buf(data)
            }

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
                            let b: u8 = <u8 as serde::Deserialize>::deserialize(Deser::new(
                                self.ev, self.cfg,
                            ))?;
                            out.push(b);
                        }
                        None => return Err(Error::eof().with_location(self.ev.last_location())),
                    }
                }
                visitor.visit_byte_buf(out)
            }

            // Scalar without binary tag → reject
            Some(Ev::Scalar { location, .. }) => {
                Err(Error::msg("bytes not supported (missing !!binary tag)")
                    .with_location(location))
            }

            // Anything else is unexpected here
            Some(other) => Err(
                Error::unexpected("scalar (!!binary) or sequence of 0..=255")
                    .with_location(other.location()),
            ),
            None => Err(Error::eof().with_location(self.ev.last_location())),
        }
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        // Only when Serde asks for Option<T> do we interpret YAML null-like scalars as None.
        match self.ev.peek()? {
            // End of input → None
            None => visitor.visit_none(),

            // YAML null forms as scalars → None
            Some(Ev::Scalar {
                     value: ref s,
                     style,
                     ..
                 }) if scalar_is_nullish_for_option(s, style) => {
                let _ = self.ev.next()?; // consume the scalar
                visitor.visit_none()
            }

            // In flow/edge cases a missing value can manifest as an immediate container end → None
            Some(Ev::MapEnd { .. }) | Some(Ev::SeqEnd { .. }) => visitor.visit_none(),

            // Otherwise there is a value → Some(...)
            Some(_) => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.ev.peek()? {
            // Accept YAML null forms or absence as unit
            None => visitor.visit_unit(),
            Some(Ev::Scalar {
                     value: ref s,
                     style,
                     ..
                 }) if scalar_is_nullish(s, style) => {
                let _ = self.ev.next()?; // consume the scalar
                visitor.visit_unit()
            }
            // End of a container where a value was expected: treat as unit in this subset
            Some(Ev::MapEnd { .. }) | Some(Ev::SeqEnd { .. }) => visitor.visit_unit(),
            // Anything else isn't a unit value
            Some(other) => {
                Err(Error::msg("unexpected value for unit").with_location(other.location()))
            }
        }
    }

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
                    Some(other) => Err(Error::msg("expected empty mapping for unit struct")
                        .with_location(other.location())),
                    None => Err(Error::eof().with_location(self.ev.last_location())),
                }
            }
            // Otherwise, delegate to unit handling (null, ~, empty scalar, EOF, etc.)
            _ => self.deserialize_unit(visitor),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _n: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        if let Some(Ev::Scalar { tag, .. }) = self.ev.peek()? {
            if is_binary_tag(tag.as_deref()) {
                let (scalar, data_location) = match self.ev.next()? {
                    Some(Ev::Scalar {
                             value, location, ..
                         }) => (value, location),
                    _ => unreachable!(),
                };
                let data =
                    decode_base64_yaml(&scalar).map_err(|err| err.with_location(data_location))?;
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
                        let deser = serde::de::value::U8Deserializer::<Error>::new(b);
                        seed.deserialize(deser).map(Some)
                    }
                }
                return visitor.visit_seq(ByteSeq { data, idx: 0 });
            }
        }
        self.expect_seq_start()?;
        struct SA<'e> {
            ev: &'e mut dyn Events,
            cfg: Cfg,
        }
        impl<'de, 'e> de::SeqAccess<'de> for SA<'e> {
            type Error = Error;
            fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Error>
            where
                T: de::DeserializeSeed<'de>,
            {
                let peeked = self.ev.peek()?;
                match peeked {
                    Some(Ev::SeqEnd { .. }) => Ok(None),
                    Some(_) => {
                        let de = Deser::new(self.ev, self.cfg);
                        seed.deserialize(de).map(Some)
                    }
                    None => Err(Error::eof().with_location(self.ev.last_location())),
                }
            }
        }
        let result = visitor.visit_seq(SA {
            ev: self.ev,
            cfg: self.cfg,
        })?;
        if let Some(Ev::SeqEnd { .. }) = self.ev.peek()? {
            let _ = self.ev.next()?;
        }
        Ok(result)
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        self.expect_map_start()?;
        struct MA<'e> {
            ev: &'e mut dyn Events,
            cfg: Cfg,
            have_key: bool,
            // For duplicate-key detection for arbitrary keys.
            seen: HashSet<KeyFingerprint>,
        }

        struct KeyNode {
            fingerprint: KeyFingerprint,
            events: Vec<Ev>,
            location: Location,
        }

        impl<'e> MA<'e> {
            /// Consume a single YAML node (scalar/sequence/mapping) from the event stream.
            fn skip_one_node(&mut self) -> Result<(), Error> {
                match self.ev.next()? {
                    Some(Ev::Scalar { .. }) => Ok(()),
                    Some(Ev::SeqStart { .. }) => {
                        // Skip until matching SeqEnd with nesting.
                        let mut depth = 1usize;
                        while let Some(ev) = self.ev.next()? {
                            match ev {
                                Ev::SeqStart { .. } | Ev::MapStart { .. } => depth += 1,
                                Ev::SeqEnd { .. } | Ev::MapEnd { .. } => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if depth != 0 {
                            return Err(Error::eof().with_location(self.ev.last_location()));
                        }
                        Ok(())
                    }
                    Some(Ev::MapStart { .. }) => {
                        let mut depth = 1usize;
                        while let Some(ev) = self.ev.next()? {
                            match ev {
                                Ev::SeqStart { .. } | Ev::MapStart { .. } => depth += 1,
                                Ev::SeqEnd { .. } | Ev::MapEnd { .. } => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if depth != 0 {
                            return Err(Error::eof().with_location(self.ev.last_location()));
                        }
                        Ok(())
                    }
                    Some(Ev::SeqEnd { location }) | Some(Ev::MapEnd { location }) => {
                        // This shouldn't occur for a value node start; treat as structural error.
                        Err(Error::msg("unexpected container end while skipping node")
                            .with_location(location))
                    }
                    None => Err(Error::eof().with_location(self.ev.last_location())),
                }
            }

            fn capture_key_node(&mut self) -> Result<KeyNode, Error> {
                let Some(event) = self.ev.next()? else {
                    return Err(Error::eof().with_location(self.ev.last_location()));
                };
                match event {
                    Ev::Scalar {
                        value,
                        tag,
                        style,
                        location,
                    } => {
                        let fingerprint = KeyFingerprint::Scalar {
                            value: value.clone(),
                            tag: tag.clone(),
                        };
                        Ok(KeyNode {
                            fingerprint,
                            events: vec![Ev::Scalar {
                                value,
                                tag,
                                style,
                                location,
                            }],
                            location,
                        })
                    }
                    Ev::SeqStart { location } => {
                        let mut events = vec![Ev::SeqStart { location }];
                        let mut elements = Vec::new();
                        loop {
                            match self.ev.peek()? {
                                Some(Ev::SeqEnd { location: end_loc }) => {
                                    let _ = self.ev.next()?;
                                    events.push(Ev::SeqEnd { location: end_loc });
                                    break;
                                }
                                Some(_) => {
                                    let child = self.capture_key_node()?;
                                    let KeyNode {
                                        fingerprint: fp,
                                        events: child_events,
                                        location: _,
                                    } = child;
                                    elements.push(fp);
                                    events.extend(child_events);
                                }
                                None => {
                                    return Err(Error::eof().with_location(self.ev.last_location()));
                                }
                            }
                        }
                        Ok(KeyNode {
                            fingerprint: KeyFingerprint::Sequence(elements),
                            events,
                            location,
                        })
                    }
                    Ev::MapStart { location } => {
                        let mut events = vec![Ev::MapStart { location }];
                        let mut entries = Vec::new();
                        loop {
                            match self.ev.peek()? {
                                Some(Ev::MapEnd { location: end_loc }) => {
                                    let _ = self.ev.next()?;
                                    events.push(Ev::MapEnd { location: end_loc });
                                    break;
                                }
                                Some(_) => {
                                    let key = self.capture_key_node()?;
                                    let KeyNode {
                                        fingerprint: key_fp,
                                        events: key_events,
                                        location: _,
                                    } = key;
                                    let value = self.capture_key_node()?;
                                    let KeyNode {
                                        fingerprint: value_fp,
                                        events: value_events,
                                        location: _,
                                    } = value;
                                    entries.push((key_fp, value_fp));
                                    events.extend(key_events);
                                    events.extend(value_events);
                                }
                                None => {
                                    return Err(Error::eof().with_location(self.ev.last_location()));
                                }
                            }
                        }
                        Ok(KeyNode {
                            fingerprint: KeyFingerprint::Mapping(entries),
                            events,
                            location,
                        })
                    }
                    Ev::SeqEnd { location } | Ev::MapEnd { location } => Err(Error::msg(
                        "unexpected container end while reading key node",
                    )
                        .with_location(location)),
                }
            }

            fn deserialize_recorded_key<'de, K>(
                &mut self,
                seed: K,
                events: Vec<Ev>,
            ) -> Result<K::Value, Error>
            where
                K: de::DeserializeSeed<'de>,
            {
                let mut replay = ReplayEvents::new(events);
                let de = Deser::new(&mut replay, self.cfg);
                seed.deserialize(de)
            }
        }

        impl<'de, 'e> de::MapAccess<'de> for MA<'e> {
            type Error = Error;

            fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
            where
                K: de::DeserializeSeed<'de>,
            {
                loop {
                    match self.ev.peek()? {
                        Some(Ev::MapEnd { .. }) => {
                            let _ = self.ev.next()?; // consume end
                            return Ok(None);
                        }
                        Some(_) => {
                            let key_node = self.capture_key_node()?;
                            let is_duplicate = self.seen.contains(&key_node.fingerprint);
                            match self.cfg.dup_policy {
                                DuplicateKeyPolicy::Error => {
                                    if is_duplicate {
                                        let msg = key_node
                                            .fingerprint
                                            .stringy_scalar_value()
                                            .map(|s| format!("duplicate mapping key: {s}"))
                                            .unwrap_or_else(|| "duplicate mapping key".to_string());
                                        return Err(
                                            Error::msg(msg).with_location(key_node.location)
                                        );
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

                            let KeyNode {
                                fingerprint,
                                events,
                                location: _,
                            } = key_node;
                            let key = self.deserialize_recorded_key(seed, events)?;
                            self.have_key = true;
                            self.seen.insert(fingerprint);
                            return Ok(Some(key));
                        }
                        None => return Err(Error::eof().with_location(self.ev.last_location())),
                    }
                }
            }

            fn next_value_seed<Vv>(&mut self, seed: Vv) -> Result<Vv::Value, Error>
            where
                Vv: de::DeserializeSeed<'de>,
            {
                if !self.have_key {
                    return Err(Error::msg("value requested before key")
                        .with_location(self.ev.last_location()));
                }
                self.have_key = false;
                let de = Deser::new(self.ev, self.cfg);
                seed.deserialize(de)
            }
        }

        visitor.visit_map(MA {
            ev: self.ev,
            cfg: self.cfg,
            have_key: false,
            seen: HashSet::new(),
        })
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        mut self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        enum Mode {
            Unit(String),
            Map(String),
        }

        let mode = match self.ev.peek()? {
            Some(Ev::Scalar { .. }) => Mode::Unit(self.take_scalar()?),
            Some(Ev::MapStart { .. }) => {
                self.expect_map_start()?;
                match self.ev.next()? {
                    Some(Ev::Scalar { value, .. }) => Mode::Map(value),
                    Some(other) => {
                        return Err(Error::msg("expected string key for externally tagged enum")
                            .with_location(other.location()));
                    }
                    None => return Err(Error::eof().with_location(self.ev.last_location())),
                }
            }
            Some(Ev::SeqStart { location }) => {
                return Err(
                    Error::msg("externally tagged enum expected scalar or mapping")
                        .with_location(location),
                );
            }
            Some(Ev::SeqEnd { location }) => {
                return Err(Error::msg("unexpected sequence end").with_location(location));
            }
            Some(Ev::MapEnd { location }) => {
                return Err(Error::msg("unexpected mapping end").with_location(location));
            }
            None => return Err(Error::eof().with_location(self.ev.last_location())),
        };

        struct EA<'e> {
            ev: &'e mut dyn Events,
            cfg: Cfg,
            variant: String,
            map_mode: bool,
        }

        impl<'de, 'e> de::EnumAccess<'de> for EA<'e> {
            type Error = Error;
            type Variant = VA<'e>;

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

        struct VA<'e> {
            ev: &'e mut dyn Events,
            cfg: Cfg,
            map_mode: bool,
        }

        impl<'e> VA<'e> {
            fn expect_map_end(&mut self) -> Result<(), Error> {
                match self.ev.next()? {
                    Some(Ev::MapEnd { .. }) => Ok(()),
                    Some(other) => Err(Error::msg(
                        "expected end of mapping after enum variant value",
                    )
                        .with_location(other.location())),
                    None => Err(Error::eof().with_location(self.ev.last_location())),
                }
            }
        }

        impl<'de, 'e> de::VariantAccess<'de> for VA<'e> {
            type Error = Error;

            fn unit_variant(mut self) -> Result<(), Error> {
                if self.map_mode {
                    match self.ev.peek()? {
                        Some(Ev::MapEnd { .. }) => {
                            let _ = self.ev.next()?;
                            Ok(())
                        }
                        Some(Ev::Scalar {
                                 value: ref s,
                                 style,
                                 ..
                             }) if scalar_is_nullish(s, style) => {
                            let _ = self.ev.next()?; // consume the null-like scalar
                            self.expect_map_end()
                        }
                        Some(other) => Err(Error::msg("unexpected value for unit enum variant")
                            .with_location(other.location())),
                        None => Err(Error::eof().with_location(self.ev.last_location())),
                    }
                } else {
                    Ok(())
                }
            }

            fn newtype_variant_seed<T>(mut self, seed: T) -> Result<T::Value, Error>
            where
                T: de::DeserializeSeed<'de>,
            {
                let value = seed.deserialize(Deser::new(self.ev, self.cfg))?;
                if self.map_mode {
                    self.expect_map_end()?;
                }
                Ok(value)
            }

            fn tuple_variant<Vv>(mut self, len: usize, visitor: Vv) -> Result<Vv::Value, Error>
            where
                Vv: Visitor<'de>,
            {
                let result = Deser::new(self.ev, self.cfg).deserialize_tuple(len, visitor)?;
                if self.map_mode {
                    self.expect_map_end()?;
                }
                Ok(result)
            }

            fn struct_variant<Vv>(
                mut self,
                fields: &'static [&'static str],
                visitor: Vv,
            ) -> Result<Vv::Value, Error>
            where
                Vv: Visitor<'de>,
            {
                let result =
                    Deser::new(self.ev, self.cfg).deserialize_struct("", fields, visitor)?;
                if self.map_mode {
                    self.expect_map_end()?;
                }
                Ok(result)
            }
        }

        let access = match mode {
            Mode::Unit(variant) => EA {
                ev: self.ev,
                cfg: self.cfg,
                variant,
                map_mode: false,
            },
            Mode::Map(variant) => EA {
                ev: self.ev,
                cfg: self.cfg,
                variant,
                map_mode: true,
            },
        };

        visitor.visit_enum(access)
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        // Delegate to `any`—callers that truly want to ignore should request `IgnoredAny`.
        self.deserialize_any(visitor)
    }
}

/// Deserialize any `T: serde::de::DeserializeOwned` directly from a YAML string.
///
/// This is the simplest entry point; it parses a single YAML document. If the
/// input contains multiple documents, this returns an error advising to use
/// [`from_multiple`] or [`from_multiple_with_options`].
///
/// Example: read a small `Config` structure from a YAML string.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
///     name: My Application
///     enabled: true
///     retries: 5
/// "#;
///
/// let cfg: Config = serde_saphyr::from_str(yaml).unwrap();
/// assert!(cfg.enabled);
/// ```
pub fn from_str<T: DeserializeOwned>(input: &str) -> Result<T, Error> {
    from_str_with_options(input, Options::default())
}

/// Deserialize a single YAML document with configurable [`Options`].
///
/// Example: read a small `Config` with a custom budget and default duplicate-key policy.
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::sf_serde::DuplicateKeyPolicy;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
///      name: My Application
///      enabled: true
///      retries: 5
/// "#;
///
/// let options = serde_saphyr::Options {
///      budget: Some(serde_saphyr::Budget {
///            max_anchors: 200,
///            .. serde_saphyr::Budget::default()
///      }),
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
///     .. serde_saphyr::Options::default()
/// };
/// let cfg: Config = serde_saphyr::from_str_with_options(yaml, options).unwrap();
/// assert_eq!(cfg.retries, 5);
/// ```
pub fn from_str_with_options<T: DeserializeOwned>(
    input: &str,
    options: Options,
) -> Result<T, Error> {
    let cfg = Cfg {
        dup_policy: options.duplicate_keys,
        legacy_octal_numbers: options.legacy_octal_numbers,
    };
    let mut src = LiveEvents::new(input, options.budget, options.alias_limits);
    let value = T::deserialize(Deser::new(&mut src, cfg))?;
    if let Some(ev) = src.peek()? {
        return Err(Error::msg(
            "multiple YAML documents detected; use from_multiple or from_multiple_with_options",
        )
            .with_location(ev.location()));
    }
    src.finish()?;
    Ok(value)
}

/// Deserialize multiple YAML documents from a single string into a vector of `T`.
/// Completely empty documents are ignored and not included into returned vector.
///
/// Example: read two `Config` documents separated by `---`.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: First
/// enabled: true
/// retries: 1
/// ---
/// name: Second
/// enabled: false
/// retries: 2
/// "#;
///
/// let cfgs: Vec<Config> = serde_saphyr::from_multiple(yaml).unwrap();
/// assert_eq!(cfgs.len(), 2);
/// assert_eq!(cfgs[0].name, "First");
/// ```
pub fn from_multiple<T: DeserializeOwned>(input: &str) -> Result<Vec<T>, Error> {
    from_multiple_with_options(input, Options::default())
}

/// Deserialize multiple YAML documents into a vector with configurable [`Options`].
///
/// Example: two `Config` documents with a custom budget.
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::sf_serde::DuplicateKeyPolicy;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: First
/// enabled: true
/// retries: 1
/// ---
/// name: Second
/// enabled: false
/// retries: 2
/// "#;
///
/// let options = serde_saphyr::Options {
///      budget: Some(serde_saphyr::Budget {
///            max_anchors: 200,
///            .. serde_saphyr::Budget::default()
///      }),
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
///     .. serde_saphyr::Options::default()
/// };
/// let cfgs: Vec<Config> = serde_saphyr::from_multiple_with_options(yaml, options).unwrap();
/// assert_eq!(cfgs.len(), 2);
/// assert!(!cfgs[1].enabled);
/// ```
pub fn from_multiple_with_options<T: DeserializeOwned>(
    input: &str,
    options: Options,
) -> Result<Vec<T>, Error> {
    let cfg = Cfg {
        dup_policy: options.duplicate_keys,
        legacy_octal_numbers: options.legacy_octal_numbers,
    };
    let mut src = LiveEvents::new(input, options.budget, options.alias_limits);
    let mut values = Vec::new();

    loop {
        match src.peek()? {
            // Skip documents that are explicit null-like scalars ("", "~", or "null").
            Some(Ev::Scalar {
                     value: ref s,
                     style,
                     ..
                 }) if scalar_is_nullish(s, style) => {
                let _ = src.next()?; // consume the null scalar document
                // Do not push anything for this document; move to the next one.
                continue;
            }
            Some(_) => {
                let value = T::deserialize(Deser::new(&mut src, cfg))?;
                values.push(value);
            }
            None => break,
        }
    }

    src.finish()?;
    Ok(values)
}

/// Deserialize a single YAML document from a UTF-8 byte slice.
///
/// This is equivalent to [`from_str`], but accepts `&[u8]` and validates it is
/// valid UTF-8 before parsing.
///
/// Example: read a small `Config` structure from bytes.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: My Application
/// enabled: true
/// retries: 5
/// "#;
/// let bytes = yaml.as_bytes();
/// let cfg: Config = serde_saphyr::from_slice(bytes).unwrap();
/// assert!(cfg.enabled);
/// ```
///
pub fn from_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    from_slice_with_options(bytes, Options::default())
}

/// Deserialize a single YAML document from a UTF-8 byte slice with configurable [`Options`].
///
/// Example: read a small `Config` with a custom budget from bytes.
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::sf_serde::DuplicateKeyPolicy;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
///      name: My Application
///      enabled: true
///      retries: 5
/// "#;
/// let bytes = yaml.as_bytes();
/// let options = serde_saphyr::Options {
///      budget: Some(serde_saphyr::Budget {
///            max_anchors: 200,
///            .. serde_saphyr::Budget::default()
///      }),
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
///     .. serde_saphyr::Options::default()
/// };
/// let cfg: Config = serde_saphyr::from_slice_with_options(bytes, options).unwrap();
/// assert_eq!(cfg.retries, 5);
/// ```
pub fn from_slice_with_options<T: DeserializeOwned>(
    bytes: &[u8],
    options: Options,
) -> Result<T, Error> {
    let s = std::str::from_utf8(bytes).map_err(|_| Error::msg("input is not valid UTF-8"))?;
    from_str_with_options(s, options)
}

/// Deserialize multiple YAML documents from a UTF-8 byte slice into a vector of `T`.
///
/// Example: read two `Config` documents separated by `---` from bytes.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: First
/// enabled: true
/// retries: 1
/// ---
/// name: Second
/// enabled: false
/// retries: 2
/// "#;
/// let bytes = yaml.as_bytes();
/// let cfgs: Vec<Config> = serde_saphyr::from_slice_multiple(bytes).unwrap();
/// assert_eq!(cfgs.len(), 2);
/// assert_eq!(cfgs[0].name, "First");
/// ```
pub fn from_slice_multiple<T: DeserializeOwned>(bytes: &[u8]) -> Result<Vec<T>, Error> {
    from_slice_multiple_with_options(bytes, Options::default())
}

/// Deserialize multiple YAML documents from bytes with configurable [`Options`].
/// Completely empty documents are ignored and not included into returned vector.
///
/// Example: two `Config` documents with a custom budget from bytes.
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::sf_serde::DuplicateKeyPolicy;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: First
/// enabled: true
/// retries: 1
/// ---
/// name: Second
/// enabled: false
/// retries: 2
/// "#;
/// let bytes = yaml.as_bytes();
/// let options = serde_saphyr::Options {
///      budget: Some(serde_saphyr::Budget {
///            max_anchors: 200,
///            .. serde_saphyr::Budget::default()
///      }),
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
///     .. serde_saphyr::Options::default()
/// };
/// let cfgs: Vec<Config> = serde_saphyr::from_slice_multiple_with_options(bytes, options).unwrap();
/// assert_eq!(cfgs.len(), 2);
/// assert!(!cfgs[1].enabled);
/// ```
pub fn from_slice_multiple_with_options<T: DeserializeOwned>(
    bytes: &[u8],
    options: Options,
) -> Result<Vec<T>, Error> {
    let s = std::str::from_utf8(bytes).map_err(|_| Error::msg("input is not valid UTF-8"))?;
    from_multiple_with_options(s, options)
}

pub(crate) fn budget_error(breach: BudgetBreach) -> Error {
    Error::msg(format!("YAML budget breached: {breach:?}"))
}
