use saphyr_parser::Parser;

#[cfg(not(feature = "include"))]
use saphyr_parser::StrInput;

#[cfg(feature = "include")]
use crate::include_stack::ParserStack;
#[cfg(feature = "include")]
use crate::input_source::IncludeResolver;

#[cfg(feature = "include")]
use crate::buffered_input::{ReaderInput, ReaderInputError};

#[cfg(feature = "include")]
pub(crate) type BaseParser<'a> = ParserStack<'a>;

#[cfg(not(feature = "include"))]
pub(crate) type BaseParser<'a, I> = Parser<'a, I>;

#[cfg(feature = "include")]
#[inline]
pub(crate) fn create_parser_from_reader_input<'input>(
    input: ReaderInput<'input>,
    io_error: ReaderInputError,
    max_reader_input_bytes: Option<usize>,
    resolver: Option<Box<IncludeResolver<'input>>>,
) -> ParserStack<'input> {
    let mut stack = ParserStack::new(io_error, max_reader_input_bytes);
    if let Some(r) = resolver {
        stack.set_resolver(r);
    }
    stack.push_stream_parser(Parser::new(input), "main".to_string());
    stack
}

// Note: in non-include builds we construct the parser directly where needed.

#[cfg(feature = "include")]
#[inline]
pub(crate) fn create_parser_from_str<'a>(
    input: &'a str,
    io_error: ReaderInputError,
    max_reader_input_bytes: Option<usize>,
    resolver: Option<Box<IncludeResolver<'a>>>,
) -> ParserStack<'a> {
    let mut stack = ParserStack::new(io_error, max_reader_input_bytes);
    if let Some(r) = resolver {
        stack.set_resolver(r);
    }
    stack.push_str_parser_with_snippet(
        Parser::new_from_str(input),
        "main".to_string(),
        Some(crate::include_stack::SnippetFrame {
            name: "main".to_string(),
            text: input.to_string(),
        }),
    );
    stack
}

#[cfg(not(feature = "include"))]
#[inline]
pub(crate) fn create_parser_from_str<'a>(
    input: &'a str,
) -> BaseParser<'a, StrInput<'a>> {
    Parser::new_from_str(input)
}
