use serde_json::Value;

// T4YY: Single quoted lines with a real trailing space and a blank line between lines.
// Replace the visible space glyph from the suite with an actual space character where intended.

#[test]
fn y_t4yy_single_quoted_lines_suite_glyphs() {
    let y = "---\n' 1st non-empty\n\n  2nd non-empty \n  3rd non-empty '\n";
    let r: Result<Value, _> = serde_saphyr::from_str(y);
    assert!(r.is_ok(), "Parser failed to handle single-quoted multi-line scalar with trailing space and blank line: {:?}", r);
}

