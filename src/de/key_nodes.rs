use std::borrow::Cow;
#[cfg(feature = "properties")]
use std::collections::HashMap;
use std::collections::HashSet;
use std::mem;
#[cfg(feature = "properties")]
use std::rc::Rc;

use granit_parser::ScalarStyle;

use super::error::Error;
use super::events::{Ev, Events, ReplayEvents};
#[cfg(feature = "properties")]
use super::options::PropertySyntax;
use super::options::{DuplicateKeyPolicy, MergeKeyPolicy};
use super::tags::SfTag;
use crate::location::Location;
use crate::parse_scalars::scalar_is_nullish;

pub(super) fn simple_tagged_enum_name(
    raw_tag: &Option<Cow<'_, str>>,
    tag: &SfTag,
) -> Option<String> {
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
pub(super) enum KeyFingerprint<'a> {
    /// Scalar fingerprint (value plus optional tag).
    Scalar { value: Cow<'a, str>, tag: SfTag },
    /// Sequence fingerprint (ordered fingerprints of children).
    Sequence(Vec<KeyFingerprint<'a>>),
    /// Mapping fingerprint (ordered list of `(key, value)` fingerprints).
    Mapping(Vec<(KeyFingerprint<'a>, KeyFingerprint<'a>)>),
    /// Should not be used, arises after taking the value away
    #[default]
    Default,
}

pub(super) fn canonical_scalar_key_tag(tag: SfTag) -> SfTag {
    if tag.can_parse_into_string() {
        SfTag::String
    } else {
        tag
    }
}

pub(super) fn is_empty_mapping_key_fingerprint(fingerprint: &KeyFingerprint<'_>) -> bool {
    matches!(fingerprint, KeyFingerprint::Mapping(pairs) if pairs.is_empty())
}

fn is_nullish_scalar_key_fingerprint(fingerprint: &KeyFingerprint<'_>) -> bool {
    match fingerprint {
        KeyFingerprint::Scalar { value, tag } => {
            *tag == SfTag::Null
                || value.is_empty()
                || value == "~"
                || value.eq_ignore_ascii_case("null")
        }
        _ => false,
    }
}

pub(super) fn is_one_entry_nullish_mapping_key_fingerprint(
    fingerprint: &KeyFingerprint<'_>,
) -> bool {
    match fingerprint {
        KeyFingerprint::Mapping(pairs) if pairs.len() == 1 => {
            is_nullish_scalar_key_fingerprint(&pairs[0].0)
        }
        _ => false,
    }
}

impl KeyFingerprint<'_> {
    /// If this fingerprint represents a string-like scalar, return its value.
    ///
    /// Returns:
    /// - `Some(&str)` when the scalar can be parsed into string (and is not `!!binary`).
    /// - `None` for non-string scalars or containers.
    ///
    /// Used by:
    /// - Error messages to print a friendly duplicate key like `duplicate mapping key: foo`.
    pub(super) fn stringy_scalar_value(&self) -> Option<&str> {
        match self {
            KeyFingerprint::Scalar { value, tag } => {
                if tag.can_parse_into_string() && tag != &SfTag::Binary {
                    Some(value.as_ref())
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// `from_slice_multiple` captured YAML node used to buffer keys/values and process merge keys.
///
/// Fields:
/// - `fingerprint`: canonical representation for duplicate detection.
/// - `events`: exact event slice that replays the node on demand.
/// - `location`: start location of the node (for diagnostics).
///
/// Comments are not part of `KeyNode`; callers that buffer a node and later
/// replay it must capture any relevant comment metadata alongside the node.
pub(super) enum KeyNode<'a> {
    Fingerprinted {
        fingerprint: KeyFingerprint<'a>,
        events: Vec<Ev<'a>>,
        location: Location,
    },
    Scalar {
        events: Vec<Ev<'a>>,
        location: Location,
    },
}

impl<'a> KeyNode<'a> {
    pub(super) fn fingerprint(&self) -> Cow<'_, KeyFingerprint<'a>> {
        match self {
            KeyNode::Fingerprinted { fingerprint, .. } => Cow::Borrowed(fingerprint),
            KeyNode::Scalar { events, .. } => {
                if let Some(Ev::Scalar { tag, value, .. }) = events.first() {
                    Cow::Owned(KeyFingerprint::Scalar {
                        tag: canonical_scalar_key_tag(*tag),
                        value: value.clone(),
                    })
                } else {
                    unreachable!()
                }
            }
        }
    }

    pub(super) fn events(&self) -> &[Ev<'a>] {
        match self {
            KeyNode::Fingerprinted { events, .. } => events,
            KeyNode::Scalar { events, .. } => events,
        }
    }

    pub(super) fn take_events(&mut self) -> Vec<Ev<'a>> {
        match self {
            KeyNode::Fingerprinted { events, .. } => mem::take(events),
            KeyNode::Scalar { events, .. } => mem::take(events),
        }
    }

    pub(super) fn take_fingerprint(&mut self) -> KeyFingerprint<'a> {
        match self {
            KeyNode::Fingerprinted { fingerprint, .. } => mem::take(fingerprint),
            KeyNode::Scalar { .. } => self.fingerprint().into_owned(),
        }
    }

    pub(super) fn location(&self) -> Location {
        let location = match self {
            KeyNode::Fingerprinted { location, .. } => location,
            KeyNode::Scalar { location, .. } => location,
        };
        *location
    }
}

/// `from_slice_multiple` pending key/value pair to be injected into the current mapping.
///
/// Produced by:
/// - Merge (`<<`) processing and by scanning the current mapping fields.
pub(super) struct PendingEntry<'a> {
    pub(super) key: KeyNode<'a>,
    pub(super) value: KeyNode<'a>,
    /// Where the key/value pair is referenced/used in YAML.
    ///
    /// For merge-derived entries, this is the `<<` entry location.
    pub(super) reference_location: Location,
    /// Comments that visually belong to this key/value field.
    ///
    /// For replayed entries these are comments captured at the use site while
    /// scanning the containing map. Definition-site comments inside an anchored
    /// mapping are not reconstructed from the recorded event buffer.
    pub(super) field_comments: Vec<Cow<'a, str>>,
    /// Same-line comments after the key/value separator.
    pub(super) value_separator_comments: Vec<Cow<'a, str>>,
    /// Comments immediately above the value node.
    pub(super) value_comments: Vec<Cow<'a, str>>,
}

/// Return the span lengths of key and value for a one-entry map encoded in `events`.
/// The expected layout is: `MapStart`, <key node>, <value node>, `MapEnd`.
/// On success returns (`key_start`, `key_end`, `val_start`, `val_end`) as indices into events.
pub(super) fn one_entry_map_spans(events: &[Ev<'_>]) -> Option<(usize, usize, usize, usize)> {
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
pub(super) fn skip_one_node_len(events: &[Ev<'_>], mut i: usize) -> Option<usize> {
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
/// This records only `Ev` values. Since `Ev` does not carry comments, callers
/// must claim comment hooks before capture when comments should survive later
/// replay.
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
pub(super) fn capture_node<'a>(ev: &mut dyn Events<'a>) -> Result<KeyNode<'a>, Error> {
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
        Ev::SeqStart {
            anchor,
            tag,
            raw_tag,
            location,
        } => {
            let mut events = vec![Ev::SeqStart {
                anchor,
                tag,
                raw_tag,
                location,
            }];
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

/// Return the simple YAML tag name for a node that can act as an enum variant selector.
pub(super) fn simple_tagged_node_name(event: &Ev<'_>) -> Option<(String, Location)> {
    match event {
        Ev::Scalar {
            tag,
            raw_tag,
            location,
            ..
        }
        | Ev::SeqStart {
            tag,
            raw_tag,
            location,
            ..
        } => simple_tagged_enum_name(raw_tag, tag).map(|name| (name, *location)),
        _ => None,
    }
}

/// Remove the YAML tag from the payload node after it has been promoted to a map key.
pub(super) fn strip_root_tag_for_externally_tagged_payload(events: &mut [Ev<'_>]) {
    match events.first_mut() {
        Some(Ev::Scalar { tag, raw_tag, .. }) => {
            *tag = SfTag::None;
            *raw_tag = None;
        }
        Some(Ev::SeqStart { tag, raw_tag, .. }) => {
            *tag = SfTag::None;
            *raw_tag = None;
        }
        _ => {}
    }
}

/// Encode a YAML tag-selected enum variant as the Serde externally-tagged map form.
///
/// Arguments:
/// - `variant`: enum variant name extracted from the YAML tag, for example `Expression`.
/// - `tag_location`: source location of the tagged YAML node.
/// - `payload_events`: captured events for the YAML node after the root tag was stripped.
///
/// Returns:
/// - A synthetic one-entry mapping equivalent to `{ Variant: payload }`.
pub(super) fn externally_tagged_payload_as_map_events(
    variant: String,
    tag_location: Location,
    mut payload_events: Vec<Ev<'_>>,
) -> Vec<Ev<'_>> {
    let end_location = payload_events.last().map_or(tag_location, Ev::location);

    let mut events = Vec::with_capacity(payload_events.len() + 3);
    events.push(Ev::MapStart {
        anchor: 0,
        location: tag_location,
    });
    events.push(Ev::Scalar {
        value: Cow::Owned(variant),
        tag: SfTag::String,
        raw_tag: None,
        style: ScalarStyle::Plain,
        anchor: 0,
        location: tag_location,
    });
    events.append(&mut payload_events);
    events.push(Ev::MapEnd {
        location: end_location,
    });
    events
}

/// Capture `!Variant payload` as a synthetic `{ Variant: payload }` event buffer.
pub(super) fn capture_simple_tagged_node_as_map_events<'a>(
    ev: &mut dyn Events<'a>,
) -> Result<Option<Vec<Ev<'a>>>, Error> {
    let Some((variant, tag_location)) = ev.peek()?.and_then(|event| simple_tagged_node_name(event))
    else {
        return Ok(None);
    };

    let mut payload_node = capture_node(ev)?;
    let mut payload_events = payload_node.take_events();
    strip_root_tag_for_externally_tagged_payload(&mut payload_events);

    Ok(Some(externally_tagged_payload_as_map_events(
        variant,
        tag_location,
        payload_events,
    )))
}

/// True if `node` is the YAML merge key (`<<`) as an untagged plain scalar.
///
/// Used by:
/// - Mapping deserialization to trigger merge value expansion.
#[inline]
pub(super) fn is_merge_key(node: &KeyNode) -> bool {
    let events = node.events();
    if events.len() != 1 {
        return false;
    }
    events.first().is_some_and(is_merge_key_event)
}

#[inline]
pub(super) fn is_merge_key_event(event: &Ev<'_>) -> bool {
    matches!(
        event,
        Ev::Scalar {
            value,
            tag,
            style: ScalarStyle::Plain,
            ..
        } if tag == &SfTag::None && value.as_ref() == "<<"
    )
}

pub(super) fn validate_no_merge_keys_in_node_events(events: &[Ev<'_>]) -> Result<(), Error> {
    fn eof_location(events: &[Ev<'_>]) -> Location {
        events.last().map_or(Location::UNKNOWN, Ev::location)
    }

    fn visit_node(events: &[Ev<'_>], mut index: usize) -> Result<usize, Error> {
        match events.get(index) {
            Some(Ev::Scalar { .. }) => Ok(index + 1),
            Some(Ev::SeqStart { .. }) => {
                index += 1;
                loop {
                    match events.get(index) {
                        Some(Ev::SeqEnd { .. }) => return Ok(index + 1),
                        Some(Ev::MapEnd { location }) => {
                            return Err(Error::UnexpectedContainerEndWhileSkippingNode {
                                location: *location,
                            });
                        }
                        Some(_) => index = visit_node(events, index)?,
                        None => return Err(Error::eof().with_location(eof_location(events))),
                    }
                }
            }
            Some(Ev::MapStart { .. }) => {
                index += 1;
                loop {
                    match events.get(index) {
                        Some(Ev::MapEnd { .. }) => return Ok(index + 1),
                        Some(Ev::SeqEnd { location }) => {
                            return Err(Error::UnexpectedContainerEndWhileSkippingNode {
                                location: *location,
                            });
                        }
                        Some(event) => {
                            if is_merge_key_event(event) {
                                return Err(Error::MergeKeyNotAllowed {
                                    location: event.location(),
                                });
                            }
                            index = visit_node(events, index)?;
                            index = visit_node(events, index)?;
                        }
                        None => return Err(Error::eof().with_location(eof_location(events))),
                    }
                }
            }
            Some(Ev::SeqEnd { location } | Ev::MapEnd { location }) => {
                Err(Error::UnexpectedContainerEndWhileSkippingNode {
                    location: *location,
                })
            }
            Some(Ev::Taken { location }) => {
                Err(Error::unexpected("consumed event").with_location(*location))
            }
            None => Err(Error::eof().with_location(eof_location(events))),
        }
    }

    let next = visit_node(events, 0)?;
    if next == events.len() {
        Ok(())
    } else {
        Err(Error::unexpected("single YAML node").with_location(events[next].location()))
    }
}

pub(super) fn apply_duplicate_key_policy_to_entries<'a>(
    mut entries: Vec<PendingEntry<'a>>,
    duplicate_keys: DuplicateKeyPolicy,
    merge_keys: MergeKeyPolicy,
) -> Result<Vec<PendingEntry<'a>>, Error> {
    let last_wins = matches!(duplicate_keys, DuplicateKeyPolicy::LastWins);
    if last_wins {
        entries.reverse();
    }

    let mut seen = HashSet::with_capacity(entries.len());
    let mut kept = Vec::with_capacity(entries.len());

    for entry in entries {
        if seen.insert(entry.key.fingerprint().into_owned()) {
            kept.push(entry);
            continue;
        }

        if matches!(duplicate_keys, DuplicateKeyPolicy::Error) {
            // This is an error path. We would rather get the fingerprint
            // a second time here for error reporting than clone it before.
            let fingerprint = entry.key.fingerprint();
            let key = fingerprint.stringy_scalar_value().map(ToOwned::to_owned);
            return Err(Error::DuplicateMappingKey {
                key,
                location: entry.key.location(),
            });
        }

        if matches!(merge_keys, MergeKeyPolicy::Error) {
            validate_no_merge_keys_in_node_events(entry.value.events())?;
        }
    }

    if last_wins {
        kept.reverse();
    }

    Ok(kept)
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
pub(super) fn pending_entries_from_events(
    events: Vec<Ev<'_>>,
    location: Location,
    reference_location: Location,
    merge_keys: MergeKeyPolicy,
    duplicate_keys: DuplicateKeyPolicy,
    #[cfg(feature = "properties")] property_map: Option<Rc<HashMap<String, String>>>,
    #[cfg(feature = "properties")] property_syntax: PropertySyntax,
) -> Result<Vec<PendingEntry<'_>>, Error> {
    let mut replay = ReplayEvents::with_reference(
        events,
        reference_location,
        #[cfg(feature = "properties")]
        property_map.clone(),
        #[cfg(feature = "properties")]
        property_syntax,
    );
    match replay.peek()? {
        Some(Ev::Scalar { value, style, .. }) if scalar_is_nullish(value.as_ref(), style) => {
            Ok(Vec::new())
        }
        Some(Ev::Scalar { location, .. }) => Err(Error::MergeValueNotMapOrSeqOfMaps {
            location: *location,
        }),
        Some(Ev::MapStart { .. }) => {
            collect_entries_from_map(&mut replay, reference_location, merge_keys, duplicate_keys)
        }
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
                            merge_keys,
                            duplicate_keys,
                            #[cfg(feature = "properties")]
                            property_map.clone(),
                            #[cfg(feature = "properties")]
                            property_syntax,
                        )?); // recursive
                    }
                    None => {
                        return Err(Error::eof().with_location(replay.last_location()));
                    }
                }
            }

            let mut merged = Vec::new();
            for mut nested in batches {
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
pub(super) fn pending_entries_from_live_events<'a>(
    ev: &mut dyn Events<'a>,
    merge_reference_location: Location,
    merge_keys: MergeKeyPolicy,
    duplicate_keys: DuplicateKeyPolicy,
) -> Result<Vec<PendingEntry<'a>>, Error> {
    #[cfg(feature = "properties")]
    let property_map = ev.property_map().map(Rc::clone);
    #[cfg(feature = "properties")]
    let property_syntax = ev.property_syntax();
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
                merge_keys,
                duplicate_keys,
                #[cfg(feature = "properties")]
                property_map,
                #[cfg(feature = "properties")]
                property_syntax,
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
                            merge_keys,
                            duplicate_keys,
                            #[cfg(feature = "properties")]
                            property_map.clone(),
                            #[cfg(feature = "properties")]
                            property_syntax,
                        )?);
                    }
                    None => return Err(Error::eof().with_location(ev.last_location())),
                }
            }
            let mut merged = Vec::new();
            for mut nested in batches {
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
pub(super) fn collect_entries_from_map<'a>(
    ev: &mut dyn Events<'a>,
    reference_location: Location,
    merge_keys: MergeKeyPolicy,
    duplicate_keys: DuplicateKeyPolicy,
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
                let key_comments = ev.take_leading_comments_for_next_node()?;
                let key = capture_node(ev)?;
                if is_merge_key(&key) {
                    match merge_keys {
                        MergeKeyPolicy::Merge => {
                            // Preserve where the merge value is referenced (use-site). For alias
                            // merges inside merged mappings, node locations point at the anchored
                            // mapping, but we want `referenced` to point at the alias token.
                            let _ = ev.peek()?;
                            let merge_ref_loc = ev.reference_location();
                            merges.push(pending_entries_from_live_events(
                                ev,
                                merge_ref_loc,
                                merge_keys,
                                duplicate_keys,
                            )?);
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
                let value_separator_comments = ev.take_separator_comments_before_mapping_value()?;
                let value_comments = ev.take_leading_comments_for_next_node()?;
                let value = capture_node(ev)?;
                fields.push(PendingEntry {
                    key,
                    value,
                    reference_location,
                    field_comments,
                    value_separator_comments,
                    value_comments,
                });
            }
            None => {
                return Err(Error::eof().with_location(ev.last_location()));
            }
        }
    }

    let mut entries = apply_duplicate_key_policy_to_entries(fields, duplicate_keys, merge_keys)?;
    for mut nested in merges {
        entries.append(&mut nested);
    }
    Ok(entries)
}
