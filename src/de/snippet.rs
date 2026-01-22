use std::fmt;

use annotate_snippets::{
    AnnotationKind, Level, Renderer, Snippet as AnnotateSnippet, renderer::DecorStyle,
};

use crate::de_error::Error;
use crate::Location;
use crate::Locations;

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
        msg: &str,
        location: &Location,
    ) -> fmt::Result {
        if location == &Location::UNKNOWN {
            return write!(f, "{msg}");
        }

        // `Location` is 1-based and uses *character* columns (not byte offsets).
        let absolute_row = location.line as usize;
        let col = location.column as usize;

        let (relative_row, window_title_row) = match self.mapping {
            LineMapping::Identity => (absolute_row, absolute_row),
            LineMapping::Offset { start_line } => {
                if absolute_row < start_line {
                    return fmt_with_location(f, msg, location);
                }
                let relative = absolute_row
                    .saturating_sub(start_line)
                    .saturating_add(1);
                (relative, absolute_row)
            }
        };

        let line_starts = line_starts(self.source.text);
        if line_starts.is_empty() {
            return fmt_with_location(f, msg, location);
        }

        // Check if the (mapped) row is within our snippet text.
        if relative_row == 0 || relative_row > line_starts.len() {
            return fmt_with_location(f, msg, location);
        }

        let Some(start) =
            line_col_to_byte_offset_with_starts(self.source.text, &line_starts, relative_row, col)
        else {
            return fmt_with_location(f, msg, location);
        };

        // Create a minimal span for the primary annotation:
        // - usually one character
        // - for EOL (pointing at '\n') or EOF, use an empty span (caret-like).
        let end = match self.source.text.as_bytes().get(start) {
            Some(b'\n') | Some(b'\r') => start,
            _ => next_char_boundary(self.source.text, start).unwrap_or(start),
        };

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
            self.source.text.len()
        };
        let window_text = &self.source.text[window_start..window_end];

        let local_start = start.saturating_sub(window_start).min(window_text.len());
        let local_end = end.saturating_sub(window_start).min(window_text.len());

        // Horizontal cropping (by character columns) for very long lines.
        // We crop lines in the vertical window to the same column window around the error,
        // so context lines remain aligned where possible.
        // Very short context lines that would otherwise crop to empty are left intact to
        // preserve useful context.
        let (window_text, local_start, local_end) = crop_window_text(
            window_text,
            window_start_row,
            relative_row,
            col,
            self.crop_radius,
            local_start,
            local_end,
        );

        // Map the window's starting line number back to absolute coordinates for display.
        let window_start_absolute_row = match self.mapping {
            LineMapping::Identity => window_start_row,
            LineMapping::Offset { start_line } => start_line
                .saturating_add(window_start_row)
                .saturating_sub(1),
        };

        let report = &[level
            .primary_title(format!(
                "line {window_title_row} column {col}: {msg}"
            ))
            .element(
                AnnotateSnippet::source(&window_text)
                    .line_start(window_start_absolute_row)
                    .path(self.source.path)
                    .fold(false)
                    .annotation(
                        AnnotationKind::Primary
                            .span(local_start..local_end)
                            .label(msg),
                    ),
            )];

        // Prefer rustc-like caret markers and avoid ANSI colors in `Display` output.
        // This keeps error strings stable (e.g. for tests) and avoids emitting escape
        // sequences when the output is not a TTY.
        let renderer = Renderer::plain().decor_style(DecorStyle::Ascii);
        write!(f, "{}", renderer.render(report))
    }
}

/// Render a rustc-like diagnostic snippet for an error that has a known [`Location`].
///
/// If snippet rendering cannot be produced (e.g., invalid coordinates), this falls back to a
/// simpler `"{msg} at line X, column Y"` style message.
pub(crate) fn fmt_with_snippet_or_fallback(
    f: &mut fmt::Formatter<'_>,
    level: Level,
    msg: &str,
    location: &Location,
    text: &str,
    path: &str,
    crop_radius: usize,
) -> fmt::Result {
    Snippet::new(text, path, crop_radius).fmt_or_fallback(f, level, msg, location)
}

