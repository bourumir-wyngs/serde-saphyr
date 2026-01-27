//! Tests for deserialization with borrowed (`&str`), `Cow<str>`, and owned (`String`) fields.
//!
//! These tests demonstrate the current behavior of serde-saphyr regarding zero-copy
//! deserialization. Currently, the library requires `DeserializeOwned`, so structures
//! with `&str` fields cannot be deserialized directly. This test file documents the
//! expected behavior and error messages.

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use std::borrow::Cow;

    // ============================================================================
    // Test structures with different string field types
    // ============================================================================

    /// Structure with borrowed string field - requires `Deserialize<'de>`, not `DeserializeOwned`
    #[derive(Debug, Deserialize, PartialEq)]
    struct BorrowedData<'a> {
        name: &'a str,
        value: i32,
    }

    /// Structure with `Cow<str>` field - works with both borrowed and owned data
    #[derive(Debug, Deserialize, PartialEq)]
    struct CowData<'a> {
        name: Cow<'a, str>,
        value: i32,
    }

    /// Structure with owned `String` field - always works with `DeserializeOwned`
    #[derive(Debug, Deserialize, PartialEq)]
    struct OwnedData {
        name: String,
        value: i32,
    }

    /// Structure with multiple field types for comprehensive testing
    #[derive(Debug, Deserialize, PartialEq)]
    struct MixedCowData<'a> {
        simple: Cow<'a, str>,
        quoted: Cow<'a, str>,
        number: i32,
    }

    /// Structure with all owned fields
    #[derive(Debug, Deserialize, PartialEq)]
    struct MixedOwnedData {
        simple: String,
        quoted: String,
        number: i32,
    }

    // ============================================================================
    // Tests for owned String fields (should always work)
    // ============================================================================

    #[test]
    fn owned_string_simple_value() {
        let yaml = "name: hello\nvalue: 42\n";
        let result: OwnedData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name, "hello");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn owned_string_quoted_value() {
        let yaml = "name: \"hello world\"\nvalue: 42\n";
        let result: OwnedData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name, "hello world");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn owned_string_with_escape_sequences() {
        let yaml = "name: \"hello\\nworld\"\nvalue: 42\n";
        let result: OwnedData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name, "hello\nworld");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn owned_string_folded_block_scalar() {
        let yaml = r#"name: >
  This is a long
  folded string
value: 42
"#;
        let result: OwnedData = serde_saphyr::from_str(yaml).unwrap();
        assert!(result.name.contains("This is a long"));
        assert_eq!(result.value, 42);
    }

    #[test]
    fn owned_string_literal_block_scalar() {
        let yaml = r#"name: |
  Line 1
  Line 2
value: 42
"#;
        let result: OwnedData = serde_saphyr::from_str(yaml).unwrap();
        assert!(result.name.contains("Line 1"));
        assert!(result.name.contains("Line 2"));
        assert_eq!(result.value, 42);
    }

    #[test]
    fn owned_string_single_quoted_with_escape() {
        let yaml = "name: 'it''s here'\nvalue: 42\n";
        let result: OwnedData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name, "it's here");
        assert_eq!(result.value, 42);
    }

    // ============================================================================
    // Tests for Cow<str> fields (should always work, may be borrowed or owned)
    // ============================================================================

    #[test]
    fn cow_string_simple_value() {
        let yaml = "name: hello\nvalue: 42\n";
        let result: CowData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name.as_ref(), "hello");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn cow_string_quoted_value() {
        let yaml = "name: \"hello world\"\nvalue: 42\n";
        let result: CowData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name.as_ref(), "hello world");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn cow_string_with_escape_sequences() {
        // Escape sequences require owned data (string transformation)
        let yaml = "name: \"hello\\nworld\"\nvalue: 42\n";
        let result: CowData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name.as_ref(), "hello\nworld");
        assert_eq!(result.value, 42);
        // Note: With current implementation, this will be Cow::Owned
        // because the string was transformed during parsing
    }

    #[test]
    fn cow_string_folded_block_scalar() {
        let yaml = r#"name: >
  This is a long
  folded string
value: 42
"#;
        let result: CowData = serde_saphyr::from_str(yaml).unwrap();
        assert!(result.name.as_ref().contains("This is a long"));
        assert_eq!(result.value, 42);
        // Note: Folded scalars require owned data due to line folding transformation
    }

    #[test]
    fn cow_string_literal_block_scalar() {
        let yaml = r#"name: |
  Line 1
  Line 2
value: 42
"#;
        let result: CowData = serde_saphyr::from_str(yaml).unwrap();
        assert!(result.name.as_ref().contains("Line 1"));
        assert_eq!(result.value, 42);
    }

    #[test]
    fn cow_string_single_quoted_with_escape() {
        // Single-quoted strings with '' escape require owned data
        let yaml = "name: 'it''s here'\nvalue: 42\n";
        let result: CowData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name.as_ref(), "it's here");
        assert_eq!(result.value, 42);
    }

    // ============================================================================
    // Tests for borrowed &str fields (now supported via unified from_str)
    // ============================================================================

    #[test]
    fn borrowed_string_with_from_str() {
        // Borrowed strings ARE now supported via the unified from_str!
        // This works for simple plain scalars that don't require transformation.
        let yaml = "name: hello\nvalue: 42\n";
        let result: BorrowedData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name, "hello");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn borrowed_string_simple_plain_scalar() {
        // Plain scalars without any transformation can be borrowed
        let yaml = "name: simple_value\nvalue: 123\n";
        let result: BorrowedData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name, "simple_value");
        assert_eq!(result.value, 123);
    }

    #[test]
    fn borrowed_string_with_spaces() {
        // Plain scalars with spaces should also work
        let yaml = "name: hello world\nvalue: 42\n";
        let result: BorrowedData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name, "hello world");
        assert_eq!(result.value, 42);
    }

    // ============================================================================
    // Tests for mixed field types
    // ============================================================================

    #[test]
    fn mixed_owned_all_simple_values() {
        let yaml = r#"
simple: hello
quoted: "world"
number: 123
"#;
        let result: MixedOwnedData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.simple, "hello");
        assert_eq!(result.quoted, "world");
        assert_eq!(result.number, 123);
    }

    #[test]
    fn mixed_cow_all_simple_values() {
        let yaml = r#"
simple: hello
quoted: "world"
number: 123
"#;
        let result: MixedCowData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.simple.as_ref(), "hello");
        assert_eq!(result.quoted.as_ref(), "world");
        assert_eq!(result.number, 123);
    }

    #[test]
    fn mixed_owned_with_transformations() {
        let yaml = r#"
simple: plain text
quoted: "with\nnewline"
number: 456
"#;
        let result: MixedOwnedData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.simple, "plain text");
        assert_eq!(result.quoted, "with\nnewline");
        assert_eq!(result.number, 456);
    }

    #[test]
    fn mixed_cow_with_transformations() {
        let yaml = r#"
simple: plain text
quoted: "with\nnewline"
number: 456
"#;
        let result: MixedCowData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.simple.as_ref(), "plain text");
        assert_eq!(result.quoted.as_ref(), "with\nnewline");
        assert_eq!(result.number, 456);
    }

    // ============================================================================
    // Tests documenting error messages for unsupported cases
    // ============================================================================

    #[test]
    fn error_message_for_str_in_deserialize_with() {
        // This test documents the error message when trying to use &str
        // in a custom deserialize_with function.
        //
        // The error message should be helpful and suggest alternatives.
        //
        // Note: This is a documentation test - the actual error occurs at
        // compile time when trying to use BorrowedData with from_str.
        
        // Verify the TransformReason error provides good guidance
        use serde_saphyr::{Error, TransformReason};
        
        let err = Error::cannot_borrow_transformed(TransformReason::EscapeSequence);
        let msg = format!("{}", err);
        
        // The error message should suggest alternatives
        assert!(msg.contains("String") || msg.contains("Cow<str>"));
        assert!(msg.contains("&str"));
    }

    // ============================================================================
    // Edge cases and special values
    // ============================================================================

    #[test]
    fn owned_empty_string() {
        let yaml = "name: \"\"\nvalue: 0\n";
        let result: OwnedData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name, "");
        assert_eq!(result.value, 0);
    }

    #[test]
    fn cow_empty_string() {
        let yaml = "name: \"\"\nvalue: 0\n";
        let result: CowData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name.as_ref(), "");
        assert_eq!(result.value, 0);
    }

    #[test]
    fn owned_unicode_string() {
        let yaml = "name: \"héllo wörld 🌍\"\nvalue: 42\n";
        let result: OwnedData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name, "héllo wörld 🌍");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn cow_unicode_string() {
        let yaml = "name: \"héllo wörld 🌍\"\nvalue: 42\n";
        let result: CowData = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.name.as_ref(), "héllo wörld 🌍");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn owned_multiline_plain_scalar() {
        // Multi-line plain scalars have whitespace normalization
        let yaml = r#"name: this is
  a multiline
  plain scalar
value: 42
"#;
        let result: OwnedData = serde_saphyr::from_str(yaml).unwrap();
        // Plain scalars fold newlines into spaces
        assert!(result.name.contains("this is"));
        assert_eq!(result.value, 42);
    }

    #[test]
    fn cow_multiline_plain_scalar() {
        let yaml = r#"name: this is
  a multiline
  plain scalar
value: 42
"#;
        let result: CowData = serde_saphyr::from_str(yaml).unwrap();
        assert!(result.name.as_ref().contains("this is"));
        assert_eq!(result.value, 42);
    }

    // ============================================================================
    // Nested structures
    // ============================================================================

    #[derive(Debug, Deserialize, PartialEq)]
    struct NestedOwned {
        outer: String,
        inner: OwnedData,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct NestedCow<'a> {
        outer: Cow<'a, str>,
        inner: CowData<'a>,
    }

    #[test]
    fn nested_owned_structure() {
        let yaml = r#"
outer: "outer value"
inner:
  name: "inner name"
  value: 99
"#;
        let result: NestedOwned = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.outer, "outer value");
        assert_eq!(result.inner.name, "inner name");
        assert_eq!(result.inner.value, 99);
    }

    #[test]
    fn nested_cow_structure() {
        let yaml = r#"
outer: "outer value"
inner:
  name: "inner name"
  value: 99
"#;
        let result: NestedCow = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.outer.as_ref(), "outer value");
        assert_eq!(result.inner.name.as_ref(), "inner name");
        assert_eq!(result.inner.value, 99);
    }

    // ============================================================================
    // Sequences with string elements
    // ============================================================================

    #[derive(Debug, Deserialize, PartialEq)]
    struct ListOwned {
        items: Vec<String>,
    }

    /// Note: ListCow with #[serde(borrow)] requires Deserialize<'de>, not DeserializeOwned.
    /// This means it cannot be used with from_str() currently.
    /// The struct is kept here to document the limitation.
    #[derive(Debug, Deserialize, PartialEq)]
    struct ListCow<'a> {
        #[serde(borrow)]
        items: Vec<Cow<'a, str>>,
    }

    #[test]
    fn list_of_owned_strings() {
        let yaml = r#"
items:
  - "first"
  - "second"
  - "third"
"#;
        let result: ListOwned = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.items, vec!["first", "second", "third"]);
    }

    #[test]
    fn list_of_cow_strings_not_supported_with_borrow() {
        // This test documents that Vec<Cow<'a, str>> with #[serde(borrow)]
        // is NOT currently supported because from_str requires DeserializeOwned.
        //
        // The following would fail to compile:
        // let yaml = "items:\n  - first\n";
        // let result: ListCow = serde_saphyr::from_str(yaml).unwrap();
        //
        // Error: implementation of `Deserialize` is not general enough
        //        `ListCow<'_>` must implement `Deserialize<'0>`, for any lifetime `'0`...
        //        ...but `ListCow<'_>` actually implements `Deserialize<'1>`, for some specific lifetime `'1`
        //
        // Workaround: Use Vec<String> instead, or don't use #[serde(borrow)]
        
        // Verify the struct can be created manually
        let data = ListCow {
            items: vec![Cow::Borrowed("first"), Cow::Owned("second".to_string())],
        };
        assert_eq!(data.items.len(), 2);
    }

    #[test]
    fn list_with_transformed_strings() {
        let yaml = r#"
items:
  - "hello\nworld"
  - 'it''s here'
  - plain text
"#;
        let result: ListOwned = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(result.items[0], "hello\nworld");
        assert_eq!(result.items[1], "it's here");
        assert_eq!(result.items[2], "plain text");
    }
}
