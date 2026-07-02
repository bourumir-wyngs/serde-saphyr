use std::fmt;

use annotate_snippets::{
    AnnotationKind, Level, Renderer, Snippet as AnnotateSnippet, renderer::DecorStyle,
};

use crate::Location;
use crate::localizer::Localizer;

/// Borrowed YAML source information used for snippet rendering.
///
/// This is intentionally lightweight (borrows `&str`s) so we can format errors without
/// storing the whole YAML input inside error values.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SnippetSource<'a> {
    /// The YAML text to render a snippet from.
    pub(crate) text: &'a str,
    /// Display path/name for the snippet header (e.g. `"<input>"`, `"config.yaml"`).
    pub(crate) path: &'a str,
}

/// Mapping from absolute YAML line numbers (stored in [`Location`]) to the line numbering
/// used by the snippet `text`.
///
/// Most entry points render snippets against the full input (`Identity`). Reader-based
/// entry points may only have a window/fragment of the input; in that case we use `Offset`
/// to translate absolute line numbers into the fragment’s coordinates.
#[derive(Clone, Copy, Debug)]
pub(crate) enum LineMapping {
    /// The snippet text starts at line 1 (normal string-based entry points).
    Identity,
    /// The snippet text is a fragment that starts at `start_line` (1-based).
    Offset { start_line: usize },
}

struct SnippetWindowRows<'a> {
    relative_row: usize,
    total_lines: usize,
    window_start_row: usize,
    window_end_row: usize,
    window_start_absolute_row: usize,
    window_start: usize,
    window_text: &'a str,
    line_starts: Vec<usize>,
}

impl SnippetWindowRows<'_> {
    fn max_display_row(&self) -> usize {
        self.window_start_absolute_row
            .saturating_add(self.window_end_row)
            .saturating_sub(self.window_start_row)
    }
}

struct SnippetRenderWindow<'a> {
    rows: SnippetWindowRows<'a>,
    error_col: usize,
    local_start: usize,
    local_end: usize,
}

fn resolve_window_rows<'a>(
    text: &'a str,
    location: &Location,
    mapping: LineMapping,
) -> Result<SnippetWindowRows<'a>, usize> {
    // Map absolute YAML line to the coordinates within `text`.
    let absolute_row = location.line as usize;
    let relative_row = match mapping {
        LineMapping::Identity => absolute_row,
        LineMapping::Offset { start_line } => {
            if absolute_row < start_line {
                return Err(start_line);
            }
            absolute_row.saturating_sub(start_line).saturating_add(1)
        }
    };

    let line_starts = line_starts(text);
    if line_starts.is_empty() {
        return Err(1);
    }
    if relative_row == 0 || relative_row > line_starts.len() {
        let start_line = match mapping {
            LineMapping::Identity => 1,
            LineMapping::Offset { start_line } => start_line,
        };
        return Err(start_line);
    }

    // Render a small window around the error location:
    // - two lines before
    // - the error line
    // - two lines after
    // clipped to input boundaries.
    let total_lines = line_starts.len();
    let window_start_row = relative_row.saturating_sub(2).max(1);
    let window_end_row = relative_row.saturating_add(2).min(total_lines);
    let window_start_row = window_start_row.min(window_end_row);

    let window_start = line_starts[window_start_row - 1];
    let window_end = if window_end_row < total_lines {
        line_starts[window_end_row]
    } else {
        text.len()
    };
    let window_text = &text[window_start..window_end];

    // Map the window's starting line number back to absolute coordinates for display.
    let window_start_absolute_row = match mapping {
        LineMapping::Identity => window_start_row,
        LineMapping::Offset { start_line } => start_line
            .saturating_add(window_start_row)
            .saturating_sub(1),
    };

    Ok(SnippetWindowRows {
        relative_row,
        total_lines,
        window_start_row,
        window_end_row,
        window_start_absolute_row,
        window_start,
        window_text,
        line_starts,
    })
}

