use crate::buffered_input::{
    buffered_input_from_reader_with_limit_shared, ReaderInput, ReaderInputError,
};
use crate::input_source::InputSource;
use saphyr_parser::{Event, Marker, Parser, ScanError, Span, StrInput};

/// Error type returned by user-provided include resolvers.
#[derive(Debug)]
pub enum IncludeResolveError {
    Io(std::io::Error),
    Message(String),
}

/// A type alias for the include resolver closure.
pub type IncludeResolver<'a> = dyn FnMut(&str) -> Result<InputSource, IncludeResolveError> + 'a;

impl From<std::io::Error> for IncludeResolveError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

fn include_err_to_scan_error(err: IncludeResolveError) -> ScanError {
    // We do not have a precise location here; the caller will attach the YAML location.
    let msg = match err {
        IncludeResolveError::Io(e) => e.to_string(),
        IncludeResolveError::Message(m) => m,
    };
    ScanError::new_str(Marker::new(0, 1, 0), &msg)
}

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
}

impl<'input> ParserStack<'input> {
    #[must_use]
    pub fn new(io_error: ReaderInputError) -> Self {
        Self { inner: InnerStack::new(), include_resolver: None, io_error }
    }

    pub fn set_resolver(
        &mut self,
        resolver: impl FnMut(&str) -> Result<InputSource, IncludeResolveError> + 'input,
    ) {
        self.include_resolver = Some(Box::new(resolver));
    }

    pub fn push_str_parser(&mut self, parser: Parser<'input, StrInput<'input>>, name: String) {
        self.inner.push_str_parser(parser, name);
    }

    pub fn push_stream_parser(&mut self, parser: Parser<'input, ReaderInput<'input>>, name: String) {
        self.inner.push_custom_parser(parser, name);
    }

    pub fn resolve(&mut self, include_str: &str) -> Result<(), ScanError> {
        let Some(resolver) = &mut self.include_resolver else {
            return Err(ScanError::new_str(
                Marker::new(0, 1, 0),
                "No include resolver set for parser stack.",
            ));
        };

        let src = resolver(include_str).map_err(include_err_to_scan_error)?;
        match src {
            InputSource::Text(s) => {
                // For text, prefer the zero-copy string parser; we must leak to match `'input`.
                let text: &'input str = Box::leak(s.into_boxed_str());
                let parser = Parser::new_from_str(text);
                self.push_str_parser(parser, include_str.to_string());
            }
            InputSource::Reader(r) => {
                let input = buffered_input_from_reader_with_limit_shared(r, None, self.io_error.clone());
                let parser = Parser::new(input);
                self.push_stream_parser(parser, include_str.to_string());
            }
        }
        Ok(())
    }
}

impl<'input> Iterator for ParserStack<'input> {
    type Item = Result<(Event<'input>, Span), ScanError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
