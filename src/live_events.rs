//! Live events: a streaming view over the YAML input.
//!
//! This module implements LiveEvents, an Events source that pulls items directly
//! from the underlying saphyr_parser::Parser as it scans the input string.
//! Unlike ReplayEvents, which iterates over a pre-recorded buffer, live events
//! are produced on demand and reflect the current position of the parser.
//!
//! Responsibilities and behavior:
//! - Skip parser-level stream/document boundary markers so consumers see only
//!   logical YAML nodes: container starts/ends, scalars, and aliases.
//! - Track and record anchors for both scalars and containers. When an alias is
//!   encountered later, the previously recorded sequence of events for that
//!   anchor is injected (replayed) back into the stream.
//! - Enforce alias-bomb hardening via AliasLimits and account replayed events
//!   per anchor and in total. BudgetEnforcer can also be attached to limit raw
//!   event production.
//! - Maintain a single-item lookahead buffer to implement peek(), and keep
//!   last_location to improve error reporting.
//!
//! LiveEvents is single-pass and does not support rewinding. Aliases expand by
//! injecting previously recorded buffers; normal parsing continues after the
//! injection is exhausted.

use std::borrow::Cow;
// use std::collections::HashMap; // ← gone

use crate::de::{AliasLimits, Budget, BudgetEnforcer, Error, Ev, Events, Location};
use crate::error::{budget_error, location_from_span};
use crate::tags::SfTag;
use saphyr_parser::{Event, Parser, ScalarStyle, StrInput};
use smallvec::SmallVec;

/// This is enough to hold a single scalar that is common  case in YAML anchors.
const SMALLVECT_INLINE: usize = 4;

/// A frame that records events for an anchored container until its end.
/// Uses SmallVec to avoid heap allocations for small anchors.
#[derive(Clone, Debug)]
struct RecFrame {
    id: usize,
    /// counts nested container starts/ends
    depth: usize,
    /// inline up to SMALLVECT_INLINE events; spills to heap beyond
    buf: SmallVec<Ev, SMALLVECT_INLINE>,
}

/// Live event source that wraps `saphyr_parser::Parser` and:
/// - Skips stream/document markers
/// - Records anchored subtrees (containers and scalars)
/// - Resolves aliases by injecting recorded buffers (replaying)
pub(crate) struct LiveEvents<'a> {
    /// Whether any content event has been produced in the current stream.
    produced_any_in_doc: bool,
    /// Whether we emitted a synthetic null scalar to represent an empty document.
    synthesized_null_emitted: bool,
    /// Underlying streaming parser that produces raw events from the input.
    parser: Parser<'a, StrInput<'a>>,
    /// Single-item lookahead buffer (peeked event not yet consumed).
    look: Option<Ev>,
    /// For alias replay: a stack of injected buffers; we always read from the top first.
    /// Holds (anchor_id, current_index).
    inject: Vec<(usize, usize)>,
    /// Recorded buffers for anchors (index = anchor_id).
    /// `None` means the id is not recorded (e.g., never anchored or cleared).
    /// Saphyr's parser anchor_id is the sequential counter.
    anchors: Vec<Option<Box<[Ev]>>>,
    /// Recording frames for currently-open anchored containers.
    rec_stack: Vec<RecFrame>,
    /// Budget (raw events); independent of alias replay limits below.
    budget: Option<BudgetEnforcer>,

    /// Location of the last yielded event (for better error reporting).
    last_location: Location,

    /// Alias-bomb hardening limits and counters.
    alias_limits: AliasLimits,
    /// Total number of replayed events across the whole stream (enforced by `alias_limits`).
    total_replayed_events: usize,
    /// Per-anchor replay expansion counters, indexed by anchor id (dense ids).
    per_anchor_expansions: Vec<usize>,
    /// In single-document mode, stop producing events when a DocumentEnd is seen.
    stop_at_doc_end: bool,
    /// Indicates whether a DocumentEnd was seen for the last parsed document.
    seen_doc_end: bool,
}