fn resolve_render_window<'a>(
    text: &'a str,
    location: &Location,
    mapping: LineMapping,
) -> Result<SnippetRenderWindow<'a>, usize> {
    let rows = resolve_window_rows(text, location, mapping)?;
    let col = location.column as usize;

    let Some(start) =
        line_col_to_byte_offset_with_starts(text, &rows.line_starts, rows.relative_row, col)
    else {
        return Err(rows.window_start_absolute_row);
    };

    // Create a minimal span for the primary annotation:
    // - usually one character
    // - for EOL (pointing at '\n') or EOF, use an empty span (caret-like).
    let end = match text.as_bytes().get(start) {
        Some(b'\n') | Some(b'\r') => start,
        _ => next_char_boundary(text, start).unwrap_or(start),
    };

    let local_start = start
        .saturating_sub(rows.window_start)
        .min(rows.window_text.len());
    let local_end = end
        .saturating_sub(rows.window_start)
        .min(rows.window_text.len());

    Ok(SnippetRenderWindow {
        rows,
        error_col: col,
        local_start,
        local_end,
    })
}

/// Crop a small source window around `location` and return `(cropped_text, start_line)`.
///
/// - `cropped_text` contains a vertical window of a few lines around the error location.
/// - `start_line` is the 1-based line number in the *original* input where `cropped_text` starts.
///
/// The returned `cropped_text` is suitable for deferred snippet rendering via
/// `Snippet::new(&cropped_text, ..).with_offset(start_line)`.
#[cold]
#[inline(never)]
pub(crate) fn crop_source_window(
    text: &str,
    location: &Location,
    mapping: LineMapping,
    crop_radius: usize,
) -> (String, usize) {
    if text.is_empty() || location == &Location::UNKNOWN {
        return (String::new(), 1);
    }

    // Keep snippet coordinates aligned with parsers that ignore a leading UTF-8 BOM.
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);

    let rows = match resolve_window_rows(text, location, mapping) {
        Ok(rows) => rows,
        Err(start_line) => return (String::new(), start_line),
    };
    let window_text = rows.window_text;

    // Storage-time horizontal cropping to avoid retaining huge single-line scalars.
    //
    // Important correctness constraint: later snippet rendering maps the *original* column
    // number from `Location` into a byte offset within the stored snippet text.
    // Therefore we must NOT remove bytes to the left of the error column on the error line.
    //
    // Policy:
    // - Error line: keep full prefix, truncate only the right side past `error_col + crop_radius`.
    // - Context lines: crop both sides using the same `[left_col, right_col]` window.
    let cropped = if crop_radius == 0 {
        window_text.to_owned()
    } else {
        // Avoid changing the rendered output for normal-sized inputs.
        // Only apply storage-time horizontal cropping when the vertical window contains
        // very large lines (e.g., base64 blobs / huge scalars), to reduce memory retention.
        let needs_storage_crop = window_text.len() > 16 * 1024
            || window_text
                .lines()
                .any(|l| l.strip_suffix('\r').unwrap_or(l).len() > 4 * 1024);

        if !needs_storage_crop {
            return (
                sanitize_terminal_snippet_preserve_len(window_text.to_owned()),
                rows.window_start_absolute_row,
            );
        }

        let error_row = rows.relative_row;
        let error_col = location.column as usize;
        let left_col = error_col.saturating_sub(crop_radius).max(1);
        let right_col = error_col.saturating_add(crop_radius);

        let mut out = String::with_capacity(window_text.len().min(4096));
        visit_window_lines(window_text, rows.window_start_row, |window_line| {
            if window_line.row == error_row {
                // Preserve the left prefix so `error_col` continues to map correctly.
                let end_col_excl = right_col.saturating_add(1);
                let end_byte = col_to_byte_offset_in_line(window_line.line, end_col_excl)
                    .unwrap_or(window_line.line.len());
                let right_clipped = end_byte < window_line.line.len();
                out.push_str(&window_line.line[..end_byte]);
                if right_clipped {
                    out.push('…');
                }
            } else {
                let (rendered, _crop) = crop_line_by_cols(window_line.line, left_col, right_col);
                out.push_str(&rendered);
            }

            if window_line.had_nl {
                out.push('\n');
            }
        });

        sanitize_terminal_snippet_preserve_len(out)
    };

    (cropped, rows.window_start_absolute_row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::localizer::DefaultEnglishLocalizer;
    use std::fmt;

    struct SecondarySnippet<'a> {
        text: &'a str,
        location: Location,
        start_line: usize,
        msg: &'a str,
    }

    impl fmt::Display for SecondarySnippet<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt_snippet_window_offset_or_fallback(
                f,
                &DefaultEnglishLocalizer,
                &self.location,
                self.text,
                self.start_line,
                self.msg,
                80,
            )
        }
    }

    #[test]
    fn crop_source_window_truncates_huge_error_line_on_the_right_only() {
        let text = format!("key: {}\n", "A".repeat(10_000));
        let loc = Location::new(1, 6);

        let (cropped, start_line) = crop_source_window(&text, &loc, LineMapping::Identity, 20);
        assert_eq!(start_line, 1);

        // Should not retain the full blob.
        assert!(cropped.len() < 512, "cropped len was {}", cropped.len());

        let line = cropped.lines().next().unwrap();
        assert!(line.starts_with("key: "));
        assert!(
            line.ends_with('…'),
            "expected ellipsis at end, got: {line:?}"
        );

        // The original column must still map: col 6 points at the first 'A'.
        let off = col_to_byte_offset_in_line(line, 6).expect("col 6 offset");
        assert_eq!(&line[off..off + 1], "A");
    }

    #[test]
    fn crop_source_window_crops_context_lines_on_both_sides() {
        let text = format!(
            "{}\nkey: {}\n{}\n",
            "X".repeat(10_000),
            "A".repeat(10_000),
            "Y".repeat(10_000)
        );
        let loc = Location::new(2, 6);

        let (cropped, _start_line) = crop_source_window(&text, &loc, LineMapping::Identity, 8);
        let mut lines = cropped.lines();

        let before = lines.next().unwrap();
        let error = lines.next().unwrap();
        let after = lines.next().unwrap();

        // With an error near the start of the line (col 6), `left_col` resolves to 1,
        // so context lines are expected to be right-truncated, not left-truncated.
        assert!(
            before.ends_with('…'),
            "before line not right-truncated: {before:?}"
        );
        assert!(
            after.ends_with('…'),
            "after line not right-truncated: {after:?}"
        );
        assert!(
            before.len() < 256,
            "before line too large: {}",
            before.len()
        );
        assert!(after.len() < 256, "after line too large: {}", after.len());

        // Error line must keep its left prefix for column mapping.
        assert!(error.starts_with("key: "));
    }

    #[test]
    fn crop_source_window_does_not_keep_full_context_lines_when_window_starts_past_eol() {
        let text = format!(
            "{}\nkey: {}\n{}\n",
            "X".repeat(8_000),
            "A".repeat(20_000),
            "Y".repeat(8_000)
        );
        let loc = Location::new(2, 15_000);

        let (cropped, _start_line) = crop_source_window(&text, &loc, LineMapping::Identity, 64);
        let mut lines = cropped.lines();

        let before = lines.next().unwrap();
        let _error = lines.next().unwrap();
        let after = lines.next().unwrap();

        assert_eq!(before, "…", "unexpected before line: {before:?}");
        assert_eq!(after, "…", "unexpected after line: {after:?}");
    }

    #[test]
    fn crop_source_window_sanitizes_normal_sized_windows() {
        let text = "field: \x1B]0;malicious title\x07\n";
        let loc = Location::new(1, 8);

        let (cropped, start_line) = crop_source_window(text, &loc, LineMapping::Identity, 80);

        assert_eq!(start_line, 1);
        assert_eq!(cropped.len(), text.len());
        assert!(!cropped.contains("\x1B]0;"));
        assert!(!cropped.contains('\x07'));
    }

    #[test]
    fn secondary_snippet_caret_aligns_with_two_digit_line_gutter() {
        let mut text = String::new();
        for line in 1..=12 {
            if line == 10 {
                text.push_str("key: value\n");
            } else {
                text.push_str("ctx\n");
            }
        }

        let rendered = SecondarySnippet {
            text: &text,
            location: Location::new(10, 6),
            start_line: 1,
            msg: "defined here",
        }
        .to_string();

        let source_line = rendered
            .lines()
            .find(|line| line.starts_with("10 | "))
            .expect("line 10 should be rendered");
        let caret_line = rendered
            .lines()
            .find(|line| line.contains('^'))
            .expect("caret line should be rendered");

        let value_index = source_line
            .find('v')
            .expect("source line should contain value");
        let caret_index = caret_line
            .find('^')
            .expect("caret line should contain caret");
        assert_eq!(
            caret_index, value_index,
            "caret should align with the referenced column:\n{rendered}"
        );
    }
}

