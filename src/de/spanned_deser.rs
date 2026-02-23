//! Internal serde wrapper implementations.
//!
//! These are used by the YAML deserializer to implement crate-level helper types
//! (like `Spanned<T>`) without bloating `de.rs` with large nested state machines.

use serde::de::{self, IntoDeserializer, Visitor};

use super::{Error, Location};
use crate::Deserializer;

#[cold]
fn unreachable_deserialize_any(what: &str) -> Error {
    Error::msg(format!("{what}::deserialize_any should not be reachable"))
}

#[cfg(not(feature = "huge_documents"))]
#[inline]
fn span_index_to_u64(v: crate::location::SpanIndex) -> u64 {
    v as u64
}

#[cfg(feature = "huge_documents")]
#[inline]
fn span_index_to_u64(v: crate::location::SpanIndex) -> u64 {
    v
}

/// Dispatch for the internal `__yaml_spanned` newtype.
///
/// This captures the current *use-site* (`referenced`) and *definition-site*
/// (`defined`) locations and then synthesizes a struct-like view:
/// `{ value: T, referenced: Location, defined: Location }`.
pub(super) fn deserialize_yaml_spanned<'de, V>(
    de: Deserializer<'de, '_>,
    visitor: V,
) -> Result<V::Value, Error>
where
    V: Visitor<'de>,
{
    // Capture the location of the next node *without consuming it*.
    let loc = match de.ev.peek()? {
        Some(ev) => ev.location(),
        None => de.ev.last_location(),
    };
    let defined: Location = loc;
    let referenced: Location = de.ev.reference_location();

    visitor.visit_newtype_struct(SpannedDeser {
        de,
        referenced,
        defined,
        state: 0,
    })
}

/// Internal deserializer used to implement `__yaml_spanned`.
///
/// Serde represents a newtype struct (`struct Wrapper(T);`) by calling
/// `Deserializer::deserialize_newtype_struct`, then asking the returned
/// deserializer for *some* representation of the wrapper.
///
/// For `Spanned<T>` we synthesize a struct-like view:
/// `{ value: T, referenced: Location, defined: Location }`.
///
/// Why:
/// - It lets `Spanned<T>` be implemented without building an intermediate YAML AST.
/// - It keeps the heavy state machine out of `de.rs`.
///
/// Lifetime note:
/// - This owns a `Deser<'a>` by value. When deserializing `value`, we must
///   *reborrow* `&mut dyn Events` from inside `Deser` rather than moving it out.
struct SpannedDeser<'de, 'e> {
    /// The underlying YAML deserializer we delegate `value: T` to.
    de: Deserializer<'de, 'e>,
    /// Use-site location: where the next node is referenced (e.g. `*a` token).
    referenced: Location,
    /// Definition-site location: where the node is defined (e.g. anchored scalar).
    defined: Location,
    /// Field iterator state for [`SpannedMapAccess`].
    state: u8,
}

impl<'de, 'e> de::Deserializer<'de> for SpannedDeser<'de, 'e> {
    type Error = Error;

    fn deserialize_any<Vv: Visitor<'de>>(self, visitor: Vv) -> Result<Vv::Value, Self::Error> {
        self.deserialize_struct("Spanned", &["value", "referenced", "defined"], visitor)
    }

    fn deserialize_struct<Vv: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: Vv,
    ) -> Result<Vv::Value, Self::Error> {
        visitor.visit_map(SpannedMapAccess {
            de: self.de,
            referenced: self.referenced,
            defined: self.defined,
            state: self.state,
        })
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map enum identifier ignored_any
    }
}

/// Map-access state machine that yields the synthesized `Spanned<T>` fields.
///
/// This yields exactly three key/value pairs in a fixed order:
/// 1) `value`
/// 2) `referenced`
/// 3) `defined`
///
/// This is intentionally a map/struct view (rather than a tuple) because the
/// public `Spanned<T>` `Deserialize` implementation uses a derived helper
/// struct (`Repr`) with named fields.
struct SpannedMapAccess<'de, 'e> {
    /// Underlying YAML deserializer.
    de: Deserializer<'de, 'e>,
    /// Use-site location (see [`Events::reference_location`]).
    referenced: Location,
    /// Definition-site location (typically `Ev::location()` from `peek()`).
    defined: Location,
    /// 0..=3 field state cursor.
    state: u8,
}

impl<'de, 'e> de::MapAccess<'de> for SpannedMapAccess<'de, 'e> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        let key = match self.state {
            0 => "value",
            1 => "referenced",
            2 => "defined",
            _ => return Ok(None),
        };
        self.state += 1;
        seed.deserialize(key.into_deserializer()).map(Some)
    }

    fn next_value_seed<Vv>(&mut self, seed: Vv) -> Result<Vv::Value, Error>
    where
        Vv: de::DeserializeSeed<'de>,
    {
        match self.state {
            1 => {
                // value
                // Reborrow the event source instead of moving `&mut` out of `self.de`.
                seed.deserialize(Deserializer::new(&mut *self.de.ev, self.de.cfg))
            }
            2 => {
                // referenced
                seed.deserialize(LocationDeser {
                    location: self.referenced,
                })
            }
            3 => {
                // defined
                seed.deserialize(LocationDeser {
                    location: self.defined,
                })
            }
            _ => Err(Error::msg("invalid Spanned<T> internal state")),
        }
    }
}

