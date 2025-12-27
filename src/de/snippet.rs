use std::fmt;

use annotate_snippets::{renderer::DecorStyle, AnnotationKind, Level, Renderer, Snippet};

use crate::Location;

/// Render a rustc-like diagnostic snippet for an error that has a known [`Location`].
///
/// If snippet rendering cannot be produced (e.g., invalid coordinates), this falls back to a
/// simpler `"{msg} at line X, column Y"` style message.
pub(crate) fn fmt_with_snippet_or_fallback(
    f: &mut fmt::Formatter<'_>,
    msg: &str,
    location: &Location,
    text: &str,
    crop_radius: usize,
) -> fmt::Result {
    if location == &Location::UNKNOWN {
        return write!(f, "{msg}");
    }

    // `Location` is 1-based.
    let row = location.line as usize;
    let col = location.column as usize;

    let Some(start) = line_col_to_byte_offset(text, row, col) else {
        return fmt_with_location(f, msg, location);
    };
    let end = next_char_boundary(text, start).unwrap_or(start);

    // Render a small window around the error location:
    // - two lines before
    // - the error line
    // - two lines after
    // clipped to input boundaries.
    let line_starts = line_starts(text);
    if line_starts.is_empty() {
        return fmt_with_location(f, msg, location);
    }
    let total_lines = line_starts.len();
    let window_start_row = row.saturating_sub(2).max(1);
    let window_end_row = (row + 2).min(total_lines);
    let window_start_row = window_start_row.min(window_end_row);

    let window_start = line_starts[window_start_row - 1];
    let window_end = if window_end_row < total_lines {
        line_starts[window_end_row]
    } else {
        text.len()
    };
    let window_text = &text[window_start..window_end];

    let local_start = start.saturating_sub(window_start).min(window_text.len());
    let local_end = end.saturating_sub(window_start).min(window_text.len());

    // Horizontal cropping (by character columns) for very long lines.
    // We crop *all* lines in the vertical window to the same column window around the error,
    // so context lines remain vertically aligned.
    let (window_text, local_start, local_end) = crop_window_text(
        window_text,
        window_start_row,
        row,
        col,
        crop_radius,
        local_start,
        local_end,
    );

    // Keep the previous error message in the new output.
    let report = &[Level::ERROR
        .primary_title(format!("<input>:{row}:{col}: {msg}"))
        .element(
            Snippet::source(&window_text)
                .line_start(window_start_row)
                .path("<input>")
                .fold(false)
                .annotation(AnnotationKind::Primary.span(local_start..local_end).label(msg)),
        )];

    // Prefer rustc-like caret markers in plain console output.
    let renderer = Renderer::styled().decor_style(DecorStyle::Ascii);
    write!(f, "{}", renderer.render(report))
}

/// Print a message optionally suffixed with `"at line X, column Y"`.
///
/// Used as a fallback when snippet rendering is not possible.
fn fmt_with_location(f: &mut fmt::Formatter<'_>, msg: &str, location: &Location) -> fmt::Result {
    if location != &Location::UNKNOWN {
        write!(
            f,
            "{msg} at line {}, column {}",
            location.line, location.column
        )
    } else {
        write!(f, "{msg}")
    }
}

/// Horizontally crop the snippet window by character columns.
///
/// Crops *all* lines of `window_text` to the same `[left_col, right_col]` window around the
/// reported error column so that context remains vertically aligned, and rebases the primary
/// annotation span (`local_start..local_end`) to the new, cropped text.
fn crop_window_text(
    window_text: &str,
    window_start_row: usize,
    error_row: usize,
    error_col: usize,
    crop_radius: usize,
    local_start: usize,
    local_end: usize,
) -> (String, usize, usize) {
    if crop_radius == 0 {
        return (window_text.to_owned(), local_start, local_end);
    }

    let left_col = error_col.saturating_sub(crop_radius).max(1);
    let right_col = error_col.saturating_add(crop_radius);

    let mut out = String::with_capacity(window_text.len().min(4096));
    let mut old_pos = 0usize;
    let mut new_local_start = local_start;
    let mut new_local_end = local_end;

    // Iterate over lines while preserving line endings.
    let mut row = window_start_row;
    while old_pos < window_text.len() {
        let next_nl = window_text[old_pos..].find('\n').map(|i| old_pos + i);
        let (line, had_nl, consumed) = match next_nl {
            Some(nl) => (&window_text[old_pos..nl], true, (nl - old_pos) + 1),
            None => (&window_text[old_pos..], false, window_text.len() - old_pos),
        };

        let line_start_old = old_pos;
        let line_start_new = out.len();

        let (cropped_line, crop) = crop_line_by_cols(line, left_col, right_col);
        out.push_str(&cropped_line);
        if had_nl {
            out.push('\n');
        }

        if row == error_row {
            // Rebase annotation span from the old window_text to the cropped output.
            let old_in_line_start = local_start.saturating_sub(line_start_old).min(line.len());
            let old_in_line_end = local_end.saturating_sub(line_start_old).min(line.len());

            let old_in_line_start = old_in_line_start.saturating_sub(crop.start_byte);
            let old_in_line_end = old_in_line_end.saturating_sub(crop.start_byte);

            new_local_start = line_start_new + crop.prefix_bytes + old_in_line_start;
            new_local_end = line_start_new + crop.prefix_bytes + old_in_line_end;

            // Clamp to produced line to avoid out-of-bounds spans.
            let max = line_start_new + cropped_line.len();
            new_local_start = new_local_start.min(max);
            new_local_end = new_local_end.min(max);
        }

        old_pos += consumed;
        row += 1;
        if !had_nl {
            break;
        }
    }

    (out, new_local_start, new_local_end)
}