/// Render a rustc-like diagnostic snippet for an error, where the text fragment
/// starts at `text_start_line` (1-based) rather than line 1.
///
/// This is used for reader-based parsing where we only have a sliding window of the input
/// (e.g., from `RingReader`). The `location` contains the absolute line/column in the
/// original stream, and we adjust it relative to the fragment.
pub(crate) fn fmt_with_snippet_offset_or_fallback(
    f: &mut fmt::Formatter<'_>,
    level: Level,
    msg: &str,
    location: &Location,
    ctx: Snippet<'_>,
) -> fmt::Result {
    ctx.fmt_or_fallback(f, level, msg, location)
}

/// Render an [`Error`] using rustc-like snippets when possible.
///
/// This is used by [`Error::with_snippet`](crate::de_error::Error::with_snippet) to pre-render a
/// cropped diagnostic without retaining the full YAML input inside the error.
///
/// When snippet rendering is disabled (`crop_radius == 0`) or cannot be produced, this falls back
/// to the plain `Display` output for the error.
#[cold]
#[inline(never)]
pub(crate) fn render_error_with_snippets(inner: &Error, text: &str, crop_radius: usize) -> String {
    // Safety: snippet rendering is best-effort; if anything goes wrong we fall back
    // to the plain error message.
    if crop_radius == 0 {
        return inner.to_string();
    }

    // Normalize the snippet text to match the coordinate system used by parsing.
    // Our string-based entry points ignore a single leading UTF-8 BOM (`\u{FEFF}`)
    // before parsing, so any `Location { line, column }` we store is relative to
    // the BOM-stripped view. Strip it here as well to keep caret positions aligned.
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);

    #[cfg(feature = "garde")]
    {
        use crate::path_map::PathMap;

        if let Error::ValidationError { report, locations } = inner {
            struct ValidationSnippetDisplay<'a> {
                report: &'a garde::Report,
                locations: &'a PathMap,
                text: &'a str,
                crop_radius: usize,
            }

            impl fmt::Display for ValidationSnippetDisplay<'_> {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    fmt_validation_error_with_snippets(
                        f,
                        self.report,
                        self.locations,
                        self.text,
                        self.crop_radius,
                    )
                }
            }

            return ValidationSnippetDisplay {
                report,
                locations,
                text,
                crop_radius,
            }
                .to_string();
        }

        if let Error::ValidationErrors { errors } = inner {
            let mut out = String::new();
            for (i, err) in errors.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                    out.push('\n');
                }

                // If the nested error already contains snippet output, keep it.
                // Otherwise, render it against the shared input text.
                match err {
                    Error::WithSnippet { .. } => out.push_str(&err.to_string()),
                    other => out.push_str(&render_error_with_snippets(other, text, crop_radius)),
                }
            }
            return out;
        }
    }

    #[cfg(feature = "validator")]
    {
        use crate::path_map::PathMap;
        use validator::ValidationErrors;

        if let Error::ValidatorError { errors, locations } = inner {
            struct ValidatorSnippetDisplay<'a> {
                errors: &'a ValidationErrors,
                locations: &'a PathMap,
                text: &'a str,
                crop_radius: usize,
            }

            impl fmt::Display for ValidatorSnippetDisplay<'_> {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    fmt_validator_error_with_snippets(
                        f,
                        self.errors,
                        self.locations,
                        self.text,
                        self.crop_radius,
                    )
                }
            }

            return ValidatorSnippetDisplay {
                errors,
                locations,
                text,
                crop_radius,
            }
                .to_string();
        }

        if let Error::ValidatorErrors { errors } = inner {
            let mut out = String::new();
            for (i, err) in errors.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                    out.push('\n');
                }

                // If the nested error already contains snippet output, keep it.
                // Otherwise, render it against the shared input text.
                match err {
                    Error::WithSnippet { .. } => out.push_str(&err.to_string()),
                    other => out.push_str(&render_error_with_snippets(other, text, crop_radius)),
                }
            }
            return out;
        }
    }

    // Handle AliasError with dual-location snippet rendering.
    if let Error::AliasError { msg, locations } = inner {
        return fmt_alias_error_with_snippets(msg, locations, text, crop_radius);
    }

    let msg = inner.to_string();
    let Some(location) = inner.location() else {
        return msg;
    };

    struct SnippetDisplay<'a> {
        msg: &'a str,
        location: &'a Location,
        text: &'a str,
        crop_radius: usize,
    }

    impl fmt::Display for SnippetDisplay<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt_with_snippet_or_fallback(
                f,
                Level::ERROR,
                self.msg,
                self.location,
                self.text,
                "<input>",
                self.crop_radius,
            )
        }
    }

    SnippetDisplay {
        msg: &msg,
        location: &location,
        text,
        crop_radius,
    }
        .to_string()
}

