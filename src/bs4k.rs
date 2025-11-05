use crate::Location;

/// Returns the approximate location of a BS4K-style invalid pattern:
/// a content line with an inline `#` comment (not inside quotes/flow),
/// immediately followed by a *top-level* (non-indented) content line
/// that would implicitly start a new document without a `---` marker.
///
/// Location points to the start (col 1) of that next top-level line.
/// Returns `None` if no such pattern is detected.
pub fn find_bs4k_issue_location(input: &str) -> Option<Location> {
    // Find the index of a YAML comment '#' that is NOT inside quotes or flow collections.
    fn find_real_comment_idx(s: &str) -> Option<usize> {
        let mut in_single = false;
        let mut in_double = false;
        let mut bracket = 0; // []
        let mut brace = 0;   // {}

        let mut iter = s.chars().enumerate().peekable();
        while let Some((i, c)) = iter.next() {
            match c {
                '\'' if !in_double => {
                    if in_single {
                        // YAML single quotes escape by doubling ('')
                        if let Some((_, next)) = iter.peek() {
                            if *next == '\'' {
                                iter.next(); // consume escaped '
                                continue;
                            }
                        }
                        in_single = false;
                    } else {
                        in_single = true;
                    }
                }
                '"' if !in_single => {
                    in_double = !in_double;
                }
                '\\' if in_double => {
                    // skip escaped char in double quotes
                    let _ = iter.next();
                }
                '[' if !in_single && !in_double => bracket += 1,
                ']' if !in_single && !in_double => bracket = (bracket - 1).max(0),
                '{' if !in_single && !in_double => brace += 1,
                '}' if !in_single && !in_double => brace = (brace - 1).max(0),
                '#' if !in_single && !in_double && bracket == 0 && brace == 0 => {
                    return Some(i);
                }
                _ => {}
            }
        }
        None
    }

    // True if `s` starts with an unindented mapping key like `key:` (before any real comment).
    fn starts_with_top_level_mapping_key(s: &str) -> bool {
        let line = s.trim_end();
        if line.is_empty() { return false; }
        if line.starts_with(' ') || line.starts_with('\t') { return false; }

        let comment_at = find_real_comment_idx(line).unwrap_or(line.len());
        let until = &line[..comment_at];

        let mut in_single = false;
        let mut in_double = false;
        let mut bracket = 0;
        let mut brace = 0;

        for (i, c) in until.chars().enumerate() {
            match c {
                '\'' if !in_double => {
                    in_single = !in_single; // acceptable heuristic for keys
                }
                '"' if !in_single => {
                    in_double = !in_double;
                }
                '[' if !in_single && !in_double => bracket += 1,
                ']' if !in_single && !in_double => bracket = (bracket - 1).max(0),
                '{' if !in_single && !in_double => brace += 1,
                '}' if !in_single && !in_double => brace = (brace - 1).max(0),
                ':' if !in_single && !in_double && bracket == 0 && brace == 0 => {
                    // ensure there is some non-whitespace before ':'
                    return !until[..i].trim().is_empty();
                }
                _ => {}
            }
        }
        false
    }

    let mut lines = input.lines().peekable();
    let mut row_idx: u32 = 0;

    while let Some(line) = lines.next() {
        row_idx += 1;

        // Locate a real YAML comment in the current line.
        let Some(hash_pos) = find_real_comment_idx(line) else { continue };

        let before = &line[..hash_pos];

        // No non-whitespace content before '#': not our case.
        if before.chars().all(|c| c.is_whitespace()) {
            continue;
        }

        // If there is a ':' before '#', treat as mapping context: skip.
        if before.contains(':') {
            continue;
        }

        // If the line starts with a sequence dash (after whitespace), skip.
        let before_trim = before.trim_start();
        if before_trim.starts_with("- ") || before_trim == "-" {
            continue;
        }

        // If flow indicators appear before the comment, skip (flow content allowed).
        if before.contains('[') || before.contains('{') {
            continue;
        }

        // Peek at next line to decide if it starts top-level content.
        if let Some(next) = lines.peek() {
            let next_line = next.trim_end();

            // Non-empty?
            if next_line.trim().is_empty() {
                continue;
            }
            // Must be *top-level* (no indentation).
            if next_line.starts_with(' ') || next_line.starts_with('\t') {
                continue;
            }
            // Ignore markers/comments.
            if next_line.starts_with("---") || next_line.starts_with("...") || next_line.starts_with('#') {
                continue;
            }
            // If the next line starts a top-level mapping key, we *don't* flag.
            if starts_with_top_level_mapping_key(next_line) {
                continue;
            }

            // We found the invalid pattern. Report the *next* line start (col 1).
            return Some(Location { row: row_idx + 1, column: 1 });
        }
    }

    None
}


#[cfg(test)]
mod tests {
    use crate::bs4k::find_bs4k_issue_location;

    fn chk(text: &str) -> bool {
        find_bs4k_issue_location(text).is_some()
    }

    #[test]
    fn no_trigger_when_comment_is_in_double_quotes() {
        let s = r#""a # b"
next"#;
        assert!(!chk(s));
    }

    #[test]
    fn trigger_on_plain_inline_comment_then_top_level_scalar() {
        let s = "foo # trailing
bar";
        assert!(chk(s));
    }

    #[test]
    fn skip_when_next_is_top_level_mapping_key() {
        let s = "foo # trailing
key: val";
        assert!(!chk(s));
    }

    #[test]
    fn skip_when_current_is_list_item() {
        let s = "- item # c
next";
        assert!(!chk(s));
    }

    #[test]
    fn skip_when_current_is_flow() {
        let s = "[1, 2] # numbers
next";
        assert!(!chk(s));
    }

    #[test]
    fn no_false_pos_with_hash_in_flow_scalar() {
        let s = r#"[ "x#y" ] # ok
Z"#;
        assert!(!chk(s));
    }

    #[test]
    fn no_trigger_if_next_is_marker() {
        let s = "foo # trailing
---";
        assert!(!chk(s));
    }
}
