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

pub use crate::budget::Budget;
pub use crate::de_error::Error;
pub use crate::location::Location;

use crate::anchor_store::{self, AnchorKind};
use crate::base64::decode_base64_yaml;
use crate::de_error::{MissingFieldLocationGuard, TransformReason};
use crate::parse_scalars::{
    leading_zero_decimal, maybe_not_string, parse_int_signed, parse_int_unsigned,
    parse_yaml11_bool, parse_yaml12_float, scalar_is_nullish, scalar_is_nullish_for_option,
};
use ahash::{HashSetExt, RandomState};
use saphyr_parser::ScalarStyle;
use serde::de::{self, Deserializer as _, IntoDeserializer, Visitor};
use std::borrow::Cow;
use std::collections::{HashSet, VecDeque};
use std::mem;

pub mod with_deserializer;
pub use with_deserializer::{
    with_deserializer_from_reader, with_deserializer_from_reader_with_options,
    with_deserializer_from_slice, with_deserializer_from_slice_with_options,
    with_deserializer_from_str, with_deserializer_from_str_with_options,
};

type FastHashSet<T> = HashSet<T, RandomState>;

use crate::location::Locations;


/// Attach both reference and defined locations to an error if it doesn't already have one.
/// When both locations are known and different, creates an `AliasError` to report both.
/// This is used for errors occurring when deserializing aliased values.
#[inline]
fn attach_alias_locations_if_missing(
    err: Error,
    reference_location: Location,
    defined_location: Location,
) -> Error {
    if err.location().is_some() {
        return err;
    }

    // If both locations are known and different, create an AliasError to show both
    if reference_location != Location::UNKNOWN
        && defined_location != Location::UNKNOWN
        && reference_location != defined_location
    {
        Error::AliasError {
            msg: err.to_string(),
            locations: Locations {
                reference_location,
                defined_location,
            },
        }
    } else {
        // Fall back to single location (prefer reference, then defined)
        let loc = if reference_location != Location::UNKNOWN {
            reference_location
        } else {
            defined_location
        };
        err.with_location(loc)
    }
}

mod spanned_deser;

// Re-export moved Options and related enums from the options module to preserve
// the public path serde_saphyr::sf_serde::*.
pub use crate::options::{AliasLimits, DuplicateKeyPolicy, Options};
use crate::tags::SfTag;

#[cfg(any(feature = "garde", feature = "validator"))]
use crate::path_map::PathRecorder;

/// Small immutable runtime configuration that `YamlDeserializer` needs.
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
    /// Ignore !!binary for string
    pub(crate) ignore_binary_tag_for_string: bool,
    /// Do not take into String type that looks like number or boolean (require quoting)
    pub(crate) no_schema: bool,
}

impl Cfg {
    #[inline]
    #[allow(deprecated)]
    pub(crate) fn from_options(options: &Options) -> Self {
        Self {
            dup_policy: options.duplicate_keys,
            legacy_octal_numbers: options.legacy_octal_numbers,
            strict_booleans: options.strict_booleans,
            angle_conversions: options.angle_conversions,
            ignore_binary_tag_for_string: options.ignore_binary_tag_for_string,
            no_schema: options.no_schema,
        }
    }
}

/// Our simplified owned event kind that we feed into Serde.
#[derive(Clone, Debug)]
pub(crate) enum Ev<'a> {
    /// Scalar value from YAML (text), with optional tag and style.
    Scalar {
        value: Cow<'a, str>,
        tag: SfTag,
        raw_tag: Option<Cow<'a, str>>,
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
    /// The event have been taken from the array, with only location remaining. This should not
    /// appear in the event stream and reserved for internal usage withing container.
    Taken { location: Location },
}

impl Default for Ev<'_> {
    // Used for optimization
    fn default() -> Self {
        Ev::Taken {
            location: Location::UNKNOWN,
        }
    }
}

impl Ev<'_> {
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
            | Ev::MapEnd { location }
            | Ev::Taken { location } => *location,
        }
    }
}

fn simple_tagged_enum_name(raw_tag: &Option<Cow<'_, str>>, tag: &SfTag) -> Option<String> {
    if !matches!(tag, SfTag::Other) {
        return None;
    }

    let raw = raw_tag.as_deref()?;
    let mut candidate =
        if let Some(inner) = raw.strip_prefix("!<").and_then(|s| s.strip_suffix('>')) {
            inner
        } else {
            raw
        };

    if let Some(stripped) = candidate.strip_prefix("tag:yaml.org,2002:") {
        candidate = stripped;
    }

    candidate = candidate.trim_start_matches('!');

    if candidate.is_empty() || candidate.contains([':', '!']) {
        return None;
    }

    Some(candidate.to_owned())
}

/// Canonical fingerprint of a YAML node for duplicate-key detection.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
enum KeyFingerprint {
    /// Scalar fingerprint (value plus optional tag).
    Scalar { value: String, tag: SfTag },
    /// Sequence fingerprint (ordered fingerprints of children).
    Sequence(Vec<KeyFingerprint>),
    /// Mapping fingerprint (ordered list of `(key, value)` fingerprints).
    Mapping(Vec<(KeyFingerprint, KeyFingerprint)>),
    /// Should not be used, arises after taking the value away
    #[default]
    Default,
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

/// from_slice_multiple captured YAML node used to buffer keys/values and process merge keys.
///
/// Fields:
/// - `fingerprint`: canonical representation for duplicate detection.
/// - `events`: exact event slice that replays the node on demand.
/// - `location`: start location of the node (for diagnostics).
enum KeyNode<'a> {
    Fingerprinted {
        fingerprint: KeyFingerprint,
        events: Vec<Ev<'a>>,
        location: Location,
    },
    Scalar {
        events: Vec<Ev<'a>>,
        location: Location,
    },
}

