#[test]
fn test_parser_direct_borrowing() {
    use saphyr_parser::{Parser, Event};
    use std::borrow::Cow;

    let input = "hello";
    let mut parser = Parser::new_from_str(input);
    
    // StreamStart
    let _ = parser.next().unwrap().unwrap();
    // DocumentStart
    let _ = parser.next().unwrap().unwrap();
    
    // Scalar
    let (event, _span) = parser.next().unwrap().unwrap();
    match event {
        Event::Scalar(cow, _, _, _) => {
            match cow {
                Cow::Borrowed(_) => (),
                Cow::Owned(s) => panic!("Expected Borrowed, got Owned: '{}'", s),
            }
            assert_eq!(cow, "hello");
        }
        _ => panic!("Expected Scalar"),
    }
}

#[test]
fn test_parser_direct_borrowing_quoted() {
    use saphyr_parser::{Parser, Event};
    use std::borrow::Cow;

    let input = "\"hello world\"";
    let mut parser = Parser::new_from_str(input);
    
    // StreamStart
    let _ = parser.next().unwrap().unwrap();
    // DocumentStart
    let _ = parser.next().unwrap().unwrap();
    
    // Scalar
    let (event, _span) = parser.next().unwrap().unwrap();
    match event {
        Event::Scalar(cow, _, _, _) => {
            match cow {
                Cow::Borrowed(_) => (),
                Cow::Owned(s) => panic!("Expected Borrowed, got Owned: '{}'", s),
            }
            assert_eq!(cow, "hello world");
        }
        _ => panic!("Expected Scalar"),
    }
}