/// Cropping metadata for a single rendered line.
///
/// Used to rebase the annotation span after applying horizontal cropping.
#[derive(Clone, Copy, Debug)]
struct LineCrop {
    start_byte: usize,
    prefix_bytes: usize,
}

/// Crop one line by 1-based character columns.
///
/// Returns the cropped line plus enough metadata to rebase a byte-offset span from the original
/// line into the cropped output. If the line is not cropped, the returned `LineCrop` will have
/// `start_byte = 0` and `prefix_bytes = 0`.
fn crop_line_by_cols(line: &str, left_col_1: usize, right_col_1: usize) -> (String, LineCrop) {
    let line_len_cols = line.chars().count();
    if line_len_cols == 0 {
        return (
            String::new(),
            LineCrop {
                start_byte: 0,
                prefix_bytes: 0,
            },
        );
    }
    if left_col_1 <= 1 && right_col_1 >= line_len_cols {
        return (
            line.to_owned(),
            LineCrop {
                start_byte: 0,
                prefix_bytes: 0,
            },
        );
    }

    let start_col = left_col_1.min(line_len_cols + 1);
    let end_col_excl = right_col_1.saturating_add(1).min(line_len_cols + 1);

    let start_byte = col_to_byte_offset_in_line(line, start_col).unwrap_or(0);
    let end_byte = col_to_byte_offset_in_line(line, end_col_excl).unwrap_or(line.len());

    let left_clipped = start_col > 1 && start_byte > 0;
    let right_clipped = end_col_excl <= line_len_cols && end_byte < line.len();

    let mut out = String::new();
    if left_clipped {
        out.push('…');
    }
    out.push_str(&line[start_byte..end_byte]);
    if right_clipped {
        out.push('…');
    }

    let prefix_bytes = if left_clipped { '…'.len_utf8() } else { 0 };
    (
        out,
        LineCrop {
            start_byte,
            prefix_bytes,
        },
    )
}

/// Convert 1-based column to a byte offset within a single line (no newlines).
///
/// Allows pointing at EOL (col == len+1).
fn col_to_byte_offset_in_line(line: &str, col_1: usize) -> Option<usize> {
    if col_1 == 0 {
        return None;
    }
    let mut col = 1usize;
    for (i, _ch) in line.char_indices() {
        if col == col_1 {
            return Some(i);
        }
        col += 1;
    }
    if col == col_1 {
        return Some(line.len());
    }
    None
}

/// Compute byte offsets of all line starts in `source`.
///
/// Returns a vector of indices such that `starts[i]` is the byte offset of the first character
/// of line `i + 1`.
fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (i, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(i + 1);
        }
    }
    // Ensure a last line start exists even if the input ends with a newline.
    if starts.last().copied() == Some(source.len()) {
        starts.pop();
    }
    starts
}

/// Convert a 1-based (row, col) to a byte offset within `source`.
fn line_col_to_byte_offset(source: &str, row_1: usize, col_1: usize) -> Option<usize> {
    if row_1 == 0 || col_1 == 0 {
        return None;
    }
    let starts = line_starts(source);
    if starts.is_empty() {
        return None;
    }
    let row_idx = row_1 - 1;
    if row_idx >= starts.len() {
        return None;
    }
    let line_start = starts[row_idx];
    let line_end = if row_idx + 1 < starts.len() {
        starts[row_idx + 1].saturating_sub(1) // strip '\n'
    } else {
        source.len()
    };
    let line = &source[line_start..line_end];
    col_to_byte_offset_in_line(line, col_1).map(|off| line_start + off)
}

/// Return the next UTF-8 character boundary after `start`.
///
/// Used to create a minimal (single-character) span for the primary annotation.
fn next_char_boundary(source: &str, start: usize) -> Option<usize> {
    if start >= source.len() {
        return None;
    }
    let s = &source[start..];
    let mut it = s.char_indices();
    let _ = it.next()?; // first char at 0
    match it.next() {
        Some((i, _)) => Some(start + i),
        None => Some(source.len()),
    }
}
