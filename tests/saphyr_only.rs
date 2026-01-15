use saphyr_parser::Parser;

// test --package serde-saphyr --test parser_misplaced_sequence_close -- --nocapture
#[test]
fn saphyr_parser_does_not_emit_misplaced_sequence_closing_event() {
    let yaml = "---\n[ a, b, c ] ]\n";

    let parser = Parser::new_from_str(yaml);
    for next in parser {
        println!("{:?}", next);
        if next.is_err() {
            break;
        }
    }
}

// BS4K: Comment between plain scalar lines
#[test]
// https://matrix.yaml.info/details/BS4K.html
fn bs4k_comment_between_plain_scalar_lines_should_fail() {
    let yaml = "word1  # comment\nword2\n";

    let parser = Parser::new_from_str(yaml);
    for next in parser {
        println!("{:?}", next);
        if next.is_err() {
            break;
        }
    }
}
