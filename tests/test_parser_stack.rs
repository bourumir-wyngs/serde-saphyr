#![cfg(all(feature = "serialize", feature = "deserialize"))]
#[cfg(feature = "include")]
use granit_parser::Parser;
#[cfg(feature = "include")]
use granit_parser::parser_stack::ParserStack;

#[cfg(feature = "include")]
#[test]
fn test_parser_stack_eof_error() {
    let mut stack: ParserStack<'_, core::iter::Empty<char>, granit_parser::StrInput<'_>> =
        ParserStack::new();
    stack.push_str_parser(Parser::new_from_str("[ 1, 2"), "main".to_string());

    let mut events = Vec::new();
    let mut errors = Vec::new();
    for res in stack {
        match res {
            Ok((ev, _span)) => {
                events.push(ev);
            }
            Err(e) => {
                errors.push(e);
            }
        }
    }
    assert_eq!(errors.len(), 1);
    assert!(format!("{:?}", errors[0]).contains("unclosed bracket"));
}