/// Parameters controlling how to render a diagnostic snippet.
///
/// Bundles together:
/// - the snippet source (`text` + display `path`),
/// - how to interpret line numbers (`mapping`),
/// - and horizontal cropping policy (`crop_radius`).
///
/// This is passed by value (Copy) to keep call sites concise and to avoid long argument lists.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Snippet<'a> {
    pub(crate) source: SnippetSource<'a>,
    pub(crate) mapping: LineMapping,
    /// Maximum number of *columns* to keep on each side of the error column when cropping
    /// very long lines. `0` effectively disables snippet rendering at higher layers.
    pub(crate) crop_radius: usize,
}

impl<'a> Snippet<'a> {
    #[inline]
    /// Create a snippet renderer configuration for a full-text source (line numbers start at 1).
    pub(crate) fn new(text: &'a str, path: &'a str, crop_radius: usize) -> Self {
        Self {
            source: SnippetSource { text, path },
            mapping: LineMapping::Identity,
            crop_radius,
        }
    }

    #[inline]
    /// Convert this snippet configuration into one that renders snippets for a text fragment.
    ///
    /// The `text` stored in this snippet is treated as starting at `start_line` (1-based)
    /// in the original YAML stream.
    pub(crate) fn with_offset(self, start_line: usize) -> Self {
        debug_assert!(start_line >= 1);
        Self {
            mapping: LineMapping::Offset { start_line },
            ..self
        }
    }

