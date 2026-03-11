use saphyr_parser::Parser;

#[cfg(feature = "include")]
use std::rc::Rc;

#[cfg(not(feature = "include"))]
use saphyr_parser::StrInput;

#[cfg(feature = "include")]
use crate::include_stack::ParserStack;
#[cfg(feature = "include")]
use crate::input_source::IncludeResolver;

#[cfg(feature = "include")]
use crate::buffered_input::{ReaderInput, ReaderInputBytesRead, ReaderInputError};

#[cfg(feature = "include")]
pub(crate) type BaseParser<'a> = ParserStack<'a>;

#[cfg(not(feature = "include"))]
pub(crate) type BaseParser<'a, I> = Parser<'a, I>;

#[cfg(feature = "include")]
#[inline]
pub(crate) fn create_parser_from_reader_input<'input>(
    input: ReaderInput<'input>,
    io_error: ReaderInputError,
    reader_bytes_read: ReaderInputBytesRead,
    max_reader_input_bytes: Option<usize>,
    max_inclusion_depth: u32,
    resolver: Option<Box<IncludeResolver<'input>>>,
) -> ParserStack<'input> {
    let mut stack = ParserStack::new(
        io_error,
        reader_bytes_read,
        max_reader_input_bytes,
        max_inclusion_depth,
    );
    if let Some(r) = resolver {
        stack.set_resolver(r);
    }
    stack.push_stream_parser(Parser::new(input), "<input>".to_string());
    stack
}

// Note: in non-include builds we construct the parser directly where needed.

#[cfg(feature = "include")]
#[inline]
pub(crate) fn create_parser_from_str<'a>(
    input: &'a str,
    io_error: ReaderInputError,
    reader_bytes_read: ReaderInputBytesRead,
    max_reader_input_bytes: Option<usize>,
    max_inclusion_depth: u32,
    resolver: Option<Box<IncludeResolver<'a>>>,
) -> ParserStack<'a> {
    let mut stack = ParserStack::new(
        io_error,
        reader_bytes_read,
        max_reader_input_bytes,
        max_inclusion_depth,
    );
    if let Some(r) = resolver {
        stack.set_resolver(r);
    }
    stack.push_str_parser_with_snippet(
        Parser::new_from_str(input),
        "<input>".to_string(),
        Some(crate::include_stack::SnippetFrame {
            name: "<input>".to_string(),
            text: Rc::from(input),
            include_location: crate::Location::UNKNOWN,
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

#[cfg(all(test, feature = "include"))]
mod tests {
    use super::*;

    #[test]
    fn create_parser_from_str_borrows_root_text_for_snippets() {
        let input = "root: 1";
        let io_error = std::rc::Rc::new(std::cell::RefCell::new(None));
        let stack = create_parser_from_str(
            input,
            io_error,
            std::rc::Rc::new(std::cell::Cell::new(0)),
            None,
            24,
            None,
        );

        let root = stack.resolved_sources.get(&1).expect("root source recorded");
        let text = root.text.as_ref().expect("root text recorded");
        assert_eq!(text.as_ref(), input);
        let stack_frame = stack
            .include_stack_snippets()
            .into_iter()
            .next()
            .expect("root snippet frame recorded");
        assert_eq!(stack_frame.1, input);
    }
}