/// Render error with snippets, where the text fragment starts at `start_line` (1-based).
///
/// This is used for reader-based parsing where we only have a sliding window of the input.
#[cold]
#[inline(never)]
pub(crate) fn render_error_with_snippets_offset(
    inner: &Error,
    text: &str,
    start_line: usize,
    crop_radius: usize,
) -> String {
    if crop_radius == 0 {
        return inner.to_string();
    }

    // Keep snippet coordinates aligned with parsers that ignore a leading UTF-8 BOM.
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);

    let msg = inner.to_string();
    let Some(location) = inner.location() else {
        return msg;
    };
    if location == Location::UNKNOWN {
        return msg;
    }

    // Check if the error location is within our text fragment.
    let error_line = location.line as usize;
    if error_line < start_line {
        // Error is before our fragment - show a location-only fallback.
        return format!(
            "{msg} at line {}, column {}",
            location.line, location.column
        );
    }

    struct SnippetOffsetDisplay<'a> {
        msg: &'a str,
        location: &'a Location,
        text: &'a str,
        start_line: usize,
        crop_radius: usize,
    }

    impl fmt::Display for SnippetOffsetDisplay<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let ctx =
                Snippet::new(self.text, "<input>", self.crop_radius).with_offset(self.start_line);
            fmt_with_snippet_offset_or_fallback(f, Level::ERROR, self.msg, self.location, ctx)
        }
    }

    SnippetOffsetDisplay {
        msg: &msg,
        location: &location,
        text,
        start_line,
        crop_radius,
    }
        .to_string()
}

#[cold]
#[inline(never)]
fn fmt_alias_error_with_snippets(
    msg: &str,
    locations: &Locations,
    text: &str,
    crop_radius: usize,
) -> String {
    struct AliasSnippetDisplay<'a> {
        msg: &'a str,
        locations: &'a Locations,
        text: &'a str,
        crop_radius: usize,
    }

    impl fmt::Display for AliasSnippetDisplay<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let ref_loc = self.locations.reference_location;
            let def_loc = self.locations.defined_location;

            match (ref_loc, def_loc) {
                (Location::UNKNOWN, Location::UNKNOWN) => write!(f, "{}", self.msg),
                (r, d) if r != Location::UNKNOWN && (d == Location::UNKNOWN || d == r) => {
                    fmt_with_snippet_or_fallback(
                        f,
                        Level::ERROR,
                        self.msg,
                        &r,
                        self.text,
                        "error occurs here",
                        self.crop_radius,
                    )
                }
                (r, d) if r == Location::UNKNOWN && d != Location::UNKNOWN => {
                    fmt_with_snippet_or_fallback(
                        f,
                        Level::ERROR,
                        self.msg,
                        &d,
                        self.text,
                        "defined here",
                        self.crop_radius,
                    )
                }
                (r, d) => {
                    fmt_with_snippet_or_fallback(
                        f,
                        Level::ERROR,
                        self.msg,
                        &r,
                        self.text,
                        "the value is used here",
                        self.crop_radius,
                    )?;
                    writeln!(f)?;
                    writeln!(
                        f,
                        "  | This value comes indirectly from the anchor at line {} column {}:",
                        d.line, d.column
                    )?;
                    fmt_with_snippet_or_fallback(
                        f,
                        Level::NOTE,
                        "anchor defined here",
                        &d,
                        self.text,
                        "defined here",
                        self.crop_radius,
                    )
                }
            }
        }
    }

    AliasSnippetDisplay {
        msg,
        locations,
        text,
        crop_radius,
    }
        .to_string()
}

