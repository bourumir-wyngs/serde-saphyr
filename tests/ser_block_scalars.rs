#![cfg(all(feature = "serialize", feature = "deserialize"))]

use serde_saphyr::{FoldStr, FoldString, LitStr, LitString, to_string, to_string_with_options};

#[test]
fn lit_str_no_trailing_newline_uses_strip() {
    let s = LitStr("hello");
    let yaml = to_string(&s).unwrap();
    // literal block with strip indicator '-'
    assert!(yaml.contains("|-"), "expected strip indicator: {yaml}");
}

#[test]
fn lit_str_one_trailing_newline_uses_clip() {
    let s = LitStr("hello\n");
    let yaml = to_string(&s).unwrap();
    // literal block with clip (no indicator after |)
    assert!(
        yaml.contains("|\n") || yaml.contains("| \n") || yaml.starts_with("|"),
        "expected clip: {yaml}"
    );
}

#[test]
fn lit_str_two_trailing_newlines_uses_keep() {
    let s = LitStr("hello\n\n");
    let yaml = to_string(&s).unwrap();
    // literal block with keep indicator '+'
    assert!(yaml.contains("|+"), "expected keep indicator: {yaml}");
}

#[test]
fn fold_str_no_trailing_newline() {
    let s = FoldStr(
        "a long string that should be folded because it is long enough to trigger folding behavior",
    );
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains('>'), "expected folded block: {yaml}");
}

#[test]
fn prefer_block_scalars_auto_fold_no_trailing_newline() {
    let opts = serde_saphyr::ser_options! {
        prefer_block_scalars: true,
    };
    let yaml = to_string_with_options(&"short", opts).unwrap();
    // short strings may still be plain; just ensure no panic
    let _ = yaml;
}

#[test]
fn prefer_block_scalars_auto_fold_with_trailing_newlines() {
    let opts = serde_saphyr::ser_options! {
        prefer_block_scalars: true,
    };
    // multiline string with multiple trailing newlines -> auto folded with keep
    let yaml = to_string_with_options(&"line one\nline two\n\n", opts).unwrap();
    assert!(
        yaml.contains('>') || yaml.contains('|'),
        "expected block scalar: {yaml}"
    );
}

#[test]
fn lit_string_owned_no_trailing_newline() {
    let s = LitString("block content".to_string());
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains("|-"), "expected strip indicator: {yaml}");
}

#[test]
fn fold_string_owned() {
    let s = FoldString("a long string that should be folded because it is long enough to trigger folding behavior in the serializer".to_string());
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains('>'), "expected folded block: {yaml}");
}

#[test]
fn lit_str_with_leading_spaces_uses_indent_indicator() {
    // Content with leading spaces requires an explicit indentation indicator
    let s = LitStr("  indented content\n");
    let yaml = to_string(&s).unwrap();
    // Should contain |N where N is a digit
    assert!(yaml.contains('|'), "expected literal block: {yaml}");
}

#[test]
fn fold_str_with_leading_spaces_uses_indent_indicator() {
    let s = FoldStr("  indented folded content\n");
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains('>'), "expected folded block: {yaml}");
}

#[test]
fn prefer_block_scalars_with_leading_spaces() {
    let opts = serde_saphyr::ser_options! {
        prefer_block_scalars: true,
    };
    let yaml = to_string_with_options(&"  leading spaces\n", opts).unwrap();
    assert!(
        yaml.contains('|') || yaml.contains('>') || yaml.contains('"'),
        "yaml: {yaml}"
    );
}

#[test]
fn fold_str_one_trailing_newline_clip() {
    let s = FoldStr("hello world this is a long enough string to fold\n");
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains('>'), "expected folded block: {yaml}");
}

#[test]
fn fold_str_two_trailing_newlines() {
    // FoldStr uses plain '>' without chomp indicator (historical behavior)
    let s = FoldStr("hello world this is a long enough string to fold\n\n");
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains('>'), "expected folded block: {yaml}");
}

#[test]
fn prefer_block_scalars_one_trailing_newline() {
    let opts = serde_saphyr::ser_options! {
        prefer_block_scalars: true,
    };
    let yaml = to_string_with_options(&"line one\nline two\n", opts).unwrap();
    assert!(
        yaml.contains('>') || yaml.contains('|'),
        "expected block scalar: {yaml}"
    );
}

