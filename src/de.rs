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

use crate::anchor_store::{self, AnchorKind};
use crate::base64::decode_base64_yaml;
pub use crate::budget::Budget;
// Re-export error types
pub use crate::error::{Error, Location};
use crate::parse_scalars::{
    leading_zero_decimal, parse_int_signed, parse_int_unsigned, parse_yaml11_bool,
    parse_yaml12_float, scalar_is_nullish, scalar_is_nullish_for_option,
};
use saphyr_parser::ScalarStyle;
use serde::de::{self, Deserializer as _, IntoDeserializer, Visitor};
use std::borrow::Cow;
use std::collections::{HashSet, VecDeque};
use std::mem;

// Re-export moved Options and related enums from the options module to preserve
// the public path serde_saphyr::sf_serde::*.
pub use crate::options::{AliasLimits, DuplicateKeyPolicy, Options};
use crate::tags::SfTag;

/// Small immutable runtime configuration that `Deser` needs.
#[derive(Clone, Copy)]
pub(crate) struct Cfg {
    /// Policy to apply for duplicate mapping keys.
    pub(crate) dup_policy: DuplicateKeyPolicy,
    /// If true, accept legacy octal numbers that start with `00`.
    pub(crate) legacy_octal_numbers: bool,
    /// If true, only accept exact literals `true`/`false` as booleans.
    pub(crate) strict_booleans: bool,
    /// If true, ROS-compliant angle resolver is enabled
    pub(crate) angle_conversions: bool,
}

impl Cfg {
    #[inline]
    pub(crate) fn from_options(options: &Options) -> Self {
        Self {
            dup_policy: options.duplicate_keys,
            legacy_octal_numbers: options.legacy_octal_numbers,
            strict_booleans: options.strict_booleans,
            angle_conversions: options.angle_conversions,
        }
    }
}

/// Our simplified owned event kind that we feed into Serde.
#[derive(Clone, Debug)]
pub(crate) enum Ev {
    /// Scalar value from YAML (text), with optional tag and style.
    Scalar {
        value: String,
        tag: SfTag,
        style: ScalarStyle,
        /// Numeric anchor id (0 if none) attached to this scalar node.
        anchor: usize,
        location: Location,
    },
    /// Start of a sequence (`[` / `-`-list).
    SeqStart { anchor: usize, location: Location },
    /// End of a sequence.
    SeqEnd { location: Location },
    /// Start of a mapping (`{` or block mapping).
    MapStart { anchor: usize, location: Location },
    /// End of a mapping.
    MapEnd { location: Location },
}

impl Ev {
    /// Get the source location attached to this event.
    ///
    /// Returns:
    /// - `Location` recorded when the event was produced.
    ///
    /// Used by:
    /// - Error reporting and "last seen location" tracking.
    pub(crate) fn location(&self) -> Location {
        match self {
            Ev::Scalar { location, .. }
            | Ev::SeqStart { location, .. }
            | Ev::SeqEnd { location }
            | Ev::MapStart { location, .. }
            | Ev::MapEnd { location } => *location,
        }
    }
}

/// Canonical fingerprint of a YAML node for duplicate-key detection.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum KeyFingerprint {
    /// Scalar fingerprint (value plus optional tag).
    Scalar { value: String, tag: SfTag },
    /// Sequence fingerprint (ordered fingerprints of children).
    Sequence(Vec<KeyFingerprint>),
    /// Mapping fingerprint (ordered list of `(key, value)` fingerprints).
    Mapping(Vec<(KeyFingerprint, KeyFingerprint)>),
    /// Should not be used, arises after taking the value away
    Default,
}

impl Default for KeyFingerprint {
    fn default() -> Self {
        KeyFingerprint::Default
    }
}