#[cfg(feature = "garde")]
fn fmt_validation_error_with_snippets(
    f: &mut fmt::Formatter<'_>,
    report: &garde::Report,
    locations: &crate::path_map::PathMap,
    text: &str,
    crop_radius: usize,
) -> fmt::Result {
    use crate::path_map::{format_path_with_resolved_leaf, path_key_from_garde};

    let mut first = true;
    for (path, entry) in report.iter() {
        if !first {
            writeln!(f)?;
        }
        first = false;

        let path_key = path_key_from_garde(path);
        let original_leaf = path_key
            .leaf_string()
            .unwrap_or_else(|| "<root>".to_string());

        let (locs, resolved_leaf) = locations
            .search(&path_key)
            .unwrap_or((Locations::UNKNOWN, original_leaf.clone()));

        let ref_loc = locs.reference_location;
        let def_loc = locs.defined_location;

        let resolved_path = format_path_with_resolved_leaf(&path_key, &resolved_leaf);
        let base_msg = format!("validation error: {entry} for `{resolved_path}`");

        match (ref_loc, def_loc) {
            (Location::UNKNOWN, Location::UNKNOWN) => {
                write!(f, "{base_msg}")?;
            }
            (r, d) if r != Location::UNKNOWN && (d == Location::UNKNOWN || d == r) => {
                fmt_with_snippet_or_fallback(
                    f,
                    Level::ERROR,
                    &base_msg,
                    &r,
                    text,
                    "(defined)",
                    crop_radius,
                )?;
            }
            (r, d) if r == Location::UNKNOWN && d != Location::UNKNOWN => {
                fmt_with_snippet_or_fallback(
                    f,
                    Level::ERROR,
                    &base_msg,
                    &d,
                    text,
                    "(defined here)",
                    crop_radius,
                )?;
            }
            (r, d) => {
                fmt_with_snippet_or_fallback(
                    f,
                    Level::ERROR,
                    &format!("invalid here, {base_msg}"),
                    &r,
                    text,
                    "the value is used here",
                    crop_radius,
                )?;
                writeln!(f)?;
                writeln!(
                    f,
                    "  | This value comes indirectly from the anchor at line {} column {}:",
                    d.line, d.column
                )?;
                fmt_snippet_window_or_fallback(f, &d, text, "defined here", crop_radius)?;
            }
        }
    }
    Ok(())
}