    pub(crate) fn fmt_or_fallback(
        self,
        f: &mut fmt::Formatter<'_>,
        level: Level,
        l10n: &dyn Localizer,
        msg: &str,
        location: &Location,
    ) -> fmt::Result {
        self.fmt_or_fallback_with_label(f, level, l10n, msg, msg, location)
    }

    pub(crate) fn fmt_or_fallback_with_label(
        self,
        f: &mut fmt::Formatter<'_>,
        level: Level,
        l10n: &dyn Localizer,
        msg: &str,
        label: &str,
        location: &Location,
    ) -> fmt::Result {
        if location == &Location::UNKNOWN {
            return write!(f, "{msg}");
        }

        let window = match resolve_render_window(self.source.text, location, self.mapping) {
            Ok(window) => window,
            Err(_) => return fmt_with_location(f, l10n, msg, location),
        };

        // Horizontal cropping (by character columns) for very long lines.
        // We crop lines in the vertical window to the same column window around the error,
        // so context lines remain aligned where possible.
        // Very short context lines that would otherwise crop to empty are left intact to
        // preserve useful context.
        let (window_text, local_start, local_end) = crop_window_text(
            window.rows.window_text,
            window.rows.window_start_row,
            window.rows.relative_row,
            window.error_col,
            self.crop_radius,
            window.local_start,
            window.local_end,
        );

        let loc_prefix = l10n.snippet_location_prefix(*location);

        let report = &[level
            .primary_title(format!("{}: {msg}", loc_prefix))
            .element(
                AnnotateSnippet::source(&window_text)
                    .line_start(window.rows.window_start_absolute_row)
                    .path(self.source.path)
                    .fold(false)
                    .annotation(
                        AnnotationKind::Primary
                            .span(local_start..local_end)
                            .label(label),
                    ),
            )];

        // Prefer rustc-like caret markers and avoid ANSI colors in `Display` output.
        // This keeps error strings stable (e.g. for tests) and avoids emitting escape
        // sequences when the output is not a TTY.
        let renderer = Renderer::plain().decor_style(DecorStyle::Ascii);
        write!(f, "{}", renderer.render(report))
    }
}

/// Like [`fmt_snippet_window_or_fallback`], but renders against a text fragment whose line
/// numbering may be offset.
pub(crate) fn fmt_snippet_window_offset_or_fallback(
    f: &mut fmt::Formatter<'_>,
    l10n: &dyn Localizer,
    location: &Location,
    text: &str,
    start_line: usize,
    msg: &str,
    crop_radius: usize,
) -> fmt::Result {
    fmt_snippet_window_with_mapping_or_fallback(
        f,
        l10n,
        location,
        text,
        LineMapping::Offset { start_line },
        msg,
        crop_radius,
    )
}

pub(crate) fn snippet_window_frame_prefix_offset(
    text: &str,
    start_line: usize,
    location: &Location,
) -> Option<String> {
    if location == &Location::UNKNOWN {
        return None;
    }

    let rows = resolve_window_rows(text, location, LineMapping::Offset { start_line }).ok()?;
    Some(snippet_window_frame_prefix(
        rows.max_display_row().to_string().len(),
    ))
}

fn snippet_window_frame_prefix(gutter_width: usize) -> String {
    format!("{:>gutter_width$} |", "")
}