impl<'a> KeyNode<'a> {
    fn fingerprint(&self) -> Cow<'_, KeyFingerprint> {
        match self {
            KeyNode::Fingerprinted { fingerprint, .. } => Cow::Borrowed(fingerprint),
            KeyNode::Scalar { events, .. } => {
                if let Some(Ev::Scalar { tag, value, .. }) = events.first() {
                    Cow::Owned(KeyFingerprint::Scalar {
                        tag: *tag,
                        value: value.to_string(),
                    })
                } else {
                    unreachable!()
                }
            }
        }
    }

    fn events(&self) -> &[Ev<'a>] {
        match self {
            KeyNode::Fingerprinted { events, .. } => events,
            KeyNode::Scalar { events, .. } => events,
        }
    }

    fn take_events(&mut self) -> Vec<Ev<'a>> {
        match self {
            KeyNode::Fingerprinted { events, .. } => mem::take(events),
            KeyNode::Scalar { events, .. } => mem::take(events),
        }
    }

    fn take_fingerprint(&mut self) -> KeyFingerprint {
        match self {
            KeyNode::Fingerprinted { fingerprint, .. } => mem::take(fingerprint),
            KeyNode::Scalar { .. } => self.fingerprint().into_owned(),
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

/// from_slice_multiple pending key/value pair to be injected into the current mapping.
///
/// Produced by:
/// - Merge (`<<`) processing and by scanning the current mapping fields.
struct PendingEntry<'a> {
    key: KeyNode<'a>,
    value: KeyNode<'a>,
    /// Where the key/value pair is referenced/used in YAML.
    ///
    /// For merge-derived entries, this is the `<<` entry location.
    reference_location: Location,
}

/// Return the span lengths of key and value for a one-entry map encoded in `events`.
/// The expected layout is: MapStart, <key node>, <value node>, MapEnd.
/// On success returns (key_start, key_end, val_start, val_end) as indices into events.
fn one_entry_map_spans<'a>(events: &[Ev<'a>]) -> Option<(usize, usize, usize, usize)> {
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
fn skip_one_node_len<'a>(events: &[Ev<'a>], mut i: usize) -> Option<usize> {
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
                    Ev::Taken { .. } => return None,
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
                    Ev::Taken { .. } => return None,
                }
                i += 1;
            }
            None
        }
        Ev::SeqEnd { .. } | Ev::MapEnd { .. } => None,
        Ev::Taken { .. } => None,
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
fn capture_node<'a>(ev: &mut dyn Events<'a>) -> Result<KeyNode<'a>, Error> {
    let Some(event) = ev.next()? else {
        return Err(Error::eof().with_location(ev.last_location()));
    };

    match event {
        Ev::Scalar {
            value,
            tag,
            raw_tag,
            style,
            anchor,
            location,
        } => {
            let scalar_ev = Ev::Scalar {
                value,
                tag,
                raw_tag,
                style,
                anchor,
                location,
            };
            Ok(KeyNode::Scalar {
                events: vec![scalar_ev],
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
                        let key_events = key.take_events();
                        let value_events = value.take_events();
                        events.reserve(key_events.len() + value_events.len());
                        events.extend(key_events);
                        events.extend(value_events);
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
        Ev::SeqEnd { location } | Ev::MapEnd { location } => {
            Err(Error::UnexpectedContainerEndWhileReadingKeyNode { location })
        }
        Ev::Taken { location } => Err(Error::unexpected("consumed event").with_location(location)),
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
        }) if tag == &SfTag::None && value.as_ref() == "<<"
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
fn pending_entries_from_events<'a>(
    events: Vec<Ev<'a>>,
    location: Location,
    reference_location: Location,
) -> Result<Vec<PendingEntry<'a>>, Error> {
    let mut replay = ReplayEvents::with_reference(events, reference_location);
    match replay.peek()? {
        Some(Ev::Scalar { value, style, .. }) if scalar_is_nullish(value.as_ref(), style) => {
            Ok(Vec::new())
        }
        Some(Ev::Scalar { location, .. }) => Err(Error::MergeValueNotMapOrSeqOfMaps {
            location: *location,
        }),
        Some(Ev::MapStart { .. }) => collect_entries_from_map(&mut replay, reference_location),
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
                        // Preserve per-element use-site location. If the element comes from alias
                        // replay (`*m1`), its events are definition-site, but `referenced` should
                        // point at the alias token.
                        let _ = replay.peek()?;
                        let element_ref_loc = replay.reference_location();
                        let mut element = capture_node(&mut replay)?;
                        batches.push(pending_entries_from_events(
                            element.take_events(),
                            element.location(),
                            element_ref_loc,
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
        Some(other) => Err(Error::MergeValueNotMapOrSeqOfMaps {
            location: other.location(),
        }),
        None => Err(Error::eof().with_location(location)),
    }
}

/// Expand a merge value node directly from a live `Events` source.
///
/// This is used for `<<: value` handling in streaming map deserialization. Unlike
/// `pending_entries_from_events` (which works over a pre-recorded buffer), this
/// function can preserve per-element use-site locations for sequence merges like:
///
/// ```yaml
/// <<: [*m1, *m2]
/// ```
///
/// because `Events::reference_location()` can still observe the alias token
/// locations while the replay injection frame is active.
fn pending_entries_from_live_events<'a>(
    ev: &mut dyn Events<'a>,
    merge_reference_location: Location,
) -> Result<Vec<PendingEntry<'a>>, Error> {
    match ev.peek()? {
        Some(Ev::Scalar { value, style, .. }) if scalar_is_nullish(value.as_ref(), style) => {
            let _ = ev.next()?;
            Ok(Vec::new())
        }
        Some(Ev::Scalar { location, .. }) => Err(Error::MergeValueNotMapOrSeqOfMaps {
            location: *location,
        }),
        Some(Ev::MapStart { .. }) => {
            let mut node = capture_node(ev)?;
            pending_entries_from_events(
                node.take_events(),
                node.location(),
                merge_reference_location,
            )
        }
        Some(Ev::SeqStart { .. }) => {
            let _ = ev.next()?; // consume SeqStart
            let mut batches = Vec::new();
            loop {
                match ev.peek()? {
                    Some(Ev::SeqEnd { .. }) => {
                        let _ = ev.next()?;
                        break;
                    }
                    Some(_) => {
                        let _ = ev.peek()?;
                        let element_ref_loc = ev.reference_location();
                        let mut element = capture_node(ev)?;
                        batches.push(pending_entries_from_events(
                            element.take_events(),
                            element.location(),
                            element_ref_loc,
                        )?);
                    }
                    None => return Err(Error::eof().with_location(ev.last_location())),
                }
            }
            let mut merged = Vec::new();
            while let Some(mut nested) = batches.pop() {
                merged.append(&mut nested);
            }
            Ok(merged)
        }
        Some(other) => Err(Error::MergeValueNotMapOrSeqOfMaps {
            location: other.location(),
        }),
        None => Err(Error::eof().with_location(ev.last_location())),
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
fn collect_entries_from_map<'a>(
    ev: &mut dyn Events<'a>,
    reference_location: Location,
) -> Result<Vec<PendingEntry<'a>>, Error> {
    let Some(Ev::MapStart { .. }) = ev.next()? else {
        return Err(Error::MergeValueNotMapOrSeqOfMaps {
            location: ev.last_location(),
        });
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
                    // Preserve where the merge value is referenced (use-site). For alias merges
                    // inside merged mappings, node locations point at the anchored mapping, but
                    // we want `referenced` to point at the alias token.
                    let _ = ev.peek()?;
                    let merge_ref_loc = ev.reference_location();
                    merges.push(pending_entries_from_live_events(ev, merge_ref_loc)?);
                } else {
                    let value = capture_node(ev)?;
                    fields.push(PendingEntry {
                        key,
                        value,
                        reference_location,
                    });
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

/// from_slice_multiple location-free representation of events for duplicate-key comparison.
/// Source of events with lookahead and alias-injection.
pub(crate) trait Events<'de> {
    /// Pull the next event from the stream.
    ///
    /// Returns:
    /// - `Ok(Some(Ev))` for a real event,
    /// - `Ok(None)` at true end-of-stream,
    /// - `Err(Error)` on parser/structure failure.
    ///
    /// Called by:
    /// - The streaming deserializer (`Deser`) and helper scanners.
    fn next(&mut self) -> Result<Option<Ev<'de>>, Error>;

    /// Peek at the next event without consuming it.
    ///
    /// Returns:
    /// - `Ok(Some(&Ev))` with the even reference
    /// - `Ok(None)` at end-of-stream,
    /// - `Err(Error)` on error.
    ///
    /// Called by:
    /// - Lookahead logic (merge, container boundaries, option/unit handling).
    fn peek(&mut self) -> Result<Option<&Ev<'de>>, Error>;

    /// Last location that `next` or `peek` has observed.
    ///
    /// Used by:
    /// - Error paths to attach a reasonable position when nothing else is available.
    fn last_location(&self) -> Location;

    /// Location of the *reference* to the next node (use-site).
    ///
    /// This is the key primitive that enables `Spanned<T>` to report two different
    /// locations:
    /// - **referenced**: where the value is *used* in the YAML (the use-site)
    /// - **defined**: where the value is *defined* (the definition-site; typically
    ///   the node's own [`Ev::location`])
    ///
    /// Contract
    /// - For a normal (non-alias) stream, `reference_location()` should be the
    ///   same as `peek()?.map(|ev| ev.location())`.
    /// - While replaying an alias (`*a`), the *events* come from the anchored
    ///   definition buffer, so their `Ev::location()` points at the definition-site.
    ///   In that situation, `reference_location()` must instead return the location
    ///   of the alias token `*a` (the use-site), so callers can attribute values to
    ///   where they were referenced.
    /// - During merge expansion (`<<: *m`), merge-derived entries should also
    ///   carry a use-site location (usually the `<<` entry / alias token) even
    ///   though the actual scalar nodes being replayed come from the merged mapping.
    ///
    /// Subtlety: this method is used *together with* `peek()`.
    /// Consumers typically do `peek()` (to ensure the next node is available), then
    /// call `reference_location()` and/or `Ev::location()` for the same node.
    /// Implementations therefore must keep the necessary context alive at least
    /// until the node is consumed.
    fn reference_location(&self) -> Location;

    /// Get the original input string for zero-copy borrowing.
    ///
    /// Returns `Some(&str)` when the input is available for borrowing (string-based parsing),
    /// or `None` when borrowing is not possible (reader-based parsing or replay buffers).
    ///
    /// Used by:
    /// - The deserializer to return borrowed `&str` references when possible.
    ///
    /// Note: This method is part of the zero-copy deserialization infrastructure.
    /// It will be used when full `Deserialize<'de>` support is implemented.
    #[allow(dead_code)]
    fn input_for_borrowing(&self) -> Option<&'de str> {
        None // Default: borrowing not supported
    }
}

