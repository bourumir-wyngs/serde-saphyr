use saphyr_parser::Parser;
#[cfg(feature = "include")]
use saphyr_parser::parser_stack::ParserStack;

#[cfg(feature = "include")]
#[test]
fn test_parser_stack_eof_error() {
    let mut stack: ParserStack<'_, core::iter::Empty<char>, saphyr_parser::StrInput<'_>> = ParserStack::new();
    stack.push_str_parser(Parser::new_from_str("[ 1, 2"), "main".to_string());
    
    let mut events = Vec::new();
    for res in stack {
        match res {
            Ok((ev, span)) => {
                println!("{:?} {:?}", ev, span);
                events.push(ev);
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }
}
