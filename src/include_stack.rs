use crate::buffered_input::{
    buffered_input_from_reader_with_limit_shared, ReaderInput, ReaderInputBytesRead,
    ReaderInputError,
};
use crate::input_source::{IncludeResolveError, IncludeResolver, InputSource, ResolvedInclude};
use saphyr_parser::{Event, Parser, ScanError, Scanner, Span, StrInput, TokenType};
use std::rc::Rc;
type InnerStack<'input> = saphyr_parser::parser_stack::ParserStack<'input, core::iter::Empty<char>, ReaderInput<'input>>;
#[derive(Clone, Debug)]
pub(crate) struct SnippetFrame {
    pub(crate) name: String,
    pub(crate) text: Rc<str>,
    #[allow(dead_code)]
    pub(crate) include_location: crate::Location,
}
#[derive(Clone, Debug)]
pub(crate) struct RecordedSource {
    #[allow(dead_code)]
    pub(crate) source_id: u32,
    pub(crate) parent_source_id: Option<u32>,
    pub(crate) name: String,
    pub(crate) text: Option<Rc<str>>,
    pub(crate) include_location: crate::Location,
}
/// A parser stack that supports serde-saphyr includes.
///
/// This delegates all anchor handling to `saphyr_parser::parser_stack::ParserStack` (which has
/// access to the parser's internal anchor-offset APIs), while allowing our include resolver to
/// return either owned text or an owned reader.
pub struct ParserStack<'input> {
    inner: InnerStack<'input>,
    include_resolver: Option<Box<IncludeResolver<'input>>>,
    io_error: ReaderInputError,
    reader_bytes_read: ReaderInputBytesRead,
    max_reader_input_bytes: Option<usize>,
    max_inclusion_depth: u32,
    active_ids: Vec<(usize, String)>,
    snippet_frames: Vec<Option<SnippetFrame>>,
    next_source_id: u32,
    active_source_ids: Vec<u32>,
    pub(crate) resolved_sources: std::collections::HashMap<u32, RecordedSource>,
}
impl<'input> ParserStack<'input> {
    #[must_use]
    pub fn new(
        io_error: ReaderInputError,
        reader_bytes_read: ReaderInputBytesRead,
        max_reader_input_bytes: Option<usize>,
        max_inclusion_depth: u32,
    ) -> Self {
        Self {
            inner: InnerStack::new(),
            include_resolver: None,
            io_error,
            reader_bytes_read,
            max_reader_input_bytes,
            max_inclusion_depth,
            active_ids: Vec::new(),
            snippet_frames: Vec::new(),
            next_source_id: 1,
            active_source_ids: Vec::new(),
            resolved_sources: std::collections::HashMap::new(),
        }
    }
    pub fn set_resolver(
        &mut self,
        resolver: impl FnMut(crate::input_source::IncludeRequest<'_>) -> Result<ResolvedInclude, IncludeResolveError> + 'input,
    ) {
        self.include_resolver = Some(Box::new(resolver));
    }
    pub fn has_resolver(&self) -> bool {
        self.include_resolver.is_some()
    }
    #[allow(dead_code)]
    pub fn push_str_parser(&mut self, parser: Parser<'input, StrInput<'input>>, name: String) {
        self.push_str_parser_with_snippet(parser, name, None);
    }
    pub fn push_stream_parser(&mut self, parser: Parser<'input, ReaderInput<'input>>, name: String) {
        self.push_stream_parser_with_snippet(parser, name, None);
    }
    pub(crate) fn push_str_parser_with_snippet(
        &mut self,
        parser: Parser<'input, StrInput<'input>>,
        name: String,
        snippet: Option<SnippetFrame>,
    ) {
        let source_id = self.next_source_id;
        self.next_source_id += 1;
        let parent_source_id = self.active_source_ids.last().copied();
        self.active_source_ids.push(source_id);
        let recorded = RecordedSource {
            source_id,
            parent_source_id,
            name: snippet
                .as_ref()
                .map(|s| s.name.clone())
                .unwrap_or_else(|| name.clone()),
            text: snippet.as_ref().map(|s| s.text.clone()),
            include_location: snippet
                .as_ref()
                .map(|s| s.include_location)
                .unwrap_or(crate::Location::UNKNOWN),
        };
        self.resolved_sources.insert(source_id, recorded);
        self.inner.push_str_parser(parser, name);
        self.snippet_frames.push(snippet);
    }
    fn push_stream_parser_with_snippet(
        &mut self,
        parser: Parser<'input, ReaderInput<'input>>,
        name: String,
        snippet: Option<SnippetFrame>,
    ) {
        let source_id = self.next_source_id;
        self.next_source_id += 1;
        let parent_source_id = self.active_source_ids.last().copied();
        self.active_source_ids.push(source_id);
        let recorded = RecordedSource {
            source_id,
            parent_source_id,
            name: snippet
                .as_ref()
                .map(|s| s.name.clone())
                .unwrap_or_else(|| name.clone()),
            text: snippet.as_ref().map(|s| s.text.clone()),
            include_location: snippet
                .as_ref()
                .map(|s| s.include_location)
                .unwrap_or(crate::Location::UNKNOWN),
        };
        self.resolved_sources.insert(source_id, recorded);
        self.inner.push_custom_parser(parser, name);
        self.snippet_frames.push(snippet);
    }
    fn push_replay_parser_with_snippet(
        &mut self,
        parser: saphyr_parser::parser_stack::ReplayParser<'input>,
        name: String,
        snippet: Option<SnippetFrame>,
    ) {
        let source_id = self.next_source_id;
        self.next_source_id += 1;
        let parent_source_id = self.active_source_ids.last().copied();
        self.active_source_ids.push(source_id);
        let recorded = RecordedSource {
            source_id,
            parent_source_id,
            name: snippet
                .as_ref()
                .map(|s| s.name.clone())
                .unwrap_or_else(|| name.clone()),
            text: snippet.as_ref().map(|s| s.text.clone()),
            include_location: snippet
                .as_ref()
                .map(|s| s.include_location)
                .unwrap_or(crate::Location::UNKNOWN),
        };
        self.resolved_sources.insert(source_id, recorded);
        self.inner.push_replay_parser(parser, name);
        self.snippet_frames.push(snippet);
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
        self.snippet_frames.truncate(current_len);
        self.active_source_ids.truncate(current_len);
    }
    pub(crate) fn prune_resolved_sources(&mut self) {
        self.resolved_sources
            .retain(|id, _| self.active_source_ids.contains(id));
    }
    #[allow(dead_code)]
    pub fn active_include_snippet_source(&self) -> Option<(&str, &str)> {
        if self.inner.stack().len() <= 1 {
            return None;
        }
        self.snippet_frames
            .last()
            .and_then(|frame| frame.as_ref())
            .map(|frame| (frame.name.as_str(), frame.text.as_ref()))
    }
    #[allow(dead_code)]
    pub fn include_stack_snippets(&self) -> Vec<(&str, &str, crate::Location)> {
        self.snippet_frames
            .iter()
            .filter_map(|frame| frame.as_ref())
            .map(|frame| (frame.name.as_str(), frame.text.as_ref(), frame.include_location))
            .collect()
    }
    pub fn resolve(&mut self, include_str: &str, location: crate::Location) -> Result<(), crate::de_error::Error> {
        let Some(resolver) = &mut self.include_resolver else {
            return Err(crate::de_error::Error::msg("No include resolver set for parser stack.").with_location(location));
        };

        let include_depth = self.inner.stack().len() as u32;
        if include_depth > self.max_inclusion_depth {
            return Err(crate::de_error::Error::Budget {
                breach: crate::budget::BudgetBreach::InclusionDepth { depth: include_depth },
                location,
            });
        }
        let stack: Vec<String> = self.inner.stack().into_iter().collect();
        let from_name = stack.last().map(|s| s.as_str()).unwrap_or("");
        let from_id = self.active_ids.last().map(|(_, id)| id.as_str());

        let request = crate::input_source::IncludeRequest {
            spec: include_str,
            from_name,
            from_id,
            stack: stack.clone(),
            size_remaining: self
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
                    include_location: location,
                };
                let cursor = std::io::Cursor::new(snippet.text.as_ref().as_bytes().to_vec());
                let input = buffered_input_from_reader_with_limit_shared(
                    cursor,
                    self.max_reader_input_bytes,
                    self.io_error.clone(),
                    self.reader_bytes_read.clone(),
                );
                let parser = Parser::new(input);
                self.push_stream_parser_with_snippet(parser, name, Some(snippet));
            }
            InputSource::Reader(r) => {
                let input = buffered_input_from_reader_with_limit_shared(
                    r,
                    self.max_reader_input_bytes,
                    self.io_error.clone(),
                    self.reader_bytes_read.clone(),
                );
                let parser = Parser::new(input);
                self.push_stream_parser_with_snippet(parser, name, None);
            }
            InputSource::AnchoredText { mut text, anchor } => {
                if text.trim().is_empty() {
                    text = "~".to_string();
                }
                let snippet = SnippetFrame {
                    name: name.clone(),
                    text: Rc::from(text.as_str()),
                    include_location: location,
                };
                let events = collect_anchor_events(
                    &text,
                    &anchor,
                    self.inner.current_anchor_offset(),
                    self.max_reader_input_bytes,
                    self.io_error.clone(),
                    self.reader_bytes_read.clone(),
                )
                    .map_err(|error| {
                    crate::de_error::Error::ResolverError {
                        target: include_str.to_string(),
                        error: crate::IncludeResolveError::Message(error),
                        stack: self.inner.stack().into_iter().collect(),
                        location,
                    }
                })?;
                self.push_replay_parser_with_snippet(
                    saphyr_parser::parser_stack::ReplayParser::new(events.events, events.anchor_offset),
                    name,
                    Some(snippet),
                );
            }
        }
        Ok(())
    }
}
struct CollectedAnchorEvents {
    events: Vec<(Event<'static>, Span)>,
    anchor_offset: usize,
}

fn anchored_event_initial_depth(event: &Event<'_>) -> usize {
    match event {
        Event::SequenceStart(_, _) | Event::MappingStart(_, _) => 1,
        _ => 0,
    }
}

fn collect_anchor_events(
    text: &str,
    target_anchor: &str,
    anchor_offset: usize,
    max_reader_input_bytes: Option<usize>,
    io_error: ReaderInputError,
    reader_bytes_read: ReaderInputBytesRead,
) -> Result<CollectedAnchorEvents, String> {
    let mut anchor_defs: Vec<(String, usize)> = Vec::new();
    let mut alias_names: Vec<String> = Vec::new();
    let scanner_input = buffered_input_from_reader_with_limit_shared(
        std::io::Cursor::new(text.as_bytes().to_vec()),
        max_reader_input_bytes,
        io_error.clone(),
        reader_bytes_read.clone(),
    );
    let mut scanner = Scanner::new(scanner_input);
    for token in &mut scanner {
        let marker_offset = token.0.start.index();
        match token.1 {
            TokenType::Anchor(name) => anchor_defs.push((name.into_owned(), marker_offset)),
            TokenType::Alias(name) => alias_names.push(name.into_owned()),
            _ => {}
        }
    }
    if let Some(err) = io_error.borrow().as_ref() {
        return Err(format!(
            "failed to scan include fragment '{}': {}",
            target_anchor, err
        ));
    }
    if let Some(err) = scanner.get_error() {
        return Err(format!(
            "failed to scan include fragment '{}': {}",
            target_anchor, err
        ));
    }
    let parser_input = buffered_input_from_reader_with_limit_shared(
        std::io::Cursor::new(text.as_bytes().to_vec()),
        max_reader_input_bytes,
        io_error.clone(),
        reader_bytes_read,
    );
    let mut parser = Parser::new(parser_input);
    parser.set_anchor_offset(anchor_offset);
    let mut events = Vec::new();
    while let Some(event) = parser.next_event() {
        let (event, span) = event.map_err(|err| {
            format!(
                "failed to parse include fragment '{}': {}",
                target_anchor, err
            )
        })?;
        events.push((own_event(event), span));
    }
    if let Some(err) = io_error.borrow().as_ref() {
        return Err(format!(
            "failed to parse include fragment '{}': {}",
            target_anchor, err
        ));
    }

    let mut anchor_nodes_by_name: std::collections::BTreeMap<String, Vec<(Event<'static>, Span)>> =
        std::collections::BTreeMap::new();
    let mut event_cursor = 0usize;
    for (name, offset) in &anchor_defs {
        while event_cursor < events.len() && events[event_cursor].1.start.index() < *offset {
            event_cursor += 1;
        }
        if event_cursor >= events.len() {
            break;
        }
        let start = event_cursor;
        let mut end = start;
        let mut depth = anchored_event_initial_depth(&events[start].0);
        if depth == 0 {
            end = start;
        } else {
            for idx in (start + 1)..events.len() {
                match &events[idx].0 {
                    Event::SequenceStart(_, _) | Event::MappingStart(_, _) => depth += 1,
                    Event::SequenceEnd | Event::MappingEnd => {
                        if depth == 0 {
                            return Err(format!(
                                "include fragment '{}' became unbalanced while replaying events",
                                target_anchor
                            ));
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
        anchor_nodes_by_name.insert(name.clone(), events[start..=end].to_vec());
        event_cursor = start + 1;
    }

    let mut target_events = anchor_nodes_by_name
        .get(target_anchor)
        .cloned()
        .ok_or_else(|| format!("include fragment '{}' was not found", target_anchor))?;

    let mut alias_index = 0usize;
    for idx in 0..target_events.len() {
        if matches!(target_events[idx].0, Event::Alias(_)) {
            let alias_name = alias_names.get(alias_index).cloned();
            alias_index += 1;
            if let Some(alias_name) = alias_name {
                if let Some(alias_events) = anchor_nodes_by_name.get(&alias_name) {
                    target_events.splice(idx..=idx, alias_events.clone());
                }
            }
        }
    }

    Ok(CollectedAnchorEvents {
        events: target_events,
        anchor_offset: parser.get_anchor_offset(),
    })
}
fn own_event(event: Event<'_>) -> Event<'static> {
    match event {
        Event::Nothing => Event::Nothing,
        Event::StreamStart => Event::StreamStart,
        Event::StreamEnd => Event::StreamEnd,
        Event::DocumentStart(explicit) => Event::DocumentStart(explicit),
        Event::DocumentEnd => Event::DocumentEnd,
        Event::Alias(anchor_id) => Event::Alias(anchor_id),
        Event::Scalar(value, style, anchor_id, tag) => Event::Scalar(
            std::borrow::Cow::Owned(value.into_owned()),
            style,
            anchor_id,
            tag.map(|tag| std::borrow::Cow::Owned(tag.into_owned())),
        ),
        Event::SequenceStart(anchor_id, tag) => {
            Event::SequenceStart(anchor_id, tag.map(|tag| std::borrow::Cow::Owned(tag.into_owned())))
        }
        Event::SequenceEnd => Event::SequenceEnd,
        Event::MappingStart(anchor_id, tag) => {
            Event::MappingStart(anchor_id, tag.map(|tag| std::borrow::Cow::Owned(tag.into_owned())))
        }
        Event::MappingEnd => Event::MappingEnd,
    }
}
impl<'input> Iterator for ParserStack<'input> {
    type Item = Result<(Event<'input>, Span), ScanError>;
    fn next(&mut self) -> Option<Self::Item> {
        let pre_stack = self.inner.stack();
        match self.inner.next() {
            Some(Err(e)) => {
                // Do not sync source tracking yet on error. The caller (like `LiveEvents`)
                // needs `current_source_id()` to map the error location correctly.
                // If they continue iterating, the next `Ok` event will sync it.
                if pre_stack.len() > 1 {
                    let msg = format!("{}\nwhile parsing {}", e.info(), pre_stack.join(" -> "));
                    Some(Err(ScanError::new(*e.marker(), msg)))
                } else {
                    Some(Err(e))
                }
            }
            Some(Ok((Event::DocumentStart(explicit), span))) => {
                self.sync_source_tracking(self.inner.stack().len());
                if self.inner.stack().len() == 1 {
                    self.prune_resolved_sources();
                }
                Some(Ok((Event::DocumentStart(explicit), span)))
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
            None,
            std::rc::Rc::new(std::cell::RefCell::new(None)),
            std::rc::Rc::new(std::cell::Cell::new(0)),
        )
        .expect("anchor collection should succeed");

        let has_user_key = collected.events.iter().any(|(event, _)| {
            matches!(event, Event::Scalar(value, _, _, _) if value.as_ref() == "user")
        });
        let has_name_key = collected.events.iter().any(|(event, _)| {
            matches!(event, Event::Scalar(value, _, _, _) if value.as_ref() == "name")
        });

        assert!(has_user_key, "target mapping key should be preserved");
        assert!(has_name_key, "alias value should be expanded from prerequisite anchor");
        assert!(
            !collected
                .events
                .iter()
                .any(|(event, _)| matches!(event, Event::Alias(_))),
            "expanded event stream should not retain unresolved aliases"
        );
    }

    #[test]
    fn collect_anchor_events_respects_reader_input_budget() {
        let err = collect_anchor_events(
            "base: &base\n  name: Alice\n",
            "base",
            0,
            Some(1),
            std::rc::Rc::new(std::cell::RefCell::new(None)),
            std::rc::Rc::new(std::cell::Cell::new(0)),
        );
        let err = match err {
            Ok(_) => panic!("budget should reject oversized anchored fragment input"),
            Err(err) => err,
        };

        assert!(
            err.contains("failed to scan include fragment 'base'"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_unused_methods() {
        let io_error = std::rc::Rc::new(std::cell::RefCell::new(None));
        let reader_bytes_read = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut stack = ParserStack::new(io_error, reader_bytes_read, None, u32::MAX);

        let parser = Parser::new_from_str("foo: bar");
        stack.push_str_parser(parser, "test.yaml".to_string());

        // At depth 1, active_include_snippet_source is None
        assert_eq!(stack.active_include_snippet_source(), None);

        stack.push_str_parser_with_snippet(
            Parser::new_from_str("baz"),
            "test2.yaml".to_string(),
            Some(SnippetFrame {
                name: "test2.yaml".to_string(),
                text: Rc::from("baz"),
                include_location: crate::Location::UNKNOWN,
            })
        );

        // At depth > 1, active_include_snippet_source returns the top snippet
        let src = stack.active_include_snippet_source().unwrap();
        assert_eq!(src.0, "test2.yaml");
        assert_eq!(src.1, "baz");
        let snippets = stack.include_stack_snippets();
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].0, "test2.yaml");
        assert_eq!(snippets[0].1, "baz");
    }
    #[test]
    fn source_ids_start_from_one_and_zero_stays_unknown() {
        let io_error = std::rc::Rc::new(std::cell::RefCell::new(None));
        let reader_bytes_read = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut stack = ParserStack::new(io_error, reader_bytes_read, None, u32::MAX);
        assert_eq!(stack.current_source_id(), 0);
        stack.push_str_parser(Parser::new_from_str("root: 1"), "root.yaml".to_string());
        assert_eq!(stack.current_source_id(), 1);
        stack.push_str_parser(Parser::new_from_str("child: 2"), "child.yaml".to_string());
        assert_eq!(stack.current_source_id(), 2);
    }
    #[test]
    fn resolved_sources_retained_after_included_parser_pops() {
        let io_error = std::rc::Rc::new(std::cell::RefCell::new(None));
        let reader_bytes_read = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut stack = ParserStack::new(io_error, reader_bytes_read, None, u32::MAX);
        stack.set_resolver(|req| {
            assert_eq!(req.spec, "child.yaml");
            Ok(ResolvedInclude {
                id: "child.yaml".to_string(),
                name: "child.yaml".to_string(),
                source: InputSource::Text("child: 1\n".to_string()),
            })
        });
        stack.push_str_parser(Parser::new_from_str("root: 1\n"), "root.yaml".to_string());
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
        let io_error = std::rc::Rc::new(std::cell::RefCell::new(None));
        let reader_bytes_read = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut stack = ParserStack::new(io_error, reader_bytes_read, None, u32::MAX);
        // A stream with two documents
        stack.push_str_parser(Parser::new_from_str("first: 1\n---\nsecond: 2\n"), "multi.yaml".to_string());

        // Push a dummy child source ID to simulate an include during the first document
        stack.resolved_sources.insert(999, RecordedSource {
            source_id: 999,
            parent_source_id: Some(1),
            name: "dummy.yaml".to_string(),
            text: None,
            include_location: crate::Location::UNKNOWN,
        });
        // Read until the first document finishes and the second document starts
        let mut doc_starts = 0;
        for item in stack.by_ref() {
            if let Ok((Event::DocumentStart(_), _)) = item {
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
        let io_error = std::rc::Rc::new(std::cell::RefCell::new(None));
        let reader_bytes_read = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut stack = ParserStack::new(io_error, reader_bytes_read, None, 0);
        stack.set_resolver(|_| {
            Ok(ResolvedInclude {
                id: "child.yaml".to_string(),
                name: "child.yaml".to_string(),
                source: InputSource::Text("child: 1\n".to_string()),
            })
        });
        stack.push_str_parser(Parser::new_from_str("root: 1\n"), "root.yaml".to_string());

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
        let io_error = std::rc::Rc::new(std::cell::RefCell::new(None));
        let reader_bytes_read = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut stack = ParserStack::new(io_error, reader_bytes_read, None, 2);
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
        stack.push_str_parser(Parser::new_from_str("root: 1\n"), "root.yaml".to_string());

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
        let io_error = std::rc::Rc::new(std::cell::RefCell::new(None));
        let reader_bytes_read = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut stack = ParserStack::new(io_error, reader_bytes_read, None, 1);
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
        stack.push_str_parser(Parser::new_from_str("root: 1\n"), "root.yaml".to_string());

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