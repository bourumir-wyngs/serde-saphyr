use saphyr_parser::{BorrowedInput, Parser, StrInput};

#[cfg(feature = "include")]
use saphyr_parser::parser_stack::ParserStack;

#[cfg(feature = "include")]
pub(crate) type BaseParser<'a, I> = ParserStack<'a, core::iter::Empty<char>, I>;

#[cfg(not(feature = "include"))]
pub(crate) type BaseParser<'a, I> = Parser<'a, I>;

#[cfg(feature = "include")]
#[inline]
pub(crate) fn create_parser<'a, I>(input: I) -> BaseParser<'a, I>
where
    I: BorrowedInput<'a>,
{
    let mut stack = ParserStack::new();
    stack.push_custom_parser(Parser::new(input), "main".to_string());
    stack
}

#[cfg(not(feature = "include"))]
#[inline]
pub(crate) fn create_parser<'a, I>(input: I) -> BaseParser<'a, I>
where
    I: BorrowedInput<'a>,
{
    Parser::new(input)
}

#[cfg(feature = "include")]
#[inline]
pub(crate) fn create_parser_from_str(input: &'_ str) -> BaseParser<'_, StrInput<'_>> {
    let mut stack = ParserStack::new();
    stack.push_str_parser(Parser::new_from_str(input), "main".to_string());
    stack
}

#[cfg(not(feature = "include"))]
#[inline]
pub(crate) fn create_parser_from_str(input: &'_ str) -> BaseParser<'_, StrInput<'_>> {
    Parser::new_from_str(input)
}
