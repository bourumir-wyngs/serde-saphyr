//! String wrapping utilities for YAML block scalars.

use std::fmt::Write;

use crate::ser::Result;

/// Find leading spaces on the first non-empty line of content.
pub fn first_line_leading_spaces(s: &str) -> usize {
    for line in s.split('\n') {
        if !line.is_empty() {
            return line.len() - line.trim_start_matches(' ').len();
        }
    }
    0
}

/// Write a folded block string body, wrapping to `folded_wrap_col` characters.
/// Preserves blank lines between paragraphs. Each emitted line is indented
/// exactly at `indent` depth.
///
/// Wrapping is only performed at ASCII space (`' '`) boundaries.
///
/// This is crucial for round-trip correctness: in YAML folded scalars (`>`), a
/// line break is typically folded back as a single space on parse. If we were
/// to wrap at a non-space whitespace character (e.g. a tab), the reader would
/// still insert a space at the folded newline and the content would change.
///
/// # Arguments
/// * `out` - The output writer
/// * `s` - The string content to write
/// * `indent` - The indentation depth (multiplied by `indent_step`)
/// * `indent_step` - Number of spaces per indentation level
/// * `folded_wrap_col` - Column at which to wrap lines
///
/// # Returns
/// A tuple of `(Result<()>, bool)` where the bool indicates `at_line_start` state.
pub fn write_folded_block<W: Write>(
    out: &mut W,
    s: &str,
    indent: usize,
    indent_step: usize,
    folded_wrap_col: usize,
) -> Result<()> {
    // Precompute indent prefix for this block body and reuse it for each emitted line.
    let mut indent_buf: String = String::new();
    let spaces = indent_step * indent;
    if spaces > 0 {
        indent_buf.reserve(spaces);
        for _ in 0..spaces {
            indent_buf.push(' ');
        }
    }
    let indent_str = indent_buf.as_str();

    for line in s.split('\n') {
        if line.is_empty() {
            // Preserve empty lines between paragraphs
            out.write_str(indent_str)?;
            out.write_char('\n')?;
            continue;
        }

        // If the line starts with a space, avoid wrapping. Wrapping could move those
        // leading spaces across a folded newline and interact with YAML's
        // "more-indented" rule.
        if line.starts_with(' ') {
            out.write_str(indent_str)?;
            out.write_str(line)?;
            out.write_char('\n')?;
            continue;
        }

        // Wrap only at ASCII-space runs.
        //
        // YAML folded scalars (`>`) fold a single line break into a single space.
        // If we break inside a run of N spaces, we must ensure that:
        //   emitted_trailing_spaces + folded_space == original_run_spaces
        // and the next emitted line must NOT start with space (to avoid the
        // "more-indented" rule changing semantics).
        //
        // To achieve that, when breaking at a run of N spaces:
        //   - emit N-1 spaces at end of the previous line,
        //   - consume the entire run,
        //   - start the next line at the first non-space char.
        // For N==1, we emit none and just consume the single space.

        let mut start = 0usize; // byte index of current line start
        let mut col = 0usize; // column in chars since `start`
        let mut last_space_run: Option<(usize, usize, usize)> = None;
        // (run_start_byte, run_end_byte, run_len_in_chars)

        let mut in_space_run = false;
        let mut run_start = 0usize;
        let mut run_len = 0usize;
        let mut prev_i = 0usize;
        let mut prev_ch_len = 0usize;

        for (i, ch) in line.char_indices() {
            // Close an open space-run if needed.
            if in_space_run && ch != ' ' {
                // run_end = previous char boundary (prev_i + prev_ch_len)
                let run_end = prev_i + prev_ch_len;
                last_space_run = Some((run_start, run_end, run_len));
                in_space_run = false;
                run_len = 0;
            }

            // Track space-runs.
            if ch == ' ' {
                if !in_space_run {
                    in_space_run = true;
                    run_start = i;
                    run_len = 1;
                } else {
                    run_len += 1;
                }
            }

            col += 1;

            if col > folded_wrap_col {
                let Some((ws_start, ws_end, ws_len)) = last_space_run else {
                    // No ASCII space within the wrap limit: do not hard-break.
                    break;
                };

                // Emit the segment up to ws_start, then (ws_len - 1) trailing spaces.
                out.write_str(indent_str)?;
                out.write_str(&line[start..ws_start])?;
                if ws_len > 1 {
                    for _ in 0..(ws_len - 1) {
                        out.write_char(' ')?;
                    }
                }
                out.write_char('\n')?;

                // Consume the whole space-run; next line starts after it.
                start = ws_end;
                col = 0;
                last_space_run = None;
            }

            prev_i = i;
            prev_ch_len = ch.len_utf8();
        }

        // Emit the remaining tail.
        out.write_str(indent_str)?;
        out.write_str(&line[start..])?;
        out.write_char('\n')?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_line_leading_spaces_no_spaces() {
        assert_eq!(first_line_leading_spaces("hello"), 0);
        assert_eq!(first_line_leading_spaces("hello\nworld"), 0);
    }

    #[test]
    fn first_line_leading_spaces_with_spaces() {
        assert_eq!(first_line_leading_spaces("  hello"), 2);
        assert_eq!(first_line_leading_spaces("   hello\nworld"), 3);
    }

    #[test]
    fn first_line_leading_spaces_empty_first_line() {
        // Empty first line is skipped, second line has 2 spaces
        assert_eq!(first_line_leading_spaces("\n  hello"), 2);
        assert_eq!(first_line_leading_spaces("\n\n   world"), 3);
    }

    #[test]
    fn first_line_leading_spaces_all_empty() {
        assert_eq!(first_line_leading_spaces(""), 0);
        assert_eq!(first_line_leading_spaces("\n\n"), 0);
    }

    #[test]
    fn write_folded_block_simple() {
        let mut out = String::new();
        write_folded_block(&mut out, "hello world", 1, 2, 80).unwrap();
        assert_eq!(out, "  hello world\n");
    }

    #[test]
    fn write_folded_block_wraps_at_space() {
        let mut out = String::new();
        write_folded_block(&mut out, "hello world", 1, 2, 8).unwrap();
        // "hello world" wraps at space after "hello" when col > 8
        assert_eq!(out, "  hello\n  world\n");
    }

    #[test]
    fn write_folded_block_preserves_empty_lines() {
        let mut out = String::new();
        write_folded_block(&mut out, "para1\n\npara2", 1, 2, 80).unwrap();
        assert_eq!(out, "  para1\n  \n  para2\n");
    }

    #[test]
    fn write_folded_block_preserves_leading_spaces() {
        let mut out = String::new();
        write_folded_block(&mut out, "  indented line", 1, 2, 80).unwrap();
        // Lines starting with space are not wrapped
        assert_eq!(out, "    indented line\n");
    }

    #[test]
    fn write_folded_block_multi_space_run() {
        let mut out = String::new();
        // "AA  BB" with wrap at 4 should break at the double-space run
        write_folded_block(&mut out, "AA  BB", 0, 2, 4).unwrap();
        // Emits trailing space to preserve the double-space run
        assert_eq!(out, "AA \nBB\n");
    }
}
