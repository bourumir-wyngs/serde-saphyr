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
pub(crate) fn create_parser_from_str<'a>(
    input: &'a str,
    resolver: Option<Box<dyn FnMut(&str) -> Result<String, saphyr_parser::ScanError> + 'a>>,
) -> BaseParser<'a, StrInput<'a>> {
    let mut stack = ParserStack::new();
    if let Some(mut r) = resolver {
        stack.set_resolver(move |s| r(s));
    }
    stack.push_str_parser(Parser::new_from_str(input), "main".to_string());
    stack
}

#[cfg(not(feature = "include"))]
#[inline]
pub(crate) fn create_parser_from_str<'a>(
    input: &'a str,
) -> BaseParser<'a, StrInput<'a>> {
    Parser::new_from_str(input)
}