fn fmt_snippet_window_with_mapping_or_fallback(
    f: &mut fmt::Formatter<'_>,
    _l10n: &dyn Localizer,
    location: &Location,
    text: &str,
    mapping: LineMapping,
    msg: &str,
    crop_radius: usize,
) -> fmt::Result {
    if location == &Location::UNKNOWN {
        return Ok(());
    }

    let window = match resolve_render_window(text, location, mapping) {
        Ok(window) => window,
        Err(_) => return Ok(()),
    };

    let (window_text, local_start, _local_end) = crop_window_text(
        window.rows.window_text,
        window.rows.window_start_row,
        window.rows.relative_row,
        window.error_col,
        crop_radius,
        window.local_start,
        window.local_end,
    );

    let gutter_width = window.rows.max_display_row().to_string().len();
    let empty_gutter = snippet_window_frame_prefix(gutter_width);
    writeln!(f, "{empty_gutter}")?;

    let mut cur_row = window.rows.window_start_row;
    for line in window_text.split_inclusive('\n') {
        let mut line = line;
        if let Some(stripped) = line.strip_suffix('\n') {
            line = stripped;
        }
        if let Some(stripped) = line.strip_suffix('\r') {
            line = stripped;
        }

        let display_row = window
            .rows
            .window_start_absolute_row
            .saturating_add(cur_row)
            .saturating_sub(window.rows.window_start_row);
        writeln!(f, "{display_row:>gutter_width$} | {line}")?;

        if cur_row == window.rows.relative_row {
            let line_byte_start = window_text[..local_start]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let caret_chars = window_text[line_byte_start..local_start].chars().count();
            if msg.is_empty() {
                writeln!(f, "{empty_gutter} {space:>caret_chars$}^", space = "")?;
            } else {
                writeln!(
                    f,
                    "{empty_gutter} {space:>caret_chars$}^ {msg}",
                    space = "",
                    msg = msg
                )?;
            }
        }

        cur_row += 1;
        if cur_row > window.rows.window_end_row {
            break;
        }
    }

    if window.rows.window_end_row == window.rows.total_lines
        && window_text.ends_with('\n')
        && cur_row <= window.rows.window_end_row
    {
        let display_row = window
            .rows
            .window_start_absolute_row
            .saturating_add(cur_row)
            .saturating_sub(window.rows.window_start_row);
        writeln!(f, "{display_row:>gutter_width$} |")?;

        if cur_row == window.rows.relative_row {
            let line_byte_start = window_text[..local_start]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let caret_chars = window_text[line_byte_start..local_start].chars().count();
            if msg.is_empty() {
                writeln!(f, "{empty_gutter} {space:>caret_chars$}^", space = "")?;
            } else {
                writeln!(
                    f,
                    "{empty_gutter} {space:>caret_chars$}^ {msg}",
                    space = "",
                    msg = msg
                )?;
            }
        }
    }

    writeln!(f, "{empty_gutter}")
}

/// Print a message optionally suffixed with a localized location suffix.
///
/// Used as a fallback when snippet rendering is not possible.
fn fmt_with_location(
    f: &mut fmt::Formatter<'_>,
    l10n: &dyn Localizer,
    msg: &str,
    location: &Location,
) -> fmt::Result {
    let out = l10n.attach_location(std::borrow::Cow::Borrowed(msg), *location);
    write!(f, "{out}")
}

/// Returns true if `text` contains no control characters that could mess with terminal/log output.
///
/// Keeps `\n` and `\t` (so the snippet stays readable).
/// Treats:
/// - ASCII C0 controls (0x00..=0x1F) and DEL (0x7F) as unsafe (except \n/\t)
/// - UTF-8 encoded C1 controls U+0080..U+009F (bytes: 0xC2 0x80..=0x9F) as unsafe.
pub(crate) fn is_terminal_snippet_clean(text: &str) -> bool {
    let b = text.as_bytes();

    // Fast scan for ASCII controls.
    for &x in b {
        if (x < 0x20 && x != b'\n' && x != b'\t') || x == 0x7F {
            return false;
        }
    }

    // Scan for UTF-8 encoded C1 control block: 0xC2 0x80..=0x9F
    let mut i = 0usize;
    while i + 1 < b.len() {
        if b[i] == 0xC2 && (0x80..=0x9F).contains(&b[i + 1]) {
            return false;
        }
        i += 1;
    }

    true
}