/// Internal deserializer that exposes a [`Location`] as `{ line, column, span }`.
///
/// This is used by `SpannedMapAccess` to emit `referenced` and `defined` fields
/// without requiring `Location` to be represented in the YAML input.
struct LocationDeser {
    /// The concrete location to serialize into a struct-like map.
    location: Location,
}

impl<'de> de::Deserializer<'de> for LocationDeser {
    type Error = Error;

    fn deserialize_any<Vv: Visitor<'de>>(self, _visitor: Vv) -> Result<Vv::Value, Self::Error> {
        Err(unreachable_deserialize_any("LocationDeser"))
    }

    fn deserialize_struct<Vv: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: Vv,
    ) -> Result<Vv::Value, Self::Error> {
        visitor.visit_map(LocationMapAccess {
            location: self.location,
            state: 0,
        })
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map enum identifier ignored_any
    }
}

/// Map-access state machine for [`LocationDeser`].
///
/// Yields fields in a fixed order:
/// 1) `line`
/// 2) `column`
/// 3) `span`
struct LocationMapAccess {
    /// Location being projected.
    location: Location,
    /// 0..=3 field state cursor.
    state: u8,
}

impl<'de> de::MapAccess<'de> for LocationMapAccess {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        let key = match self.state {
            0 => "line",
            1 => "column",
            2 => "span",
            _ => return Ok(None),
        };
        self.state += 1;
        seed.deserialize(key.into_deserializer()).map(Some)
    }

    fn next_value_seed<Vv>(&mut self, seed: Vv) -> Result<Vv::Value, Error>
    where
        Vv: de::DeserializeSeed<'de>,
    {
        match self.state {
            1 => seed.deserialize(self.location.line.into_deserializer()),
            2 => seed.deserialize(self.location.column.into_deserializer()),
            3 => seed.deserialize(SpanDeser {
                span: self.location.span,
            }),
            _ => Err(Error::msg("invalid Location internal state")),
        }
    }
}

/// Internal deserializer that exposes a [`crate::Span`] as `{ offset, len, byte_info }`.
struct SpanDeser {
    span: crate::Span,
}

impl<'de> de::Deserializer<'de> for SpanDeser {
    type Error = Error;

    fn deserialize_any<Vv: Visitor<'de>>(self, _visitor: Vv) -> Result<Vv::Value, Self::Error> {
        Err(unreachable_deserialize_any("SpanDeser"))
    }

    fn deserialize_struct<Vv: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: Vv,
    ) -> Result<Vv::Value, Self::Error> {
        visitor.visit_map(SpanMapAccess {
            span: self.span,
            state: 0,
        })
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map enum identifier ignored_any
    }
}

/// Deserializer for the `(SpanIndex, SpanIndex)` byte_info tuple.
struct ByteInfoTupleDeser((crate::location::SpanIndex, crate::location::SpanIndex));

impl<'de> de::Deserializer<'de> for ByteInfoTupleDeser {
    type Error = Error;

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(unreachable_deserialize_any("ByteInfoTupleDeser"))
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(ByteInfoSeqAccess {
            byte_info: self.0,
            index: 0,
        })
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq
        tuple_struct map struct enum identifier ignored_any
    }
}

struct ByteInfoSeqAccess {
    byte_info: (crate::location::SpanIndex, crate::location::SpanIndex),
    index: u8,
}

impl<'de> de::SeqAccess<'de> for ByteInfoSeqAccess {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.index {
            0 => {
                self.index += 1;
                let v = span_index_to_u64(self.byte_info.0);
                seed.deserialize(v.into_deserializer()).map(Some)
            }
            1 => {
                self.index += 1;
                let v = span_index_to_u64(self.byte_info.1);
                seed.deserialize(v.into_deserializer()).map(Some)
            }
            _ => Ok(None),
        }
    }
}

struct SpanMapAccess {
    span: crate::Span,
    state: u8,
}

impl<'de> de::MapAccess<'de> for SpanMapAccess {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        let key = match self.state {
            0 => "offset",
            1 => "len",
            2 => "byte_info",
            _ => return Ok(None),
        };
        self.state += 1;
        seed.deserialize(key.into_deserializer()).map(Some)
    }

    fn next_value_seed<Vv>(&mut self, seed: Vv) -> Result<Vv::Value, Error>
    where
        Vv: de::DeserializeSeed<'de>,
    {
        match self.state {
            1 => {
                let v = span_index_to_u64(self.span.raw_offset());
                seed.deserialize(v.into_deserializer())
            }
            2 => {
                let v = span_index_to_u64(self.span.raw_len());
                seed.deserialize(v.into_deserializer())
            }
            3 => seed.deserialize(ByteInfoTupleDeser(self.span.raw_byte_info())),
            _ => Err(Error::msg("invalid Span internal state")),
        }
    }
}
