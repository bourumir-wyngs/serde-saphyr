use crate::buffered_input::{
    buffered_input_from_reader_with_limit_shared, ReaderInput, ReaderInputError,
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
    max_reader_input_bytes: Option<usize>,
    active_ids: Vec<(usize, String)>,
    snippet_frames: Vec<Option<SnippetFrame>>,
    next_source_id: u32,
    active_source_ids: Vec<u32>,
    pub(crate) resolved_sources: std::collections::HashMap<u32, RecordedSource>,
}

impl<'input> ParserStack<'input> {
    #[must_use]
    pub fn new(io_error: ReaderInputError, max_reader_input_bytes: Option<usize>) -> Self {
        Self {
            inner: InnerStack::new(),
            include_resolver: None,
            io_error,
            max_reader_input_bytes,
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

        let stack: Vec<String> = self.inner.stack().into_iter().collect();
        let from_name = stack.last().map(|s| s.as_str()).unwrap_or("");
        let from_id = self.active_ids.last().map(|(_, id)| id.as_str());
        
        let request = crate::input_source::IncludeRequest {
            spec: include_str,
            from_name,
            from_id,
            stack: stack.clone(),
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
        let include_depth = self.inner.stack().len() + 1;
        self.active_ids.push((include_depth, resolved.id));

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
                    self.io_error.clone()
                );
                let parser = Parser::new(input);
                self.push_stream_parser_with_snippet(parser, name, Some(snippet));
            }
            InputSource::Reader(r) => {
                let input = buffered_input_from_reader_with_limit_shared(r, self.max_reader_input_bytes, self.io_error.clone());
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
                let events = collect_anchor_events(&text, &anchor, self.inner.current_anchor_offset())
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

fn collect_anchor_events(
    text: &str,
    target_anchor: &str,
    anchor_offset: usize,
) -> Result<CollectedAnchorEvents, String> {
    let mut anchor_names = Vec::new();
    let mut scanner = Scanner::new(StrInput::new(text));
    for token in &mut scanner {
        if let TokenType::Anchor(name) = token.1 {
            anchor_names.push(name.into_owned());
        }
    }
    if let Some(err) = scanner.get_error() {
        return Err(format!(
            "failed to scan include fragment '{}': {}",
            target_anchor, err
        ));
    }

    let mut parser = Parser::new_from_str(text);
    parser.set_anchor_offset(anchor_offset);

    let mut anchor_index = 0usize;
    let mut collecting = false;
    let mut depth = 0usize;
    let mut events = Vec::new();
    while let Some(event) = parser.next_event() {
        let (event, span) = event.map_err(|err| {
            format!(
                "failed to parse include fragment '{}': {}",
                target_anchor, err
            )
        })?;
        let anchor_id = match &event {
            Event::Scalar(_, _, anchor_id, _)
            | Event::SequenceStart(anchor_id, _)
            | Event::MappingStart(anchor_id, _) => *anchor_id,
            _ => 0,
        };
        if anchor_id == 0 {
            if collecting {
                match event {
                    Event::SequenceStart(_, _) | Event::MappingStart(_, _) => depth += 1,
                    Event::SequenceEnd | Event::MappingEnd => {
                        if depth == 0 {
                            return Err(format!(
                                "include fragment '{}' became unbalanced while replaying events",
                                target_anchor
                            ));
                        }
                        depth -= 1;
                    }
                    _ => {}
                }
                events.push((own_event(event), span));
                if depth == 0 {
                    break;
                }
            }
            continue;
        }

        let Some(anchor_name) = anchor_names.get(anchor_index) else {
            return Err(format!(
                "failed to map include anchor '{}' to parser events",
                target_anchor
            ));
        };
        anchor_index += 1;

        if collecting {
            events.push((own_event(event), span));
            if depth == 0 {
                break;
            }
            continue;
        }

        if anchor_name == target_anchor {
            depth = match &event {
                Event::Scalar(_, _, _, _) => 0,
                Event::SequenceStart(_, _) | Event::MappingStart(_, _) => 1,
                _ => 0,
            };
            events.push((own_event(event), span));
            if depth == 0 {
                break;
            }
            collecting = true;
        }
    }

    if events.is_empty() {
        Err(format!(
            "include fragment '{}' was not found",
            target_anchor
        ))
    } else {
        Ok(CollectedAnchorEvents {
            events,
            anchor_offset: parser.get_anchor_offset(),
        })
    }
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
                if pre_stack.len() > 1 {
                    let msg = format!("{}\nwhile parsing {}", e.info(), pre_stack.join(" -> "));
                    Some(Err(ScanError::new(*e.marker(), msg)))
                } else {
                    Some(Err(e))
                }
            }
            other => {
                let current_len = self.inner.stack().len();
                self.active_ids.retain(|(depth, _)| *depth <= current_len);
                self.snippet_frames.truncate(current_len);
                self.active_source_ids.truncate(current_len);
                other
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unused_methods() {
        let io_error = std::rc::Rc::new(std::cell::RefCell::new(None));
        let mut stack = ParserStack::new(io_error, None);
        
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
        let mut stack = ParserStack::new(io_error, None);

        assert_eq!(stack.current_source_id(), 0);

        stack.push_str_parser(Parser::new_from_str("root: 1"), "root.yaml".to_string());
        assert_eq!(stack.current_source_id(), 1);

        stack.push_str_parser(Parser::new_from_str("child: 2"), "child.yaml".to_string());
        assert_eq!(stack.current_source_id(), 2);
    }

}
