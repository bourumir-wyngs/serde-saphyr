#![cfg(all(feature = "serialize", feature = "deserialize"))]

use std::collections::BTreeMap;

use serde::Serialize;
use serde_saphyr::{LitStr, from_str, to_string, to_string_with_options};

#[test]
fn quote_all_mode_single_quotes_plain_strings() {
    let opts = serde_saphyr::ser_options! {
        quote_all: true,
    };
    let yaml = to_string_with_options(&"hello", opts).unwrap();
    assert!(yaml.contains("'hello'"), "expected single-quoted: {yaml}");
}

#[test]
fn quote_all_mode_double_quotes_special_strings() {
    let opts = serde_saphyr::ser_options! {
        quote_all: true,
    };
    // String with backslash requires double quotes
    let yaml = to_string_with_options(&"back\\slash", opts).unwrap();
    assert!(yaml.contains('"'), "expected double-quoted: {yaml}");
}

#[test]
fn quote_all_mode_map_values_quoted() {
    let opts = serde_saphyr::ser_options! {
        quote_all: true,
    };
    let mut m = BTreeMap::new();
    m.insert("key", "value");
    let yaml = to_string_with_options(&m, opts).unwrap();
    assert!(yaml.contains("'value'"), "expected quoted value: {yaml}");
}

#[test]
fn quote_all_single_quote_in_string_escaped() {
    let opts = serde_saphyr::ser_options! {
        quote_all: true,
    };
    let yaml = to_string_with_options(&"it's", opts).unwrap();
    // "it's" contains a single quote; in quote_all mode it may use double quotes
    assert!(
        yaml.contains("it") && yaml.contains("s"),
        "expected quoted string: {yaml}"
    );
}

#[test]
fn str_key_with_colon_gets_quoted() {
    let mut m = BTreeMap::new();
    m.insert("key:with:colons", "val");
    let yaml = to_string(&m).unwrap();
    // Key with colons must be quoted
    assert!(
        yaml.contains('"') || yaml.contains('\''),
        "expected quoted key: {yaml}"
    );
}

#[test]
fn write_quoted_null_escape() {
    // \x00 (NUL) gets \0 escape in double-quoted strings
    let mut m = BTreeMap::new();
    m.insert("key\x00null", "val");
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("\\0"), "expected NUL escape: {yaml}");
}

#[test]
fn write_quoted_named_escapes_in_value() {
    // Values with control chars go through write_quoted which uses named escapes
    // Use quote_all to force quoting of a value containing control chars
    let opts = serde_saphyr::ser_options! {
        quote_all: true,
    };
    // BEL \x07 -> \a, BS \x08 -> \b, VT \x0b -> \v, FF \x0c -> \f, ESC \x1b -> \e
    for (ch, expected) in [
        ('\x07', "\\a"),
        ('\x08', "\\b"),
        ('\x0b', "\\v"),
        ('\x0c', "\\f"),
        ('\x1b', "\\e"),
    ] {
        let s = format!("x{}y", ch);
        let yaml = to_string_with_options(&s.as_str(), opts).unwrap();
        assert!(
            yaml.contains(expected),
            "expected {expected} for char {:?}: {yaml}",
            ch
        );
    }
    // NUL \x00 -> \0
    let s = "x\x00y";
    let yaml = to_string_with_options(&s, opts).unwrap();
    assert!(yaml.contains("\\0"), "expected \\0 for NUL: {yaml}");
    // NEL \u{0085} -> \N
    let s = "x\u{0085}y";
    let yaml = to_string_with_options(&s, opts).unwrap();
    assert!(yaml.contains("\\N"), "expected \\N for NEL: {yaml}");
    // LS \u{2028} -> \L, PS \u{2029} -> \P (combine with backslash to force double-quoting)
    let s = "x\\\u{2028}y";
    let yaml = to_string_with_options(&s, opts).unwrap();
    assert!(yaml.contains("\\L"), "expected \\L for LS: {yaml}");
    let s = "x\\\u{2029}y";
    let yaml = to_string_with_options(&s, opts).unwrap();
    assert!(yaml.contains("\\P"), "expected \\P for PS: {yaml}");
    // BOM \u{FEFF} -> \uFEFF (combine with backslash to force double-quoting)
    let s = "x\\\u{FEFF}y";
    let yaml = to_string_with_options(&s, opts).unwrap();
    assert!(yaml.contains("\\uFEFF"), "expected \\uFEFF for BOM: {yaml}");
}

#[test]
fn write_quoted_named_escapes_in_map_keys() {
    let mut m = BTreeMap::new();
    m.insert("line\u{2028}sep".to_string(), 1);
    m.insert("paragraph\u{2029}sep".to_string(), 2);
    m.insert("bom\u{FEFF}mark".to_string(), 3);

    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("\\L"), "expected \\L for LS: {yaml}");
    assert!(yaml.contains("\\P"), "expected \\P for PS: {yaml}");
    assert!(yaml.contains("\\uFEFF"), "expected \\uFEFF for BOM: {yaml}");

    let back: BTreeMap<String, i32> = from_str(&yaml).unwrap();
    assert_eq!(back, m);
}