impl<'a> LiveEvents<'a> {
    /// Create a new live event source.
    ///
    /// # Parameters
    /// - `input`: YAML source string.
    /// - `budget`: Optional budget info for raw events (external `BudgetEnforcer`).
    /// - `alias_limits`: Alias replay limits to mitigate alias bombs.
    ///
    /// # Returns
    /// A configured `LiveEvents` ready to stream events.
    pub(crate) fn new(
        input: &'a str,
        budget: Option<Budget>,
        alias_limits: AliasLimits,
        stop_at_doc_end: bool,
    ) -> Self {
        Self {
            produced_any_in_doc: false,
            synthesized_null_emitted: false,
            parser: Parser::new_from_str(input),
            look: None,
            inject: Vec::new(),
            anchors: Vec::new(),
            rec_stack: Vec::new(),
            budget: budget.map(BudgetEnforcer::new),

            last_location: Location::UNKNOWN,

            alias_limits,
            total_replayed_events: 0,
            per_anchor_expansions: Vec::new(),
            stop_at_doc_end,
            seen_doc_end: false,
        }
    }

    /// Core event pump: pulls the next logical event.
    ///
    /// Order of precedence:
    /// - If there is an injected replay buffer (from an alias), serve from it first.
    /// - Otherwise, pull from the underlying parser, skipping stream/document markers.
    ///
    /// During parsing it:
    /// - Tracks and records anchors for scalars and containers.
    /// - Injects recorded buffers on aliases, enforcing alias-bomb hardening limits and budget.
    /// - Maintains last_location for better error messages.
    ///
    /// Returns Some(event) when an event is produced, or Ok(None) on true EOF.
    fn next_impl(&mut self) -> Result<Option<Ev>, Error> {
        // 1) Serve from injected buffers first (alias replay)
        if let Some((anchor_id, idx)) = self.inject.last_mut() {
            let buf = self
                .anchors
                .get(*anchor_id)
                .and_then(|o| o.as_ref())
                .ok_or_else(|| {
                    Error::unknown_anchor(*anchor_id).with_location(self.last_location)
                })?;

            if *idx < buf.len() {
                let ev = buf[*idx].clone();
                *idx += 1;
                if *idx == buf.len() {
                    self.inject.pop();
                }
                // Count replayed events for alias-bomb hardening.
                self.total_replayed_events = self
                    .total_replayed_events
                    .checked_add(1)
                    .ok_or_else(|| Error::msg("alias replay counter overflow"))
                    .map_err(|err| err.with_location(ev.location()))?;
                if self.total_replayed_events > self.alias_limits.max_total_replayed_events {
                    return Err(Error::msg(format!(
                        "alias replay limit exceeded: total_replayed_events={} > {}",
                        self.total_replayed_events, self.alias_limits.max_total_replayed_events
                    ))
                    .with_location(ev.location()));
                }
                self.observe_budget_for_replay(&ev)?;
                self.record(
                    &ev, /*is_start*/ false, /*seeded_new_frame*/ false,
                );
                self.last_location = ev.location();
                self.produced_any_in_doc = true;
                return Ok(Some(ev));
            } else {
                self.inject.pop();
            }
        }

        // 2) Pull from the real parser
        while let Some(item) = self.parser.next() {
            let (raw, span) = item.map_err(Error::from_scan_error)?;
            let location = location_from_span(&span);

            if let Some(ref mut budget) = self.budget {
                if let Err(breach) = budget.observe(&raw) {
                    return Err(budget_error(breach).with_location(location));
                }
            }

            match raw {
                Event::StreamStart | Event::StreamEnd => {
                    // Skip stream markers.
                    self.last_location = location;
                    continue;
                }

                Event::DocumentStart(_) => {
                    // Skip doc start and reset per-document state.
                    self.reset_document_state();
                    self.last_location = location;
                    continue;
                }
                Event::DocumentEnd => {
                    // On document end: in single-document mode, mark and stop producing events.
                    self.reset_document_state();
                    self.seen_doc_end = true;
                    self.last_location = location;
                    if self.stop_at_doc_end {
                        return Ok(None);
                    }
                    continue;
                }

                Event::Scalar(val, mut style, anchor_id, tag) => {
                    if matches!(style, ScalarStyle::Folded)
                        && span.start.col() == 0
                        && !val.trim().is_empty()
                    {
                        return Err(Error::msg("folded block scalars must indent their content")
                            .with_location(location));
                    }

                    let s = match val {
                        Cow::Borrowed(v) => v.to_string(),
                        Cow::Owned(v) => v,
                    };
                    let tag_s = SfTag::from_optional_cow(&tag);
                    if s.is_empty()
                        && anchor_id != 0
                        && matches!(style, ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted)
                    {
                        // Normalize: anchored empty scalars should behave like plain empty (null-like)
                        style = ScalarStyle::Plain;
                    }
                    let ev = Ev::Scalar {
                        value: s,
                        tag: tag_s,
                        style,
                        anchor: anchor_id,
                        location,
                    };
                    self.record(&ev, false, false);
                    if anchor_id != 0 {
                        self.ensure_anchor_capacity(anchor_id);
                        self.anchors[anchor_id] = Some(vec![ev.clone()].into_boxed_slice());
                    }
                    self.last_location = location;
                    self.produced_any_in_doc = true;
                    return Ok(Some(ev));
                }

                Event::SequenceStart(anchor_id, _tag) => {
                    let ev = Ev::SeqStart {
                        anchor: anchor_id,
                        location,
                    };
                    // Existing frames go deeper with this start.
                    self.bump_depth_on_start();
                    // Start recording for this anchor *after* bumping other frames,
                    // and include the start event in the new buffer.
                    if anchor_id != 0 {
                        let mut buf: SmallVec<Ev, SMALLVECT_INLINE> = SmallVec::new();
                        buf.push(ev.clone());
                        self.rec_stack.push(RecFrame {
                            id: anchor_id,
                            depth: 1,
                            buf,
                        });
                    }
                    // Correct recording semantics:
                    // - If we *just* created a new frame for this start, the start was already seeded.
                    // - For ordinary (non-anchored) starts, record into *all* frames.
                    self.record(
                        &ev,
                        /*is_start*/ true,
                        /*seeded_new_frame*/ anchor_id != 0,
                    );
                    self.last_location = location;
                    self.produced_any_in_doc = true;
                    return Ok(Some(ev));
                }
                Event::SequenceEnd => {
                    let ev = Ev::SeqEnd { location };
                    self.record(&ev, false, false);
                    self.bump_depth_on_end()
                        .map_err(|err| err.with_location(location))?; // may finalize frames
                    self.last_location = location;
                    self.produced_any_in_doc = true;
                    return Ok(Some(ev));
                }

                Event::MappingStart(anchor_id, _tag) => {
                    let ev = Ev::MapStart {
                        anchor: anchor_id,
                        location,
                    };
                    self.bump_depth_on_start();
                    if anchor_id != 0 {
                        let mut buf: SmallVec<Ev, SMALLVECT_INLINE> = SmallVec::new();
                        buf.push(ev.clone());
                        self.rec_stack.push(RecFrame {
                            id: anchor_id,
                            depth: 1,
                            buf,
                        });
                    }
                    self.record(
                        &ev,
                        /*is_start*/ true,
                        /*seeded_new_frame*/ anchor_id != 0,
                    );
                    self.last_location = location;
                    self.produced_any_in_doc = true;
                    return Ok(Some(ev));
                }
                Event::MappingEnd => {
                    let ev = Ev::MapEnd { location };
                    self.record(&ev, false, false);
                    self.bump_depth_on_end()
                        .map_err(|err| err.with_location(location))?;
                    self.last_location = location;
                    self.produced_any_in_doc = true;
                    return Ok(Some(ev));
                }

                Event::Alias(anchor_id) => {
                    // Alias replay hardening.
                    if anchor_id >= self.per_anchor_expansions.len() {
                        self.per_anchor_expansions.resize(anchor_id + 1, 0);
                    }
                    self.per_anchor_expansions[anchor_id] =
                        self.per_anchor_expansions[anchor_id].saturating_add(1);
                    let count = self.per_anchor_expansions[anchor_id];
                    if count > self.alias_limits.max_alias_expansions_per_anchor {
                        return Err(Error::msg(format!(
                            "alias expansion limit exceeded for anchor id {}: {} > {}",
                            anchor_id, count, self.alias_limits.max_alias_expansions_per_anchor
                        ))
                        .with_location(location));
                    }

                    // Push for replay; enforce stack depth limit.
                    let next_depth = self.inject.len() + 1;
                    if next_depth > self.alias_limits.max_replay_stack_depth {
                        return Err(Error::msg(format!(
                            "alias replay stack depth exceeded: depth={} > {}",
                            next_depth, self.alias_limits.max_replay_stack_depth
                        ))
                        .with_location(location));
                    }

                    // Ensure the anchor exists now (fail fast); store only id + idx.
                    let exists = self
                        .anchors
                        .get(anchor_id)
                        .and_then(|o| o.as_ref())
                        .is_some();
                    if !exists {
                        return Err(Error::unknown_anchor(anchor_id).with_location(location));
                    }
                    self.inject.push((anchor_id, 0));
                    return self.next_impl();
                }

                Event::Nothing => continue,
            }
        }

        // True EOF. If we have not produced any content in the current document,
        // synthesize a single null scalar event to represent an empty document.
        if !self.produced_any_in_doc {
            let ev = Ev::Scalar {
                value: String::new(),
                tag: SfTag::Null,
                style: ScalarStyle::Plain,
                anchor: 0,
                location: self.last_location,
            };
            self.produced_any_in_doc = true;
            self.synthesized_null_emitted = true;
            self.last_location = ev.location();
            return Ok(Some(ev));
        }

        Ok(None)
    }

