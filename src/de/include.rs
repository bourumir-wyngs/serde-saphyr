#[cfg(feature = "include")]
use crate::de::AliasLimits;
use granit_parser::{Options as ParserOptions, Parser};

#[cfg(feature = "include")]
use std::rc::Rc;

use granit_parser::StrInput;

#[cfg(feature = "include")]
use crate::include_stack::ParserStack;
#[cfg(feature = "include")]
use crate::input_source::IncludeResolver;

#[cfg(feature = "include")]
use crate::buffered_input::{ReaderInput, ReaderInputBytesRead};

#[cfg(feature = "include")]
pub(crate) type BaseParser<'a> = ParserStack<'a>;

#[cfg(not(feature = "include"))]
pub(crate) type BaseParser<'a, I> = Parser<'a, I>;

#[cfg(feature = "include")]
#[inline]
pub(crate) fn create_parser_from_reader_input<'input>(
    input: ReaderInput<'input>,
    reader_bytes_read: ReaderInputBytesRead,
    budget: &crate::Budget,
    alias_limits: AliasLimits,
    parser_options: ParserOptions,
    resolver: Option<Box<IncludeResolver<'input>>>,
) -> ParserStack<'input> {
    let mut stack = ParserStack::with_parser_options_and_alias_limits(
        reader_bytes_read,
        budget,
        parser_options.clone(),
        alias_limits,
    );
    if let Some(r) = resolver {
        stack.set_resolver(r);
    }
    stack.push_stream_parser(
        Parser::with_options(input, parser_options),
        "<input>".to_string(),
    );
    stack
}

// Note: in non-include builds we construct the parser directly where needed.

#[cfg(feature = "include")]
#[inline]
pub(crate) fn create_parser_from_str<'a>(
    input: &'a str,
    reader_bytes_read: ReaderInputBytesRead,
    budget: &crate::Budget,
    alias_limits: AliasLimits,
    parser_options: ParserOptions,
    resolver: Option<Box<IncludeResolver<'a>>>,
) -> ParserStack<'a> {
    let mut stack = ParserStack::with_parser_options_and_alias_limits(
        reader_bytes_read,
        budget,
        parser_options.clone(),
        alias_limits,
    );
    if let Some(r) = resolver {
        stack.set_resolver(r);
    }
    let snippet = crate::include_stack::SnippetFrame {
        name: "<input>".to_string(),
        text: Rc::from(input),
    };
    stack.push_str_parser_with_snippet(
        Parser::with_options(StrInput::new(input), parser_options),
        "<input>".to_string(),
        Some(&snippet),
        crate::Location::UNKNOWN,
    );
    stack
}

#[cfg(not(feature = "include"))]
#[inline]
pub(crate) fn create_parser_from_str<'a>(
    input: &'a str,
    parser_options: ParserOptions,
) -> BaseParser<'a, StrInput<'a>> {
    Parser::with_options(StrInput::new(input), parser_options)
}

#[cfg(all(test, feature = "include"))]
mod tests {
    use super::*;

    #[test]
    fn create_parser_from_str_borrows_root_text_for_snippets() {
        let input = "root: 1";
        let budget = crate::Budget::default();
        let parser_options = crate::Options::default().parser_options();
        let stack = create_parser_from_str(
            input,
            std::rc::Rc::new(std::cell::Cell::new(0)),
            &budget,
            AliasLimits::default(),
            parser_options,
            None,
        );

        let root = stack
            .resolved_sources
            .get(&1)
            .expect("root source recorded");
        let text = root.text.as_ref().expect("root text recorded");
        assert_eq!(text.as_ref(), input);
    }
}