/// Sanitize snippet text for terminal/log display *without changing the byte length*.
///
/// This is important because snippet renderers tend to use byte offsets for spans.
/// We:
/// - Replace ASCII control bytes (except '\n' and '\t') and DEL with space (0x20).
/// - Replace UTF-8 encoded C1 controls (0xC2 0x80..=0x9F) with NBSP (0xC2 0xA0).
///
/// This breaks ANSI/OSC escapes by neutralizing their introducers (ESC or C1).
pub(crate) fn sanitize_terminal_snippet_preserve_len(s: String) -> String {
    let mut bytes = s.into_bytes();

    // ASCII C0 controls + DEL
    for x in &mut bytes {
        let b = *x;
        if (b < 0x20 && b != b'\n' && b != b'\t') || b == 0x7F {
            *x = b' ';
        }
    }

    // UTF-8 encoded C1 controls (U+0080..U+009F): 0xC2 0x80..=0x9F
    // Convert to NBSP (U+00A0): 0xC2 0xA0 (same length).
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] == 0xC2 && (0x80..=0x9F).contains(&bytes[i + 1]) {
            bytes[i + 1] = 0xA0;
            i += 2;
            continue;
        }
        i += 1;
    }

    // This should never fail because we preserved UTF-8 validity.
    match String::from_utf8(bytes) {
        Ok(out) => out,
        Err(e) => String::from_utf8_lossy(&e.into_bytes()).into_owned(),
    }
}

struct WindowLine<'a> {
    row: usize,
    line_start_old: usize,
    line: &'a str,
    had_nl: bool,
}

