use serde::Serialize;

#[test]
fn test_quote_all_simple_strings() {
    // Simple strings should use single quotes when quote_all is enabled
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let result = serde_saphyr::to_string_with_options(&"hello", opts).unwrap();
    assert_eq!(result, "'hello'\n");
}

#[test]
fn test_quote_all_string_with_single_quote() {
    // Strings containing single quotes should use double quotes
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let result = serde_saphyr::to_string_with_options(&"it's", opts).unwrap();
    assert_eq!(result, "\"it's\"\n");
}

#[test]
fn test_quote_all_string_with_newline() {
    // Strings containing newlines (control chars) should use double quotes with escapes
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let result = serde_saphyr::to_string_with_options(&"line1\nline2", opts).unwrap();
    assert_eq!(result, "\"line1\\nline2\"\n");
}

#[test]
fn test_quote_all_string_with_tab() {
    // Strings containing tabs (control chars) should use double quotes with escapes
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let result = serde_saphyr::to_string_with_options(&"col1\tcol2", opts).unwrap();
    assert_eq!(result, "\"col1\\tcol2\"\n");
}

#[test]
fn test_quote_all_string_with_backslash() {
    // Strings containing backslashes should use double quotes with escapes
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let result = serde_saphyr::to_string_with_options(&"path\\to\\file", opts).unwrap();
    assert_eq!(result, "\"path\\\\to\\\\file\"\n");
}

#[test]
fn test_quote_all_boolean_like_string() {
    // Boolean-like strings should use single quotes (no escape sequences needed)
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let result = serde_saphyr::to_string_with_options(&"true", opts).unwrap();
    assert_eq!(result, "'true'\n");
}

#[test]
fn test_quote_all_number_like_string() {
    // Number-like strings should use single quotes (no escape sequences needed)
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let result = serde_saphyr::to_string_with_options(&"123", opts).unwrap();
    assert_eq!(result, "'123'\n");
}

#[test]
fn test_quote_all_struct() {
    #[derive(Serialize)]
    struct Person {
        name: String,
        city: String,
    }

    let person = Person {
        name: "Alice".to_string(),
        city: "New York".to_string(),
    };

    let opts = serde_saphyr::ser_options! { quote_all: true };
    let result = serde_saphyr::to_string_with_options(&person, opts).unwrap();
    // Keys remain plain (struct field names), values are quoted
    assert_eq!(result, "name: 'Alice'\ncity: 'New York'\n");
}

#[test]
fn test_quote_all_disabled_by_default() {
    // With default options, simple strings should be plain (no quotes)
    let result = serde_saphyr::to_string(&"hello").unwrap();
    assert_eq!(result, "hello\n");
}

#[test]
fn test_quote_all_no_block_scalars() {
    // When quote_all is enabled, block scalars should NOT be used even for multiline strings
    let opts = serde_saphyr::ser_options! {
        quote_all: true,
        prefer_block_scalars: true, // This should be ignored when quote_all is true
    };
    let result = serde_saphyr::to_string_with_options(&"line1\nline2\n", opts).unwrap();
    // Should use double quotes with \n escapes, not block scalar style
    assert_eq!(result, "\"line1\\nline2\\n\"\n");
}

#[test]
fn test_quote_all_long_string_no_folding() {
    // When quote_all is enabled, long strings should NOT use folded block style
    let long_string = "This is a very long string that would normally be wrapped using folded block scalar style but should remain as a single quoted line when quote_all is enabled";
    
    let opts = serde_saphyr::ser_options! {
        quote_all: true,
        prefer_block_scalars: true,
        folded_wrap_chars: 80,
    };
    let result = serde_saphyr::to_string_with_options(&long_string, opts).unwrap();
    // Should be single-quoted, not folded block style
    assert!(result.starts_with("'"));
    assert!(result.ends_with("'\n"));
    assert!(!result.contains(">"));
}

#[test]
fn test_quote_all_roundtrip() {
    // Verify that quoted strings can be parsed back correctly
    let original = "hello world";
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let yaml = serde_saphyr::to_string_with_options(&original, opts).unwrap();
    let parsed: String = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(parsed, original);
}

#[test]
fn test_quote_all_roundtrip_with_special_chars() {
    // Verify roundtrip with strings that need double quotes
    let original = "it's a test\nwith newline";
    let opts = serde_saphyr::ser_options! { quote_all: true };
    let yaml = serde_saphyr::to_string_with_options(&original, opts).unwrap();
    let parsed: String = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(parsed, original);
}
