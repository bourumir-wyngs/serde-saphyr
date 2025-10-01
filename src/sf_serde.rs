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
use std::collections::{HashSet, VecDeque};
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

    /// Create a `Location` with explicit 1-based `row` and `column`.
    ///
    /// Parameters:
    /// - `row`: 1-based line index in the source.
    /// - `column`: 1-based column index in the source.
    ///
    /// Returns:
    /// - A `Location` carrying the given coordinates.
    ///
    /// Called by:
    /// - `location_from_span` when converting parser spans to user-facing locations.
    const fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }

    /// Shorthand for an unknown/unspecified source location.
    ///
    /// Returns:
    /// - A sentinel `Location` with both coordinates set to `0`.
    ///
    /// Called by:
    /// - Error constructors where exact location is not yet known.
    pub(crate) const fn unknown() -> Self {
        Self::UNKNOWN
    }

    /// Whether this `Location` carries meaningful (non-zero) coordinates.
    ///
    /// Returns:
    /// - `true` if both `row` and `column` are non-zero; otherwise `false`.
    ///
    /// Called by:
    /// - Error formatting/printing to decide whether to append location info.
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
    /// Construct an `Error::Message` from displayable content with unknown location.
    ///
    /// Parameters:
    /// - `s`: message text.
    ///
    /// Returns:
    /// - An `Error::Message` with `location = Location::unknown()`.
    ///
    /// Called by:
    /// - Many parsing/conversion helpers when surfacing human-readable errors.
    pub(crate) fn msg<S: Into<String>>(s: S) -> Self {
        Error::Message {
            msg: s.into(),
            location: Location::unknown(),
        }
    }

    /// Internal helper for "unexpected event" errors prior to knowing location.
    ///
    /// Parameters:
    /// - `what`: textual description of what was expected.
    ///
    /// Returns:
    /// - `Error::Unexpected` with an unknown location; caller usually `.with_location(...)`.
    ///
    /// Called by:
    /// - `Deser` helper methods when the next event is structurally invalid.
    fn unexpected(what: &'static str) -> Self {
        Error::Unexpected {
            expected: what,
            location: Location::unknown(),
        }
    }

    /// Internal helper to create an EOF error with unknown location.
    ///
    /// Returns:
    /// - `Error::Eof` with unknown location (callers typically set one).
    ///
    /// Called by:
    /// - Event consumers when reaching end-of-stream unexpectedly.
    fn eof() -> Self {
        Error::Eof {
            location: Location::unknown(),
        }
    }

    /// Construct an error for an alias that references an unknown anchor id.
    ///
    /// Parameters:
    /// - `id`: missing anchor id.
    ///
    /// Returns:
    /// - `Error::UnknownAnchor` with unknown location (caller sets it later).
    ///
    /// Called by:
    /// - Alias handling in `LiveEvents` and replay logic.
    pub(crate) fn unknown_anchor(id: usize) -> Self {
        Error::UnknownAnchor {
            id,
            location: Location::unknown(),
        }
    }

    /// Attach a concrete source `Location` to an existing error value.
    ///
    /// Parameters:
    /// - `set_location`: location to associate with the error.
    ///
    /// Returns:
    /// - The same `Error` with its location field updated.
    ///
    /// Called by:
    /// - Most error sites once a `Span`/`Location` is available.
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

    /// Read back the error location if present (non-zero), otherwise `None`.
    ///
    /// Returns:
    /// - `Some(Location)` when known; `None` when unknown.
    ///
    /// Called by:
    /// - Error formatting/printing and external consumers wanting to highlight source positions.
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

    /// Convert a `saphyr_parser::ScanError` to our `Error`, preserving location.
    ///
    /// Parameters:
    /// - `err`: parser scan error with a marker.
    ///
    /// Returns:
    /// - `Error::Message` with the parser's message and computed `Location`.
    ///
    /// Called by:
    /// - Event pump in `LiveEvents` on parser failure.
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
    /// Human-readable formatter that includes location when available.
    ///
    /// Called by:
    /// - Standard error reporting (`{}` formatting), including test assertions and logs.
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
    /// Serde hook to construct our error from a generic displayable message.
    ///
    /// Parameters:
    /// - `msg`: error text provided by Serde.
    ///
    /// Returns:
    /// - `Error::Message` with unknown location (callers typically set one later).
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::msg(msg.to_string())
    }
}

