//! Streaming Serde deserializer over granit-parser events (no Node AST).
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
//! - Duplicate key policy: Error (default), `FirstWins` (skip later pairs), or `LastWins` (let later override).
//!
//! Multiple documents:
//! - `from_str*` rejects multiple docs.
//! - `from_multiple*` collects non-empty docs; empty docs are skipped.

#[cfg(feature = "deserialize")]
pub(crate) mod base64;
#[cfg(feature = "deserialize")]
pub mod budget;
#[cfg(feature = "deserialize")]
pub(crate) mod buffered_input;
#[cfg(feature = "deserialize")]
pub(crate) mod error;
#[cfg(feature = "figment")]
pub mod figment;
#[cfg(feature = "figment2")]
pub mod figment2;
#[cfg(feature = "deserialize")]
pub(crate) mod include;
#[cfg(all(feature = "deserialize", feature = "include"))]
pub(crate) mod include_stack;
#[cfg(feature = "deserialize")]
pub(crate) mod indentation;
#[cfg(feature = "deserialize")]
pub(crate) mod input_source;
#[cfg(any(feature = "garde", feature = "validator"))]
pub(crate) mod lib_validate;
#[cfg(feature = "deserialize")]
pub(crate) mod live_events;
#[cfg(feature = "deserialize")]
pub mod localizer;
#[cfg(feature = "deserialize")]
pub(crate) mod message_formatters;
#[cfg(feature = "miette")]
pub mod miette;
#[cfg(feature = "deserialize")]
pub mod options;
#[cfg(any(feature = "garde", feature = "validator"))]
pub mod path_map;
#[cfg(feature = "properties")]
pub mod properties;
#[cfg(feature = "deserialize")]
pub(crate) mod properties_redaction;
#[cfg(feature = "deserialize")]
pub(crate) mod ring_reader;
#[cfg(feature = "robotics")]
pub mod robotics;
#[cfg(all(feature = "deserialize", feature = "include_fs"))]
pub(crate) mod safe_resolver;
#[cfg(feature = "deserialize")]
pub(crate) mod snippet;
#[cfg(feature = "deserialize")]
pub(crate) mod tags;

pub(crate) mod api;
mod cfg;
mod commented_deser;
mod deserializer;
mod events;
mod key_nodes;
mod spanned_deser;
#[cfg(test)]
mod tests;

pub mod with_deserializer;
pub use with_deserializer::{
    with_deserializer_from_reader, with_deserializer_from_reader_with_options,
    with_deserializer_from_slice, with_deserializer_from_slice_with_options,
    with_deserializer_from_str, with_deserializer_from_str_with_options,
};

pub use self::budget::Budget;
pub use self::deserializer::YamlDeserializer;
pub use self::error::Error;
pub use self::options::{AliasLimits, DuplicateKeyPolicy, MergeKeyPolicy, Options};
pub use crate::location::Location;

pub(crate) use self::cfg::Cfg;
pub(crate) use self::deserializer::with_root_redaction;
pub(crate) use self::events::{Ev, Events};