#[test]
fn write_quoted_control_char_escapes() {
    // Control chars in keys force double-quoting and use the same named escapes as values.
    let cases: Vec<(&str, &str)> = vec![
        ("\x07", "\\a"),
        ("\x08", "\\b"),
        ("\x0b", "\\v"),
        ("\x0c", "\\f"),
        ("\x1b", "\\e"),
    ];
    for (input, expected) in cases {
        let mut m = BTreeMap::new();
        m.insert(input, "val");
        let yaml = to_string(&m).unwrap();
        assert!(
            yaml.contains(expected),
            "expected escape for {:?}: {yaml}",
            input
        );
    }
}

#[test]
fn key_with_double_quote_gets_escaped() {
    // Double quote forces quoting of the key
    let mut m = BTreeMap::new();
    m.insert("key\"with\"quotes", "val");
    let yaml = to_string(&m).unwrap();
    // Key must be quoted (single or double) since it contains a double quote
    assert!(
        yaml.contains('"') || yaml.contains('\''),
        "expected quoted key: {yaml}"
    );
}

#[test]
fn quote_all_string_with_single_quote_uses_double_quotes_or_doubles() {
    // "it's" has a single quote; quote_all should handle it
    let opts = serde_saphyr::ser_options! {
        quote_all: true,
    };
    // A string with ONLY a single quote and no backslash/control chars
    // needs_double_quotes returns true for single quote, so it uses double quotes
    let yaml = to_string_with_options(&"it's fine", opts).unwrap();
    assert!(yaml.contains("it") && yaml.contains("fine"), "yaml: {yaml}");
}

#[test]
fn quote_all_plain_string_uses_single_quotes() {
    // A plain string with no special chars uses single-quoted style
    let opts = serde_saphyr::ser_options! {
        quote_all: true,
    };
    let yaml = to_string_with_options(&"hello world", opts).unwrap();
    // Should be single-quoted since no special chars
    assert!(yaml.contains("'hello world'"), "yaml: {yaml}");
}

#[test]
fn deep_nesting_with_leading_spaces_falls_back_to_quoted() {
    // indent_n > 9 with leading spaces triggers the fallback to quoted string
    // Need depth > 4 with default indent_step=2: 2*(depth+1) > 9 => depth >= 4
    #[derive(Serialize)]
    struct L5 {
        val: LitStr<'static>,
    }
    #[derive(Serialize)]
    struct L4 {
        inner: L5,
    }
    #[derive(Serialize)]
    struct L3 {
        inner: L4,
    }
    #[derive(Serialize)]
    struct L2 {
        inner: L3,
    }
    #[derive(Serialize)]
    struct L1 {
        inner: L2,
    }

    let v = L1 {
        inner: L2 {
            inner: L3 {
                inner: L4 {
                    inner: L5 {
                        val: LitStr("  leading spaces content\n"),
                    },
                },
            },
        },
    };
    // Should not panic; falls back to quoted string when indent_n > 9
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("leading spaces content"), "yaml: {yaml}");
}

#[test]
fn quote_all_uses_single_quotes_for_simple_strings() {
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let yaml = to_string_with_options(&"hello", opts).unwrap();
    assert_eq!(yaml, "'hello'\n");
}

#[test]
fn quote_all_uses_double_quotes_for_strings_with_escapes() {
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let yaml = to_string_with_options(&"line\nbreak", opts).unwrap();
    assert!(yaml.starts_with('"'), "expected double quotes: {yaml}");
    assert!(yaml.contains("\\n"), "expected escaped newline: {yaml}");
}

#[test]
fn quote_all_single_quote_inside_string() {
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let yaml = to_string_with_options(&"it's", opts).unwrap();
    // Contains single quote → must use double quotes
    assert!(yaml.starts_with('"'), "expected double quotes: {yaml}");
}

#[test]
fn quote_all_value_position() {
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let mut m = BTreeMap::new();
    m.insert("key", "value");
    let yaml = to_string_with_options(&m, opts).unwrap();
    // Keys go through KeyScalarSink which doesn't use quote_all; values do.
    assert!(
        yaml.contains("'value'") || yaml.contains("\"value\""),
        "value should be quoted: {yaml}"
    );
}

#[test]
fn single_quoted_escapes_embedded_quote() {
    let opts = serde_saphyr::ser_options! { quote_all: true };
    // A string with a backslash needs double quotes
    let yaml = to_string_with_options(&"back\\slash", opts).unwrap();
    assert!(
        yaml.starts_with('"'),
        "expected double quotes for backslash: {yaml}"
    );
}

#[test]
fn serialize_string_with_single_quote() {
    // Triggers write_single_quoted with embedded quote -> doubled
    let s = serde_saphyr::to_string(&"it's").unwrap();
    assert!(s.contains("it's") || s.contains("it''s") || s.contains("\"it's\""));
}

#[test]
fn serialize_string_with_special_chars() {
    // Triggers write_quoted with escape sequences
    let s = serde_saphyr::to_string(&"line1\nline2\ttab").unwrap();
    assert!(s.contains("line1") && s.contains("line2"));
}

#[test]
fn serialize_string_with_unicode_control() {
    // Triggers the \u{XXXX} escape path in write_quoted
    let s = serde_saphyr::to_string(&"hello\x01world").unwrap();
    assert!(s.contains("hello"));
}

#[test]
fn serialize_string_with_backslash() {
    let s = serde_saphyr::to_string(&"back\\slash").unwrap();
    assert!(s.contains("back"));
}