/// Format `{msg}` optionally suffixed with `at line X, column Y` if known.
///
/// Parameters:
/// - `f`: formatter.
/// - `msg`: core message text.
/// - `location`: where the error occurred (may be unknown).
///
/// Returns:
/// - Standard `fmt::Result`.
///
/// Called by:
/// - `impl Display for Error`.
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

/// Compute a `Location` from a `saphyr_parser::Span`.
///
/// Parameters:
/// - `span`: parser span with start coordinates.
///
/// Returns:
/// - 1-based `Location` corresponding to the span's start.
///
/// Called by:
/// - `LiveEvents` when mapping parser items to user-facing errors.
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
    /// Extract the source `Location` associated with this event.
    ///
    /// Returns:
    /// - Location of this event.
    ///
    /// Called by:
    /// - Error reporting helpers and event sources on lookahead/consumption.
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

/// Whether a scalar should be treated as YAML null in general (not `Option`-specific).
///
/// Parameters:
/// - `value`: scalar text.
/// - `style`: scalar style (quoted/plain).
///
/// Returns:
/// - `true` if empty string, `~`, or `null` (case-insensitive) in **plain** style; else `false`.
///
/// Called by:
/// - Unit deserialization (`deserialize_unit`), merge-key processing, and document-skipping.
fn scalar_is_nullish(value: &str, style: ScalarStyle) -> bool {
    if !matches!(style, ScalarStyle::Plain) {
        return false;
    }
    value.is_empty() || value == "~" || value.eq_ignore_ascii_case("null")
}

