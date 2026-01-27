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

use crate::budget::{BudgetEnforcer, EnforcingPolicy};
use crate::buffered_input::{ChunkedChars, buffered_input_from_reader_with_limit};
use crate::de::{AliasLimits, Budget, Error, Ev, Events, Location};
use crate::de_error::budget_error;
use crate::location::location_from_span;
use crate::tags::SfTag;
use saphyr_parser::{BufferedInput, Event, Parser, ScalarStyle, ScanError, Span, StrInput};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use crate::options::BudgetReportCallback;

type StreamReader<'a> = Box<dyn std::io::Read + 'a>;
type StreamBufReader<'a> = std::io::BufReader<StreamReader<'a>>;
type StreamInput<'a> = BufferedInput<ChunkedChars<StreamBufReader<'a>>>;
type StreamParser<'a> = Parser<'a, StreamInput<'a>>;


/// This is enough to hold a single scalar that is common  case in YAML anchors.
const SMALLVECT_INLINE: usize = 8;

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

/// Handle input polymorphism
pub(crate) enum SaphyrParser<'a> {
    StringParser(Parser<'a, StrInput<'a>>),
    StreamParser(StreamParser<'a>),
}

impl<'input> SaphyrParser<'input> {
    fn next(&mut self) -> Option<Result<(Event<'input>, Span), ScanError>> {
        match self {
            SaphyrParser::StringParser(parser) => parser.next(),
            SaphyrParser::StreamParser(parser) => parser.next(),
        }
    }
}

/// Live event source that wraps `saphyr_parser::Parser` and:
/// - Skips stream/document markers
/// - Records anchored subtrees (containers and scalars)
/// - Resolves aliases by injecting recorded buffers (replaying)
pub(crate) struct LiveEvents<'a> {
    /// Underlying streaming parser that produces raw events from the input.
    parser: SaphyrParser<'a>,
    /// Original input string (for zero-copy borrowing). `None` for reader-based input.
    input: Option<&'a str>,

    /// Whether any content event has been produced in the current stream.
    produced_any_in_doc: bool,
    /// Whether we emitted a synthetic null scalar to represent an empty document.
    synthesized_null_emitted: bool,
    /// Single-item lookahead buffer (peeked event not yet consumed).
    look: Option<Ev>,
    /// For alias replay: a stack of injected buffers; we always read from the top first.
    inject: Vec<InjectFrame>,
    /// Recorded buffers for anchors (index = anchor_id).
    /// `None` means the id is not recorded (e.g., never anchored or cleared).
    /// Saphyr's parser anchor_id is the sequential counter.
    anchors: Vec<Option<Box<[Ev]>>>,
    /// Recording frames for currently-open anchored containers.
    rec_stack: Vec<RecFrame>,
    /// Budget (raw events); independent of alias replay limits below.
    budget: Option<BudgetEnforcer>,
    /// Optional reporter to expose budget usage once parsing completes.
    budget_report: Option<fn(&crate::budget::BudgetReport)>,
    /// Optional reporter (new API)
    budget_report_cb: Option<BudgetReportCallback>,
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

    /// Error reference that is checked at the end of parsing.
    error: Rc<RefCell<Option<std::io::Error>>>,
}

/// A single alias-replay stack frame (one active `*alias` expansion).
#[derive(Clone, Copy, Debug)]
struct InjectFrame {
    /// Anchor id being replayed.
    ///
    /// This is the numeric anchor id produced by `saphyr_parser` (dense, increasing).
    /// It indexes into [`LiveEvents::anchors`], which stores the recorded event buffer
    /// for each anchored node.
    anchor_id: usize,

    /// Index of the next event to yield from the recorded anchor buffer.
    ///
    /// Invariant:
    /// - `idx <= anchors[anchor_id].len()`.
    /// - When `idx == len`, the frame is considered exhausted and will be popped,
    ///   but *not immediately* (see below).
    idx: usize,

