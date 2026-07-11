use crate::budget::BudgetBreach;
use crate::buffered_input::{
    ReaderInput, ReaderInputBytesRead, ReaderInputError,
    buffered_input_from_reader_with_limit_shared,
};
use crate::input_source::{IncludeResolveError, IncludeResolver, InputSource, ResolvedInclude};
use granit_parser::{Event, Parser, ScanError, Scanner, Span, StrInput, TokenType};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    io::Cursor,
    ops::RangeInclusive,
    rc::Rc,
};

type InnerStack<'input> =
    granit_parser::parser_stack::ParserStack<'input, core::iter::Empty<char>, ReaderInput<'input>>;
#[derive(Clone, Debug)]
pub(crate) struct SnippetFrame {
    pub(crate) name: String,
    pub(crate) text: Rc<str>,
}
#[derive(Clone, Debug)]
pub(crate) struct RecordedSource {
    pub(crate) parent_source_id: Option<u32>,
    pub(crate) name: String,
    pub(crate) text: Option<Rc<str>>,
    pub(crate) include_location: crate::Location,
}
/// A parser stack that supports serde-saphyr includes.
///
/// This delegates all anchor handling to `granit_parser::parser_stack::ParserStack` (which has
/// access to the parser's internal anchor-offset APIs), while allowing our include resolver to
/// return either owned text or an owned reader.
pub struct ParserStack<'input> {
    inner: InnerStack<'input>,
    include_resolver: Option<Box<IncludeResolver<'input>>>,
    io_error: ReaderInputError,
    reader_bytes_read: ReaderInputBytesRead,
    budget: crate::Budget,
    active_ids: Vec<(usize, String)>,
    next_source_id: u32,
    active_source_ids: Vec<u32>,
    pub(crate) resolved_sources: HashMap<u32, RecordedSource>,
}
impl<'input> ParserStack<'input> {
    #[must_use]
    pub fn new(
        io_error: ReaderInputError,
        reader_bytes_read: ReaderInputBytesRead,
        budget: &crate::Budget,
    ) -> Self {
        Self {
            inner: InnerStack::new(),
            include_resolver: None,
            io_error,
            reader_bytes_read,
            budget: budget.clone(),
            active_ids: Vec::new(),
            next_source_id: 1,
            active_source_ids: Vec::new(),
            resolved_sources: HashMap::new(),
        }
    }
    pub fn set_resolver(
        &mut self,
        resolver: impl FnMut(
            crate::input_source::IncludeRequest<'_>,
        ) -> Result<ResolvedInclude, IncludeResolveError>
        + 'input,
    ) {
        self.include_resolver = Some(Box::new(resolver));
    }
    pub fn has_resolver(&self) -> bool {
        self.include_resolver.is_some()
    }
    pub fn push_stream_parser(
        &mut self,
        parser: Parser<'input, ReaderInput<'input>>,
        name: String,
    ) {
        self.push_stream_parser_with_snippet(parser, name, None, crate::Location::UNKNOWN);
    }
    pub(crate) fn push_str_parser_with_snippet(
        &mut self,
        parser: Parser<'input, StrInput<'input>>,
        name: String,
        snippet: Option<SnippetFrame>,
        include_location: crate::Location,
    ) {
        let source_id = self.next_source_id;
        self.next_source_id += 1;
        let parent_source_id = self.active_source_ids.last().copied();
        self.active_source_ids.push(source_id);
        let recorded = RecordedSource {
            parent_source_id,
            name: snippet
                .as_ref()
                .map(|s| s.name.clone())
                .unwrap_or_else(|| name.clone()),
            text: snippet.as_ref().map(|s| s.text.clone()),
            include_location,
        };
        self.resolved_sources.insert(source_id, recorded);
        self.inner.push_str_parser(parser, name);
    }
    fn push_stream_parser_with_snippet(
        &mut self,
        parser: Parser<'input, ReaderInput<'input>>,
        name: String,
        snippet: Option<SnippetFrame>,
        include_location: crate::Location,
    ) {
        let source_id = self.next_source_id;
        self.next_source_id += 1;
        let parent_source_id = self.active_source_ids.last().copied();
        self.active_source_ids.push(source_id);
        let recorded = RecordedSource {
            parent_source_id,
            name: snippet
                .as_ref()
                .map(|s| s.name.clone())
                .unwrap_or_else(|| name.clone()),
            text: snippet.as_ref().map(|s| s.text.clone()),
            include_location,
        };
        self.resolved_sources.insert(source_id, recorded);
        self.inner.push_custom_parser(parser, name);
    }
    fn push_replay_parser_with_snippet(
        &mut self,
        parser: granit_parser::parser_stack::ReplayParser<'input>,
        name: String,
        snippet: Option<SnippetFrame>,
        include_location: crate::Location,
    ) {
        let source_id = self.next_source_id;
        self.next_source_id += 1;
        let parent_source_id = self.active_source_ids.last().copied();
        self.active_source_ids.push(source_id);
        let recorded = RecordedSource {
            parent_source_id,
            name: snippet
                .as_ref()
                .map(|s| s.name.clone())
                .unwrap_or_else(|| name.clone()),
            text: snippet.as_ref().map(|s| s.text.clone()),
            include_location,
        };
        self.resolved_sources.insert(source_id, recorded);
        self.inner.push_replay_parser(parser, name);
    }
    pub fn current_source_id(&self) -> u32 {
        self.active_source_ids.last().copied().unwrap_or(0)
    }
    pub(crate) fn recorded_source_chain(&self, source_id: u32) -> Vec<&RecordedSource> {
        let mut chain = Vec::new();
        let mut current = self.resolved_sources.get(&source_id);
        while let Some(source) = current {
            chain.push(source);
            current = source
                .parent_source_id
                .and_then(|parent_source_id| self.resolved_sources.get(&parent_source_id));
        }
        chain
    }
    fn sync_source_tracking(&mut self, current_len: usize) {
        self.active_ids.retain(|(depth, _)| *depth <= current_len);
        self.active_source_ids.truncate(current_len);
    }
    pub(crate) fn prune_resolved_sources(&mut self) {
        let active_source_ids: HashSet<u32> = self.active_source_ids.iter().copied().collect();
        self.resolved_sources
            .retain(|id, _| active_source_ids.contains(id));
    }
    pub fn resolve(
        &mut self,
        include_str: &str,
        location: crate::Location,
    ) -> Result<(), crate::de_error::Error> {
        let Some(resolver) = &mut self.include_resolver else {
            return Err(
                crate::de_error::Error::msg("No include resolver set for parser stack.")
                    .with_location(location),
            );
        };

        let include_depth = self.inner.stack().len() as u32;
        if include_depth > self.budget.max_inclusion_depth {
            return Err(crate::de_error::Error::Budget {
                breach: crate::budget::BudgetBreach::InclusionDepth {
                    depth: include_depth,
                },
                location,
            });
        }
        let from_name = self
            .active_source_ids
            .last()
            .and_then(|source_id| self.resolved_sources.get(source_id))
            .map(|source| source.name.as_str())
            .unwrap_or("");
        let from_id = self.active_ids.last().map(|(_, id)| id.as_str());

        let request = crate::input_source::IncludeRequest {
            spec: include_str,
            from_name,
            from_id,
            stack: self.inner.stack().into_iter().collect(),
            size_remaining: self
                .budget
                .max_reader_input_bytes
                .map(|limit| limit.saturating_sub(self.reader_bytes_read.get())),
            location,
        };
        let resolved = match resolver(request) {
            Ok(r) => r,
            Err(e) => {
                let stack = self.inner.stack().into_iter().collect();
                return Err(crate::de_error::Error::ResolverError {
                    target: include_str.to_string(),
                    error: e,
                    stack,
                    location,
                });
            }
        };
        if self.active_ids.iter().any(|(_, id)| id == &resolved.id) {
            let stack = self.inner.stack().into_iter().collect();
            return Err(crate::de_error::Error::CyclicInclude {
                id: resolved.id,
                stack,
                location,
            });
        }
        // Track the include as active at the depth of the pushed parser.
        let active_depth = self.inner.stack().len() + 1;
        self.active_ids.push((active_depth, resolved.id));
        let name = resolved.name;
        match resolved.source {
            InputSource::Text(mut s) => {
                if s.trim().is_empty() {
                    s = "~".to_string();
                }
                let snippet = SnippetFrame {
                    name: name.clone(),
                    text: Rc::from(s),
                };
                let cursor = Cursor::new(snippet.text.as_ref().as_bytes().to_vec());
                let input = buffered_input_from_reader_with_limit_shared(
                    cursor,
                    self.budget.max_reader_input_bytes,
                    self.io_error.clone(),
                    self.reader_bytes_read.clone(),
                );
                let parser = Parser::new(input);
                self.push_stream_parser_with_snippet(parser, name, Some(snippet), location);
            }
            InputSource::Reader(r) => {
                let input = buffered_input_from_reader_with_limit_shared(
                    r,
                    self.budget.max_reader_input_bytes,
                    self.io_error.clone(),
                    self.reader_bytes_read.clone(),
                );
                let parser = Parser::new(input);
                self.push_stream_parser_with_snippet(parser, name, None, crate::Location::UNKNOWN);
            }
            InputSource::AnchoredText { mut text, anchor } => {
                let text_len = text.len();
                if let Some(limit) = self.budget.max_reader_input_bytes {
                    let current = self.reader_bytes_read.get();
                    if current + text_len > limit {
                        return Err(crate::de_error::Error::ResolverError {
                            target: include_str.to_string(),
                            error: crate::IncludeResolveError::Message(format!(
                                "input byte limit {} exceeded by {} included bytes",
                                limit, text_len
                            )),
                            stack: self.inner.stack().into_iter().collect(),
                            location,
                        });
                    }
                    self.reader_bytes_read.set(current + text_len);
                }

                if text.trim().is_empty() {
                    text = "~".to_string();
                }
                let snippet = SnippetFrame {
                    name: name.clone(),
                    text: Rc::from(text.as_str()),
                };
                let events = collect_anchor_events(
                    &text,
                    &anchor,
                    self.inner.current_anchor_offset(),
                    &self.budget,
                )
                .map_err(|error| match error {
                    CollectAnchorEventsError::Budget(breach) => {
                        crate::de_error::budget_error(breach).with_location(location)
                    }
                    CollectAnchorEventsError::Message(message) => {
                        crate::de_error::Error::ResolverError {
                            target: include_str.to_string(),
                            error: crate::IncludeResolveError::Message(message),
                            stack: self.inner.stack().into_iter().collect(),
                            location,
                        }
                    }
                })?;
                self.push_replay_parser_with_snippet(
                    granit_parser::parser_stack::ReplayParser::new(
                        events.events,
                        events.anchor_offset,
                    ),
                    name,
                    Some(snippet),
                    location,
                );
            }
        }
        Ok(())
    }
}
#[derive(Debug)]
struct CollectedAnchorEvents {
    events: Vec<(Event<'static>, Span)>,
    anchor_offset: usize,
}

#[derive(Debug)]
enum CollectAnchorEventsError {
    Message(String),
    Budget(BudgetBreach),
}

fn anchored_event_initial_depth(event: &Event<'_>) -> usize {
    match event {
        Event::SequenceStart(_, _, _) | Event::MappingStart(_, _, _) => 1,
        _ => 0,
    }
}

/// Adds one expanded event's retained comment bytes to the include-fragment budget.
fn observe_expanded_comment_budget(
    event: &Event<'_>,
    total_expanded_comment_bytes: &mut usize,
    budget: &crate::Budget,
) -> Result<(), CollectAnchorEventsError> {
    if let Event::Comment(text, _) = event {
        *total_expanded_comment_bytes = total_expanded_comment_bytes.saturating_add(text.len());

        if *total_expanded_comment_bytes > budget.max_total_comment_bytes {
            return Err(CollectAnchorEventsError::Budget(
                BudgetBreach::CommentBytes {
                    total_comment_bytes: *total_expanded_comment_bytes,
                },
            ));
        }
    }

    Ok(())
}

/// Adds one expanded event's retained scalar bytes to the include-fragment budget.
fn observe_expanded_scalar_budget(
    event: &Event<'_>,
    total_expanded_scalar_bytes: &mut usize,
    budget: &crate::Budget,
) -> Result<(), CollectAnchorEventsError> {
    if let Event::Scalar(value, _, _, _) = event {
        *total_expanded_scalar_bytes = total_expanded_scalar_bytes.saturating_add(value.len());

        if *total_expanded_scalar_bytes > budget.max_total_scalar_bytes {
            return Err(CollectAnchorEventsError::Budget(
                BudgetBreach::ScalarBytes {
                    total_scalar_bytes: *total_expanded_scalar_bytes,
                },
            ));
        }
    }

    Ok(())
}

fn collect_anchor_events(
    text: &str,
    target_anchor: &str,
    anchor_offset: usize,
    budget: &crate::Budget,
) -> Result<CollectedAnchorEvents, CollectAnchorEventsError> {
    let mut document_count = 0usize;
    let mut anchor_defs: Vec<(String, usize)> = Vec::new();
    let mut scanner = Scanner::new(StrInput::new(text));
    for token in &mut scanner {
        let marker_offset = token.0.start.index();
        if let TokenType::Anchor(name) = token.1 {
            anchor_defs.push((name.into_owned(), marker_offset));
            if anchor_defs.len() > budget.max_anchors {
                return Err(CollectAnchorEventsError::Budget(BudgetBreach::Anchors {
                    anchors: anchor_defs.len(),
                }));
            }
        }
    }
    if let Some(err) = scanner.get_error() {
        return Err(CollectAnchorEventsError::Message(format!(
            "failed to scan include fragment '{}': {}",
            target_anchor, err
        )));
    }
    let mut parser = Parser::new_from_str(text);
    parser.set_anchor_offset(anchor_offset.max(1));
    let mut events = Vec::new();
    let mut current_depth: usize = 0;
    let mut total_scalar_bytes: usize = 0;
    let mut total_comment_bytes: usize = 0;
    while let Some(event) = parser.next_event() {
        let (event, span) = event.map_err(|err| {
            CollectAnchorEventsError::Message(format!(
                "failed to parse include fragment '{}': {}",
                target_anchor, err
            ))
        })?;
        if matches!(event, Event::DocumentStart(..)) {
            document_count += 1;
            if document_count > 1 {
                return Err(CollectAnchorEventsError::Message(format!(
                    "include fragment '{}' must come from a single YAML document",
                    target_anchor
                )));
            }
        }
        match &event {
            Event::SequenceStart(_, _, _) | Event::MappingStart(_, _, _) => {
                current_depth += 1;
                if current_depth > budget.max_depth {
                    return Err(CollectAnchorEventsError::Budget(BudgetBreach::Depth {
                        depth: current_depth,
                    }));
                }
            }
            Event::SequenceEnd | Event::MappingEnd => {
                current_depth = current_depth.saturating_sub(1);
            }
            _ => {}
        }
        if let Event::Scalar(ref value, _, _, _) = event {
            total_scalar_bytes = total_scalar_bytes.saturating_add(value.len());
            if total_scalar_bytes > budget.max_total_scalar_bytes {
                return Err(CollectAnchorEventsError::Budget(
                    BudgetBreach::ScalarBytes { total_scalar_bytes },
                ));
            }
        }
        if let Event::Comment(ref text, _) = event {
            total_comment_bytes = total_comment_bytes.saturating_add(text.len());
            if total_comment_bytes > budget.max_total_comment_bytes {
                return Err(CollectAnchorEventsError::Budget(
                    BudgetBreach::CommentBytes {
                        total_comment_bytes,
                    },
                ));
            }
        }
        events.push((own_event(event), span));
        if events.len() > budget.max_events {
            return Err(CollectAnchorEventsError::Budget(BudgetBreach::Events {
                events: events.len(),
            }));
        }
    }

    let mut anchor_nodes_by_name: HashMap<String, RangeInclusive<usize>> =
        HashMap::with_capacity(anchor_defs.len());
    let mut anchor_nodes_by_id: HashMap<usize, RangeInclusive<usize>> =
        HashMap::with_capacity(anchor_defs.len());
    let mut event_cursor = 0usize;

    for (name, offset) in &anchor_defs {
        while event_cursor < events.len() && events[event_cursor].1.start.index() < *offset {
            event_cursor += 1;
        }
        if event_cursor >= events.len() {
            break;
        }

        let mut node_start = event_cursor;
        while node_start < events.len() && !events[node_start].0.is_node() {
            node_start += 1;
        }
        if node_start >= events.len() {
            break;
        }

        let start = events[event_cursor..node_start]
            .iter()
            .position(|(event, _)| matches!(event, Event::Comment(_, _)))
            .map_or(node_start, |comment_offset| event_cursor + comment_offset);
        let mut end = node_start;
        let mut depth = anchored_event_initial_depth(&events[node_start].0);
        if depth == 0 {
            end = node_start;
        } else {
            for (idx, (event, _)) in events.iter().enumerate().skip(node_start + 1) {
                match event {
                    Event::SequenceStart(_, _, _) | Event::MappingStart(_, _, _) => depth += 1,
                    Event::SequenceEnd | Event::MappingEnd => {
                        if depth == 0 {
                            return Err(CollectAnchorEventsError::Message(format!(
                                "include fragment '{}' became unbalanced while replaying events",
                                target_anchor
                            )));
                        }
                        depth -= 1;
                        if depth == 0 {
                            end = idx;
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        let node_range = start..=end;
        if let Some(anchor_id) = events[node_start].0.anchor_id() {
            anchor_nodes_by_id.insert(anchor_id, node_range.clone());
        }
        anchor_nodes_by_name.insert(name.clone(), node_range);
        event_cursor = node_start + 1;
    }

    let target_events = anchor_nodes_by_name
        .get(target_anchor)
        .cloned()
        .ok_or_else(|| {
            CollectAnchorEventsError::Message(format!(
                "include fragment '{}' was not found",
                target_anchor
            ))
        })?;

    let mut expanded_events = Vec::new();
    let mut to_process: Vec<usize> = target_events.rev().collect();
    let mut expansion_count = 0;
    let mut expanded_scalar_bytes = 0usize;
    let mut expanded_comment_bytes = 0usize;

    while let Some(event_index) = to_process.pop() {
        let (event, span) = &events[event_index];
        if let Event::Alias(id) = &event
            && let Some(alias_events) = anchor_nodes_by_id.get(id)
        {
            expansion_count += 1;
            if expansion_count > budget.max_aliases {
                return Err(CollectAnchorEventsError::Budget(BudgetBreach::Aliases {
                    aliases: expansion_count,
                }));
            }
            for alias_event_index in alias_events.clone().rev() {
                to_process.push(alias_event_index);
            }
            continue;
        }
        observe_expanded_scalar_budget(event, &mut expanded_scalar_bytes, budget)?;
        observe_expanded_comment_budget(event, &mut expanded_comment_bytes, budget)?;
        expanded_events.push((event.clone(), *span));
        if expanded_events.len() > budget.max_events {
            return Err(CollectAnchorEventsError::Budget(BudgetBreach::Events {
                events: expanded_events.len(),
            }));
        }
    }

    Ok(CollectedAnchorEvents {
        events: expanded_events,
        anchor_offset: parser.get_anchor_offset(),
    })
}
fn own_event(event: Event<'_>) -> Event<'static> {
    match event {
        Event::Nothing => Event::Nothing,
        Event::StreamStart => Event::StreamStart,
        Event::StreamEnd => Event::StreamEnd,
        Event::DocumentStart(explicit, version) => Event::DocumentStart(explicit, version),
        Event::DocumentEnd => Event::DocumentEnd,
        Event::Alias(anchor_id) => Event::Alias(anchor_id),
        Event::Scalar(value, style, anchor_id, tag) => Event::Scalar(
            Cow::Owned(value.into_owned()),
            style,
            anchor_id,
            tag.map(|tag| Cow::Owned(tag.into_owned())),
        ),
        Event::SequenceStart(style, anchor_id, tag) => Event::SequenceStart(
            style,
            anchor_id,
            tag.map(|tag| Cow::Owned(tag.into_owned())),
        ),
        Event::SequenceEnd => Event::SequenceEnd,
        Event::MappingStart(style, anchor_id, tag) => Event::MappingStart(
            style,
            anchor_id,
            tag.map(|tag| Cow::Owned(tag.into_owned())),
        ),
        Event::MappingEnd => Event::MappingEnd,
        Event::Comment(text, placement) => Event::Comment(Cow::Owned(text.into_owned()), placement),
    }
}
impl<'input> Iterator for ParserStack<'input> {
    type Item = Result<(Event<'input>, Span), ScanError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(Err(e)) => {
                // Do not sync source tracking yet on error. The caller (like `LiveEvents`)
                // needs `current_source_id()` to map the error location correctly.
                // If they continue iterating, the next `Ok` event will sync it.
                Some(Err(e))
            }
            Some(Ok((Event::DocumentStart(explicit, version), span))) => {
                self.sync_source_tracking(self.inner.stack().len());
                if self.inner.stack().len() == 1 {
                    self.prune_resolved_sources();
                }
                Some(Ok((Event::DocumentStart(explicit, version), span)))
            }
            other => {
                self.sync_source_tracking(self.inner.stack().len());
                other
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    fn expect_budget_breach(error: CollectAnchorEventsError) -> BudgetBreach {
        match error {
            CollectAnchorEventsError::Budget(breach) => breach,
            CollectAnchorEventsError::Message(message) => {
                panic!("expected budget breach, got message error: {message}")
            }
        }
    }

    fn expect_fragment_message(error: CollectAnchorEventsError) -> String {
        match error {
            CollectAnchorEventsError::Message(message) => message,
            CollectAnchorEventsError::Budget(breach) => {
                panic!("expected message error, got budget breach: {breach:?}")
            }
        }
    }

    fn push_test_str_parser<'input>(
        stack: &mut ParserStack<'input>,
        parser: Parser<'input, StrInput<'input>>,
        name: &str,
    ) {
        stack.push_str_parser_with_snippet(
            parser,
            name.to_string(),
            None,
            crate::Location::UNKNOWN,
        );
    }

    #[test]
    fn collect_anchor_events_expands_aliases_defined_outside_target_anchor() {
        let mut scanner = Scanner::new(StrInput::new(
            "base: &base\n  name: Alice\n\nselected: &selected\n  user: *base\n",
        ));
        let mut scanned = Vec::new();
        for token in &mut scanner {
            if let TokenType::Anchor(name) = token.1 {
                scanned.push(name.into_owned());
            }
        }
        assert_eq!(scanned, vec!["base".to_string(), "selected".to_string()]);

        let collected = collect_anchor_events(
            "base: &base\n  name: Alice\n\nselected: &selected\n  user: *base\n",
            "selected",
            0,
            &crate::Budget::default(),
        )
        .expect("anchor collection should succeed");

        let has_user_key = collected.events.iter().any(
            |(event, _)| matches!(event, Event::Scalar(value, _, _, _) if value.as_ref() == "user"),
        );
        let has_name_key = collected.events.iter().any(
            |(event, _)| matches!(event, Event::Scalar(value, _, _, _) if value.as_ref() == "name"),
        );

        assert!(has_user_key, "target mapping key should be preserved");
        assert!(
            has_name_key,
            "alias value should be expanded from prerequisite anchor"
        );
        assert!(
            !collected
                .events
                .iter()
                .any(|(event, _)| matches!(event, Event::Alias(_))),
            "expanded event stream should not retain unresolved aliases"
        );
    }

    #[test]
    fn collect_anchor_events_skips_comment_between_anchor_and_node() {
        let collected = collect_anchor_events(
            "selected: &selected # note\n  user: Alice\n",
            "selected",
            0,
            &crate::Budget::default(),
        )
        .expect("anchor collection should select the mapping after the comment");

        let first_node = collected.events.iter().find(|(event, _)| event.is_node());
        assert!(
            matches!(first_node, Some((Event::MappingStart(_, _, _), _))),
            "target anchor's first node should be the mapping, got: {first_node:?}"
        );
        assert!(
            collected.events.iter().any(
                |(event, _)| matches!(event, Event::Scalar(value, _, _, _) if value.as_ref() == "user"),
            ),
            "target mapping key should be preserved"
        );
        assert!(
            !matches!(collected.events.as_slice(), [(Event::Comment(_, _), _)]),
            "comment must not be the only collected anchor event"
        );
    }

    #[test]
    fn collect_anchor_events_resolves_alias_to_preceding_shadowed_anchor() {
        let collected = collect_anchor_events(
            "x: &x 1\nselected: &selected\n  value: *x\nx: &x 2\n",
            "selected",
            0,
            &crate::Budget::default(),
        )
        .expect("anchor collection should resolve alias by parser anchor id");

        assert!(
            collected.events.iter().any(
                |(event, _)| matches!(event, Event::Scalar(value, _, _, _) if value.as_ref() == "1"),
            ),
            "alias should expand to the preceding &x definition"
        );
        assert!(
            !collected.events.iter().any(
                |(event, _)| matches!(event, Event::Scalar(value, _, _, _) if value.as_ref() == "2"),
            ),
            "later &x redefinition must not affect an earlier alias"
        );
        assert!(
            !collected
                .events
                .iter()
                .any(|(event, _)| matches!(event, Event::Alias(_))),
            "expanded event stream should not retain unresolved aliases"
        );
    }

    #[test]
    fn collect_anchor_events_uses_materialized_text_without_reader_budget_counter() {
        let collected = collect_anchor_events(
            "base: &base\n  name: Alice\n",
            "base",
            0,
            &crate::Budget::default(),
        )
        .expect("materialized fragment should parse without shared reader counting");

        assert!(
            !collected.events.is_empty(),
            "materialized fragment should still parse"
        );
    }

    #[test]
    fn collect_anchor_events_enforces_max_events_while_collecting_raw_events() {
        let budget = crate::Budget {
            max_events: 6,
            ..crate::Budget::default()
        };

        let error = match collect_anchor_events(
            "first: 1\nsecond: 2\nselected: &selected 3\n",
            "selected",
            0,
            &budget,
        ) {
            Ok(_) => panic!("raw event collection should stop once the budget is exceeded"),
            Err(error) => error,
        };

        assert!(matches!(
            expect_budget_breach(error),
            BudgetBreach::Events { events } if events > 6
        ));
    }

    #[test]
    fn collect_anchor_events_enforces_max_depth() {
        let budget = crate::Budget {
            max_depth: 2,
            ..crate::Budget::default()
        };
        // 3 levels of nesting: mapping > sequence > mapping
        let yaml = "root: &root\n  items:\n    - nested:\n        deep: value\n";
        let error = collect_anchor_events(yaml, "root", 0, &budget)
            .expect_err("should reject deeply nested fragment");
        assert!(matches!(
            expect_budget_breach(error),
            BudgetBreach::Depth { depth } if depth > 2
        ));
    }

    #[test]
    fn collect_anchor_events_enforces_max_anchors() {
        let budget = crate::Budget {
            max_anchors: 1,
            ..crate::Budget::default()
        };
        let yaml = "a: &a 1\nb: &b 2\nc: &c 3\n";
        let error = collect_anchor_events(yaml, "a", 0, &budget)
            .expect_err("should reject too many anchors");
        assert!(matches!(
            expect_budget_breach(error),
            BudgetBreach::Anchors { anchors } if anchors > 1
        ));
    }

    #[test]
    fn collect_anchor_events_enforces_max_total_scalar_bytes() {
        let budget = crate::Budget {
            max_total_scalar_bytes: 5,
            ..crate::Budget::default()
        };
        let yaml = "a: &a hello_world\nb: &b tiny\n";
        let error = collect_anchor_events(yaml, "a", 0, &budget)
            .expect_err("should reject when scalar bytes exceed budget");
        assert!(matches!(
            expect_budget_breach(error),
            BudgetBreach::ScalarBytes { total_scalar_bytes } if total_scalar_bytes > 5
        ));
    }

    #[test]
    fn collect_anchor_events_enforces_max_total_comment_bytes() {
        let budget = crate::Budget {
            max_total_comment_bytes: 5,
            ..crate::Budget::default()
        };
        let yaml = "#abcdef\na: &a ok\n";
        let error = collect_anchor_events(yaml, "a", 0, &budget)
            .expect_err("should reject when comment bytes exceed budget");
        assert!(matches!(
            expect_budget_breach(error),
            BudgetBreach::CommentBytes { total_comment_bytes } if total_comment_bytes > 5
        ));
    }

    #[test]
    fn collect_anchor_events_enforces_expanded_comment_bytes() {
        let budget = crate::Budget {
            max_total_comment_bytes: 8,
            ..crate::Budget::default()
        };

        let yaml = "\
base: &base
  #abcdef
  k: v
selected: &selected
  a: *base
  b: *base
";

        let error = collect_anchor_events(yaml, "selected", 0, &budget)
            .expect_err("alias-expanded comments should exceed expanded comment budget");

        assert!(matches!(
            expect_budget_breach(error),
            BudgetBreach::CommentBytes { total_comment_bytes } if total_comment_bytes > 8
        ));
    }

    #[test]
    fn collect_anchor_events_enforces_expanded_scalar_bytes() {
        let budget = crate::Budget {
            max_total_scalar_bytes: 25,
            ..crate::Budget::default()
        };

        let yaml = "\
base: &base abcdefghij
selected: &selected
  - *base
  - *base
  - *base
";

        let error = collect_anchor_events(yaml, "selected", 0, &budget)
            .expect_err("alias-expanded scalars should exceed expanded scalar budget");

        assert!(matches!(
            expect_budget_breach(error),
            BudgetBreach::ScalarBytes { total_scalar_bytes } if total_scalar_bytes > 25
        ));
    }

    #[test]
    fn anchored_text_expanded_scalar_budget_surfaces_as_budget_error() {
        let io_error = Rc::new(RefCell::new(None));
        let reader_bytes_read = Rc::new(Cell::new(0));
        let budget = crate::Budget {
            max_total_scalar_bytes: 25,
            ..crate::Budget::default()
        };
        let mut stack = ParserStack::new(io_error, reader_bytes_read, &budget);
        let anchored_text = "\
base: &base abcdefghij
selected: &selected
  - *base
  - *base
  - *base
"
        .to_string();
        stack.set_resolver(move |req| {
            assert_eq!(req.spec, "f.yml#selected");
            Ok(ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: InputSource::AnchoredText {
                    text: anchored_text.clone(),
                    anchor: "selected".to_string(),
                },
            })
        });
        push_test_str_parser(&mut stack, Parser::new_from_str("root: 1\n"), "root.yaml");

        let error = stack
            .resolve("f.yml#selected", crate::Location::UNKNOWN)
            .expect_err("expanded scalar bytes should surface as a budget error");

        assert!(
            matches!(
                error,
                crate::de_error::Error::Budget {
                    breach: BudgetBreach::ScalarBytes { total_scalar_bytes },
                    ..
                } if total_scalar_bytes > 25
            ),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn collect_anchor_events_rejects_multi_document_sources() {
        let error = collect_anchor_events(
            "first: &first 1\n---\nselected: &selected\n  user: *first\n",
            "selected",
            0,
            &crate::Budget::default(),
        )
        .expect_err("fragment collection should reject multi-document sources");
        let error = expect_fragment_message(error);

        assert!(
            error.contains("must come from a single YAML document"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn source_ids_start_from_one_and_zero_stays_unknown() {
        let io_error = Rc::new(RefCell::new(None));
        let reader_bytes_read = Rc::new(Cell::new(0));
        let mut stack = ParserStack::new(io_error, reader_bytes_read, &crate::Budget::default());
        assert_eq!(stack.current_source_id(), 0);
        push_test_str_parser(&mut stack, Parser::new_from_str("root: 1"), "root.yaml");
        assert_eq!(stack.current_source_id(), 1);
        push_test_str_parser(&mut stack, Parser::new_from_str("child: 2"), "child.yaml");
        assert_eq!(stack.current_source_id(), 2);
    }
    #[test]
    fn resolved_sources_retained_after_included_parser_pops() {
        let io_error = Rc::new(RefCell::new(None));
        let reader_bytes_read = Rc::new(Cell::new(0));
        let mut stack = ParserStack::new(io_error, reader_bytes_read, &crate::Budget::default());
        stack.set_resolver(|req| {
            assert_eq!(req.spec, "child.yaml");
            Ok(ResolvedInclude {
                id: "child.yaml".to_string(),
                name: "child.yaml".to_string(),
                source: InputSource::Text("child: 1\n".to_string()),
            })
        });
        push_test_str_parser(&mut stack, Parser::new_from_str("root: 1\n"), "root.yaml");
        assert_eq!(stack.resolved_sources.len(), 1);
        stack
            .resolve("child.yaml", crate::Location::UNKNOWN)
            .expect("child include resolves");
        assert_eq!(stack.current_source_id(), 2);
        assert_eq!(stack.resolved_sources.len(), 2);
        while stack.current_source_id() == 2 {
            let item = stack.next().expect("child parser still yields events");
            assert!(item.is_ok(), "child parser should parse successfully");
        }
        assert_eq!(stack.current_source_id(), 1);
        assert_eq!(stack.resolved_sources.len(), 2);
        assert!(stack.resolved_sources.contains_key(&1));
        assert!(stack.resolved_sources.contains_key(&2));
    }
    #[test]
    fn resolved_sources_pruned_on_next_document_start() {
        let io_error = Rc::new(RefCell::new(None));
        let reader_bytes_read = Rc::new(Cell::new(0));
        let mut stack = ParserStack::new(io_error, reader_bytes_read, &crate::Budget::default());
        // A stream with two documents
        push_test_str_parser(
            &mut stack,
            Parser::new_from_str("first: 1\n---\nsecond: 2\n"),
            "multi.yaml",
        );

        // Push a dummy child source ID to simulate an include during the first document
        stack.resolved_sources.insert(
            999,
            RecordedSource {
                parent_source_id: Some(1),
                name: "dummy.yaml".to_string(),
                text: None,
                include_location: crate::Location::UNKNOWN,
            },
        );
        // Read until the first document finishes and the second document starts
        let mut doc_starts = 0;
        for item in stack.by_ref() {
            if let Ok((Event::DocumentStart(..), _)) = item {
                doc_starts += 1;
                if doc_starts == 2 {
                    break;
                }
            }
        }
        assert_eq!(doc_starts, 2);
        // The root source (id 1) should be retained, but the dummy child (id 999) pruned
        assert!(stack.resolved_sources.contains_key(&1));
        assert!(!stack.resolved_sources.contains_key(&999));
        assert_eq!(stack.resolved_sources.len(), 1);
    }

    #[test]
    fn max_inclusion_depth_zero_disables_includes() {
        let io_error = Rc::new(RefCell::new(None));
        let reader_bytes_read = Rc::new(Cell::new(0));
        let budget = crate::Budget {
            max_inclusion_depth: 0,
            ..crate::Budget::default()
        };
        let mut stack = ParserStack::new(io_error, reader_bytes_read, &budget);
        stack.set_resolver(|_| {
            Ok(ResolvedInclude {
                id: "child.yaml".to_string(),
                name: "child.yaml".to_string(),
                source: InputSource::Text("child: 1\n".to_string()),
            })
        });
        push_test_str_parser(&mut stack, Parser::new_from_str("root: 1\n"), "root.yaml");

        let err = stack
            .resolve("child.yaml", crate::Location::UNKNOWN)
            .expect_err("include depth 0 should reject includes");

        assert!(matches!(
            err,
            crate::de_error::Error::Budget {
                breach: crate::budget::BudgetBreach::InclusionDepth { depth: 1 },
                ..
            }
        ));
    }

    #[test]
    fn max_inclusion_depth_allows_nested_includes_up_to_limit() {
        let io_error = Rc::new(RefCell::new(None));
        let reader_bytes_read = Rc::new(Cell::new(0));
        let budget = crate::Budget {
            max_inclusion_depth: 2,
            ..crate::Budget::default()
        };
        let mut stack = ParserStack::new(io_error, reader_bytes_read, &budget);
        stack.set_resolver(|req| match req.spec {
            "child.yaml" => Ok(ResolvedInclude {
                id: "child.yaml".to_string(),
                name: "child.yaml".to_string(),
                source: InputSource::Text("child: 1\n".to_string()),
            }),
            "grandchild.yaml" => Ok(ResolvedInclude {
                id: "grandchild.yaml".to_string(),
                name: "grandchild.yaml".to_string(),
                source: InputSource::Text("grandchild: 1\n".to_string()),
            }),
            other => panic!("unexpected include request: {other}"),
        });
        push_test_str_parser(&mut stack, Parser::new_from_str("root: 1\n"), "root.yaml");

        stack
            .resolve("child.yaml", crate::Location::UNKNOWN)
            .expect("first include within limit");
        stack
            .resolve("grandchild.yaml", crate::Location::UNKNOWN)
            .expect("second include within limit");

        assert_eq!(stack.current_source_id(), 3);
    }

    #[test]
    fn max_inclusion_depth_rejects_nested_include_beyond_limit() {
        let io_error = Rc::new(RefCell::new(None));
        let reader_bytes_read = Rc::new(Cell::new(0));
        let budget = crate::Budget {
            max_inclusion_depth: 1,
            ..crate::Budget::default()
        };
        let mut stack = ParserStack::new(io_error, reader_bytes_read, &budget);
        stack.set_resolver(|req| match req.spec {
            "child.yaml" => Ok(ResolvedInclude {
                id: "child.yaml".to_string(),
                name: "child.yaml".to_string(),
                source: InputSource::Text("child: 1\n".to_string()),
            }),
            "grandchild.yaml" => Ok(ResolvedInclude {
                id: "grandchild.yaml".to_string(),
                name: "grandchild.yaml".to_string(),
                source: InputSource::Text("grandchild: 1\n".to_string()),
            }),
            other => panic!("unexpected include request: {other}"),
        });
        push_test_str_parser(&mut stack, Parser::new_from_str("root: 1\n"), "root.yaml");

        stack
            .resolve("child.yaml", crate::Location::UNKNOWN)
            .expect("first include within limit");

        let err = stack
            .resolve("grandchild.yaml", crate::Location::UNKNOWN)
            .expect_err("second include should exceed limit");

        assert!(matches!(
            err,
            crate::de_error::Error::Budget {
                breach: crate::budget::BudgetBreach::InclusionDepth { depth: 2 },
                ..
            }
        ));
    }
}
