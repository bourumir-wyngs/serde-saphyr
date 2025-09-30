//!
//! Live events: a compact layer over the YAML event stream from `saphyr_parser::Parser`.
//!
//! Responsibilities
//! - Provide owned, simplified events (`Ev`) for the Serde deserializer.
//! - Hide stream/document markers; expose only logical data events.
//! - Track source locations for diagnostics.
//! - Record anchors and replay aliases.
//! - Enforce budgets and alias-replay limits.
//!
//! Anchors and aliases
//! - Anchored scalar: store a single `Ev::Scalar` in `anchors[id]`.
//! - Anchored sequence/mapping: start a recording frame, record until the end,
//!   then save the buffer to `anchors[id]`.
//! - Alias `*id`: push the recorded buffer to the replay stack (`inject`) and
//!   inject its events. Enforce:
//!   - `max_total_replayed_events` (document-wide),
//!   - `max_alias_expansions_per_anchor` (per id),
//!   - `max_replay_stack_depth` (nested replays).
//! - Apply budget checks to replayed events by reconstructing raw events.
//!
//! Event flow
//! - If a replay buffer is pending, serve from it first.
//! - Otherwise, pull the next parser event and translate into `Ev`, skipping
//!   stream/doc markers.
//! - Maintain one-item lookahead (`look`).
//!
//! Document boundaries
//! - On `---`/`...`, clear replay buffers, anchors, recording frames, and
//!   alias-expansion counters.
//!
//! Locations
//! - Each `Ev` carries a `Location`. The last yielded location is tracked for
//!   EOF and structural errors.

use std::borrow::Cow;
use std::collections::HashMap;

use saphyr_parser::{Event, Parser, ScalarStyle, StrInput};

use crate::sf_serde::{
    budget_error, location_from_span, AliasLimits, Budget, BudgetEnforcer, Error, Ev, Events,
    Location,
};

/// A frame that records events for an anchored container until its end.
#[derive(Clone, Debug)]
struct RecFrame {
    id: usize,
    depth: usize, // counts nested container starts/ends
    buf: Vec<Ev>,
}