/// Event source that replays a pre-recorded buffer.
struct ReplayEvents<'a> {
    buf: Vec<Ev<'a>>,
    /// Index of the next event to yield (0..=buf.len()).
    idx: usize,
    /// Optional override for the reference location (use-site) of the next node.
    /// When we replay a captured subtree (e.g. an anchored mapping) we often want to
    /// preserve *where it was referenced*, not just where it was originally defined.
    ///
    /// Scope/when it applies
    /// - The override is used by [`Events::reference_location`].
    /// - It is intended to apply to the node currently at `idx` (i.e. the node visible via
    ///   `peek()`), and is typically kept for the whole replay.
    /// - `next()` does not clear it: callers that need different reference locations for
    ///   different nested nodes should create nested replay sources (which we do during
    ///   recursive merge expansion).
    ref_override: Option<Location>,
}

impl<'a> ReplayEvents<'a> {
    /// Create a replay source over `buf`, initially positioned at index 0.
    ///
    /// Arguments:
    /// - `buf`: previously captured events.
    ///
    /// Called by:
    /// - Merge expansion and recorded key/value deserialization.
    fn new(buf: Vec<Ev<'a>>) -> Self {
        Self {
            buf,
            idx: 0,
            ref_override: None,
        }
    }

    /// Create a replay source over `buf` with a fixed reference (use-site) location.
    ///
    /// This is primarily used when a recorded node is replayed in a *different place*
    /// than where it was defined:
    /// - alias replay (`*a`) where the replayed events come from the anchor definition,
    ///   but `Spanned<T>.referenced` should point at the alias token.
    /// - merge expansion (`<<: *m`) where merge-derived fields should point at the merge
    ///   entry (use-site) even though the actual events come from the merged mapping.
    ///
    /// Note that this does not change the events themselves: `Ev::location()` still
    /// points to where each event was originally produced/captured (definition-site).
    /// The override only affects [`Events::reference_location`].
    fn with_reference(buf: Vec<Ev<'a>>, reference: Location) -> Self {
        Self {
            buf,
            idx: 0,
            ref_override: Some(reference),
        }
    }
}

