use crate::Location;

/// Returns the approximate location of a BS4K-style invalid pattern:
/// a content line with an inline `#` comment (not inside quotes/flow),
/// immediately followed by a *top-level* (non-indented) content line
/// that would implicitly start a new document without a `---` marker.
///
/// Location points to the start (col 1) of that next top-level line.
/// Returns `None` if no such pattern is detected.
/// Faster detector: single-pass, byte-wise, quote/flow aware.
/// Returns the approximate location (row, column=1) of the *next* top-level line
/// that would implicitly start a new document after an inline `#` comment,
/// or `None` if no violation is found.
pub fn find_bs4k_issue_location(input: &str) -> Option<Location> {
    #[inline]
    fn has_non_ws(bytes: &[u8]) -> bool {
        bytes.iter().any(|&b| b != b' ' && b != b'\t' && b != b'\r' && b != b'\n')
    }

    /// Scan a line once. Returns:
    /// - comment_pos: position of a real YAML comment `#` (not in quotes/flow)
    /// - has_colon_before_comment: saw `:` outside quotes/flow before `#`
    /// - saw_flow_opener_before_comment: saw `[` or `{` before `#`
    /// - first_non_ws: index of first non-space/tab (None if none)
    #[inline]
    fn scan_line(bytes: &[u8]) -> (Option<usize>, bool, bool, Option<usize>) {
        let mut in_single = false;
        let mut in_double = false;
        let mut depth_bracket: i32 = 0; // []
        let mut depth_brace: i32 = 0;   // {}
        let mut has_colon = false;
        let mut saw_flow = false;

        // first non space/tab
        let mut first_non_ws = None;
        for (i, &b) in bytes.iter().enumerate() {
            if first_non_ws.is_none() && b != b' ' && b != b'\t' {
                first_non_ws = Some(i);
            }

            match b {
                b'\'' if !in_double => {
                    if in_single {
                        // YAML single-quote escape: '' -> literal '
                        if let Some(&b'\'') = bytes.get(i + 1) {
                            // consume the escaped quote
                            // (loop will move to next i anyway; we just skip its effect)
                            continue;
                        }
                        in_single = false;
                    } else {
                        in_single = true;
                    }
                }
                b'"' if !in_single => {
                    in_double = !in_double;
                }
                b'\\' if in_double => {
                    // skip next byte in double-quoted escapes
                    // (bounds check okay; if None, it's fine to do nothing)
                    // we can't mutate `i` here; just let the next iteration consume it
                }
                b'[' if !in_single && !in_double => {
                    depth_bracket += 1;
                    saw_flow = true;
                }
                b']' if !in_single && !in_double => {
                    if depth_bracket > 0 { depth_bracket -= 1; }
                }
                b'{' if !in_single && !in_double => {
                    depth_brace += 1;
                    saw_flow = true;
                }
                b'}' if !in_single && !in_double => {
                    if depth_brace > 0 { depth_brace -= 1; }
                }
                b':' if !in_single && !in_double && depth_bracket == 0 && depth_brace == 0 => {
                    has_colon = true;
                }
                b'#' if !in_single && !in_double && depth_bracket == 0 && depth_brace == 0 => {
                    return (Some(i), has_colon, saw_flow, first_non_ws);
                }
                _ => {}
            }
        }
        (None, has_colon, saw_flow, first_non_ws)
    }

    /// True if the (already known) *unindented* line starts a mapping key like `key: ...`
    /// before any real comment `#`. Quote/flow aware, single pass.
    #[inline]
    fn starts_with_top_level_mapping_key_unindented(line: &str) -> bool {
        let b = line.as_bytes();

        // Trim only trailing \r (from CRLF) to avoid allocations.
        let end = if b.last() == Some(&b'\r') { b.len().saturating_sub(1) } else { b.len() };
        let b = &b[..end];

        // Empty or comment/marker checks are done by caller; here we just find a colon.
        let (comment_pos, _, _, first_non_ws) = scan_line(b);
        let limit = comment_pos.unwrap_or(b.len());

        // Find a ':' outside quotes/flow before comment; ensure some non-ws before it.
        let mut in_single = false;
        let mut in_double = false;
        let mut depth_bracket: i32 = 0;
        let mut depth_brace: i32 = 0;

        for i in 0..limit {
            let c = b[i];
            match c {
                b'\'' if !in_double => {
                    if in_single {
                        if i + 1 < limit && b[i + 1] == b'\'' { /* '' escape */ }
                        else { in_single = false; }
                    } else {
                        in_single = true;
                    }
                }
                b'"' if !in_single => { in_double = !in_double; }
                b'[' if !in_single && !in_double => { depth_bracket += 1; }
                b']' if !in_single && !in_double => { if depth_bracket > 0 { depth_bracket -= 1; } }
                b'{' if !in_single && !in_double => { depth_brace += 1; }
                b'}' if !in_single && !in_double => { if depth_brace > 0 { depth_brace -= 1; } }
                b':' if !in_single && !in_double && depth_bracket == 0 && depth_brace == 0 => {
                    let key_start = first_non_ws.unwrap_or(0);
                    // ensure there is some non-whitespace before ':'
                    let has_key = b[key_start..i].iter().any(|&ch| ch != b' ' && ch != b'\t');
                    return has_key;
                }
                _ => {}
            }
        }
        false
    }

    let mut lines = input.lines().peekable();
    let mut row: u32 = 0;

    while let Some(line) = lines.next() {
        row += 1;

        // Right-trim only '\r' (avoid allocs and keep semantics of input.lines()).
        let bytes = line.as_bytes();
        let end = if bytes.last() == Some(&b'\r') { bytes.len().saturating_sub(1) } else { bytes.len() };
        let cur = &bytes[..end];

        // Scan current line once.
        let (comment_pos, has_colon_before_comment, saw_flow_before_comment, first_non_ws) = scan_line(cur);

        let Some(hash_idx) = comment_pos else { continue };

        // Slice before '#'
        let before = &cur[..hash_idx];

        // Skip if no non-whitespace content before '#'
        if !has_non_ws(before) {
            continue;
        }

        // If there is a ':' before '#', skip (mapping context).
        if has_colon_before_comment {
            continue;
        }

        // If line starts with a sequence dash after whitespace, skip.
        if let Some(nz) = first_non_ws {
            if before.get(nz) == Some(&b'-') && (nz + 1 == before.len() || before.get(nz + 1) == Some(&b' ')) {
                continue;
            }
        }

        // If flow indicators appeared before the comment, skip (flow content allowed).
        if saw_flow_before_comment {
            continue;
        }

        // Check the next line without consuming it.
        if let Some(next_line) = lines.peek() {
            let nb = next_line.as_bytes();
            // right-trim only '\r'
            let nend = if nb.last() == Some(&b'\r') { nb.len().saturating_sub(1) } else { nb.len() };
            let nb = &nb[..nend];

            // Is next line empty? (any non ws?)
            if !has_non_ws(nb) {
                continue;
            }

            // Must be top-level (no leading space/tab).
            if nb.first() == Some(&b' ') || nb.first() == Some(&b'\t') {
                continue;
            }

            // Ignore markers/comments.
            if nb.starts_with(b"---") || nb.starts_with(b"...") || nb.first() == Some(&b'#') {
                continue;
            }

            // If the next line starts a top-level mapping key, do not trigger.
            if starts_with_top_level_mapping_key_unindented(next_line) {
                continue;
            }

            // Violation: report next line, column 1.
            return Some(Location { row: row + 1, column: 1 });
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