    /// Use-site (reference) location of the alias token that caused this replay.
    ///
    /// Why do we need this:
    /// - While replaying an alias (`*a`), we yield events captured from the *anchored
    ///   definition*. Those events carry definition-site locations in [`Ev::location`].
    /// - For `Spanned<T>` we also want the use-site (“where the value was referenced in
    ///   the YAML”), so [`Events::reference_location`] needs to return the location of
    ///   the alias token rather than the replayed events' own locations.
    ///
    /// Lifetime/scope:
    /// - This location applies to the *next node* being deserialized from the replay.
    /// - We intentionally keep an exhausted frame on the stack until the next pump
    ///   in [`LiveEvents::next_impl`], so consumers can still query
    ///   `reference_location()` while deserializing the last yielded node.
    reference_location: Location,
}

impl<'a> LiveEvents<'a> {
    pub(crate) fn from_reader<R: std::io::Read + 'a>(
        inputs: R,
        budget: Option<Budget>,
        budget_report: Option<fn(&crate::budget::BudgetReport)>,
        budget_report_cb: Option<BudgetReportCallback>,
        alias_limits: AliasLimits,
        stop_at_doc_end: bool,
        policy: EnforcingPolicy,
    ) -> Self {
        // Build a streaming character iterator from the byte reader, honoring input byte cap if configured
        let max_bytes = budget.as_ref().and_then(|b| b.max_reader_input_bytes);
        let (input, error) = buffered_input_from_reader_with_limit(inputs, max_bytes);
        let parser = Parser::new(input);
        Self {
            produced_any_in_doc: false,
            synthesized_null_emitted: false,
            parser: SaphyrParser::StreamParser(parser),
            input: None, // Reader-based input cannot support zero-copy borrowing
            look: None,
            inject: Vec::with_capacity(2),
            anchors: Vec::with_capacity(8),
            rec_stack: Vec::with_capacity(2),
            budget: budget.map(|budget| BudgetEnforcer::new(budget, policy)),

            budget_report,
            budget_report_cb,

            last_location: Location::UNKNOWN,

            alias_limits,
            total_replayed_events: 0,
            per_anchor_expansions: Vec::new(),
            stop_at_doc_end,
            seen_doc_end: false,

            error,
        }
    }
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
    pub(crate) fn from_str(
        input: &'a str,
        budget: Option<Budget>,
        budget_report: Option<fn(&crate::budget::BudgetReport)>,
        budget_report_cb: Option<BudgetReportCallback>,
        alias_limits: AliasLimits,
        stop_at_doc_end: bool,
    ) -> Self {
        Self {
            produced_any_in_doc: false,
            synthesized_null_emitted: false,
            parser: SaphyrParser::StringParser(Parser::new_from_str(input)),
            input: Some(input),
            look: None,
            inject: Vec::with_capacity(2),
            anchors: Vec::with_capacity(8),
            rec_stack: Vec::with_capacity(2),
            budget: budget.map(|budget| BudgetEnforcer::new(budget, EnforcingPolicy::AllContent)),

            budget_report,
            budget_report_cb,

            last_location: Location::UNKNOWN,

            alias_limits,
            total_replayed_events: 0,
            per_anchor_expansions: Vec::new(),
            stop_at_doc_end,
            seen_doc_end: false,

            // Error field is provided but for string, nothing is ever reported
            error: Rc::new(RefCell::new(None)),
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
        //
        // Important subtlety: we keep an exhausted injection frame on the stack until
        // the *next* pump so `reference_location()` remains valid while deserializing
        // the last replayed node. That means the top of the stack may contain frames
        // with `idx == buf.len()`. Before we consider pulling from the real parser,
        // we must pop any such exhausted frames.
        loop {
            let Some(frame) = self.inject.last_mut() else {
                break;
            };
            let anchor_id = frame.anchor_id;
            let idx = &mut frame.idx;
            let buf = self
                .anchors
                .get(anchor_id)
                .and_then(|o| o.as_ref())
                .ok_or_else(|| {
                    Error::unknown_anchor(anchor_id).with_location(self.last_location)
                })?;

            if *idx >= buf.len() {
                // Exhausted: pop and continue (there may be another injected frame beneath).
                self.inject.pop();
                continue;
            }

            let ev = buf[*idx].clone();
            *idx += 1;
            // Do not pop the injection frame yet. `Spanned<T>` (and other consumers)
            // may query `reference_location()` while deserializing this just-yielded
            // node. We will pop the frame at the top of the next `next_impl()` call
            // if it is exhausted.

            match ev {
                Ev::SeqStart { .. } | Ev::MapStart { .. } => {}
                Ev::SeqEnd { .. } | Ev::MapEnd { .. } => {}
                Ev::Scalar { .. } => {}
                Ev::Taken { location } => {
                    return Err(Error::unexpected("consumed event").with_location(location));
                }
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
        }

        // 2) Pull from the real parser
        while let Some(item) = self.parser.next() {
            let (raw, span) = item.map_err(Error::from_scan_error)?;
            let location = location_from_span(&span);

            if let Some(ref mut budget) = self.budget
                && let Err(breach) = budget.observe(&raw)
            {
                return Err(budget_error(breach).with_location(location));
            }

            match raw {
                Event::Scalar(val, mut style, anchor_id, tag) => {
                    if matches!(style, ScalarStyle::Folded)
                        && span.start.col() == 0
                        && !val.trim().is_empty()
                    {
                        return Err(Error::msg("folded block scalars must indent their content")
                            .with_location(location));
                    }
                    
                    // Convert to owned string
                    let s = match val {
                        Cow::Borrowed(v) => v.to_string(),
                        Cow::Owned(v) => v,
                    };
                    
                    // Determine if this scalar can be borrowed from the input.
                    // For plain scalars without multi-line content, borrowing is possible.
                    // We defer the actual slice extraction to deserialize_str to avoid
                    // the O(n) character-to-byte index conversion on every scalar.
                    //
                    // Quoted and block scalars may have transformations (escapes, folding, etc.)
                    // so we don't attempt to borrow those.
                    let can_borrow = self.input.is_some() 
                        && matches!(style, ScalarStyle::Plain)
                        && !s.contains('\n');  // Multi-line plain scalars have folding
                    
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
                        raw_tag: tag.as_ref().map(|t| t.to_string()),
                        style,
                        anchor: anchor_id,
                        location,
                        can_borrow,
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
                    // Container-balance: count open containers independent of budgets/anchors.
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

                    if self.rec_stack.iter().any(|frame| frame.id == anchor_id) {
                        if crate::anchor_store::recursive_anchor_in_progress(anchor_id) {
                            let ev = Ev::Scalar {
                                value: String::new(),
                                tag: SfTag::Null,
                                raw_tag: None,
                                style: ScalarStyle::Plain,
                                anchor: anchor_id,
                                location,
                                can_borrow: false,
                            };
                            self.record(&ev, false, false);
                            self.last_location = location;
                            self.produced_any_in_doc = true;
                            return Ok(Some(ev));
                        }
                        return Err(Error::msg(
                            "Recursive references require weak recursion types",
                        )
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
                    self.inject.push(InjectFrame {
                        anchor_id,
                        idx: 0,
                        reference_location: location,
                    });
                    return self.next_impl();
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
                        // One-step lookahead to distinguish multi-doc streams from garbage
                        // after an explicit end marker. If the very next token is a
                        // DocumentStart, signal multi-doc error; otherwise ignore anything else.
                        if let Some(Ok((Event::DocumentStart(_), span2))) = self.parser.next() {
                            let loc2 = location_from_span(&span2);
                            return Err(
                                Error::msg(
                                    "multiple YAML documents detected; use from_multiple or from_multiple_with_options",
                                )
                                .with_location(loc2),
                            );
                        }
                        return Ok(None);
                    }
                    continue;
                }

                Event::StreamStart | Event::StreamEnd => {
                    // Skip stream markers.
                    self.last_location = location;
                    continue;
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
                raw_tag: None,
                style: ScalarStyle::Plain,
                anchor: 0,
                location: self.last_location,
                can_borrow: false,
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
        // Clear injected replay buffers and recording stack but keep capacity.
        self.inject.clear();
        self.rec_stack.clear();

        // Anchors are per-document. Instead of dropping the whole vec (which frees
        // capacity and may cause re-allocation in the next document), keep the
        // allocation and just clear the entries.
        for slot in &mut self.anchors {
            *slot = None;
        }

        // Reset per-anchor expansion counters without dropping capacity.
        for cnt in &mut self.per_anchor_expansions {
            *cnt = 0;
        }

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
            Ev::Taken { location } => {
                return Err(Error::unexpected("consumed event").with_location(*location));
            }
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
    #[cold]
    pub(crate) fn finish(&mut self) -> Result<(), Error> {
        self.io_error()?;
        if let Some(budget) = self.budget.take() {
            let report = budget.finalize();
            if let Some(callback) = self.budget_report {
                callback(&report);
            }
            let breached = report.breached.clone();
            if let Some(callback) = &self.budget_report_cb {
                callback.borrow_mut()(report);
            }
            if let Some(breach) = breached {
                return Err(budget_error(breach).with_location(self.last_location));
            }
        }
        Ok(())
    }

    #[cold]
    fn io_error(&self) -> Result<(), Error> {
        if let Some(error) = self.error.take() {
            Err(Error::IOError { cause: error })
        } else {
            Ok(())
        }
    }
}

impl<'de> Events<'de> for LiveEvents<'de> {
    /// Get the next event, using a single-item lookahead buffer if present.
    /// Updates last_location to the yielded event's location.
    fn next(&mut self) -> Result<Option<Ev>, Error> {
        self.io_error()?;

        if let Some(ev) = self.look.take() {
            self.last_location = ev.location();
            return Ok(Some(ev));
        }
        self.next_impl()
    }
    /// Peek at the next event without consuming it, filling the lookahead buffer if empty.
    fn peek(&mut self) -> Result<Option<&Ev>, Error> {
        self.io_error()?;

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

    fn reference_location(&self) -> Location {
        if let Some(frame) = self.inject.last() {
            return frame.reference_location;
        }
        self.look
            .as_ref()
            .map(|e| e.location())
            .unwrap_or(self.last_location)
    }

    fn input_for_borrowing(&self) -> Option<&'de str> {
        self.input
    }
}

impl<'a> LiveEvents<'a> {
    pub(crate) fn seen_doc_end(&self) -> bool {
        self.seen_doc_end
    }
    pub(crate) fn synthesized_null_emitted(&self) -> bool {
        self.synthesized_null_emitted
    }

    /// Skip events until the next document boundary or EOF.
    ///
    /// This is used for error recovery in the streaming reader: after a deserialization
    /// error mid-document, we consume remaining events until we see a `DocumentStart`
    /// (indicating the next document) or reach EOF. This allows the iterator to continue
    /// with subsequent documents.
    ///
    /// Returns `true` if a new document was found, `false` if EOF was reached.
    /// Syntax errors during skipping cause the method to return `false` (EOF-like).
    pub(crate) fn skip_to_next_document(&mut self) -> bool {
        // Clear any peeked event and injection state
        self.look = None;
        self.inject.clear();
        self.rec_stack.clear();

        // Pull raw events from the parser until we see DocumentStart or EOF
        while let Some(item) = self.parser.next() {
            let Ok((raw, span)) = item else {
                // Syntax error while skipping; treat as EOF
                return false;
            };
            let location = location_from_span(&span);
            self.last_location = location;

            match raw {
                Event::DocumentStart(_) => {
                    // Found the start of the next document
                    self.reset_document_state();
                    self.produced_any_in_doc = false;
                    return true;
                }
                Event::DocumentEnd => {
                    // End of current document; reset state and continue looking for next
                    self.reset_document_state();
                    self.produced_any_in_doc = false;
                }
                Event::StreamEnd => {
                    // End of stream
                    return false;
                }
                _ => {
                    // Skip all other events (scalars, mappings, sequences, etc.)
                    continue;
                }
            }
        }

        // Parser exhausted
        false
    }
}