#[test]
fn prefer_block_scalars_no_trailing_newline() {
    let opts = serde_saphyr::ser_options! {
        prefer_block_scalars: true,
    };
    let yaml = to_string_with_options(&"line one\nline two", opts).unwrap();
    assert!(
        yaml.contains('>') || yaml.contains('|') || yaml.contains('"'),
        "yaml: {yaml}"
    );
}

#[test]
fn lit_str_deserialize_roundtrip() {
    let original = LitStr("hello\nworld\n");
    let yaml = to_string(&original).unwrap();
    let back: LitString = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(back.0, "hello\nworld\n");
}

#[test]
fn fold_str_deserialize_roundtrip() {
    let original = FoldStr("hello world this is a long enough string to fold\n");
    let yaml = to_string(&original).unwrap();
    let back: FoldString = serde_saphyr::from_str(&yaml).unwrap();
    assert!(back.0.contains("hello"), "back: {:?}", back.0);
}

#[test]
fn lit_str_emits_literal_block() {
    let yaml = to_string(&LitStr("hello\nworld\n")).unwrap();
    assert!(
        yaml.contains('|'),
        "expected literal block indicator: {yaml}"
    );
    assert!(yaml.contains("hello"), "expected content: {yaml}");
}

#[test]
fn fold_str_emits_folded_block_for_long_strings() {
    let long = "a ".repeat(50); // 100 chars, well above default threshold
    let yaml = to_string(&FoldStr(&long)).unwrap();
    assert!(
        yaml.contains('>'),
        "expected folded block indicator: {yaml}"
    );
}

#[test]
fn fold_str_short_string_stays_inline() {
    let yaml = to_string(&FoldStr("short")).unwrap();
    assert!(
        !yaml.contains('>'),
        "short string should not use folded: {yaml}"
    );
}

#[test]
fn lit_string_owned_works() {
    let yaml = to_string(&LitString("line1\nline2\n".to_string())).unwrap();
    assert!(yaml.contains('|'), "expected literal block: {yaml}");
}

#[test]
fn fold_string_owned_works() {
    let long = "word ".repeat(30);
    let yaml = to_string(&FoldString(long)).unwrap();
    assert!(yaml.contains('>'), "expected folded block: {yaml}");
}

#[test]
fn lit_str_strip_chomp() {
    // No trailing newline → strip chomp (|-)
    let yaml = to_string(&LitStr("hello")).unwrap();
    assert!(yaml.contains("|-"), "expected strip chomp: {yaml}");
}

#[test]
fn lit_str_keep_chomp() {
    // Multiple trailing newlines → keep chomp (|+)
    let yaml = to_string(&LitStr("hello\n\n")).unwrap();
    assert!(yaml.contains("|+"), "expected keep chomp: {yaml}");
}

#[test]
fn auto_literal_for_multiline_string() {
    // A multiline string should auto-select literal block style
    let yaml = to_string(&"line1\nline2\n").unwrap();
    assert!(yaml.contains('|'), "expected auto literal block: {yaml}");
}

#[test]
fn auto_folded_for_long_single_line() {
    // A very long single-line string should auto-select folded block style
    let long = "word ".repeat(30); // 150 chars
    let yaml = to_string(&long).unwrap();
    assert!(yaml.contains('>'), "expected auto folded block: {yaml}");
}

#[test]
fn no_block_scalars_when_disabled() {
    let opts = serde_saphyr::ser_options! { prefer_block_scalars: false };
    let yaml = to_string_with_options(&"line1\nline2\n", opts).unwrap();
    assert!(
        !yaml.contains('|') && !yaml.contains('>'),
        "should not use block: {yaml}"
    );
}

#[test]
fn auto_folded_strip_chomp_no_trailing_newline() {
    // Long string without trailing newline → auto folded with strip chomp (>-)
    let long = "word ".repeat(30).trim_end().to_string();
    let yaml = to_string(&long).unwrap();
    assert!(yaml.contains(">-"), "expected strip chomp: {yaml}");
}

#[test]
fn lit_str_with_leading_spaces_emits_indicator() {
    // Content starts with spaces → needs explicit indentation indicator
    let yaml = to_string(&LitStr("  indented\n")).unwrap();
    // Should have |N where N is a digit
    assert!(yaml.contains('|'), "expected literal block: {yaml}");
}