impl<'a> Events<'a> for ReplayEvents<'a> {
    /// See [`Events::next`]. Replays and advances the internal index.
    fn next(&mut self) -> Result<Option<Ev<'a>>, Error> {
        if self.idx >= self.buf.len() {
            return Ok(None);
        }
        let location = self.buf[self.idx].location();
        // Flag as taken to avoid unexpected reuse.
        let ev = mem::replace(&mut self.buf[self.idx], Ev::Taken { location });
        self.idx += 1;
        Ok(Some(ev))
    }

    fn peek(&mut self) -> Result<Option<&Ev<'a>>, Error> {
        Ok(self.buf.get(self.idx))
    }

    fn last_location(&self) -> Location {
        let last = self.idx.saturating_sub(1);
        self.buf
            .get(last)
            .map(|e| e.location())
            .unwrap_or(Location::UNKNOWN)
    }

    fn reference_location(&self) -> Location {
        if let Some(loc) = self.ref_override {
            return loc;
        }
        self.buf
            .get(self.idx)
            .map(|e| e.location())
            .unwrap_or_else(|| self.last_location())
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
    ev: &'e mut dyn Events<'de>,
    cfg: Cfg,
    /// True when deserializing a map key.
    in_key: bool,
    /// True when the recorded key node was exactly an empty mapping (MapStart followed by MapEnd).
    key_empty_map_node: bool,

    #[cfg(any(feature = "garde", feature = "validator"))]
    garde: Option<&'e mut PathRecorder>,
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
            key_empty_map_node: false,

            #[cfg(any(feature = "garde", feature = "validator"))]
            garde: None,
        }
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
            key_empty_map_node: false,
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
        match self.ev.next()? {
            Some(Ev::Scalar {
                value,
                tag,
                location,
                ..
            }) => Ok((value.into_owned(), tag, location)),
            Some(other) => Err(Error::unexpected("string scalar").with_location(other.location())),
            None => Err(Error::eof().with_location(self.ev.last_location())),
        }
    }

    /// Consume the next scalar event and return it without allocating a new `String` (if possible).
    ///
    /// This keeps the scalar text in its existing `Cow` container, which is cheap to clone
    /// and allows primitive parsers (bool/int/float/char) to work directly on `&str`.
    fn take_scalar_cow_event(&mut self) -> Result<(Cow<'de, str>, SfTag, Location), Error> {
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

    /// Consume a scalar and return it without allocating a `String`.
    fn take_scalar_cow_with_location(&mut self) -> Result<(Cow<'de, str>, SfTag, Location), Error> {
        let (value, tag, location) = self.take_scalar_cow_event()?;
        Ok((value, tag, location))
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
            let text = String::from_utf8(data)
                .map_err(|_| Error::BinaryNotUtf8 { location })?;
            return Ok(text);
        }

        // For non-binary, ensure the tag allows string deserialization.
        if !tag.can_parse_into_string()
            && tag != SfTag::NonSpecific
            && !(self.cfg.ignore_binary_tag_for_string && tag == SfTag::Binary)
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

impl<'de, 'e> de::Deserializer<'de> for YamlDeserializer<'de, 'e> {
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
                tag,
                style,
                value,
                ..
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
                    // For string-ish scalars, rely on the parser's own zero-copy capability:
                    // if the scalar is returned as `Cow::Borrowed`, we can pass it through.
                    // Otherwise we fall back to owning.
                    if *tag == SfTag::Binary && !self.cfg.ignore_binary_tag_for_string {
                        return visitor.visit_string(self.take_string_scalar()?);
                    }
                    if !tag.can_parse_into_string()
                        && *tag != SfTag::NonSpecific
                        && !(self.cfg.ignore_binary_tag_for_string && *tag == SfTag::Binary)
                    {
                        let location = self.ev.peek()?.unwrap().location();
                        return Err(Error::TaggedScalarCannotDeserializeIntoString { location });
                    }

                    let (cow, _tag2, _location) = self.take_scalar_cow_event()?;
                    return match cow {
                        Cow::Borrowed(b) => visitor.visit_borrowed_str(b),
                        Cow::Owned(s) => visitor.visit_string(s),
                    };
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
                Err(Error::UnexpectedSequenceEnd { location: *location })
            }
            Some(Ev::MapEnd { location }) => {
                Err(Error::UnexpectedMappingEnd { location: *location })
            }
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
        }
    }

    /// Parse a YAML 1.1 boolean literal into `bool`.
    ///
    /// Caller: Serde when target expects `bool`.
    /// Flow: scalar text → `Visitor::visit_bool`.
    fn deserialize_bool<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_cow_with_location()?;
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
            parse_yaml11_bool(s)
                .map_err(|_| Error::InvalidScalar { ty: "boolean", location })?
        };
        visitor.visit_bool(b)
    }

    /// Parse a signed 8-bit integer.
    fn deserialize_i8<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_cow_with_location()?;
        let v: i8 = parse_int_signed(s.as_ref(), "i8", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i8(v)
    }
    /// Parse a signed 16-bit integer.
    fn deserialize_i16<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_cow_with_location()?;
        let v: i16 = parse_int_signed(s.as_ref(), "i16", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i16(v)
    }
    /// Parse a signed 32-bit integer.
    fn deserialize_i32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_cow_with_location()?;
        let v: i32 = parse_int_signed(s.as_ref(), "i32", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i32(v)
    }
    /// Parse a signed 64-bit integer.
    fn deserialize_i64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_cow_with_location()?;
        let v: i64 = parse_int_signed(s.as_ref(), "i64", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i64(v)
    }
    /// Parse a signed 128-bit integer.
    fn deserialize_i128<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_cow_with_location()?;
        let v: i128 = parse_int_signed(s.as_ref(), "i128", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_i128(v)
    }

    /// Parse an unsigned 8-bit integer.
    fn deserialize_u8<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_cow_with_location()?;
        let v: u8 = parse_int_unsigned(s.as_ref(), "u8", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u8(v)
    }
    /// Parse an unsigned 16-bit integer.
    fn deserialize_u16<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_cow_with_location()?;
        let v: u16 = parse_int_unsigned(s.as_ref(), "u16", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u16(v)
    }
    /// Parse an unsigned 32-bit integer.
    fn deserialize_u32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_cow_with_location()?;
        let v: u32 = parse_int_unsigned(s.as_ref(), "u32", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u32(v)
    }
    /// Parse an unsigned 64-bit integer.
    fn deserialize_u64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_cow_with_location()?;
        let v: u64 = parse_int_unsigned(s.as_ref(), "u64", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u64(v)
    }
    /// Parse an unsigned 128-bit integer.
    fn deserialize_u128<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, _tag, location) = self.take_scalar_cow_with_location()?;
        let v: u128 = parse_int_unsigned(s.as_ref(), "u128", location, self.cfg.legacy_octal_numbers)?;
        visitor.visit_u128(v)
    }

    /// Parse a 32-bit float (supports YAML 1.2 `+.inf`, `-.inf`, `.nan`).
    fn deserialize_f32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_with_location()?;
        let v: f32 = parse_yaml12_float(s.as_ref(), location, tag, self.cfg.angle_conversions)?;
        visitor.visit_f32(v)
    }
    /// Parse a 64-bit float (supports YAML 1.2 `+.inf`, `-.inf`, `.nan`).
    fn deserialize_f64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Self::Error> {
        let (s, tag, location) = self.take_scalar_cow_with_location()?;
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
        if let Some(Ev::Scalar {
            tag, style, value, ..
        }) = self.ev.peek()?
            && tag != &SfTag::String
        {
            // Reject YAML null for char (allow quoted values like "null").
            if tag == &SfTag::Null || scalar_is_nullish(value, style) {
                let (_value, _tag, location) = self.take_scalar_event()?;
                return Err(Error::InvalidCharNull { location });
            } else if self.cfg.no_schema && maybe_not_string(value, style) {
                // Require quoting for ambiguous plain scalars in no_schema mode.
                let (value, _tag, location) = self.take_scalar_event()?;
                return Err(Error::quoting_required(&value).with_location(location));
            }
        }

        // Now consume the scalar and validate it contains exactly one Unicode scalar value.
        let (s, _tag, location) = self.take_scalar_cow_with_location()?;
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
        let location = match self.ev.peek()? {
            Some(Ev::Scalar {
                location,
                tag,
                style,
                value,
                ..
            }) => {
                // Check for null - not valid for string deserialization
                if tag == &SfTag::Null || scalar_is_nullish(value, style) {
                    let loc = *location;
                    let _ = self.ev.next()?;
                    return Err(Error::NullIntoString { location: loc });
                }
                *location
            }
            Some(other) => {
                return Err(Error::unexpected("string scalar").with_location(other.location()));
            }
            None => return Err(Error::eof().with_location(self.ev.last_location())),
        };

        // Consume scalar and rely on the parser's own borrowing promise:
        // `Cow::Borrowed` indicates the scalar exists verbatim in the backing input.
        let (cow, _tag, _loc) = self.take_scalar_cow_with_location()?;
        if let Cow::Borrowed(b) = cow {
            return visitor.visit_borrowed_str(b);
        }

        // Fall back to owned string. If the caller required a borrowed string (`&str`),
        // convert the generic error into our richer message.
        let cannot_borrow_reason = if self.ev.input_for_borrowing().is_none() {
            TransformReason::InputNotBorrowable
        } else {
            TransformReason::ParserReturnedOwned
        };

        let res: Result<V::Value, Self::Error> = visitor.visit_string(cow.into_owned());
        match res {
            Ok(v) => Ok(v),
            Err(err) => {
                let msg = err.to_string();
                if msg.contains("expected a borrowed string") {
                    return Err(
                        Error::cannot_borrow_transformed(cannot_borrow_reason)
                            .with_location(location),
                    );
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
        let _location = if let Some(Ev::Scalar {
            tag,
            style,
            value,
            location,
            ..
        }) = self.ev.peek()?
        {
             // If explicitly tagged as null, or plain null-like, this is not a valid String.
             if (tag == &SfTag::Null || scalar_is_nullish(value, style)) && tag != &SfTag::String {
                // Consume the scalar to anchor the error at the correct location.
                let (_value, _tag, location) = self.take_scalar_event()?;
                return Err(Error::NullIntoString { location });
            } else if self.cfg.no_schema && maybe_not_string(value, style) && tag != &SfTag::String {
                // Consume the scalar to anchor the error at the correct location.
                let (value, _tag, location) = self.take_scalar_event()?;
                return Err(Error::quoting_required(&value).with_location(location));
            }
            *location
        } else {
             // Let take_string_scalar handle the error if it's not a scalar
             return visitor.visit_string(self.take_string_scalar()?);
        };

        // Prefer `Cow::Borrowed` when the parser can provide it. Keep `!!binary` semantics.
        if let Some(Ev::Scalar { tag, .. }) = self.ev.peek()?
        {
            if *tag == SfTag::Binary && !self.cfg.ignore_binary_tag_for_string {
                return visitor.visit_string(self.take_string_scalar()?);
            }
            if !tag.can_parse_into_string()
                && *tag != SfTag::NonSpecific
                && !(self.cfg.ignore_binary_tag_for_string && *tag == SfTag::Binary)
            {
                let location = self.ev.peek()?.unwrap().location();
                return Err(Error::TaggedScalarCannotDeserializeIntoString { location });
            }
        }

        let (cow, _tag, _loc) = self.take_scalar_cow_with_location()?;
        match cow {
            Cow::Borrowed(b) => visitor.visit_borrowed_str(b),
            Cow::Owned(s) => visitor.visit_string(s),
        }
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
                            let b: u8 = <u8 as serde::Deserialize>::deserialize(
                                YamlDeserializer::new(self.ev, self.cfg),
                            )?;
                            out.push(b);
                        }
                        None => return Err(Error::eof().with_location(self.ev.last_location())),
                    }
                }
                visitor.visit_byte_buf(out)
            }

            // Scalar without binary tag → reject
            Some(Ev::Scalar { location, .. }) => {
                Err(Error::BytesNotSupportedMissingBinaryTag {
                    location: *location,
                })
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
                Err(Error::UnexpectedValueForUnit {
                    location: other.location(),
                })
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
                    Some(other) => Err(Error::ExpectedEmptyMappingForUnitStruct {
                        location: other.location(),
                    }),
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
            // Internal wrapper types use `__yaml_*` names (see `__yaml_rc_anchor`, etc.).
            "__yaml_spanned" => spanned_deser::deserialize_yaml_spanned(self, visitor),
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
        struct SA<'de, 'e> {
            ev: &'e mut dyn Events<'de>,
            cfg: Cfg,

            #[cfg(any(feature = "garde", feature = "validator"))]
            garde: Option<&'e mut PathRecorder>,
            #[cfg(any(feature = "garde", feature = "validator"))]
            idx: usize,
        }
        impl<'de, 'e> de::SeqAccess<'de> for SA<'de, 'e> {
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
                        None => return Err(Error::eof().with_location(self.ev.last_location())),
                    }
                };

                if is_end {
                    return Ok(None);
                }

                // The peek borrow is now released, so it's safe to query other cursor state.
                let reference_location = self.ev.reference_location();
                let _missing_field_guard = MissingFieldLocationGuard::new(reference_location);

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

                        let de =
                            YamlDeserializer::new_with_path_recorder(self.ev, self.cfg, recorder);
                        let res = seed
                            .deserialize(de)
                            .map(Some)
                            .map_err(|e| attach_alias_locations_if_missing(e, reference_location, defined_location));

                        recorder.current = prev;
                        self.idx += 1;
                        return res;
                    }
                }

                let de = YamlDeserializer::new(self.ev, self.cfg);
                seed.deserialize(de)
                    .map(Some)
                    .map_err(|e| attach_alias_locations_if_missing(e, reference_location, defined_location))
            }
        }

        #[cfg(any(feature = "garde", feature = "validator"))]
        let garde = self.garde;

        let result = visitor.visit_seq(SA {
            ev: self.ev,
            cfg: self.cfg,

            #[cfg(any(feature = "garde", feature = "validator"))]
            garde,
            #[cfg(any(feature = "garde", feature = "validator"))]
            idx: 0,
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
            && (tag == &SfTag::Null || scalar_is_nullish(s, style))
        {
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
            seen: FastHashSet<KeyFingerprint>,
            pending: VecDeque<PendingEntry<'de>>,
            merge_stack: Vec<Vec<PendingEntry<'de>>>,
            flushing_merges: bool,
            pending_value: Option<(Vec<Ev<'de>>, Location)>,
        }

        impl<'de, 'e> MA<'de, 'e> {
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
                        return Err(Error::UnexpectedContainerEndWhileSkippingNode { location });
                    }
                    Some(Ev::Taken { location }) => {
                        return Err(Error::unexpected("consumed event").with_location(location));
                    }
                    None => return Err(Error::eof().with_location(self.ev.last_location())),
                }
                while depth != 0 {
                    match self.ev.next()? {
                        Some(Ev::SeqStart { .. }) | Some(Ev::MapStart { .. }) => depth += 1,
                        Some(Ev::SeqEnd { .. }) | Some(Ev::MapEnd { .. }) => depth -= 1,
                        Some(Ev::Scalar { .. }) => {}
                        Some(Ev::Taken { location }) => {
                            return Err(Error::unexpected("consumed event").with_location(location));
                        }
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
            fn deserialize_recorded_key<'de2, K>(
                &mut self,
                seed: K,
                events: Vec<Ev<'de2>>,
                kemn: bool,
            ) -> Result<K::Value, Error>
            where
                K: de::DeserializeSeed<'de2>,
            {
                let mut replay = ReplayEvents::new(events);

                // Get location from replay events for error reporting.
                let location = replay.reference_location();

                let de = YamlDeserializer::<'de2, '_> {
                    ev: &mut replay,
                    cfg: self.cfg,
                    in_key: true,
                    key_empty_map_node: kemn,

                    #[cfg(any(feature = "garde", feature = "validator"))]
                    garde: None,
                };
                seed.deserialize(de)
                    .map_err(|e| if e.location().is_none() { e.with_location(location) } else { e })
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

        impl<'de, 'e> de::MapAccess<'de> for MA<'de, 'e> {
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
                        } = entry;
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
                                        let key = fingerprint
                                            .stringy_scalar_value()
                                            .map(|s| s.to_owned());
                                        return Err(Error::DuplicateMappingKey { key, location });
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
                        if !kemn
                            && let KeyFingerprint::Mapping(ref pairs) = fingerprint
                            && pairs.len() == 1
                            && let (
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
                                if let Some((_ks, _ke, vs, ve)) = one_entry_map_spans(&events) {
                                    // Replace value_events with inner value events and key events with empty map
                                    value_events = events.drain(vs..ve).collect();
                                    // Build empty map events using the first and last from original events
                                    let start = match events.first() {
                                        Some(Ev::MapStart { anchor, location }) => Ev::MapStart {
                                            anchor: *anchor,
                                            location: *location,
                                        },
                                        Some(other) => other.clone(),
                                        None => {
                                            return Err(Error::unexpected("mapping start")
                                                .with_location(location));
                                        }
                                    };
                                    let end = match events.last() {
                                        Some(Ev::MapEnd { location }) => Ev::MapEnd {
                                            location: *location,
                                        },
                                        Some(other) => other.clone(),
                                        None => {
                                            return Err(Error::unexpected("mapping end")
                                                .with_location(location));
                                        }
                                    };
                                    events = vec![start, end];
                                    kemn = true;
                                }
                            }
                        }
                        let key_seed = match seed.take() {
                            Some(s) => s,
                            None => {
                                return Err(Error::InternalSeedReusedForMapKey { location });
                            }
                        };
                        let key_value = self.deserialize_recorded_key(key_seed, events, kemn)?;
                        self.have_key = true;
                        self.pending_value = Some((value_events, reference_location));

                        // Keep the current fallback location set to the key span so Serde can
                        // attribute errors like `unknown_field` to the offending key.
                        self.fallback_guard = Some(MissingFieldLocationGuard::new(location));

                        #[cfg(any(feature = "garde", feature = "validator"))]
                        {
                            self.pending_path_segment =
                                fingerprint.stringy_scalar_value().map(|s| s.to_owned());
                        }

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
                                // Preserve where the merge value is *referenced* (use-site).
                                // For alias merges (`<<: *m`), `key_node`/`value_node` locations
                                // will point at the anchored mapping, but we want `referenced`
                                // to point at the alias token.
                                let _ = self.ev.peek()?;
                                let merge_ref_loc = self.ev.reference_location();
                                let entries =
                                    pending_entries_from_live_events(self.ev, merge_ref_loc)?;
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
                                // IMPORTANT: preserve where the value is *referenced* (use-site).
                                // If the value is an alias (`*a`), `capture_node` will record events
                                // from the anchor definition, so `value_node.location()` would point
                                // at the definition-site. `Spanned<T>` wants the alias token location
                                // in `referenced`, so take it from `Events::reference_location()`.
                                let _ = self.ev.peek()?;
                                let reference_location = self.ev.reference_location();
                                let value_node = capture_node(self.ev)?;
                                self.enqueue_entries(vec![PendingEntry {
                                    key: key_node,
                                    value: value_node,
                                    reference_location,
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
                                        return Err(Error::InternalSeedReusedForMapKey { location });
                                    }
                                };
                                let key_value =
                                    self.deserialize_recorded_key(key_seed, events, kemn_direct)?;
                                self.have_key = true;
                                self.pending_value = None; // value will be read live

                                // Same as the buffered path above: keep the key location alive
                                // across the caller boundary.
                                self.fallback_guard =
                                    Some(MissingFieldLocationGuard::new(location));

                                #[cfg(any(feature = "garde", feature = "validator"))]
                                {
                                    self.pending_path_segment =
                                        fingerprint.stringy_scalar_value().map(|s| s.to_owned());
                                }

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
                    return Err(Error::ValueRequestedBeforeKey {
                        location: self.ev.last_location(),
                    });
                }
                self.have_key = false;

                #[cfg(any(feature = "garde", feature = "validator"))]
                let pending_segment = self.pending_path_segment.take();

                if let Some(events) = self.pending_value.take() {
                    let (events, reference_location) = events;
                    let mut replay = ReplayEvents::with_reference(events, reference_location);

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

                            let de = YamlDeserializer::new_with_path_recorder(
                                &mut replay,
                                self.cfg,
                                recorder,
                            );
                            let res = seed
                                .deserialize(de)
                                .map_err(|e| attach_alias_locations_if_missing(e, reference_location, defined_location));
                            recorder.current = prev;
                            return res;
                        }
                    }

                    let de = YamlDeserializer::new(&mut replay, self.cfg);
                    seed.deserialize(de)
                        .map_err(|e| attach_alias_locations_if_missing(e, reference_location, defined_location))
                } else {
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

                            let de = YamlDeserializer::new_with_path_recorder(
                                self.ev, self.cfg, recorder,
                            );
                            let res = seed
                                .deserialize(de)
                                .map_err(|e| attach_alias_locations_if_missing(e, reference_location, defined_location));
                            recorder.current = prev;
                            return res;
                        }
                    }

                    let de = YamlDeserializer::new(self.ev, self.cfg);
                    seed.deserialize(de)
                        .map_err(|e| attach_alias_locations_if_missing(e, reference_location, defined_location))
                }
            }
        }

        #[cfg(any(feature = "garde", feature = "validator"))]
        let garde = self.garde;

        visitor.visit_map(MA {
            ev: self.ev,
            cfg: self.cfg,
            have_key: false,

            fallback_guard: None,

            #[cfg(any(feature = "garde", feature = "validator"))]
            garde,
            #[cfg(any(feature = "garde", feature = "validator"))]
            pending_path_segment: None,

            seen: FastHashSet::with_capacity(8),
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
            Unit(String, Location),
            Map(String, Location),
        }

        let mut tagged_enum = None;

        let mode = match self.ev.peek()? {
            Some(Ev::Scalar {
                tag,
                style,
                value,
                raw_tag,
                location,
                ..
            }) => {
                if let Some(tag_name) = simple_tagged_enum_name(raw_tag, tag) {
                    tagged_enum = Some((tag_name, *location));
                }
                if self.cfg.no_schema && *tag != SfTag::String && maybe_not_string(value, style) {
                    let (v, _t, loc) = self.take_scalar_event()?;
                    return Err(Error::quoting_required(&v).with_location(loc));
                }
                let (value, _tag, loc) = self.take_scalar_event()?;
                Mode::Unit(value, loc)
            }
            Some(Ev::MapStart { .. }) => {
                self.expect_map_start()?;
                match self.ev.next()? {
                    Some(Ev::Scalar {
                        value,
                        tag,
                        style,
                        location,
                        ..
                    }) => {
                        if self.cfg.no_schema
                            && tag != SfTag::String
                            && maybe_not_string(&value, &style)
                        {
                            return Err(Error::quoting_required(&value).with_location(location));
                        }
                        Mode::Map(value.to_string(), location)
                    }
                    Some(other) => {
                        return Err(Error::ExpectedStringKeyForExternallyTaggedEnum {
                            location: other.location(),
                        });
                    }
                    None => return Err(Error::eof().with_location(self.ev.last_location())),
                }
            }
            Some(Ev::SeqStart { location, .. }) => {
                return Err(
                    Error::ExternallyTaggedEnumExpectedScalarOrMapping { location: *location },
                );
            }
            Some(Ev::SeqEnd { location }) => {
                return Err(Error::UnexpectedSequenceEnd { location: *location });
            }
            Some(Ev::MapEnd { location }) => {
                return Err(Error::UnexpectedMappingEnd { location: *location });
            }
            Some(Ev::Taken { location }) => {
                return Err(Error::unexpected("consumed event").with_location(*location));
            }
            None => return Err(Error::eof().with_location(self.ev.last_location())),
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
            variant: String,
            map_mode: bool,
            variant_location: Location,
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
                    variant_location,
                } = self;
                let v = seed.deserialize(variant.into_deserializer()).map_err(
                    |err: serde::de::value::Error| {
                        Error::SerdeVariantId {
                            msg: err.to_string(),
                            location: variant_location,
                        }
                    },
                )?;
                Ok((v, VA { ev, cfg, map_mode }))
            }
        }

        struct VA<'de, 'e> {
            ev: &'e mut dyn Events<'de>,
            cfg: Cfg,
            map_mode: bool,
        }

        impl<'de, 'e> VA<'de, 'e> {
            /// In map mode (`{ Variant: ... }`) ensure the closing `}` is present.
            fn expect_map_end(&mut self) -> Result<(), Error> {
                match self.ev.next()? {
                    Some(Ev::MapEnd { .. }) => Ok(()),
                    Some(other) => Err(Error::ExpectedMappingEndAfterEnumVariantValue {
                        location: other.location(),
                    }),
                    None => Err(Error::eof().with_location(self.ev.last_location())),
                }
            }
        }

        impl<'de, 'e> de::VariantAccess<'de> for VA<'de, 'e> {
            type Error = Error;

            /// Handle unit variants: `Variant` or `{ Variant: null/~ }`.
            fn unit_variant(mut self) -> Result<(), Error> {
                if self.map_mode {
                    match self.ev.peek()? {
                        Some(Ev::MapEnd { .. }) => {
                            let _ = self.ev.next()?;
                            Ok(())
                        }
                        Some(Ev::Scalar { value: s, style, .. }) if scalar_is_nullish(s, style) => {
                            let _ = self.ev.next()?; // consume the null-like scalar
                            self.expect_map_end()
                        }
                        Some(other) => Err(Error::UnexpectedValueForUnitEnumVariant {
                            location: other.location(),
                        }),
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
                // Get locations for error reporting before deserializing.
                let defined_location = self
                    .ev
                    .peek()?
                    .map(|ev: &Ev| ev.location())
                    .unwrap_or_else(|| self.ev.last_location());
                let reference_location = self.ev.reference_location();

                let value = seed
                    .deserialize(YamlDeserializer::new(self.ev, self.cfg))
                    .map_err(|e| attach_alias_locations_if_missing(e, reference_location, defined_location))?;
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

        let access = match mode {
            Mode::Unit(variant, variant_location) => EA {
                ev: self.ev,
                cfg: self.cfg,
                variant,
                map_mode: false,
                variant_location,
            },
            Mode::Map(variant, variant_location) => EA {
                ev: self.ev,
                cfg: self.cfg,
                variant,
                map_mode: true,
                variant_location,
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