#[cfg(feature = "validator")]
fn fmt_validator_error_with_snippets(
    f: &mut fmt::Formatter<'_>,
    errors: &validator::ValidationErrors,
    locations: &crate::path_map::PathMap,
    text: &str,
    crop_radius: usize,
) -> fmt::Result {
    use crate::path_map::format_path_with_resolved_leaf;

    let entries = collect_validator_entries(errors);
    let mut first = true;

    for (path, entry) in entries {
        if !first {
            writeln!(f)?;
        }
        first = false;

        let original_leaf = path.leaf_string().unwrap_or_else(|| "<root>".to_string());
        let (locs, resolved_leaf) = locations
            .search(&path)
            .unwrap_or((Locations::UNKNOWN, original_leaf.clone()));

        let resolved_path = format_path_with_resolved_leaf(&path, &resolved_leaf);
        let base_msg = format!("validation error: {entry} for `{resolved_path}`");

        match (locs.reference_location, locs.defined_location) {
            (Location::UNKNOWN, Location::UNKNOWN) => {
                write!(f, "{base_msg}")?;
            }
            (r, d) if r != Location::UNKNOWN && (d == Location::UNKNOWN || d == r) => {
                fmt_with_snippet_or_fallback(
                    f,
                    Level::ERROR,
                    &base_msg,
                    &r,
                    text,
                    "(defined)",
                    crop_radius,
                )?;
            }
            (r, d) if r == Location::UNKNOWN && d != Location::UNKNOWN => {
                fmt_with_snippet_or_fallback(
                    f,
                    Level::ERROR,
                    &base_msg,
                    &d,
                    text,
                    "(defined here)",
                    crop_radius,
                )?;
            }
            (r, d) => {
                fmt_with_snippet_or_fallback(
                    f,
                    Level::ERROR,
                    &format!("invalid here, {base_msg}"),
                    &r,
                    text,
                    "the value is used here",
                    crop_radius,
                )?;
                writeln!(f)?;
                writeln!(
                    f,
                    "  | This value comes indirectly from the anchor at line {} column {}:",
                    d.line, d.column
                )?;
                fmt_snippet_window_or_fallback(f, &d, text, "defined here", crop_radius)?;
            }
        }
    }

    Ok(())
}

#[cfg(feature = "validator")]
fn collect_validator_entries(
    errors: &validator::ValidationErrors,
) -> Vec<(crate::path_map::PathKey, String)> {
    let mut out = Vec::new();
    let root = crate::path_map::PathKey::empty();
    collect_validator_entries_inner(errors, &root, &mut out);
    out
}

#[cfg(feature = "validator")]
fn collect_validator_entries_inner(
    errors: &validator::ValidationErrors,
    path: &crate::path_map::PathKey,
    out: &mut Vec<(crate::path_map::PathKey, String)>,
) {
    use validator::ValidationErrorsKind;

    for (field, kind) in errors.errors() {
        let field_path = path.clone().join(field.as_ref());
        match kind {
            ValidationErrorsKind::Field(entries) => {
                for entry in entries {
                    out.push((field_path.clone(), entry.to_string()));
                }
            }
            ValidationErrorsKind::Struct(inner) => {
                collect_validator_entries_inner(inner, &field_path, out);
            }
            ValidationErrorsKind::List(list) => {
                for (idx, inner) in list {
                    let index_path = field_path.clone().join(*idx);
                    collect_validator_entries_inner(inner, &index_path, out);
                }
            }
        }
    }
}

