use std::borrow::Cow;
#[cfg(feature = "properties")]
use std::cell::{Cell, RefCell};
#[cfg(feature = "properties")]
use std::collections::HashMap;
use std::mem;
#[cfg(feature = "properties")]
use std::rc::Rc;

use granit_parser::ScalarStyle;

use super::error::Error;
#[cfg(feature = "properties")]
use super::options::PropertySyntax;
use super::tags::SfTag;
#[cfg(feature = "properties")]
use crate::Budget;
#[cfg(feature = "properties")]
use crate::budget::BudgetBreach;
use crate::location::{Location, Locations};

/// Property interpolation configuration and aggregate work shared by live and replay sources.
#[cfg(feature = "properties")]
#[derive(Clone)]
pub(crate) struct PropertyInterpolation {
    property_map: Option<Rc<HashMap<String, String>>>,
    syntax: PropertySyntax,
    max_expansion_depth: usize,
    max_total_work: usize,
    total_work: Rc<Cell<usize>>,
    breach: Rc<RefCell<Option<BudgetBreach>>>,
}

#[cfg(feature = "properties")]
impl PropertyInterpolation {
    pub(super) fn new(
        property_map: Option<Rc<HashMap<String, String>>>,
        syntax: PropertySyntax,
        budget: Option<&Budget>,
    ) -> Self {
        let (max_expansion_depth, max_total_work) =
            budget.map_or((usize::MAX, usize::MAX), |budget| {
                (
                    budget.max_property_expansion_depth,
                    budget.max_total_property_interpolation_work,
                )
            });
        Self {
            property_map,
            syntax,
            max_expansion_depth,
            max_total_work,
            total_work: Rc::new(Cell::new(0)),
            breach: Rc::new(RefCell::new(None)),
        }
    }

    pub(super) fn property_map(&self) -> Option<&Rc<HashMap<String, String>>> {
        self.property_map.as_ref()
    }

    pub(super) fn syntax(&self) -> PropertySyntax {
        self.syntax
    }

    pub(super) fn max_expansion_depth(&self) -> usize {
        self.max_expansion_depth
    }

    pub(super) fn max_total_work(&self) -> usize {
        self.max_total_work
    }

    pub(super) fn total_work(&self) -> &Cell<usize> {
        self.total_work.as_ref()
    }

    pub(super) fn record_breach(&self, breach: BudgetBreach) {
        let mut recorded = self.breach.borrow_mut();
        if recorded.is_none() {
            *recorded = Some(breach);
        }
    }

    pub(super) fn breach(&self) -> Option<BudgetBreach> {
        self.breach.borrow().clone()
    }
}

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
        let loc = if reference_location == Location::UNKNOWN {
            defined_location
        } else {
            reference_location
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
    MapStart {
        anchor: usize,
        tag: SfTag,
        raw_tag: Option<Cow<'a, str>>,
        location: Location,
    },
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

    /// Bytes that an [`Ev::clone`] must allocate for owned string payloads.
    pub(crate) fn owned_payload_bytes(&self) -> usize {
        match self {
            Ev::Scalar { value, raw_tag, .. } => {
                let value_bytes = match value {
                    Cow::Borrowed(_) => 0,
                    Cow::Owned(value) => value.len(),
                };
                let raw_tag_bytes = match raw_tag {
                    Some(Cow::Owned(raw_tag)) => raw_tag.len(),
                    Some(Cow::Borrowed(_)) | None => 0,
                };
                value_bytes.saturating_add(raw_tag_bytes)
            }
            Ev::SeqStart { raw_tag, .. } | Ev::MapStart { raw_tag, .. } => match raw_tag {
                Some(Cow::Owned(raw_tag)) => raw_tag.len(),
                Some(Cow::Borrowed(_)) | None => 0,
            },
            Ev::SeqEnd { .. } | Ev::MapEnd { .. } | Ev::Taken { .. } => 0,
        }
    }

    /// Clear the YAML tag attached to a scalar or container-start event.
    pub(super) fn strip_node_tag(&mut self) -> bool {
        match self {
            Ev::Scalar { tag, raw_tag, .. }
            | Ev::SeqStart { tag, raw_tag, .. }
            | Ev::MapStart { tag, raw_tag, .. } => {
                *tag = SfTag::None;
                *raw_tag = None;
                true
            }
            Ev::SeqEnd { .. } | Ev::MapEnd { .. } | Ev::Taken { .. } => false,
        }
    }
}

/// `from_slice_multiple` location-free representation of events for duplicate-key comparison.
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

    /// Strip the tag from the node currently exposed by [`Self::peek`].
    ///
    /// This lets typed enum handling reinterpret a YAML tag as the enum variant while
    /// continuing to deserialize the payload directly from the original event source.
    fn strip_peeked_node_tag(&mut self) -> Result<(), Error>;

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

    #[cfg(feature = "properties")]
    fn property_interpolation(&self) -> &PropertyInterpolation;
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
    /// Index of the next event to yield (`0..=buf.len()`).
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
    property_interpolation: PropertyInterpolation,
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
        #[cfg(feature = "properties")] property_interpolation: PropertyInterpolation,
    ) -> Self {
        Self {
            buf,
            idx: 0,
            ref_override: None,
            #[cfg(feature = "properties")]
            property_interpolation,
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
        #[cfg(feature = "properties")] property_interpolation: PropertyInterpolation,
    ) -> Self {
        Self {
            buf,
            idx: 0,
            ref_override: Some(reference),
            #[cfg(feature = "properties")]
            property_interpolation,
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

    fn strip_peeked_node_tag(&mut self) -> Result<(), Error> {
        let location = self
            .buf
            .get(self.idx)
            .map_or_else(|| self.last_location(), Ev::location);
        match self.buf.get_mut(self.idx) {
            Some(event) => event
                .strip_node_tag()
                .then_some(())
                .ok_or_else(|| Error::unexpected("tagged node").with_location(location)),
            None => Err(Error::unexpected("tagged node").with_location(location)),
        }
    }

    fn last_location(&self) -> Location {
        let last = self.idx.saturating_sub(1);
        self.buf.get(last).map_or(Location::UNKNOWN, Ev::location)
    }

    fn reference_location(&self) -> Location {
        if let Some(loc) = self.ref_override {
            return loc;
        }
        self.buf
            .get(self.idx)
            .map_or_else(|| self.last_location(), Ev::location)
    }

    #[cfg(feature = "properties")]
    fn property_interpolation(&self) -> &PropertyInterpolation {
        &self.property_interpolation
    }
}