impl KeyFingerprint {
    /// If this fingerprint represents a string-like scalar, return its value.
    ///
    /// Returns:
    /// - `Some(&str)` when the scalar can be parsed into string (and is not `!!binary`).
    /// - `None` for non-string scalars or containers.
    ///
    /// Used by:
    /// - Error messages to print a friendly duplicate key like `duplicate mapping key: foo`.
    fn stringy_scalar_value(&self) -> Option<&str> {
        match self {
            KeyFingerprint::Scalar { value, tag } => {
                if tag.can_parse_into_string() && tag != &SfTag::Binary {
                    Some(value.as_str())
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// A captured YAML node used to buffer keys/values and process merge keys.
///
/// Fields:
/// - `fingerprint`: canonical representation for duplicate detection.
/// - `events`: exact event slice that replays the node on demand.
/// - `location`: start location of the node (for diagnostics).
enum KeyNode {
    Fingerprinted {
        fingerprint: KeyFingerprint,
        events: Vec<Ev>,
        location: Location,
    },
    Scalar {
        events: Vec<Ev>,
        location: Location,
    },
}

impl KeyNode {
    fn fingerprint(&self) -> Cow<'_, KeyFingerprint> {
        match self {
            KeyNode::Fingerprinted { fingerprint, .. } => Cow::Borrowed(fingerprint),
            KeyNode::Scalar { events, .. } => {
                if let Some(first_event) = events.first() {
                    match first_event {
                        Ev::Scalar { tag, value, .. } => Cow::Owned(KeyFingerprint::Scalar {
                            tag: *tag,
                            value: value.clone(),
                        }),
                        _ => {
                            unreachable!()
                        }
                    }
                } else {
                    unreachable!()
                }
            }
        }
    }

    fn events(&self) -> &Vec<Ev> {
        match self {
            KeyNode::Fingerprinted { events, .. } => &events,
            KeyNode::Scalar { events, .. } => &events,
        }
    }

    fn take_events(&mut self) -> Vec<Ev> {
        match self {
            KeyNode::Fingerprinted { events, .. } => mem::take(events),
            KeyNode::Scalar { events, .. } => mem::take(events),
        }
    }

    fn take_fingerprint(&mut self) -> KeyFingerprint {
        match self {
            KeyNode::Fingerprinted { fingerprint, .. } => mem::take(fingerprint),
            KeyNode::Scalar { .. } => self.fingerprint().into_owned()
        }
    }

    fn location(&self) -> Location {
        let location = match self {
            KeyNode::Fingerprinted { location, .. } => location,
            KeyNode::Scalar { location, .. } => location,
        };
        *location
    }
}

/// A pending key/value pair to be injected into the current mapping.
///
/// Produced by:
/// - Merge (`<<`) processing and by scanning the current mapping fields.
struct PendingEntry {
    key: KeyNode,
    value: KeyNode,
}

/// Return the span lengths of key and value for a one-entry map encoded in `events`.
/// The expected layout is: MapStart, <key node>, <value node>, MapEnd.
/// On success returns (key_start, key_end, val_start, val_end) as indices into events.
fn one_entry_map_spans(events: &[Ev]) -> Option<(usize, usize, usize, usize)> {
    if events.len() < 4 {
        return None;
    }
    match events.first()? {
        Ev::MapStart { .. } => {}
        _ => return None,
    }
    match events.last()? {
        Ev::MapEnd { .. } => {}
        _ => return None,
    }
    // Cursor over the interior
    let mut i = 1; // after MapStart
    let key_start = i;
    i += skip_one_node_len(events, i)?;
    let key_end = i;
    let val_start = i;
    i += skip_one_node_len(events, i)?;
    let val_end = i;
    if i != events.len() - 1 {
        return None;
    }
    Some((key_start, key_end, val_start, val_end))
}

/// Skip one complete node in `events` starting at index `i`, returning the number of
/// events consumed. Returns None if the slice is malformed.
fn skip_one_node_len(events: &[Ev], mut i: usize) -> Option<usize> {
    match events.get(i)? {
        Ev::Scalar { .. } => Some(1),
        Ev::SeqStart { .. } => {
            let start = i;
            let mut depth = 1i32;
            i += 1;
            while i < events.len() {
                match events.get(i)? {
                    Ev::SeqStart { .. } => depth += 1,
                    Ev::SeqEnd { .. } => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(i - start + 1);
                        }
                    }
                    Ev::MapStart { .. } => depth += 1,
                    Ev::MapEnd { .. } => {
                        depth -= 1;
                    }
                    Ev::Scalar { .. } => {}
                }
                i += 1;
            }
            None
        }
        Ev::MapStart { .. } => {
            let start = i;
            let mut depth = 1i32;
            i += 1;
            while i < events.len() {
                match events.get(i)? {
                    Ev::MapStart { .. } => depth += 1,
                    Ev::MapEnd { .. } => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(i - start + 1);
                        }
                    }
                    Ev::SeqStart { .. } => depth += 1,
                    Ev::SeqEnd { .. } => {
                        depth -= 1;
                    }
                    Ev::Scalar { .. } => {}
                }
                i += 1;
            }
            None
        }
        Ev::SeqEnd { .. } | Ev::MapEnd { .. } => None,
    }
}