/// Render only the snippet source window (no `error:` / `note:` title line).
///
/// This is used for secondary context in garde validation errors (e.g. anchors), where we want to
/// print a custom explanatory line and then show just the relevant source window.
#[cfg(any(feature = "garde", feature = "validator"))]
pub(crate) fn fmt_snippet_window_or_fallback(
    f: &mut fmt::Formatter<'_>,
    location: &Location,
    text: &str,
    msg: &str,
    crop_radius: usize,
) -> fmt::Result {
    if location == &Location::UNKNOWN {
        return Ok(());
    }

    // `Location` is 1-based and uses *character* columns (not byte offsets).
    let row = location.line as usize;
    let col = location.column as usize;

    let line_starts = line_starts(text);
    if line_starts.is_empty() {
        return Ok(());
    }

    let Some(start) = line_col_to_byte_offset_with_starts(text, &line_starts, row, col) else {
        return Ok(());
    };

    // Minimal span for caret placement.
    let end = match text.as_bytes().get(start) {
        Some(b'\n') | Some(b'\r') => start,
        _ => next_char_boundary(text, start).unwrap_or(start),
    };

    // Same vertical window policy as `fmt_with_snippet_or_fallback`.
    let total_lines = line_starts.len();
    let window_start_row = row.saturating_sub(2).max(1);
    let window_end_row = row.saturating_add(2).min(total_lines);
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

    let (window_text, local_start, _local_end) = crop_window_text(
        window_text,
        window_start_row,
        row,
        col,
        crop_radius,
        local_start,
        local_end,
    );

    let gutter_width = window_end_row.to_string().len();
    writeln!(f, "  |")?;

    let mut cur_row = window_start_row;
    for line in window_text.split_inclusive('\n') {
        let mut line = line;
        if let Some(stripped) = line.strip_suffix('\n') {
            line = stripped;
        }
        if let Some(stripped) = line.strip_suffix('\r') {
            line = stripped;
        }

        writeln!(f, "{cur_row:>gutter_width$} | {line}")?;

        if cur_row == row {
            // Compute caret column within the window line by counting chars from the start of the
            // error line to `local_start`.
            let line_byte_start = window_text[..local_start]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let caret_chars = window_text[line_byte_start..local_start].chars().count();
            if msg.is_empty() {
                writeln!(f, "  | {space:>caret_chars$}^", space = "")?;
            } else {
                writeln!(f, "  | {space:>caret_chars$}^ {msg}", space = "", msg = msg)?;
            }
        }

        cur_row += 1;
        if cur_row > window_end_row {
            break;
        }
    }

    // If the window ends with '\n', there is an implicit trailing empty line.
    // `split_inclusive('\n')` does not yield that final empty line, so print it explicitly
    // when it's inside the requested window (common for EOF / "at end" diagnostics).
    if window_end_row == total_lines && window_text.ends_with('\n') && cur_row <= window_end_row {
        writeln!(f, "{cur_row:>gutter_width$} |")?;

        if cur_row == row {
            let line_byte_start = window_text[..local_start]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let caret_chars = window_text[line_byte_start..local_start].chars().count();
            if msg.is_empty() {
                writeln!(f, "  | {space:>caret_chars$}^", space = "")?;
            } else {
                writeln!(f, "  | {space:>caret_chars$}^ {msg}", space = "", msg = msg)?;
            }
        }
    }

    writeln!(f, "  |")
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
    // Fast path: no horizontal cropping and no CRs to strip.
    if crop_radius == 0 && !window_text.as_bytes().contains(&b'\r') {
        return (window_text.to_owned(), local_start, local_end);
    }

    let do_crop = crop_radius != 0;
    let left_col = error_col.saturating_sub(crop_radius).max(1);
    let right_col = error_col.saturating_add(crop_radius);

    let mut out = String::with_capacity(window_text.len().min(4096));
    let mut old_pos = 0usize;
    let mut new_local_start = local_start;
    let mut new_local_end = local_end;
    let mut rebased = false;

    // Iterate over lines while preserving '\n' endings (and normalizing away '\r').
    let mut row = window_start_row;
    while old_pos < window_text.len() {
        let next_nl = window_text[old_pos..].find('\n').map(|i| old_pos + i);
        let (line_raw, had_nl, consumed) = match next_nl {
            Some(nl) => (&window_text[old_pos..nl], true, (nl - old_pos) + 1),
            None => (&window_text[old_pos..], false, window_text.len() - old_pos),
        };

        // Normalize CRLF: strip a trailing '\r' from the line content if present.
        let line = line_raw.strip_suffix('\r').unwrap_or(line_raw);

        let line_start_old = old_pos;
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
        if had_nl {
            out.push('\n');
        }

        if row == error_row {
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

        old_pos += consumed;
        row += 1;
        if !had_nl {
            break;
        }
    }

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

    // If the crop window starts at/after EOL for this line, keep it intact.
    // This avoids turning short context lines into just "…".
    if left_col_1 >= line_len_cols.saturating_add(1) {
        return (
            line.to_owned(),
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
