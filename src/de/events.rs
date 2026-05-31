use std::borrow::Cow;
#[cfg(feature = "properties")]
use std::collections::HashMap;
use std::mem;
#[cfg(feature = "properties")]
use std::rc::Rc;

use granit_parser::ScalarStyle;

use super::error::Error;
use super::tags::SfTag;
use crate::location::{Location, Locations};

/// Attach both reference and defined locations to an error for alias replay scenarios.
/// When both locations are known and different, creates an `AliasError` to report both.
/// This is used for errors occurring when deserializing aliased values.
///
/// During alias replay, errors may already have a location attached (the anchor's definition
/// location from the replayed events). We still want to create an `AliasError` with both
/// locations when the reference (alias) and defined (anchor) locations differ.
#[inline]
pub(super) fn attach_alias_locations_if_missing(
    err: Error,
    reference_location: Location,
    defined_location: Location,
) -> Error {
    // If both locations are known and different, create an AliasError to show both.
    // This applies even if the error already has a location (from replayed anchor events),
    // because we want to show where the alias was used, not just where the anchor was defined.
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
    } else if err.location().is_some() {
        // Error already has a location and we don't have dual locations to add
        err
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

/// Our simplified owned event kind that we feed into Serde.
///
/// This intentionally carries semantic YAML node data, anchors, tags, style, and
/// locations, but not presentation metadata such as comments. Live streams expose
/// comments through the `Events` comment hooks; replay streams can only preserve
/// comments that callers captured separately before buffering/replay.
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
    SeqStart {
        anchor: usize,
        tag: SfTag,
        raw_tag: Option<Cow<'a, str>>,
        location: Location,
    },
    /// End of a sequence.
    SeqEnd { location: Location },
    /// Start of a mapping (`{` or block mapping).
    MapStart { anchor: usize, location: Location },
    /// End of a mapping.
    MapEnd { location: Location },
    /// The event has been taken from the array, with only its location remaining.
    /// This should not appear in the event stream and is reserved for internal container state.
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
    /// - `Ok(Some(&Ev))` with the event reference,
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

    /// Take comments immediately above the next data node.
    ///
    /// Implementations may fill lookahead while doing this. The default is empty
    /// for replay buffers that do not carry presentation metadata. If a caller
    /// needs comments for a captured node, it must take them from the live stream
    /// before calling `capture_node` and carry them separately.
    fn take_leading_comments_for_next_node(&mut self) -> Result<Vec<Cow<'de, str>>, Error> {
        Ok(Vec::new())
    }

    /// Take same-line comments after a mapping key/value separator.
    ///
    /// This is the `# comment` in `key: # comment`, separated from comments
    /// immediately above the value node so nested containers do not treat the
    /// separator comment as a child-key comment.
    fn take_separator_comments_before_mapping_value(
        &mut self,
    ) -> Result<Vec<Cow<'de, str>>, Error> {
        Ok(Vec::new())
    }

    /// Take same-line comments after a block sequence item marker.
    ///
    /// This is the `# comment` in `- # comment`, separated from ordinary
    /// trailing comments after the previous sequence value.
    fn take_separator_comments_before_sequence_item_value(
        &mut self,
    ) -> Result<Vec<Cow<'de, str>>, Error> {
        Ok(Vec::new())
    }

    /// Take same-line comments immediately after the node that was just deserialized.
    fn take_trailing_comments_after_node(&mut self) -> Result<Vec<Cow<'de, str>>, Error> {
        Ok(Vec::new())
    }

    /// Get the original input string for zero-copy borrowing.
    ///
    /// Returns `Some(&str)` when the input is available for borrowing (string-based parsing),
    /// or `None` when borrowing is not possible (reader-based parsing or replay buffers).
    ///
    /// Used by:
    /// - The deserializer to return borrowed `&str` references when possible.
    ///
    /// This is used by string deserialization to return borrowed scalars when possible.
    fn input_for_borrowing(&self) -> Option<&'de str> {
        None // Default: borrowing not supported
    }

    /// Return the property map used for variable interpolation, if configured.
    #[cfg(feature = "properties")]
    fn property_map(&self) -> Option<&Rc<HashMap<String, String>>> {
        None
    }
}

#[cold]
pub(super) fn eof_with_loc(events: &dyn Events<'_>) -> Error {
    Error::eof().with_location(events.last_location())
}

/// Event source that replays a pre-recorded buffer.
///
/// Replay buffers contain `Ev` values only. Comment hooks therefore use the
/// trait defaults and return empty comment sets; use-site comments must be passed
/// around separately by the map/sequence access code.
pub(super) struct ReplayEvents<'a> {
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

    #[cfg(feature = "properties")]
    property_map: Option<Rc<HashMap<String, String>>>,
}

impl<'a> ReplayEvents<'a> {
    /// Create a replay source over `buf`, initially positioned at index 0.
    ///
    /// Arguments:
    /// - `buf`: previously captured events.
    ///
    /// Called by:
    /// - Merge expansion and recorded key/value deserialization.
    pub(super) fn new(
        buf: Vec<Ev<'a>>,
        #[cfg(feature = "properties")] property_map: Option<Rc<HashMap<String, String>>>,
    ) -> Self {
        Self {
            buf,
            idx: 0,
            ref_override: None,
            #[cfg(feature = "properties")]
            property_map,
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
    pub(super) fn with_reference(
        buf: Vec<Ev<'a>>,
        reference: Location,
        #[cfg(feature = "properties")] property_map: Option<Rc<HashMap<String, String>>>,
    ) -> Self {
        Self {
            buf,
            idx: 0,
            ref_override: Some(reference),
            #[cfg(feature = "properties")]
            property_map,
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

    #[cfg(feature = "properties")]
    fn property_map(&self) -> Option<&Rc<HashMap<String, String>>> {
        self.property_map.as_ref()
    }
}