    /// Ensure the anchors vec is large enough for `anchor_id`.
    fn ensure_anchor_capacity(&mut self, anchor_id: usize) {
        if anchor_id >= self.anchors.len() {
            // Allocate at once place for more anchors than just one
            self.anchors.resize_with(anchor_id + 8, || None);
        }
    }

    /// Reset per-document state when encountering a document boundary.
    ///
    /// Clears injected replay buffers, recorded anchors, current recording frames,
    /// and alias-expansion counters. Does not modify global parser state.
    fn reset_document_state(&mut self) {
        self.inject.clear();
        self.anchors.clear(); // drop all per-doc anchors
        self.rec_stack.clear();
        self.per_anchor_expansions.clear();
        self.total_replayed_events = 0;
        self.seen_doc_end = false;
    }

    /// Observe the configured budget for a replayed (injected) event.
    ///
    /// Reconstructs a parser Event equivalent to the Ev and passes it to the
    /// BudgetEnforcer, attaching the event's location on error.
    fn observe_budget_for_replay(&mut self, ev: &Ev) -> Result<(), Error> {
        let Some(budget) = self.budget.as_mut() else {
            return Ok(());
        };

        let raw = match ev {
            Ev::Scalar { value, style, .. } => Event::Scalar(Cow::Borrowed(value), *style, 0, None),
            Ev::SeqStart { .. } => Event::SequenceStart(0, None),
            Ev::SeqEnd { .. } => Event::SequenceEnd,
            Ev::MapStart { .. } => Event::MappingStart(0, None),
            Ev::MapEnd { .. } => Event::MappingEnd,
        };

        budget
            .observe(&raw)
            .map_err(|breach| budget_error(breach).with_location(ev.location()))
    }

