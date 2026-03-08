use crate::buffered_input::{
    buffered_input_from_reader_with_limit_shared, ReaderInput, ReaderInputError,
};
use crate::input_source::{IncludeResolveError, IncludeResolver, InputSource, ResolvedInclude};
use saphyr_parser::{Event, Parser, ScanError, Span, StrInput};



type InnerStack<'input> = saphyr_parser::parser_stack::ParserStack<'input, core::iter::Empty<char>, ReaderInput<'input>>;

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
}

impl<'input> ParserStack<'input> {
    #[must_use]
    pub fn new(io_error: ReaderInputError, max_reader_input_bytes: Option<usize>) -> Self {
        Self { inner: InnerStack::new(), include_resolver: None, io_error, max_reader_input_bytes, active_ids: Vec::new() }
    }

    pub fn set_resolver(
        &mut self,
        resolver: impl FnMut(&str) -> Result<ResolvedInclude, IncludeResolveError> + 'input,
    ) {
        self.include_resolver = Some(Box::new(resolver));
    }

    pub fn push_str_parser(&mut self, parser: Parser<'input, StrInput<'input>>, name: String) {
        self.inner.push_str_parser(parser, name);
    }

    pub fn push_stream_parser(&mut self, parser: Parser<'input, ReaderInput<'input>>, name: String) {
        self.inner.push_custom_parser(parser, name);
    }

    pub fn resolve(&mut self, include_str: &str, location: crate::Location) -> Result<(), crate::de_error::Error> {
        let Some(resolver) = &mut self.include_resolver else {
            return Err(crate::de_error::Error::msg("No include resolver set for parser stack.").with_location(location));
        };

        let resolved = match resolver(include_str) {
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
            InputSource::Text(s) => {
                let cursor = std::io::Cursor::new(s.into_bytes());
                let input = buffered_input_from_reader_with_limit_shared(
                    Box::new(cursor),
                    self.max_reader_input_bytes,
                    self.io_error.clone()
                );
                let parser = Parser::new(input);
                self.push_stream_parser(parser, name);
            }
            InputSource::Reader(r) => {
                let input = buffered_input_from_reader_with_limit_shared(r, self.max_reader_input_bytes, self.io_error.clone());
                let parser = Parser::new(input);
                self.push_stream_parser(parser, name);
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
                other
            }
        }
    }
}
