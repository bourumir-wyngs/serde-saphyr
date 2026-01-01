//! Internal serde wrapper implementations.
//!
//! These are used by the YAML deserializer to implement crate-level helper types
//! (like `Spanned<T>`) without bloating `de.rs` with large nested state machines.

use serde::de::{self, IntoDeserializer, Visitor};

use super::{Cfg, Error, Events, Location};
use crate::YamlDeserializer;

/// Dispatch for the internal `__yaml_spanned` newtype.
///
/// This captures the current *use-site* (`referenced`) and *definition-site*
/// (`defined`) locations and then synthesizes a struct-like view:
/// `{ value: T, referenced: Location, defined: Location }`.
pub(super) fn deserialize_yaml_spanned<'de, V>(de: YamlDeserializer<'_>, visitor: V) -> Result<V::Value, Error>
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
struct SpannedDeser<'a> {
    /// The underlying YAML deserializer we delegate `value: T` to.
    de: YamlDeserializer<'a>,
    /// Use-site location: where the next node is referenced (e.g. `*a` token).
    referenced: Location,
    /// Definition-site location: where the node is defined (e.g. anchored scalar).
    defined: Location,
    /// Field iterator state for [`SpannedMapAccess`].
    state: u8,
}

impl<'de> de::Deserializer<'de> for SpannedDeser<'_> {
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
struct SpannedMapAccess<'a> {
    /// Underlying YAML deserializer.
    de: YamlDeserializer<'a>,
    /// Use-site location (see [`Events::reference_location`]).
    referenced: Location,
    /// Definition-site location (typically `Ev::location()` from `peek()`).
    defined: Location,
    /// 0..=3 field state cursor.
    state: u8,
}

impl<'de> de::MapAccess<'de> for SpannedMapAccess<'_> {
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
                seed.deserialize(YamlDeserializer::new(&mut *self.de.ev, self.de.cfg))
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

/// Internal deserializer that exposes a [`Location`] as `{ line, column }`.
///
/// This is used by `SpannedMapAccess` to emit `referenced` and `defined` fields
/// without requiring `Location` to be represented in the YAML input.
struct LocationDeser {
    /// The concrete location to serialize into a struct-like map.
    location: Location,
}

impl<'de> de::Deserializer<'de> for LocationDeser {
    type Error = Error;

    fn deserialize_any<Vv: Visitor<'de>>(self, visitor: Vv) -> Result<Vv::Value, Self::Error> {
        self.deserialize_struct("Location", &["line", "column"], visitor)
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
/// Yields two fields in a fixed order:
/// 1) `line`
/// 2) `column`
struct LocationMapAccess {
    /// Location being projected.
    location: Location,
    /// 0..=2 field state cursor.
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
            _ => Err(Error::msg("invalid Location internal state")),
        }
    }
}

// NOTE: `Cfg` and `Events` are imported to make it obvious this module is tightly
// coupled to the YAML deserializer internals (and not part of the public API).
#[allow(dead_code)]
/// Dummy function that references internal types so accidental decoupling is visible.
///
/// This module is intentionally not a generic “serde helpers” module: it depends on
/// YAML-specific `Cfg` and the `Events` abstraction (notably `reference_location()`).
fn _internal_tie_to_deser(cfg: Cfg, ev: &mut dyn Events) {
    let _ = (cfg, ev);
}