    /// Record an event into active recording frames.
    ///
    /// # Parameters
    /// - `ev`: the event to record.
    /// - `is_start`: whether this is a container start event.
    /// - `seeded_new_frame`: true **only** when a new frame was just created and already
    ///   seeded with the same start event (i.e., anchored container start).
    fn record(&mut self, ev: &Ev, is_start: bool, seeded_new_frame: bool) {
        if self.rec_stack.is_empty() {
            return;
        }
        if is_start {
            if seeded_new_frame {
                let last = self.rec_stack.len() - 1;
                for (i, fr) in self.rec_stack.iter_mut().enumerate() {
                    if i != last {
                        fr.buf.push(ev.clone());
                    }
                }
            } else {
                for fr in &mut self.rec_stack {
                    fr.buf.push(ev.clone());
                }
            }
        } else {
            for fr in &mut self.rec_stack {
                fr.buf.push(ev.clone());
            }
        }
    }

    /// Increase recording depth for all active anchored frames on a container start.
    fn bump_depth_on_start(&mut self) {
        for fr in &mut self.rec_stack {
            fr.depth += 1;
        }
    }

    /// Decrease recording depth on a container end and finalize any frames
    /// that reach depth 0 by storing their recorded buffers in `anchors`.
    ///
    /// Returns an error if internal depth accounting underflows.
    fn bump_depth_on_end(&mut self) -> Result<(), Error> {
        for fr in &mut self.rec_stack {
            if fr.depth == 0 {
                return Err(Error::msg("internal depth underflow"));
            }
            fr.depth -= 1;
        }
        // Finalize frames that just reached depth == 0 (only possible at the top).
        while let Some(top) = self.rec_stack.last() {
            if top.depth == 0 {
                let done = self
                    .rec_stack
                    .pop()
                    .ok_or_else(|| Error::msg("internal recursion stack empty"))?;
                // Convert SmallVec into Box<[Ev]> and store by anchor_id.
                self.ensure_anchor_capacity(done.id);
                self.anchors[done.id] = Some(done.buf.into_vec().into_boxed_slice());
            } else {
                break;
            }
        }
        Ok(())
    }