/// Capture a complete node (scalar/sequence/mapping) from an `Events` source,
/// returning both a fingerprint (for duplicate checks) and a replayable buffer.
/// This is recursive function.
///
/// Arguments:
/// - `ev`: event source supporting lookahead and consumption.
///
/// Returns:
/// - `Ok(KeyNode)` describing the captured subtree.
/// - `Err(Error)` on structural errors or EOF.
///
/// Called by:
/// - Mapping deserialization to stage keys and values, and by merge processing.
fn capture_node(ev: &mut dyn Events) -> Result<KeyNode, Error> {
    let Some(event) = ev.next()? else {
        return Err(Error::eof().with_location(ev.last_location()));
    };

    match event {
        Ev::Scalar {
            value,
            tag,
            style,
            anchor,
            location,
        } => {
            let ev = Ev::Scalar {
                value,
                tag,
                style,
                anchor,
                location,
            };
            Ok(KeyNode::Scalar {
                events: vec![ev],
                location,
            })
        }
        Ev::SeqStart { anchor, location } => {
            let mut events = vec![Ev::SeqStart { anchor, location }];
            let mut elements = Vec::new();
            loop {
                match ev.peek()? {
                    Some(Ev::SeqEnd { location: end_loc }) => {
                        let end_loc = *end_loc;
                        let _ = ev.next()?;
                        events.push(Ev::SeqEnd { location: end_loc });
                        break;
                    }
                    Some(_) => {
                        let mut child = capture_node(ev)?; // recursive
                        let fp = child.take_fingerprint();
                        let child_events = child.take_events();
                        elements.push(fp);
                        events.reserve(child_events.len());
                        events.extend(child_events);
                    }
                    None => {
                        return Err(Error::eof().with_location(ev.last_location()));
                    }
                }
            }
            Ok(KeyNode::Fingerprinted {
                fingerprint: KeyFingerprint::Sequence(elements),
                events,
                location,
            })
        }
        Ev::MapStart { anchor, location } => {
            let mut events = vec![Ev::MapStart { anchor, location }];
            let mut entries = Vec::new();
            loop {
                match ev.peek()? {
                    Some(Ev::MapEnd { location: end_loc }) => {
                        let end_loc = *end_loc;
                        let _ = ev.next()?;
                        events.push(Ev::MapEnd { location: end_loc });
                        break;
                    }
                    Some(_) => {
                        let mut key = capture_node(ev)?; // recursive
                        let key_fp = key.take_fingerprint();
                        let mut value = capture_node(ev)?; // recursive
                        let value_fp = value.take_fingerprint();
                        entries.push((key_fp, value_fp));
                        events.extend(key.take_events());
                        events.extend(value.take_events());
                    }
                    None => {
                        return Err(Error::eof().with_location(ev.last_location()));
                    }
                }
            }
            Ok(KeyNode::Fingerprinted {
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

/// True if `node` is the YAML merge key (`<<`) as an untagged plain scalar.
///
/// Used by:
/// - Mapping deserialization to trigger merge value expansion.
#[inline]
fn is_merge_key(node: &KeyNode) -> bool {
    let events = node.events();
    if events.len() != 1 {
        return false;
    }
    matches!(
        events.first(),
        Some(Ev::Scalar {
            value,
            tag,
            style: ScalarStyle::Plain,
            ..
        }) if tag == &SfTag::None && value == "<<"
    )
}

/// Expand a merge value node into a queue of `PendingEntry`s in correct order.
///
/// Arguments:
/// - `events`: recorded events that make up the merge value (mapping or sequence of mappings).
/// - `location`: start location of the merge value (for diagnostics).
///
/// Returns:
/// - `Ok(Vec<PendingEntry>)` entries to be enqueued into the current map in merge order.
/// - `Err(Error)` if the merge value is not a mapping/sequence-of-mappings.
///
/// Called by:
/// - Mapping deserialization when encountering `<<: value`.
fn pending_entries_from_events(
    events: Vec<Ev>,
    location: Location,
) -> Result<Vec<PendingEntry>, Error> {
    let mut replay = ReplayEvents::new(events);
    match replay.peek()? {
        Some(Ev::MapStart { .. }) => collect_entries_from_map(&mut replay),
        Some(Ev::SeqStart { .. }) => {
            let mut batches = Vec::new();
            let _ = replay.next()?; // consume SeqStart
            loop {
                match replay.peek()? {
                    Some(Ev::SeqEnd { .. }) => {
                        let _ = replay.next()?;
                        break;
                    }
                    Some(_) => {
                        let mut element = capture_node(&mut replay)?;
                        batches.push(pending_entries_from_events(
                            element.take_events(),
                            element.location(),
                        )?); // recursive
                    }
                    None => {
                        return Err(Error::eof().with_location(replay.last_location()));
                    }
                }
            }

            let mut merged = Vec::new();
            while let Some(mut nested) = batches.pop() {
                merged.append(&mut nested);
            }
            Ok(merged)
        }
        Some(Ev::Scalar { value, style, .. }) if scalar_is_nullish(value.as_str(), style) => {
            Ok(Vec::new())
        }
        Some(Ev::Scalar { location, .. }) => Err(Error::msg(
            "YAML merge value must be mapping or sequence of mappings",
        )
        .with_location(*location)),
        Some(other) => Err(
            Error::msg("YAML merge value must be mapping or sequence of mappings")
                .with_location(other.location()),
        ),
        None => Err(Error::eof().with_location(location)),
    }
}

/// Collect `(key,value)` entries from a mapping at the current position.
///
/// Arguments:
/// - `ev`: event source currently positioned at `MapStart`.
///
/// Returns:
/// - All entries from that mapping, with any nested merges expanded in-order.
///
/// Called by:
/// - Merge expansion (`pending_entries_from_events`) and map scanning.
fn collect_entries_from_map(ev: &mut dyn Events) -> Result<Vec<PendingEntry>, Error> {
    let Some(Ev::MapStart { .. }) = ev.next()? else {
        return Err(
            Error::msg("YAML merge value must be mapping or sequence of mappings")
                .with_location(ev.last_location()),
        );
    };

    let mut fields = Vec::new();
    let mut merges = Vec::new();

    loop {
        match ev.peek()? {
            Some(Ev::MapEnd { .. }) => {
                let _ = ev.next()?;
                break;
            }
            Some(_) => {
                let key = capture_node(ev)?;
                if is_merge_key(&key) {
                    let mut value = capture_node(ev)?;
                    merges.push(pending_entries_from_events(
                        value.take_events(),
                        value.location(),
                    )?);
                } else {
                    let value = capture_node(ev)?;
                    fields.push(PendingEntry { key, value });
                }
            }
            None => {
                return Err(Error::eof().with_location(ev.last_location()));
            }
        }
    }

    let mut entries = fields;
    while let Some(mut nested) = merges.pop() {
        entries.append(&mut nested);
    }
    Ok(entries)
}

/// A location-free representation of events for duplicate-key comparison.
/// Source of events with lookahead and alias-injection.
pub(crate) trait Events {
    /// Pull the next event from the stream.
    ///
    /// Returns:
    /// - `Ok(Some(Ev))` for a real event,
    /// - `Ok(None)` at true end-of-stream,
    /// - `Err(Error)` on parser/structure failure.
    ///
    /// Called by:
    /// - The streaming deserializer (`Deser`) and helper scanners.
    fn next(&mut self) -> Result<Option<Ev>, Error>;

    /// Peek at the next event without consuming it.
    ///
    /// Returns:
    /// - `Ok(Some(&Ev))` with the even reference
    /// - `Ok(None)` at end-of-stream,
    /// - `Err(Error)` on error.
    ///
    /// Called by:
    /// - Lookahead logic (merge, container boundaries, option/unit handling).
    fn peek(&mut self) -> Result<Option<&Ev>, Error>;

    /// Last location that `next` or `peek` has observed.
    ///
    /// Used by:
    /// - Error paths to attach a reasonable position when nothing else is available.
    fn last_location(&self) -> Location;
}

/// Event source that replays a pre-recorded buffer.
struct ReplayEvents {
    last_location: Location,
    buf: VecDeque<Ev>,
}

impl ReplayEvents {
    /// Create a replay source over `buf`, initially positioned at index 0.
    ///
    /// Arguments:
    /// - `buf`: previously captured events.
    ///
    /// Called by:
    /// - Merge expansion and recorded key/value deserialization.
    fn new(buf: Vec<Ev>) -> Self {
        Self {
            last_location: if let Some(first) = buf.first() {
                first.location()
            } else {
                Location::UNKNOWN
            },
            buf: VecDeque::from(buf),
        }
    }
}

impl Events for ReplayEvents {
    /// See [`Events::next`]. Replays and advances the internal index.
    fn next(&mut self) -> Result<Option<Ev>, Error> {
        if self.buf.is_empty() {
            return Ok(None);
        }

        if let Some(ev) = self.buf.pop_front() {
            self.last_location = ev.location();
            Ok(Some(ev))
        } else {
            Ok(None)
        }
    }

    fn peek(&mut self) -> Result<Option<&Ev>, Error> {
        if let Some(ev) = self.buf.front() {
            self.last_location = ev.location();
            Ok(Some(ev))
        } else {
            Ok(None)
        }
    }

    fn last_location(&self) -> Location {
        self.last_location
    }
}

/// The streaming Serde deserializer over `Events`.
///
/// Where do values come from: From an `Events` stream (typically [`LiveEvents`])
/// that yields simplified YAML events.  
/// Where do values go: Into a Serde `Visitor` provided by the caller's
/// `T: Deserialize`, which drives how we walk the event stream and construct `T`.
///
/// This type is *stateless* with respect to ownership: it borrows the event source
/// (`'e`) and forwards requests into it, translating YAML shapes into Serde calls.
pub(crate) struct Deser<'e> {
    ev: &'e mut dyn Events,
    cfg: Cfg,
    /// True when deserializing a map key.
    in_key: bool,
    /// True when the recorded key node was exactly an empty mapping (MapStart followed by MapEnd).
    key_empty_map_node: bool,
}

impl<'e> Deser<'e> {
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
    pub(crate) fn new(ev: &'e mut dyn Events, cfg: Cfg) -> Self {
        Self {
            ev,
            cfg,
            in_key: false,
            key_empty_map_node: false,
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

    /// Consume a scalar and return `(value, tag)` (dropping location).
    fn take_scalar_with_tag(&mut self) -> Result<(String, SfTag), Error> {
        let (value, tag, _) = self.take_scalar_event()?;
        Ok((value, tag))
    }

    /// Consume a scalar and return just the value.
    fn take_scalar(&mut self) -> Result<String, Error> {
        self.take_scalar_with_tag().map(|(value, _)| value)
    }

    /// Consume a scalar and return `(value, location)` (dropping tag).
    fn take_scalar_with_location(&mut self) -> Result<(String, SfTag, Location), Error> {
        let (value, tag, location) = self.take_scalar_event()?;
        Ok((value, tag, location))
    }

    /// Read a scalar as `String`, decoding `!!binary` into UTF-8 text if needed.
    ///
    /// Errors if the tag is incompatible with strings or if the binary payload
    /// is not valid UTF-8.
    fn take_string_scalar(&mut self) -> Result<String, Error> {
        let (value, tag, location) = self.take_scalar_event()?;

        // Special-case binary: decode base64 and require valid UTF-8.
        if tag == SfTag::Binary {
            let data = decode_base64_yaml(&value).map_err(|err| err.with_location(location))?;
            let text = std::str::from_utf8(&data).map_err(|_| {
                Error::msg("!!binary scalar is not valid UTF-8").with_location(location)
            })?;
            return Ok(text.to_owned());
        }

        // For non-binary, ensure the tag allows string deserialization.
        if !tag.can_parse_into_string() && tag != SfTag::NonSpecific {
            return Err(
                Error::msg("cannot deserialize scalar tagged into string").with_location(location)
            );
        }

        Ok(value)
    }

    /// Expect a sequence start and consume it, or error otherwise.
    fn expect_seq_start(&mut self) -> Result<(), Error> {
        match self.ev.next()? {
            Some(Ev::SeqStart { .. }) => Ok(()),
            Some(other) => Err(Error::unexpected("sequence start").with_location(other.location())),
            None => Err(Error::eof().with_location(self.ev.last_location())),
        }
    }

    /// Expect a mapping start and consume it, or error otherwise.
    fn expect_map_start(&mut self) -> Result<(), Error> {
        match self.ev.next()? {
            Some(Ev::MapStart { .. }) => Ok(()),
            Some(other) => Err(Error::unexpected("mapping start").with_location(other.location())),
            None => Err(Error::eof().with_location(self.ev.last_location())),
        }
    }

    /// Peek at the next event's anchor id, if any (0 indicates no anchor).
    fn peek_anchor_id(&mut self) -> Result<Option<usize>, Error> {
        match self.ev.peek()? {
            Some(Ev::Scalar { anchor, .. })
            | Some(Ev::SeqStart { anchor, .. })
            | Some(Ev::MapStart { anchor, .. }) => {
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

impl<'de, 'e> de::Deserializer<'de> for Deser<'e> {
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
    /// Can we force Serde to call the typed methods (deserialize_u8, deserialize_bool, ...)?
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
        match self.ev.peek()? {
            Some(Ev::Scalar {
                tag, style, value, ..
            }) => {
                // Tagged nulls map to unit/null regardless of style
                if tag == &SfTag::Null {
                    let _ = self.take_scalar_event()?; // consume
                    return visitor.visit_unit();
                }
                let is_plain = matches!(style, ScalarStyle::Plain);
                // Treat all YAML null-like scalars (null, ~, empty) as null when typeless.
                if scalar_is_nullish(value, style) {
                    let _ = self.ev.next()?; // consume
                    return visitor.visit_unit();
                }
                if !is_plain || !tag.can_parse_into_string() || tag == &SfTag::Binary {
                    return visitor.visit_string(self.take_string_scalar()?);
                }

                // Consume the scalar and attempt typed parses in order: bool -> int -> float.
                let (s, tag, location) = self.take_scalar_event()?;

                // Try booleans.
                if self.cfg.strict_booleans {
                    let tt = s.trim();
                    if tt.eq_ignore_ascii_case("true") {
                        return visitor.visit_bool(true);
                    } else if tt.eq_ignore_ascii_case("false") {
                        return visitor.visit_bool(false);
                    }
                    // otherwise not a bool in strict mode; continue to numbers/float/string
                } else if let Ok(b) = parse_yaml11_bool(&s) {
                    return visitor.visit_bool(b);
                }

                // Try integers: prefer signed if leading '-', else unsigned. Fallbacks use 64-bit.
                let t = s.trim();
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

                // Try float per YAML 1.2 forms.
                if let Ok(v) =
                    parse_yaml12_float::<f64>(&s, location, tag, self.cfg.angle_conversions)
                {
                    // serde_json::Value (and possibly other typeless consumers) cannot represent
                    // non-finite floats. In `deserialize_any`, prefer returning a canonical string
                    // for NaN/±Inf so that these values round-trip as strings rather than becoming
                    // null or erroring. Concrete f32/f64 entry points still yield the float values.
                    if v.is_finite() {
                        return visitor.visit_f64(v);
                    } else {
                        let canon = if v.is_nan() {
                            ".nan".to_string()
                        } else if v.is_sign_negative() {
                            "-.inf".to_string()
                        } else {
                            ".inf".to_string()
                        };
                        return visitor.visit_string(canon);
                    }
                }

                // Fallback: treat as string as-is.
                visitor.visit_string(s)
            }
            Some(Ev::SeqStart { .. }) => self.deserialize_seq(visitor),
            Some(Ev::MapStart { .. }) => self.deserialize_map(visitor),
            Some(Ev::SeqEnd { location }) => {
                Err(Error::msg("unexpected sequence end").with_location(*location))
            }
            Some(Ev::MapEnd { location }) => {
                Err(Error::msg("unexpected mapping end").with_location(*location))
            }
            None => {
                // When deserializing typeless positions (for example
                // `serde_json::Value`) a completely empty document should be
                // treated as YAML null rather than an EOF error. Structured
                // entry points like `deserialize_map` still surface EOF
                // through their dedicated `expect_*` helpers.
                visitor.visit_unit()
            }
        }
    }

    /// Parse a YAML 1.1 boolean literal into `bool`.
    ///
    /// Caller: Serde when target expects `bool`.
    /// Flow: scalar text → `Visitor::visit_bool`.
    fn deserialize_bool<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_with_location()?;
        let t = s.trim();
        let b: bool = if self.cfg.strict_booleans {
            if t.eq_ignore_ascii_case("true") {
                true
            } else if t.eq_ignore_ascii_case("false") {
                false
            } else {
                return Err(
                    Error::msg("invalid boolean (strict mode expects true/false)")
                        .with_location(location),
                );
            }
        } else {
            parse_yaml11_bool(&s).map_err(|msg| Error::msg(msg).with_location(location))?
        };
        visitor.visit_bool(b)
    }

    /// Parse a signed 8-bit integer.
    fn deserialize_i8<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_with_location()?;
        let v: i8 = parse_int_signed(&s, "i8", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i8(v)
    }
    /// Parse a signed 16-bit integer.
    fn deserialize_i16<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_with_location()?;
        let v: i16 = parse_int_signed(&s, "i16", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i16(v)
    }
    /// Parse a signed 32-bit integer.
    fn deserialize_i32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_with_location()?;
        let v: i32 = parse_int_signed(&s, "i32", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i32(v)
    }
    /// Parse a signed 64-bit integer.
    fn deserialize_i64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_with_location()?;
        let v: i64 = parse_int_signed(&s, "i64", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i64(v)
    }
    /// Parse a signed 128-bit integer.
    fn deserialize_i128<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_with_location()?;
        let v: i128 = parse_int_signed(&s, "i128", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i128(v)
    }

    /// Parse an unsigned 8-bit integer.
    fn deserialize_u8<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_with_location()?;
        let v: u8 = parse_int_unsigned(&s, "u8", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u8(v)
    }
    /// Parse an unsigned 16-bit integer.
    fn deserialize_u16<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_with_location()?;
        let v: u16 = parse_int_unsigned(&s, "u16", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u16(v)
    }
    /// Parse an unsigned 32-bit integer.
    fn deserialize_u32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_with_location()?;
        let v: u32 = parse_int_unsigned(&s, "u32", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u32(v)
    }
    /// Parse an unsigned 64-bit integer.
    fn deserialize_u64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_with_location()?;
        let v: u64 = parse_int_unsigned(&s, "u64", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u64(v)
    }
    /// Parse an unsigned 128-bit integer.
    fn deserialize_u128<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_with_location()?;
        let v: u128 = parse_int_unsigned(&s, "u128", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u128(v)
    }

    /// Parse a 32-bit float (supports YAML 1.2 `+.inf`, `-.inf`, `.nan`).
    fn deserialize_f32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_with_location()?;
        let v: f32 = parse_yaml12_float(&s, location, tag, self.cfg.angle_conversions)?;
        visitor.visit_f32(v)
    }
    /// Parse a 64-bit float (supports YAML 1.2 `+.inf`, `-.inf`, `.nan`).
    fn deserialize_f64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_with_location()?;
        let v: f64 = parse_yaml12_float(&s, location, tag, self.cfg.angle_conversions)?;
        visitor.visit_f64(v)
    }

    /// Parse a single Unicode scalar value (`char`).
    ///
    /// **Note:** YAML null forms are rejected for `char`.
    fn deserialize_char<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_with_location()?;
        // Treat YAML null forms as invalid for `char`
        if s.is_empty() || s == "~" || s.eq_ignore_ascii_case("null") {
            return Err(Error::msg("invalid char: value cannot be 'null'").with_location(location));
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

    /// Deserialize a borrowed string; delegate to owned `String` and replace the generic
    /// borrowed-string failure with a more actionable message that includes location.
    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        // Capture location if the next event is a scalar, so we can attach it to our message.
        let loc_hint = match self.ev.peek()? {
            Some(Ev::Scalar { location, .. }) => Some(*location),
            Some(other) => {
                return Err(Error::unexpected("string scalar").with_location(other.location()));
            }
            None => return Err(Error::eof().with_location(self.ev.last_location())),
        };

        match self.deserialize_string(visitor) {
            Ok(v) => Ok(v),
            Err(err) => {
                // Serde will typically produce an Error::Message like
                // "invalid type: string \"...\", expected a borrowed string" when a
                // &str visitor rejects an owned String. Detect that shape and replace it
                // with an actionable hint.
                let msg = err.to_string();
                if msg.contains("expected a borrowed string") || msg.contains("borrowed string") {
                    let location = loc_hint.unwrap();
                    Err(Error::msg(
                        r#"YAML string cannot be deserialized as &str in
`deserialize_with`. &str deserialization here would need a borrowed
string with longer life span than this format can provide. Change your
helper to deserialize into String or Cow<'de, str> instead
(e.g. String::deserialize(deserializer)?))"#,
                    )
                    .with_location(location))
                } else {
                    Err(err)
                }
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
        if let Some(next) = self.ev.peek()? {
            if let Ev::Scalar {
                tag, style, value, ..
            } = next
            {
                // If explicitly tagged as null, or plain null-like, this is not a valid String.
                if tag != &SfTag::String {
                    if tag == &SfTag::Null || scalar_is_nullish(value, style) {
                        // Consume the scalar to anchor the error at the correct location.
                        let (_value, _tag, location) = self.take_scalar_event()?;
                        return Err(Error::msg(
                            "cannot deserialize null into string; use Option<String>",
                        )
                        .with_location(location));
                    }
                }
            }
        }
        visitor.visit_string(self.take_string_scalar()?)
    }

    /// Deserialize bytes either from `!!binary` or from a sequence of integers (0..=255).
    ///
    /// **From/To:**
    /// - Tagged scalar → base64-decoded `Vec<u8>` into `Visitor::visit_byte_buf`.
    /// - Sequence of integers → packed into `Vec<u8>` and visited.
    fn deserialize_bytes<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.ev.peek()? {
            // Tagged binary scalar → base64-decode
            Some(Ev::Scalar { tag, .. }) if tag == &SfTag::Binary => {
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
                    .with_location(*location))
            }

            // Anything else is unexpected here
            Some(other) => Err(
                Error::unexpected("scalar (!!binary) or sequence of 0..=255")
                    .with_location(other.location()),
            ),
            None => Err(Error::eof().with_location(self.ev.last_location())),
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
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
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
                None => return Err(Error::eof().with_location(self.ev.last_location())),
            }
            match self.ev.next()? {
                Some(Ev::MapEnd { .. }) => {}
                Some(other) => {
                    return Err(
                        Error::unexpected("empty mapping end").with_location(other.location())
                    );
                }
                None => return Err(Error::eof().with_location(self.ev.last_location())),
            }
            return visitor.visit_none();
        }
        match self.ev.peek()? {
            // End of input → None
            None => visitor.visit_none(),

            // Tagged null → None regardless of style/value
            Some(Ev::Scalar { tag, .. }) if tag == &SfTag::Null => {
                let _ = self.ev.next()?; // consume
                visitor.visit_none()
            }

            // YAML null forms as scalars → None
            Some(Ev::Scalar {
                value: s, style, ..
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

    /// Deserialize the unit type `()`.
    ///
    /// **What is “unit” here?** Rust's `()` indicates “no value”. In Serde it
    /// commonly appears in unit structs/variants or fields intentionally
    /// ignored.  
    /// **Accepted YAML forms:** end-of-input, container end, or a null-like
    /// scalar in plain style (`""`, `~`, `null`).
    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.ev.peek()? {
            // Accept YAML null forms or absence as unit
            None => visitor.visit_unit(),
            Some(Ev::Scalar {
                value: s, style, ..
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
                    Some(other) => Err(Error::msg("expected empty mapping for unit struct")
                        .with_location(other.location())),
                    None => Err(Error::eof().with_location(self.ev.last_location())),
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
        if let Some(Ev::Scalar {
            value: s,
            tag,
            style,
            ..
        }) = self.ev.peek()?
        {
            // Treat null-like scalar as an empty sequence.
            if tag == &SfTag::Null || scalar_is_nullish(s, style) {
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
            if tag == &SfTag::Binary {
                let (scalar, data_location) = match self.ev.next()? {
                    Some(Ev::Scalar {
                        value, location, ..
                    }) => (value, location),
                    _ => unreachable!(),
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
            /// Produce the next element by recursively deserializing from the same event source.
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

    /// Deserialize a tuple; identical mechanics to sequences (fixed length checked by caller).
    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    /// Deserialize a tuple struct; identical mechanics to sequences.
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
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
        if let Some(Ev::Scalar {
            value: s,
            tag,
            style,
            ..
        }) = self.ev.peek()?
        {
            if tag == &SfTag::Null || scalar_is_nullish(s, style) {
                let _ = self.ev.next()?; // consume the null-like scalar
                struct EmptyMap;
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
                        unreachable!("no values in empty map")
                    }
                }
                return visitor.visit_map(EmptyMap);
            }
        }
        self.expect_map_start()?;
        /// Streaming `MapAccess` over the underlying `Events`.
        struct MA<'e> {
            ev: &'e mut dyn Events,
            cfg: Cfg,
            have_key: bool,
            // For duplicate-key detection for arbitrary keys.
            seen: HashSet<KeyFingerprint>,
            pending: VecDeque<PendingEntry>,
            merge_stack: Vec<Vec<PendingEntry>>,
            flushing_merges: bool,
            pending_value: Option<Vec<Ev>>,
        }

        impl<'e> MA<'e> {
            /// Skip exactly one YAML node (scalar/sequence/mapping) in the live stream.
            ///
            /// Used by:
            /// - `DuplicateKeyPolicy::FirstWins` to discard a later value.
            fn skip_one_node(&mut self) -> Result<(), Error> {
                let mut depth; // assigned later
                match self.ev.next()? {
                    Some(Ev::Scalar { .. }) => return Ok(()),
                    Some(Ev::SeqStart { .. }) | Some(Ev::MapStart { .. }) => depth = 1,
                    Some(Ev::SeqEnd { location }) | Some(Ev::MapEnd { location }) => {
                        return Err(Error::msg("unexpected container end while skipping node")
                            .with_location(location));
                    }
                    None => return Err(Error::eof().with_location(self.ev.last_location())),
                }
                while depth != 0 {
                    match self.ev.next()? {
                        Some(Ev::SeqStart { .. }) | Some(Ev::MapStart { .. }) => depth += 1,
                        Some(Ev::SeqEnd { .. }) | Some(Ev::MapEnd { .. }) => depth -= 1,
                        Some(Ev::Scalar { .. }) => {}
                        None => return Err(Error::eof().with_location(self.ev.last_location())),
                    }
                }
                Ok(())
            }

            /// Deserialize a recorded key using a temporary `ReplayEvents`.
            ///
            /// Arguments:
            /// - `seed`: Serde seed for the key type.
            /// - `events`: recorded node events for the key.
            fn deserialize_recorded_key<'de, K>(
                &mut self,
                seed: K,
                events: Vec<Ev>,
                kemn: bool,
            ) -> Result<K::Value, Error>
            where
                K: de::DeserializeSeed<'de>,
            {
                let mut replay = ReplayEvents::new(events);
                let de = Deser {
                    ev: &mut replay,
                    cfg: self.cfg,
                    in_key: true,
                    key_empty_map_node: kemn,
                };
                seed.deserialize(de)
            }

            /// Push a batch of entries to the front of the pending queue in order.
            fn enqueue_entries(&mut self, entries: Vec<PendingEntry>) {
                self.pending.reserve(entries.len());
                for entry in entries.into_iter().rev() {
                    self.pending.push_front(entry);
                }
            }

            /// Pop the next merge batch and enqueue its entries; return whether anything was queued.
            fn enqueue_next_merge_batch(&mut self) -> bool {
                while let Some(entries) = self.merge_stack.pop() {
                    if entries.is_empty() {
                        continue;
                    }
                    self.enqueue_entries(entries);
                    return true;
                }
                false
            }
        }

        impl<'de, 'e> de::MapAccess<'de> for MA<'e> {
            type Error = Error;

            /// Produce the next key for the visitor, honoring duplicate policy and merges.
            fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
            where
                K: de::DeserializeSeed<'de>,
            {
                let mut seed = Some(seed);

                loop {
                    if let Some(entry) = self.pending.pop_front() {
                        let PendingEntry { mut key, mut value } = entry;
                        let fingerprint = key.take_fingerprint();
                        let location = key.location();
                        let mut events = key.take_events();
                        let is_duplicate = self.seen.contains(&fingerprint);
                        if self.flushing_merges {
                            if is_duplicate {
                                continue;
                            }
                        } else {
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
                        }

                        let mut value_events = value.take_events();
                        // Special-case: explicit empty key captured as a one-entry mapping { null: V }
                        // In this case, we want key=None and the outer value to be V.
                        let kemn_direct =
                            matches!(fingerprint, KeyFingerprint::Mapping(ref v) if v.is_empty());
                        let mut kemn = kemn_direct;
                        if !kemn {
                            if let KeyFingerprint::Mapping(ref pairs) = fingerprint {
                                if pairs.len() == 1 {
                                    if let (
                                        KeyFingerprint::Scalar {
                                            value: sv,
                                            tag: stag,
                                        },
                                        _,
                                    ) = &pairs[0]
                                    {
                                        let is_nullish = *stag == SfTag::Null
                                            || sv.is_empty()
                                            || sv == "~"
                                            || sv.eq_ignore_ascii_case("null");
                                        if is_nullish {
                                            // Zero-copy probe over recorded events to extract inner key/value spans
                                            if let Some((_ks, _ke, vs, ve)) =
                                                one_entry_map_spans(&events)
                                            {
                                                // Replace value_events with inner value events and key events with empty map
                                                value_events = events[vs..ve].to_vec();
                                                // Build empty map events using the first and last from original events
                                                let start = match events.first() {
                                                    Some(Ev::MapStart { anchor, location }) => {
                                                        Ev::MapStart {
                                                            anchor: *anchor,
                                                            location: *location,
                                                        }
                                                    }
                                                    Some(other) => other.clone(),
                                                    None => {
                                                        return Err(Error::unexpected(
                                                            "mapping start",
                                                        )
                                                        .with_location(location));
                                                    }
                                                };
                                                let end = match events.last() {
                                                    Some(Ev::MapEnd { location }) => Ev::MapEnd {
                                                        location: *location,
                                                    },
                                                    Some(other) => other.clone(),
                                                    None => {
                                                        return Err(Error::unexpected(
                                                            "mapping end",
                                                        )
                                                        .with_location(location));
                                                    }
                                                };
                                                events = vec![start, end];
                                                kemn = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        let key_seed = match seed.take() {
                            Some(s) => s,
                            None => {
                                return Err(Error::msg("internal error: seed reused for map key")
                                    .with_location(location));
                            }
                        };
                        let key_value = self.deserialize_recorded_key(key_seed, events, kemn)?;
                        self.have_key = true;
                        self.pending_value = Some(value_events);
                        self.seen.insert(fingerprint);
                        return Ok(Some(key_value));
                    }

                    if self.flushing_merges {
                        if self.enqueue_next_merge_batch() {
                            continue;
                        }
                        self.flushing_merges = false;
                        return Ok(None);
                    }

                    match self.ev.peek()? {
                        Some(Ev::MapEnd { .. }) => {
                            let _ = self.ev.next()?; // consume end
                            if self.merge_stack.is_empty() {
                                return Ok(None);
                            }
                            self.flushing_merges = true;
                            if self.enqueue_next_merge_batch() {
                                continue;
                            }
                            self.flushing_merges = false;
                            return Ok(None);
                        }
                        Some(_) => {
                            let mut key_node = capture_node(self.ev)?;
                            if is_merge_key(&key_node) {
                                let mut value_node = capture_node(self.ev)?;
                                let entries = pending_entries_from_events(
                                    value_node.take_events(),
                                    value_node.location(),
                                )?;
                                if !entries.is_empty() {
                                    self.merge_stack.push(entries);
                                }
                                continue;
                            }

                            let fingerprint = key_node.fingerprint();
                            let is_duplicate = self.seen.contains(&fingerprint);
                            match self.cfg.dup_policy {
                                DuplicateKeyPolicy::Error => {
                                    if is_duplicate {
                                        let msg = key_node
                                            .fingerprint()
                                            .stringy_scalar_value()
                                            .map(|s| format!("duplicate mapping key: {s}"))
                                            .unwrap_or_else(|| "duplicate mapping key".to_string());
                                        return Err(
                                            Error::msg(msg).with_location(key_node.location())
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

                            // Decide whether we need the slow recorded path (only for the tricky
                            // explicit-empty-key-as-one-entry-map-with-nullish-inner-key case).
                            let kemn_direct = matches!(*fingerprint, KeyFingerprint::Mapping(ref v) if v.is_empty());
                            let kemn_one_entry_nullish = match &*fingerprint {
                                KeyFingerprint::Mapping(pairs) if pairs.len() == 1 => {
                                    if let (
                                        KeyFingerprint::Scalar {
                                            value: sv,
                                            tag: stag,
                                        },
                                        _,
                                    ) = &pairs[0]
                                    {
                                        *stag == SfTag::Null
                                            || sv.is_empty()
                                            || sv == "~"
                                            || sv.eq_ignore_ascii_case("null")
                                    } else {
                                        false
                                    }
                                }
                                _ => false,
                            };

                            if kemn_one_entry_nullish {
                                // Slow path needed: capture value and enqueue so pending branch can
                                // swap inner value to outer and treat key as None.
                                let value_node = capture_node(self.ev)?;
                                self.enqueue_entries(vec![PendingEntry {
                                    key: key_node,
                                    value: value_node,
                                }]);
                                continue;
                            } else {
                                // Fast path: deserialize key now from recorded events, do not buffer value.
                                let fingerprint = fingerprint.into_owned();
                                let location = key_node.location();
                                let events = key_node.take_events();
                                let key_seed = match seed.take() {
                                    Some(s) => s,
                                    None => {
                                        return Err(Error::msg(
                                            "internal error: seed reused for map key",
                                        )
                                        .with_location(location));
                                    }
                                };
                                let key_value = self.deserialize_recorded_key(
                                    key_seed,
                                    events,
                                    kemn_direct,
                                )?;
                                self.have_key = true;
                                self.pending_value = None; // value will be read live

                                self.seen.insert(fingerprint);
                                return Ok(Some(key_value));
                            }
                        }
                        None => return Err(Error::eof().with_location(self.ev.last_location())),
                    }
                }
            }

            /// Provide the value corresponding to the most recently yielded key.
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
            seen: HashSet::with_capacity(8),
            pending: VecDeque::new(),
            merge_stack: Vec::new(),
            flushing_merges: false,
            pending_value: None,
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
        self.deserialize_map(visitor)
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
            Some(Ev::SeqStart { location, .. }) => {
                return Err(
                    Error::msg("externally tagged enum expected scalar or mapping")
                        .with_location(*location),
                );
            }
            Some(Ev::SeqEnd { location }) => {
                return Err(Error::msg("unexpected sequence end").with_location(*location));
            }
            Some(Ev::MapEnd { location }) => {
                return Err(Error::msg("unexpected mapping end").with_location(*location));
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

        struct VA<'e> {
            ev: &'e mut dyn Events,
            cfg: Cfg,
            map_mode: bool,
        }

        impl<'e> VA<'e> {
            /// In map mode (`{ Variant: ... }`) ensure the closing `}` is present.
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
                        Some(other) => Err(Error::msg("unexpected value for unit enum variant")
                            .with_location(other.location())),
                        None => Err(Error::eof().with_location(self.ev.last_location())),
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
                let value = seed.deserialize(Deser::new(self.ev, self.cfg))?;
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
                let result = Deser::new(self.ev, self.cfg).deserialize_tuple(len, visitor)?;
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