/// Live event source that wraps `saphyr_parser::Parser` and:
/// - Skips stream/document markers
/// - Records anchored subtrees (containers and scalars)
/// - Resolves aliases by injecting recorded buffers (replaying)
pub(crate) struct LiveEvents<'a> {
    /// Underlying streaming parser that produces raw events from the input.
    parser: Parser<'a, StrInput<'a>>,
    /// Single-item lookahead buffer (peeked event not yet consumed).
    look: Option<Ev>,
    /// For alias replay: a stack of injected buffers; we always read from the top first.
    inject: Vec<(Vec<Ev>, usize)>,
    /// Recorded buffers for anchors (id -> event slice).
    anchors: HashMap<usize, Vec<Ev>>,
    /// Recording frames for currently-open anchored containers.
    rec_stack: Vec<RecFrame>,
    /// Budget (raw events); independent of alias replay limits below.
    budget: Option<BudgetEnforcer>,

    /// Location of the last yielded event (for better error reporting).
    last_location: Location,

    /// Alias-bomb hardening limits and counters.
    /// Hard limit configuration for alias replaying.
    alias_limits: AliasLimits,
    /// Total number of replayed events across the whole stream (enforced by `alias_limits`).
    total_replayed_events: usize,
    /// Per-anchor replay expansion counters: anchor id -> number of expansions.
    per_anchor_expansions: HashMap<usize, usize>,
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
    pub(crate) fn new(input: &'a str, budget: Option<Budget>, alias_limits: AliasLimits) -> Self {
        Self {
            parser: Parser::new_from_str(input),
            look: None,
            inject: Vec::new(),
            anchors: HashMap::new(),
            rec_stack: Vec::new(),
            budget: budget.map(BudgetEnforcer::new),

            last_location: Location::unknown(),

            alias_limits,
            total_replayed_events: 0,
            per_anchor_expansions: HashMap::new(),
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
        if let Some((buf, idx)) = self.inject.last_mut() {
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

                Event::DocumentStart(_) | Event::DocumentEnd => {
                    // Skip document markers and reset per-document state.
                    self.reset_document_state();
                    self.last_location = location;
                    continue;
                }

                Event::Scalar(val, mut style, anchor_id, tag) => {
                    let s = match val {
                        Cow::Borrowed(v) => v.to_string(),
                        Cow::Owned(v) => v,
                    };
                    let tag_s = tag.map(|t| t.to_string());
                    if s.is_empty() && anchor_id != 0 && matches!(style, ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted) {
                        // Normalize: anchored empty scalars should behave like plain empty (null-like)
                        style = ScalarStyle::Plain;
                    }
                    let ev = Ev::Scalar {
                        value: s,
                        tag: tag_s,
                        style,
                        location,
                    };
                    self.record(&ev, false, false);
                    if anchor_id != 0 {
                        self.anchors.insert(anchor_id, vec![ev.clone()]);
                    }
                    self.last_location = location;
                    return Ok(Some(ev));
                }

                Event::SequenceStart(anchor_id, _tag) => {
                    let ev = Ev::SeqStart { location };
                    // Existing frames go deeper with this start.
                    self.bump_depth_on_start();
                    // Start recording for this anchor *after* bumping other frames,
                    // and include the start event in the new buffer.
                    if anchor_id != 0 {
                        self.rec_stack.push(RecFrame {
                            id: anchor_id,
                            depth: 1,
                            buf: vec![ev.clone()],
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
                    return Ok(Some(ev));
                }
                Event::SequenceEnd => {
                    let ev = Ev::SeqEnd { location };
                    self.record(&ev, false, false);
                    self.bump_depth_on_end()
                        .map_err(|err| err.with_location(location))?; // may finalize frames
                    self.last_location = location;
                    return Ok(Some(ev));
                }

                Event::MappingStart(anchor_id, _tag) => {
                    let ev = Ev::MapStart { location };
                    self.bump_depth_on_start();
                    if anchor_id != 0 {
                        self.rec_stack.push(RecFrame {
                            id: anchor_id,
                            depth: 1,
                            buf: vec![ev.clone()],
                        });
                    }
                    self.record(
                        &ev,
                        /*is_start*/ true,
                        /*seeded_new_frame*/ anchor_id != 0,
                    );
                    self.last_location = location;
                    return Ok(Some(ev));
                }
                Event::MappingEnd => {
                    let ev = Ev::MapEnd { location };
                    self.record(&ev, false, false);
                    self.bump_depth_on_end()
                        .map_err(|err| err.with_location(location))?;
                    self.last_location = location;
                    return Ok(Some(ev));
                }

                Event::Alias(anchor_id) => {
                    // Alias replay hardening.
                    let buf = self
                        .anchors
                        .get(&anchor_id)
                        .ok_or_else(|| Error::unknown_anchor(anchor_id).with_location(location))?
                        .clone();

                    let count = self
                        .per_anchor_expansions
                        .entry(anchor_id)
                        .and_modify(|c| *c += 1)
                        .or_insert(1);
                    if *count > self.alias_limits.max_alias_expansions_per_anchor {
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
                    self.inject.push((buf, 0));
                    return self.next_impl();
                }

                Event::Nothing => continue,
            }
        }

        Ok(None)
    }

    /// Reset per-document state when encountering a document boundary.
    ///
    /// Clears injected replay buffers, recorded anchors, current recording frames,
    /// and alias-expansion counters. Does not modify global parser state.
    fn reset_document_state(&mut self) {
        self.inject.clear();
        self.anchors.clear();
        self.rec_stack.clear();
        self.per_anchor_expansions.clear();
        self.total_replayed_events = 0;
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
            Ev::Scalar { value, style, .. } => {
                Event::Scalar(Cow::Owned(value.clone()), *style, 0, None)
            }
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
                // Push into all frames except the newest one (it already has this start).
                let last = self.rec_stack.len() - 1;
                for (i, fr) in self.rec_stack.iter_mut().enumerate() {
                    if i != last {
                        fr.buf.push(ev.clone());
                    }
                }
            } else {
                // Ordinary start: record into *all* frames.
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
                self.anchors.insert(done.id, done.buf);
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
    fn peek(&mut self) -> Result<Option<Ev>, Error> {
        if self.look.is_none() {
            self.look = self.next_impl()?;
        }
        Ok(self.look.clone())
    }
    fn last_location(&self) -> Location {
        self.last_location
    }
}