    /// Finalize the stream: flush and report budget breaches, if any.
    ///
    /// Should be called after parsing completes to surface any delayed
    /// budget enforcement errors with the last known location.
    pub(crate) fn finish(&mut self) -> Result<(), Error> {
        if let Some(budget) = self.budget.take() {
            let report = budget.finalize();
            if let Some(breach) = report.breached {
                return Err(budget_error(breach).with_location(self.last_location));
            }
        }
        Ok(())
    }
}

impl<'a> Events for LiveEvents<'a> {
    /// Get the next event, using a single-item lookahead buffer if present.
    /// Updates last_location to the yielded event's location.
    fn next(&mut self) -> Result<Option<Ev>, Error> {
        if let Some(ev) = self.look.take() {
            self.last_location = ev.location();
            return Ok(Some(ev));
        }
        self.next_impl()
    }
    /// Peek at the next event without consuming it, filling the lookahead buffer if empty.
    fn peek(&mut self) -> Result<Option<&Ev>, Error> {
        if self.look.is_none() {
            self.look = self.next_impl()?;
        }
        if let Some(ev) = self.look.as_ref() {
            self.last_location = ev.location();
        };

        Ok((&self.look).into())
    }
    fn last_location(&self) -> Location {
        self.last_location
    }
}

impl<'a> LiveEvents<'a> {
    pub(crate) fn seen_doc_end(&self) -> bool {
        self.seen_doc_end
    }
    pub(crate) fn synthesized_null_emitted(&self) -> bool {
        self.synthesized_null_emitted
    }
}
