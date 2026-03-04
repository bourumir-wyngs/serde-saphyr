use saphyr_parser::{BorrowedInput, Parser, StrInput};

#[inline]
pub(crate) fn create_parser<'a, I>(input: I) -> Parser<'a, I>
where
    I: BorrowedInput<'a>,
{
    Parser::new(input)
}

#[inline]
pub(crate) fn create_parser_from_str(input: &'_ str) -> Parser<'_, StrInput<'_>> {
    Parser::new_from_str(input)
}