/// Whether a scalar should be treated as YAML `None` when deserializing an `Option<T>`.
///
/// Parameters:
/// - `value`: scalar text.
/// - `style`: scalar style.
///
/// Returns:
/// - `true` for empty **unquoted** scalar and for plain `~`/`null`; else `false`.
///
/// Called by:
/// - `deserialize_option` only (does not affect other types).
fn scalar_is_nullish_for_option(value: &str, style: ScalarStyle) -> bool {
    // For Option: treat empty unquoted scalar as null, and plain "~"/"null" as null.
    let empty_unquoted =
        value.is_empty() && !matches!(style, ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted);
    let plain_nullish =
        matches!(style, ScalarStyle::Plain) && (value == "~" || value.eq_ignore_ascii_case("null"));
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
    /// If this fingerprint represents a string-like scalar, return its raw value.
    ///
    /// Returns:
    /// - `Some(&str)` when tag permits string parsing and it is not `!!binary`; else `None`.
    ///
    /// Called by:
    /// - Duplicate-key error messages to include a user-friendly key when possible.
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

struct KeyNode {
    fingerprint: KeyFingerprint,
    events: Vec<Ev>,
    location: Location,
}

struct PendingEntry {
    key: KeyNode,
    value: KeyNode,
}

/// Capture a single YAML node (scalar/sequence/mapping) from the event stream.
///
/// Parameters:
/// - `ev`: event source with lookahead.
///
/// Returns:
/// - `Ok(KeyNode)` containing a fingerprint, recorded events, and node location.
/// - `Err(Error)` on early EOF or structural mismatch.
///
/// Called by:
/// - Mapping handling for duplicate-key detection and merge-key processing.
/// - `collect_entries_from_map` / `pending_entries_from_events`.
fn capture_node(ev: &mut dyn Events) -> Result<KeyNode, Error> {
    let Some(event) = ev.next()? else {
        return Err(Error::eof().with_location(ev.last_location()));
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
                match ev.peek()? {
                    Some(Ev::SeqEnd { location: end_loc }) => {
                        let _ = ev.next()?;
                        events.push(Ev::SeqEnd { location: end_loc });
                        break;
                    }
                    Some(_) => {
                        let child = capture_node(ev)?;
                        let KeyNode {
                            fingerprint: fp,
                            events: child_events,
                            location: _,
                        } = child;
                        elements.push(fp);
                        events.extend(child_events);
                    }
                    None => {
                        return Err(Error::eof().with_location(ev.last_location()));
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
                match ev.peek()? {
                    Some(Ev::MapEnd { location: end_loc }) => {
                        let _ = ev.next()?;
                        events.push(Ev::MapEnd { location: end_loc });
                        break;
                    }
                    Some(_) => {
                        let key = capture_node(ev)?;
                        let KeyNode {
                            fingerprint: key_fp,
                            events: key_events,
                            location: _,
                        } = key;
                        let value = capture_node(ev)?;
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
                        return Err(Error::eof().with_location(ev.last_location()));
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

/// Whether the given `KeyNode` is a YAML merge key (`<<`) in plain style w/o tag.
///
/// Parameters:
/// - `node`: key node previously captured.
///
/// Returns:
/// - `true` if it is a merge key; otherwise `false`.
///
/// Called by:
/// - Mapping deserializer when processing `<<` expansions and pending entries.
fn is_merge_key(node: &KeyNode) -> bool {
    if node.events.len() != 1 {
        return false;
    }
    matches!(
        node.events.first(),
        Some(Ev::Scalar {
            value,
            tag,
            style: ScalarStyle::Plain,
            ..
        }) if tag.is_none() && value == "<<"
    )
}

/// Convert a merge value node into a flat list of `(key,value)` pairs to merge.
///
/// Parameters:
/// - `events`: recorded events constituting the merge value (map or seq-of-maps).
/// - `location`: location of the merge value (for error reporting).
///
/// Returns:
/// - `Ok(Vec<PendingEntry>)` flattened from nested merges.
/// - `Err(Error)` if the value is not a mapping or sequence-of-mappings or on EOF.
///
/// Called by:
/// - MapAccess implementation when encountering a merge key (`<<`).
fn pending_entries_from_events(
    events: Vec<Ev>,
    location: Location,
) -> Result<Vec<PendingEntry>, Error> {
    let mut replay = ReplayEvents::new(events);
    match replay.peek()? {
        Some(Ev::MapStart { .. }) => collect_entries_from_map(&mut replay),
        Some(Ev::SeqStart { .. }) => {
            let mut merged = Vec::new();
            let _ = replay.next()?; // consume SeqStart
            loop {
                match replay.peek()? {
                    Some(Ev::SeqEnd { .. }) => {
                        let _ = replay.next()?;
                        break;
                    }
                    Some(_) => {
                        let element = capture_node(&mut replay)?;
                        let mut nested =
                            pending_entries_from_events(element.events, element.location)?;
                        merged.append(&mut nested);
                    }
                    None => {
                        return Err(Error::eof().with_location(replay.last_location()));
                    }
                }
            }
            Ok(merged)
        }
        Some(Ev::Scalar {
                 ref value, style, ..
             }) if scalar_is_nullish(value.as_str(), style) => Ok(Vec::new()),
        Some(Ev::Scalar { location, .. }) => Err(Error::msg(
            "YAML merge value must be mapping or sequence of mappings",
        )
            .with_location(location)),
        Some(other) => Err(
            Error::msg("YAML merge value must be mapping or sequence of mappings")
                .with_location(other.location()),
        ),
        None => Err(Error::eof().with_location(location)),
    }
}

/// Collect all `(key,value)` entries from a mapping event stream position.
///
/// Parameters:
/// - `ev`: event source positioned at (or before) a `MapStart`.
///
/// Returns:
/// - `Ok(Vec<PendingEntry>)` containing captured key/value nodes in order.
/// - `Err(Error)` on EOF or structural issues.
///
/// Called by:
/// - `pending_entries_from_events` and merge-key expansion code.
fn collect_entries_from_map(ev: &mut dyn Events) -> Result<Vec<PendingEntry>, Error> {
    let Some(Ev::MapStart { .. }) = ev.next()? else {
        return Err(
            Error::msg("YAML merge value must be mapping or sequence of mappings")
                .with_location(ev.last_location()),
        );
    };
    let mut entries = Vec::new();
    loop {
        match ev.peek()? {
            Some(Ev::MapEnd { .. }) => {
                let _ = ev.next()?;
                break;
            }
            Some(_) => {
                let key = capture_node(ev)?;
                let value = capture_node(ev)?;
                entries.push(PendingEntry { key, value });
            }
            None => {
                return Err(Error::eof().with_location(ev.last_location()));
            }
        }
    }
    Ok(entries)
}

/// A location-free representation of events for duplicate-key comparison.
/// Source of events with lookahead and alias-injection.
pub(crate) trait Events {
    /// Consume and return the next logical event, or `Ok(None)` on EOF.
    ///
    /// Returns:
    /// - `Ok(Some(Ev))` for the next event, `Ok(None)` for end-of-stream, `Err(Error)` on failure.
    ///
    /// Called by:
    /// - Deserializer (`Deser`) and helper utilities while walking the stream.
    fn next(&mut self) -> Result<Option<Ev>, Error>;

    /// Peek at the next event without consuming it.
    ///
    /// Returns:
    /// - `Ok(Some(Ev))` if there is a buffered/next event; `Ok(None)` on EOF; `Err(Error)` on failure.
    ///
    /// Called by:
    /// - Many structural decisions (e.g., detecting container ends, merge keys).
    fn peek(&mut self) -> Result<Option<Ev>, Error>;

    /// Location of the last yielded/peeked event for error reporting.
    ///
    /// Returns:
    /// - `Location` of the most recent event (or unknown at stream start).
    ///
    /// Called by:
    /// - Error constructors when an exact location is needed.
    fn last_location(&self) -> Location;
}

/// Event source that replays a pre-recorded buffer.
struct ReplayEvents {
    buf: Vec<Ev>,
    idx: usize,
    last_location: Location,
}

impl ReplayEvents {
    /// Create a `ReplayEvents` over a previously captured event buffer.
    ///
    /// Parameters:
    /// - `buf`: events to replay from index 0.
    ///
    /// Returns:
    /// - A `ReplayEvents` instance implementing the `Events` trait.
    ///
    /// Called by:
    /// - Merge-key expansion and duplicate-key deserialization of recorded keys/values.
    fn new(buf: Vec<Ev>) -> Self {
        Self {
            buf,
            idx: 0,
            last_location: Location::unknown(),
        }
    }
}

impl Events for ReplayEvents {
    /// Pop the next replayed event from the buffer.
    ///
    /// Returns:
    /// - `Ok(Some(Ev))` until the buffer is exhausted; then `Ok(None)`.
    ///
    /// Called by:
    /// - Deserializer when feeding recorded nodes back into Serde.
    fn next(&mut self) -> Result<Option<Ev>, Error> {
        if self.idx >= self.buf.len() {
            return Ok(None);
        }
        let ev = self.buf[self.idx].clone();
        self.idx += 1;
        self.last_location = ev.location();
        Ok(Some(ev))
    }

    /// Peek at the next buffered event without advancing.
    ///
    /// Returns:
    /// - `Ok(Some(Ev))` if any; `Ok(None)` if at end.
    ///
    /// Called by:
    /// - Control-flow decisions (e.g., end detection) while replaying.
    fn peek(&mut self) -> Result<Option<Ev>, Error> {
        if self.idx >= self.buf.len() {
            Ok(None)
        } else {
            Ok(Some(self.buf[self.idx].clone()))
        }
    }

    /// Last location observed in this replay stream.
    ///
    /// Returns:
    /// - `Location` of the last event produced or peeked.
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
    /// Construct a `Deser` over an `Events` source and configuration.
    ///
    /// Parameters:
    /// - `ev`: event source (live parser or replay).
    /// - `cfg`: duplicate-key policy and numeric parsing switches.
    ///
    /// Returns:
    /// - A `Deser` ready to implement `serde::Deserializer`.
    ///
    /// Called by:
    /// - Top-level `from_*` entry points and nested deserializations inside sequences/maps.
    fn new(ev: &'e mut dyn Events, cfg: Cfg) -> Self {
        Self { ev, cfg }
    }

    /// Consume and return the next **scalar** event triple.
    ///
    /// Returns:
    /// - `Ok((value, tag, location))` on scalar; `Err` if not a scalar or on EOF.
    ///
    /// Called by:
    /// - Scalar decoding helpers (`take_*`) and numeric/bool/char/string deserializers.
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

    /// Like `take_scalar_event` but discard the location.
    ///
    /// Returns:
    /// - `Ok((value, tag))` or an error if the next event is not a scalar.
    ///
    /// Called by:
    /// - `take_scalar` and callers that only need tag presence.
    fn take_scalar_with_tag(&mut self) -> Result<(String, Option<String>), Error> {
        let (value, tag, _) = self.take_scalar_event()?;
        Ok((value, tag))
    }

    /// Return the next scalar's string contents only.
    ///
    /// Returns:
    /// - `Ok(value)` or error if not a scalar.
    ///
    /// Called by:
    /// - Stringy conversions that ignore tags (caller may validate tag separately).
    fn take_scalar(&mut self) -> Result<String, Error> {
        self.take_scalar_with_tag().map(|(value, _)| value)
    }

    /// Return the next scalar's string contents together with its `Location`.
    ///
    /// Returns:
    /// - `Ok((value, location))` or error if not a scalar.
    ///
    /// Called by:
    /// - Numeric/bool/char parsing to include precise error locations.
    fn take_scalar_with_location(&mut self) -> Result<(String, Location), Error> {
        let (value, _, location) = self.take_scalar_event()?;
        Ok((value, location))
    }

    /// Interpret the next scalar as a `String`, honoring YAML tags and `!!binary`.
    ///
    /// - Accepts un/quoted scalars with string-compatible tags.
    /// - If tagged `!!binary`, base64-decodes and validates UTF-8.
    ///
    /// Returns:
    /// - `Ok(String)` or a typed error (wrong tag, invalid base64/UTF-8).
    ///
    /// Called by:
    /// - `deserialize_string`/`deserialize_str` and `deserialize_any` for scalars.
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

    /// Expect and consume a sequence start; error otherwise.
    ///
    /// Returns:
    /// - `Ok(())` on `SeqStart`; `Err` with location if another event or EOF.
    ///
    /// Called by:
    /// - `deserialize_seq` and tuple-like deserializers.
    fn expect_seq_start(&mut self) -> Result<(), Error> {
        match self.ev.next()? {
            Some(Ev::SeqStart { .. }) => Ok(()),
            Some(other) => Err(Error::unexpected("sequence start").with_location(other.location())),
            None => Err(Error::eof().with_location(self.ev.last_location())),
        }
    }

    /// Expect and consume a mapping start; error otherwise.
    ///
    /// Returns:
    /// - `Ok(())` on `MapStart`; `Err` with location if another event or EOF.
    ///
    /// Called by:
    /// - `deserialize_map` and struct-like deserializers.
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

    /// Entry point used by Serde when the target type accepts "any" data.
    ///
    /// Behavior:
    /// - Dispatches based on next event kind: scalar → string visitor, seq → `deserialize_seq`,
    ///   map → `deserialize_map`. Container-end events cause an error.
    ///
    /// Called by:
    /// - Serde's derive machinery for types that visit arbitrary content.
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

    /// Deserialize a YAML 1.1 boolean, handling "y/n/on/off" etc.
    ///
    /// Called by:
    /// - Serde when the destination type is `bool`.
    fn deserialize_bool<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let b: bool =
            parse_yaml11_bool(&s).map_err(|msg| Error::msg(msg).with_location(location))?;
        visitor.visit_bool(b)
    }

    /// Deserialize a signed 8-bit integer (`i8`).
    ///
    /// Called by:
    /// - Serde when the destination type is `i8`.
    fn deserialize_i8<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: i8 = parse_int_signed(s, "i8", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i8(v)
    }
    /// Deserialize a signed 16-bit integer (`i16`).
    fn deserialize_i16<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: i16 = parse_int_signed(s, "i16", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i16(v)
    }
    /// Deserialize a signed 32-bit integer (`i32`).
    fn deserialize_i32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: i32 = parse_int_signed(s, "i32", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i32(v)
    }
    /// Deserialize a signed 64-bit integer (`i64`).
    fn deserialize_i64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: i64 = parse_int_signed(s, "i64", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i64(v)
    }
    /// Deserialize a signed 128-bit integer (`i128`).
    fn deserialize_i128<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: i128 = parse_int_signed(s, "i128", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i128(v)
    }

    /// Deserialize an unsigned 8-bit integer (`u8`).
    fn deserialize_u8<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: u8 = parse_int_unsigned(s, "u8", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u8(v)
    }
    /// Deserialize an unsigned 16-bit integer (`u16`).
    fn deserialize_u16<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: u16 = parse_int_unsigned(s, "u16", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u16(v)
    }
    /// Deserialize an unsigned 32-bit integer (`u32`).
    fn deserialize_u32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: u32 = parse_int_unsigned(s, "u32", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u32(v)
    }
    /// Deserialize an unsigned 64-bit integer (`u64`).
    fn deserialize_u64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: u64 = parse_int_unsigned(s, "u64", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u64(v)
    }
    /// Deserialize an unsigned 128-bit integer (`u128`).
    fn deserialize_u128<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: u128 = parse_int_unsigned(s, "u128", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u128(v)
    }

    /// Deserialize a 32-bit float (`f32`), honoring YAML 1.2 `.nan` and `±.inf`.
    fn deserialize_f32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: f32 = parse_yaml12_f32(&s, location)?;
        visitor.visit_f32(v)
    }
    /// Deserialize a 64-bit float (`f64`), honoring YAML 1.2 `.nan` and `±.inf`.
    fn deserialize_f64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, location) = self.take_scalar_with_location()?;
        let v: f64 = parse_yaml12_f64(&s, location)?;
        visitor.visit_f64(v)
    }

    /// Deserialize a single Unicode scalar value (`char`).
    ///
    /// - Rejects YAML nullish forms (empty, `~`, `null`) explicitly.
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

    /// Alias of `deserialize_string` for borrowed string types.
    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_string(visitor)
    }

    /// Deserialize a YAML string into `String`, honoring `!!binary` → UTF-8.
    fn deserialize_string<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_string(self.take_string_scalar()?)
    }

    /// Deserialize binary data.
    ///
    /// Behavior:
    /// - If the next scalar is tagged `!!binary`, base64-decodes into `Vec<u8>`.
    /// - Else, if a sequence starts, expects a sequence of integers (0..=255) and packs them.
    /// - Else, errors (scalar without `!!binary` tag is rejected).
    ///
    /// Called by:
    /// - Serde when destination is `Vec<u8>`/`bytes`-compatible types.
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

    /// Delegate to `deserialize_bytes`.
    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_bytes(visitor)
    }

    /// Deserialize an `Option<T>`.
    ///
    /// Behavior:
    /// - Returns `None` on EOF, container end, or YAML nullish for `Option` semantics.
    /// - Otherwise delegates to `Some(self)`.
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

    /// Deserialize a unit `()` value.
    ///
    /// Behavior:
    /// - Accepts YAML nullish forms, container ends, or EOF as unit.
    /// - Errors on other content.
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

    /// Deserialize a unit struct by accepting an empty mapping `{}` or any unit value.
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

    /// Deserialize a newtype struct by delegating its single field to `self`.
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _n: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    /// Deserialize a sequence (`[ ... ]` or block list).
    ///
    /// Behavior:
    /// - If the next event is a `!!binary` scalar, exposes bytes as a sequence of `u8`.
    /// - Else, expects `SeqStart`, yields elements until `SeqEnd`, and tolerates missing end.
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
                /// SeqAccess over already-decoded bytes for `!!binary` case.
                struct ByteSeq {
                    data: Vec<u8>,
                    idx: usize,
                }
                impl<'de> de::SeqAccess<'de> for ByteSeq {
                    type Error = Error;
                    /// Yield the next `u8` from the decoded buffer.
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
        /// Streaming `SeqAccess` over the underlying `Events`.
        struct SA<'e> {
            ev: &'e mut dyn Events,
            cfg: Cfg,
        }
        impl<'de, 'e> de::SeqAccess<'de> for SA<'e> {
            type Error = Error;
            /// Deserialize the next element in the sequence (or `None` on `SeqEnd`).
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

    /// Deserialize a fixed-length tuple (delegates to `deserialize_seq`).
    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    /// Deserialize a tuple struct (delegates to `deserialize_seq`).
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    /// Deserialize a mapping (`{ ... }` or block map), including duplicate-key and merge-key logic.
    fn deserialize_map<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        self.expect_map_start()?;
        /// MapAccess implementation that handles duplicate keys and merge keys.
        struct MA<'e> {
            ev: &'e mut dyn Events,
            cfg: Cfg,
            have_key: bool,
            // For duplicate-key detection for arbitrary keys.
            seen: HashSet<KeyFingerprint>,
            pending: VecDeque<PendingEntry>,
            pending_value: Option<Vec<Ev>>,
        }

        impl<'e> MA<'e> {
            /// Consume exactly one YAML node (scalar/sequence/mapping) from the stream.
            ///
            /// Returns:
            /// - `Ok(())` after skipping over the node; `Err` on EOF/structural mismatch.
            ///
            /// Called by:
            /// - Duplicate-key policy `FirstWins` to advance over ignored values.
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

            /// Deserialize a key from a **recorded** event buffer using a fresh `Deser`.
            ///
            /// Parameters:
            /// - `seed`: Serde seed for the key type.
            /// - `events`: recorded sequence representing the key node.
            ///
            /// Returns:
            /// - The deserialized key value.
            ///
            /// Called by:
            /// - `next_key_seed` when pulling keys from `pending` or directly captured nodes.
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

            /// Yield the next key for the visitor, handling merge keys and duplicates.
            ///
            /// - Expands `<<` merge values into a queue of pending entries.
            /// - Enforces duplicate-key policy (`Error`, `FirstWins`, `LastWins`).
            fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
            where
                K: de::DeserializeSeed<'de>,
            {
                let mut seed = Some(seed);

                loop {
                    if let Some(entry) = self.pending.pop_front() {
                        if is_merge_key(&entry.key) {
                            let PendingEntry { key: _, value } = entry;
                            let mut nested =
                                pending_entries_from_events(value.events, value.location)?;
                            while let Some(inner) = nested.pop() {
                                self.pending.push_front(inner);
                            }
                            continue;
                        }

                        let PendingEntry { key, value } = entry;
                        let KeyNode {
                            fingerprint,
                            events,
                            location,
                        } = key;
                        let is_duplicate = self.seen.contains(&fingerprint);
                        match self.cfg.dup_policy {
                            DuplicateKeyPolicy::Error => {
                                if is_duplicate {
                                    let msg = fingerprint
                                        .stringy_scalar_value()
                                        .map(|s| format!("duplicate mapping key: {s}"))
                                        .unwrap_or_else(|| "duplicate mapping key".to_string());
                                    return Err(Error::msg(msg).with_location(location));
                                }
                            }
                            DuplicateKeyPolicy::FirstWins => {
                                if is_duplicate {
                                    continue;
                                }
                            }
                            DuplicateKeyPolicy::LastWins => {}
                        }

                        let value_events = value.events;
                        let key_value = self.deserialize_recorded_key(
                            seed.take().expect("seed reused for map key"),
                            events,
                        )?;
                        self.have_key = true;
                        self.pending_value = Some(value_events);
                        self.seen.insert(fingerprint);
                        return Ok(Some(key_value));
                    }

                    match self.ev.peek()? {
                        Some(Ev::MapEnd { .. }) => {
                            let _ = self.ev.next()?; // consume end
                            return Ok(None);
                        }
                        Some(_) => {
                            let key_node = capture_node(self.ev)?;
                            if is_merge_key(&key_node) {
                                let value_node = capture_node(self.ev)?;
                                let mut nested = pending_entries_from_events(
                                    value_node.events,
                                    value_node.location,
                                )?;
                                while let Some(entry) = nested.pop() {
                                    self.pending.push_front(entry);
                                }
                                continue;
                            }

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
                            let key = self.deserialize_recorded_key(
                                seed.take().expect("seed reused for map key"),
                                events,
                            )?;
                            self.have_key = true;
                            self.seen.insert(fingerprint);
                            return Ok(Some(key));
                        }
                        None => return Err(Error::eof().with_location(self.ev.last_location())),
                    }
                }
            }

            /// Produce the value for the previously yielded key.
            ///
            /// - Uses `pending_value` when the pair came from a merge expansion;
            ///   otherwise reads directly from `ev`.
            fn next_value_seed<Vv>(&mut self, seed: Vv) -> Result<Vv::Value, Error>
            where
                Vv: de::DeserializeSeed<'de>,
            {
                if !self.have_key {
                    return Err(Error::msg("value requested before key")
                        .with_location(self.ev.last_location()));
                }
                self.have_key = false;
                if let Some(events) = self.pending_value.take() {
                    let mut replay = ReplayEvents::new(events);
                    let de = Deser::new(&mut replay, self.cfg);
                    seed.deserialize(de)
                } else {
                    let de = Deser::new(self.ev, self.cfg);
                    seed.deserialize(de)
                }
            }
        }

        visitor.visit_map(MA {
            ev: self.ev,
            cfg: self.cfg,
            have_key: false,
            seen: HashSet::new(),
            pending: VecDeque::new(),
            pending_value: None,
        })
    }

    /// Deserialize a struct by delegating to `deserialize_map`.
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_map(visitor)
    }

    /// Deserialize an externally tagged enum (scalar or `{ Variant: value }`).
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

        /// EnumAccess that carries the resolved variant name and mode.
        struct EA<'e> {
            ev: &'e mut dyn Events,
            cfg: Cfg,
            variant: String,
            map_mode: bool,
        }

        impl<'de, 'e> de::EnumAccess<'de> for EA<'e> {
            type Error = Error;
            type Variant = VA<'e>;

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

        /// VariantAccess that drives the value shape (unit/newtype/tuple/struct).
        struct VA<'e> {
            ev: &'e mut dyn Events,
            cfg: Cfg,
            map_mode: bool,
        }

        impl<'e> VA<'e> {
            /// Expect `MapEnd` after reading a variant value in map mode.
            ///
            /// Returns:
            /// - `Ok(())` when the mapping ends properly; `Err` on mismatch/EOF.
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

            /// Unit-like variant (no payload).
            ///
            /// In map mode, accepts `{ Variant: ~ }`, `{ Variant: null }`, or `{ Variant: }`.
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

            /// Newtype-like variant: delegate its single field to a nested `Deser`.
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

            /// Tuple-like variant: delegate to `deserialize_tuple`.
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

            /// Struct-like variant: delegate to `deserialize_struct`.
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

    /// Deserialize an identifier (delegates to `deserialize_str`).
    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_str(visitor)
    }

    /// Let the visitor inspect the value without discarding it (`IgnoredAny` should be used to skip).
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

/// Convert a `BudgetBreach` into a user-facing `Error` message.
///
/// Parameters:
/// - `breach`: which limit tripped during scanning/replay.
///
/// Returns:
/// - `Error::Message` describing the breach (location added by caller).
///
/// Called by:
/// - `LiveEvents` when enforcing budgets over raw or replayed events.
pub(crate) fn budget_error(breach: BudgetBreach) -> Error {
    Error::msg(format!("YAML budget breached: {breach:?}"))
}
