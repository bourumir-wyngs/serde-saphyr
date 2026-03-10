use crate::buffered_input::{
    buffered_input_from_reader_with_limit_shared, ReaderInput, ReaderInputError,
};
use crate::input_source::{IncludeResolveError, IncludeResolver, InputSource, ResolvedInclude};
use saphyr_parser::{Event, Parser, ScanError, Span, StrInput};



type InnerStack<'input> = saphyr_parser::parser_stack::ParserStack<'input, core::iter::Empty<char>, ReaderInput<'input>>;

#[derive(Clone, Debug)]
pub(crate) struct SnippetFrame {
    pub(crate) name: String,
    pub(crate) text: String,
    #[allow(dead_code)]
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
    pub(crate) resolved_sources: std::collections::HashMap<u32, SnippetFrame>,
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
            next_source_id: 0,
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
        self.active_source_ids.push(source_id);
        if let Some(s) = &snippet {
            self.resolved_sources.insert(source_id, s.clone());
        }
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
        self.active_source_ids.push(source_id);
        if let Some(s) = &snippet {
            self.resolved_sources.insert(source_id, s.clone());
        }
        self.inner.push_custom_parser(parser, name);
        self.snippet_frames.push(snippet);
    }

    pub fn current_source_id(&self) -> u32 {
        self.active_source_ids.last().copied().unwrap_or(0)
    }

    #[allow(dead_code)]
    pub fn active_include_snippet_source(&self) -> Option<(&str, &str)> {
        if self.inner.stack().len() <= 1 {
            return None;
        }

        self.snippet_frames
            .last()
            .and_then(|frame| frame.as_ref())
            .map(|frame| (frame.name.as_str(), frame.text.as_str()))
    }

    #[allow(dead_code)]
    pub fn include_stack_snippets(&self) -> Vec<(&str, &str, crate::Location)> {
        self.snippet_frames
            .iter()
            .filter_map(|frame| frame.as_ref())
            .map(|frame| (frame.name.as_str(), frame.text.as_str(), frame.include_location))
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
                    text: s.clone(),
                    include_location: location,
                };
                let cursor = std::io::Cursor::new(s.into_bytes());
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
        }
        Ok(())
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
                text: "baz".to_string(),
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
}