fn visit_window_lines(
    window_text: &str,
    window_start_row: usize,
    mut visit: impl FnMut(WindowLine<'_>),
) -> usize {
    let mut old_pos = 0usize;
    let mut row = window_start_row;

    while old_pos < window_text.len() {
        let next_nl = window_text[old_pos..].find('\n').map(|i| old_pos + i);
        let (line_raw, had_nl, consumed) = match next_nl {
            Some(nl) => (&window_text[old_pos..nl], true, (nl - old_pos) + 1),
            None => (
                &window_text[old_pos..],
                false,
                window_text.len().saturating_sub(old_pos),
            ),
        };

        // Normalize CRLF: strip a trailing '\r' from the line content if present.
        let line = line_raw.strip_suffix('\r').unwrap_or(line_raw);

        visit(WindowLine {
            row,
            line_start_old: old_pos,
            line,
            had_nl,
        });

        old_pos = old_pos.saturating_add(consumed);
        row = row.saturating_add(1);
        if !had_nl {
            break;
        }
    }

    row
}

/// Horizontally crop the snippet window by character columns and normalize CRLF.
///
/// - Crops lines of `window_text` to the same `[left_col, right_col]` window around the
///   reported error column so that context remains vertically aligned where possible.
///   Very short context lines that would otherwise crop to empty are left intact to preserve
///   useful context.
/// - Rebases the primary annotation span (`local_start..local_end`) to the new, cropped text.
/// - Strips a trailing `\r` from each line (CRLF normalization) to avoid column skew and
///   terminal rendering issues.
fn crop_window_text(
    window_text: &str,
    window_start_row: usize,
    error_row: usize,
    error_col: usize,
    crop_radius: usize,
    local_start: usize,
    local_end: usize,
) -> (String, usize, usize) {
    // Fast path: no horizontal cropping, no CRs, and no other unsafe control chars.
    if crop_radius == 0
        && !window_text.as_bytes().contains(&b'\r')
        && is_terminal_snippet_clean(window_text)
    {
        return (window_text.to_owned(), local_start, local_end);
    }

    let do_crop = crop_radius != 0;
    let left_col = error_col.saturating_sub(crop_radius).max(1);
    let right_col = error_col.saturating_add(crop_radius);

    let mut out = String::with_capacity(window_text.len().min(4096));
    let mut new_local_start = local_start;
    let mut new_local_end = local_end;
    let mut rebased = false;

    // Iterate over lines while preserving '\n' endings (and normalizing away '\r').
    let row = visit_window_lines(window_text, window_start_row, |window_line| {
        let line = window_line.line;
        let line_start_old = window_line.line_start_old;
        let line_start_new = out.len();

        let (rendered_line, crop) = if do_crop {
            crop_line_by_cols(line, left_col, right_col)
        } else {
            (
                line.to_owned(),
                LineCrop {
                    start_byte: 0,
                    prefix_bytes: 0,
                },
            )
        };

        out.push_str(&rendered_line);
        if window_line.had_nl {
            out.push('\n');
        }

        if window_line.row == error_row {
            // Rebase annotation span from the old window_text to the new output.
            //
            // local_start/local_end are byte offsets into the original `window_text`.
            // Clamp into the (possibly CR-stripped) `line` slice so EOL/CRLF cases remain valid.
            let mut old_in_line_start = local_start.saturating_sub(line_start_old);
            let mut old_in_line_end = local_end.saturating_sub(line_start_old);

            // `line_raw` can be longer than `line` by exactly 1 byte (a trailing '\r').
            // Clamping to `line.len()` maps any reference to '\r' or '\n' (EOL) to the
            // end of the visible line, which matches user-facing columns.
            old_in_line_start = old_in_line_start.min(line.len());
            old_in_line_end = old_in_line_end.min(line.len());

            // Apply horizontal cropping rebase.
            old_in_line_start = old_in_line_start.saturating_sub(crop.start_byte);
            old_in_line_end = old_in_line_end.saturating_sub(crop.start_byte);

            new_local_start = line_start_new + crop.prefix_bytes + old_in_line_start;
            new_local_end = line_start_new + crop.prefix_bytes + old_in_line_end;

            // Clamp to the produced line (exclude the pushed '\n').
            let max = line_start_new + rendered_line.len();
            new_local_start = new_local_start.min(max);
            new_local_end = new_local_end.min(max);
            if new_local_end < new_local_start {
                new_local_end = new_local_start;
            }

            rebased = true;
        }
    });

    // If the window ends with '\n', there is an implicit trailing empty line.
    // If the error location is on that empty line (common for EOF errors), rebase the
    // annotation span to the new end of output.
    if !rebased && window_text.ends_with('\n') && row == error_row {
        new_local_start = out.len();
        new_local_end = out.len();
    }

    // Final safety clamp.
    let max = out.len();
    new_local_start = new_local_start.min(max);
    new_local_end = new_local_end.min(max);
    if new_local_end < new_local_start {
        new_local_end = new_local_start;
    }

    // Before returning:
    let out = sanitize_terminal_snippet_preserve_len(out);

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
///
/// Notes:
/// - For non-empty input, this always contains at least `0`.
/// - If `source` ends with `\n`, the returned vector includes an extra start at `source.len()`
///   to represent the trailing empty line.
/// - For empty input, returns an empty vector.
fn line_starts(source: &str) -> Vec<usize> {
    if source.is_empty() {
        return Vec::new();
    }

    let mut starts = vec![0usize];
    for (i, b) in source.as_bytes().iter().enumerate() {
        if *b == b'\n' {
            // Safe UTF-8 boundary: '\n' is ASCII (1 byte).
            starts.push(i + 1);
        }
    }
    starts
}

/// Convert a 1-based (row, col) to a byte offset within `source`, given precomputed line starts.
///
/// Parameters:
/// - `source`: Full source text.
/// - `starts`: Output of [`line_starts`], i.e. byte indices of each line start.
/// - `row_1`: 1-based line number.
/// - `col_1`: 1-based character column within that line (Unicode scalar values; not bytes).
///
/// Returns:
/// - `Some(byte_offset)` into `source` if the coordinates are valid.
/// - `None` if `row_1`/`col_1` are invalid.
///
/// CRLF handling:
/// - If the line ends with `\r\n`, the `\r` is stripped for column computation so that columns
///   match what users typically see (and so that column `len+1` means "EOL before newline").
fn line_col_to_byte_offset_with_starts(
    source: &str,
    starts: &[usize],
    row_1: usize,
    col_1: usize,
) -> Option<usize> {
    if row_1 == 0 || col_1 == 0 {
        return None;
    }
    if starts.is_empty() {
        return None;
    }

    let row_idx = row_1 - 1;
    if row_idx >= starts.len() {
        return None;
    }

    let line_start = starts[row_idx];
    let mut line_end = if row_idx + 1 < starts.len() {
        starts[row_idx + 1].saturating_sub(1) // strip '\n'
    } else {
        source.len()
    };

    // Handle CRLF: strip '\r' too so columns match what users see.
    if line_end > line_start && source.as_bytes().get(line_end - 1) == Some(&b'\r') {
        line_end -= 1;
    }

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
