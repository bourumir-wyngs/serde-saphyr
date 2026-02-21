//! Tests for deserialization borrowing behavior across `&str`, `Cow<str>`, and `String`.
//!
//! This file consolidates borrowing verification, deserializer behavior checks, and
//! regression tests for edge cases (aliases, reader input, quoting, escapes).
//!
//! # Cow limitation
//!
//! `serde-saphyr` offers borrowed strings via `visit_borrowed_str` when possible,
//! but Serde's core `Deserialize` impl for `Cow<'a, T>` always deserializes
//! `T::Owned` and wraps it in `Cow::Owned`, so a direct `Cow<str>` cannot borrow
//! even when `visit_borrowed_str` is offered.

#[cfg(test)]
mod tests {
    use serde::de::{self, Deserializer, Visitor};
    use serde::Deserialize;
    use serde_saphyr::{from_str, Error};
    use std::borrow::Cow;
    use std::fmt;

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
    // Borrowing verification helpers (custom visitor)
    // ============================================================================

    /// A wrapper around `Cow<'a, str>` that implements `Deserialize` via `deserialize_string`.
    /// It allows verifying which visitor method was called.
    #[derive(Debug)]
    struct VerifiableCow<'a>(Cow<'a, str>);

    impl<'de: 'a, 'a> Deserialize<'de> for VerifiableCow<'a> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct CowVisitor;

            impl<'de> Visitor<'de> for CowVisitor {
                type Value = VerifiableCow<'de>;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("a string")
                }

                fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    // Success! The deserializer offered a borrowed string.
                    Ok(VerifiableCow(Cow::Borrowed(v)))
                }

                fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    // Fallback to owned.
                    Ok(VerifiableCow(Cow::Owned(v)))
                }

                fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    Ok(VerifiableCow(Cow::Owned(v.to_owned())))
                }
            }

            // Crucially, we request `deserialize_string`.
            deserializer.deserialize_string(CowVisitor)
        }
    }

    // ============================================================================
    // Cow<str> field borrowing checks
    // ============================================================================

    /// A struct wrapper around `Cow<'a, str>` to test field deserialization.
    #[derive(Debug, Deserialize)]
    struct CowStruct<'a> {
        /// This field should be `Cow::Borrowed` when the input allows it.
        ///
        /// Note: The lifetime `'a` is tied to the input string of `from_str`.
        #[serde(borrow)]
        s: Cow<'a, str>,
    }

    /// A wrapper to test borrowing when a struct is referenced via an anchor.
    #[derive(Debug, Deserialize)]
    struct AnchoredCowStruct<'a> {
        #[serde(borrow)]
        original: CowStruct<'a>,
        #[serde(borrow)]
        alias: CowStruct<'a>,
    }

    /// Verifies that a simple plain scalar (no quotes, no special chars)
    /// deserializes into a `Cow::Borrowed`.
    ///
    /// # Input
    /// `s: hello`
    ///
    /// # Expected Behavior
    /// Since "hello" exists verbatim in the input string, `deserialize_string`
    /// calls `visit_borrowed_str`. The `Cow` should store a reference to the
    /// input slice.
    #[test]
    fn test_cow_borrowing_simple() {
        let input = "s: hello\n";
        let cow: CowStruct = from_str(input).unwrap();

        match &cow.s {
            Cow::Borrowed(b) => {
                // Ensure it really points to the input string
                assert_eq!(*b, "hello");
            }
            Cow::Owned(s) => {
                panic!(
                    "Expected Cow::Borrowed for simple scalar 'hello', but got Cow::Owned('{}')",
                    s
                );
            }
        }
    }

    /// Verifies that a quoted string without escape sequences deserializes
    /// into a `Cow::Borrowed`.
    ///
    /// # Input
    /// `s: "hello world"`
    ///
    /// # Expected Behavior
    /// "hello world" exists verbatim in the input (excluding the surrounding quotes,
    /// which the parser handles by identifying the inner slice). `serde-saphyr`
    /// detects this and calls `visit_borrowed_str` with the inner slice.
    #[test]
    fn test_cow_borrowing_quoted_no_escape() {
        let input = "s: \"hello world\"\n";
        let cow: CowStruct = from_str(input).unwrap();

        match &cow.s {
            Cow::Borrowed(b) => {
                assert_eq!(*b, "hello world");
            }
            Cow::Owned(s) => {
                panic!(
                    "Expected Cow::Borrowed for quoted string \"hello world\", but got Cow::Owned('{}')",
                    s
                );
            }
        }
    }

    /// Verifies that a struct value referenced via an anchor still borrows
    /// the inner string when possible.
    ///
    /// # Input
    ///
    /// ```yaml
    /// original: &item
    ///   s: hello
    /// alias: *item
    /// ```
    #[test]
    fn test_cow_borrowing_anchor_struct() {
        let input = "original: &item\n  s: hello\nalias: *item\n";
        let cow: AnchoredCowStruct = from_str(input).unwrap();

        match &cow.original.s {
            Cow::Borrowed(b) => {
                assert_eq!(*b, "hello");
            }
            Cow::Owned(s) => {
                panic!(
                    "Expected Cow::Borrowed for anchored struct 'hello', but got Cow::Owned('{}')",
                    s
                );
            }
        }

        match &cow.alias.s {
            Cow::Borrowed(b) => {
                assert_eq!(*b, "hello");
            }
            Cow::Owned(s) => {
                panic!(
                    "Expected Cow::Borrowed for alias struct 'hello', but got Cow::Owned('{}')",
                    s
                );
            }
        }
    }

    /// Verifies that a top-level `&'a str` deserializes into a borrowed slice.
    ///
    /// # Input
    /// `hello`
    ///
    /// # Expected Behavior
    /// Since the scalar exists verbatim in the input, `deserialize_str` should
    /// call `visit_borrowed_str` and return a borrowed `&str`.
    #[test]
    fn test_str_borrowing_direct() {
        let input = "hello";
        let value: &str = from_str(input).unwrap();

        assert_eq!(value, "hello");
    }

    // ============================================================================
    // Tests for owned String fields (should always work)
    // ============================================================================

    #[test]
    fn owned_string_simple_value() {
        let yaml = "name: hello\nvalue: 42\n";
        let result: OwnedData = from_str(yaml).unwrap();
        assert_eq!(result.name, "hello");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn owned_string_quoted_value() {
        let yaml = "name: \"hello world\"\nvalue: 42\n";
        let result: OwnedData = from_str(yaml).unwrap();
        assert_eq!(result.name, "hello world");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn owned_string_with_escape_sequences() {
        let yaml = "name: \"hello\\nworld\"\nvalue: 42\n";
        let result: OwnedData = from_str(yaml).unwrap();
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
        let result: OwnedData = from_str(yaml).unwrap();
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
        let result: OwnedData = from_str(yaml).unwrap();
        assert!(result.name.contains("Line 1"));
        assert!(result.name.contains("Line 2"));
        assert_eq!(result.value, 42);
    }

    #[test]
    fn owned_string_single_quoted_with_escape() {
        let yaml = "name: 'it''s here'\nvalue: 42\n";
        let result: OwnedData = from_str(yaml).unwrap();
        assert_eq!(result.name, "it's here");
        assert_eq!(result.value, 42);
    }

    // ============================================================================
    // Tests for Cow<str> fields (should always work, may be borrowed or owned)
    // ============================================================================

    #[test]
    fn cow_string_simple_value() {
        let yaml = "name: hello\nvalue: 42\n";
        let result: CowData = from_str(yaml).unwrap();
        assert_eq!(result.name.as_ref(), "hello");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn cow_string_quoted_value() {
        let yaml = "name: \"hello world\"\nvalue: 42\n";
        let result: CowData = from_str(yaml).unwrap();
        assert_eq!(result.name.as_ref(), "hello world");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn cow_string_with_escape_sequences() {
        // Escape sequences require owned data (string transformation)
        let yaml = "name: \"hello\\nworld\"\nvalue: 42\n";
        let result: CowData = from_str(yaml).unwrap();
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
        let result: CowData = from_str(yaml).unwrap();
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
        let result: CowData = from_str(yaml).unwrap();
        assert!(result.name.as_ref().contains("Line 1"));
        assert_eq!(result.value, 42);
    }

    #[test]
    fn cow_string_single_quoted_with_escape() {
        // Single-quoted strings with '' escape require owned data
        let yaml = "name: 'it''s here'\nvalue: 42\n";
        let result: CowData = from_str(yaml).unwrap();
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
        let result: BorrowedData = from_str(yaml).unwrap();
        assert_eq!(result.name, "hello");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn borrowed_string_simple_plain_scalar() {
        // Plain scalars without any transformation can be borrowed
        let yaml = "name: simple_value\nvalue: 123\n";
        let result: BorrowedData = from_str(yaml).unwrap();
        assert_eq!(result.name, "simple_value");
        assert_eq!(result.value, 123);
    }

    #[test]
    fn borrowed_string_with_spaces() {
        // Plain scalars with spaces should also work
        let yaml = "name: hello world\nvalue: 42\n";
        let result: BorrowedData = from_str(yaml).unwrap();
        assert_eq!(result.name, "hello world");
        assert_eq!(result.value, 42);
    }

    fn assert_transform_error(err: Error) {
        match err.without_snippet() {
            Error::CannotBorrowTransformedString { .. } => {
                // fine
            }
            _ => unreachable!("Expected CannotBorrowTransformedString, got {:?}", err),
        };
    }

    #[test]
    fn borrowed_string_rejects_escape_sequences() {
        // Double-quoted strings with escape processing cannot be borrowed into `&str`.
        let yaml = "name: \"hello\\nworld\"\nvalue: 42\n";
        let err = from_str::<BorrowedData>(yaml).unwrap_err();
        assert_transform_error(err);
    }

    #[test]
    fn borrowed_string_rejects_single_quote_escape_even_if_value_appears_nearby() {
        // Regression: previously, borrowing used a heuristic substring search near the reported
        // span, which could accidentally "borrow" the wrong occurrence of the final value.
        // Here, the parsed value is "it's" (after processing `''`), but the input contains an
        // unrelated "it's" nearby.
        let yaml = "other: it's\nname: 'it''s'\nvalue: 42\n";
        let err = from_str::<BorrowedData>(yaml).unwrap_err();
        assert_transform_error(err);
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
        let result: MixedOwnedData = from_str(yaml).unwrap();
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
        let result: MixedCowData = from_str(yaml).unwrap();
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
        let result: MixedOwnedData = from_str(yaml).unwrap();
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
        let result: MixedCowData = from_str(yaml).unwrap();
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
        let result: OwnedData = from_str(yaml).unwrap();
        assert_eq!(result.name, "");
        assert_eq!(result.value, 0);
    }

    #[test]
    fn cow_empty_string() {
        let yaml = "name: \"\"\nvalue: 0\n";
        let result: CowData = from_str(yaml).unwrap();
        assert_eq!(result.name.as_ref(), "");
        assert_eq!(result.value, 0);
    }

    #[test]
    fn owned_unicode_string() {
        let yaml = "name: \"héllo wörld 🌍\"\nvalue: 42\n";
        let result: OwnedData = from_str(yaml).unwrap();
        assert_eq!(result.name, "héllo wörld 🌍");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn cow_unicode_string() {
        let yaml = "name: \"héllo wörld 🌍\"\nvalue: 42\n";
        let result: CowData = from_str(yaml).unwrap();
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
        let result: OwnedData = from_str(yaml).unwrap();
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
        let result: CowData = from_str(yaml).unwrap();
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
        let result: NestedOwned = from_str(yaml).unwrap();
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
        let result: NestedCow = from_str(yaml).unwrap();
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
        let result: ListOwned = from_str(yaml).unwrap();
        assert_eq!(result.items, vec!["first", "second", "third"]);
    }

    #[test]
    fn list_of_cow_strings_not_supported_with_borrow() {
        // This test documents that Vec<Cow<'a, str>> with #[serde(borrow)]
        // is NOT currently supported because from_str requires DeserializeOwned.
        //
        // The following would fail to compile:
        // let yaml = "items:\n  - first\n";
        // let result: ListCow = from_str(yaml).unwrap();
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
        let result: ListOwned = from_str(yaml).unwrap();
        assert_eq!(result.items[0], "hello\nworld");
        assert_eq!(result.items[1], "it's here");
        assert_eq!(result.items[2], "plain text");
    }

    // ============================================================================
    // Tests for reader-based input (cannot borrow)
    // ============================================================================

    #[test]
    fn reader_based_input_cannot_borrow_str_compile_time_check() {
        // Reader-based input cannot support zero-copy borrowing because
        // the input is consumed incrementally and not available as a single slice.
        //
        // This is enforced at COMPILE TIME by the `from_reader` API which requires
        // `DeserializeOwned` (i.e., `for<'de> Deserialize<'de>`).
        //
        // The following code would NOT compile:
        // ```
        // use std::io::Cursor;
        // let yaml = b"name: hello\nvalue: 42\n";
        // let reader = Cursor::new(yaml);
        // let result: Result<BorrowedData, _> = serde_saphyr::from_reader(reader);
        // ```
        //
        // Error: implementation of `Deserialize` is not general enough
        //        `BorrowedData<'_>` must implement `Deserialize<'0>`, for any lifetime `'0`...
        //        ...but `BorrowedData<'_>` actually implements `Deserialize<'1>`, for some specific lifetime `'1`
        //
        // This is the correct behavior - the API prevents misuse at compile time.

        // Verify the TransformReason::InputNotBorrowable exists and has a good message
        use serde_saphyr::TransformReason;
        let reason = TransformReason::InputNotBorrowable;
        let msg = format!("{}", reason);
        assert!(msg.contains("not available") || msg.contains("borrowing"));
    }

    #[test]
    fn reader_based_input_works_with_owned_string() {
        // Reader-based input should work fine with owned String fields
        use std::io::Cursor;

        let yaml = b"name: hello\nvalue: 42\n";
        let reader = Cursor::new(yaml);

        let result: OwnedData = serde_saphyr::from_reader(reader).unwrap();
        assert_eq!(result.name, "hello");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn reader_based_input_works_with_cow_string() {
        // Reader-based input should work with Cow<str> (will be Owned variant)
        use std::io::Cursor;

        let yaml = b"name: hello\nvalue: 42\n";
        let reader = Cursor::new(yaml);

        let result: CowData = serde_saphyr::from_reader(reader).unwrap();
        assert_eq!(result.name.as_ref(), "hello");
        assert_eq!(result.value, 42);
        // Note: With reader-based input, Cow will always be Owned
    }

    // ============================================================================
    // Tests for aliases (replay events cannot borrow)
    // ============================================================================

    #[test]
    fn alias_works_with_owned_string() {
        // Aliases replay recorded events, which should work with owned String
        let yaml = r#"
anchor: &name "shared value"
first: *name
second: *name
"#;

        #[derive(Debug, Deserialize)]
        struct AliasOwned {
            anchor: String,
            first: String,
            second: String,
        }

        let result: AliasOwned = from_str(yaml).unwrap();
        assert_eq!(result.anchor, "shared value");
        assert_eq!(result.first, "shared value");
        assert_eq!(result.second, "shared value");
    }

    #[test]
    fn alias_works_with_cow_string() {
        // Aliases should work with Cow<str> (aliased values will be Owned)
        let yaml = r#"
anchor: &name "shared value"
first: *name
second: *name
"#;

        #[derive(Debug, Deserialize)]
        struct AliasCow<'a> {
            anchor: Cow<'a, str>,
            first: Cow<'a, str>,
            second: Cow<'a, str>,
        }

        let result: AliasCow = from_str(yaml).unwrap();
        assert_eq!(result.anchor.as_ref(), "shared value");
        assert_eq!(result.first.as_ref(), "shared value");
        assert_eq!(result.second.as_ref(), "shared value");
    }

    // ============================================================================
    // Borrowing improvement tests via deserialize_string
    // ============================================================================

    #[test]
    fn test_deserialize_string_borrows_simple_scalar() {
        let input = "hello_world";
        let cow: VerifiableCow = from_str(input).unwrap();

        if let Cow::Owned(_) = cow.0 {
            panic!("deserialize_string failed to borrow a simple scalar!");
        }
        assert_eq!(cow.0, "hello_world");
    }

    #[test]
    fn test_deserialize_string_borrows_quoted_scalar() {
        // Quoted strings can be borrowed if they have no escapes
        let input = "\"hello world\"";
        let cow: VerifiableCow = from_str(input).unwrap();

        if let Cow::Owned(_) = cow.0 {
            panic!("deserialize_string failed to borrow a simple quoted scalar!");
        }
        assert_eq!(cow.0, "hello world");
    }

    #[test]
    fn test_deserialize_string_falls_back_to_owned_for_escapes() {
        // Escapes require transformation, so must be owned
        let input = "\"hello\\nworld\"";
        let cow: VerifiableCow = from_str(input).unwrap();

        match cow.0 {
            Cow::Owned(_) => {}
            Cow::Borrowed(_) => panic!("Should not borrow string with escapes!"),
        }
        assert_eq!(cow.0, "hello\nworld");
    }

    // Unicode-specific edge-case tests for zero-copy deserialization.

    #[cfg(test)]
    mod unicode_tests {
        use serde::Deserialize;

        #[derive(Debug, Deserialize, PartialEq)]
        struct Data<'a> {
            text: &'a str,
        }

        #[test]
        fn borrow_unicode_ascii_mix() {
            let yaml = "text: \u{1F980} and friends\n";
            let result: Data = serde_saphyr::from_str(yaml).unwrap();
            assert_eq!(result.text, "\u{1F980} and friends");
        }

        #[test]
        fn borrow_unicode_at_boundaries() {
            let yaml = "text: \"\u{1F980}\"\n"; // Double quoted, now should borrow
            let result: Data = serde_saphyr::from_str(yaml).unwrap();
            assert_eq!(result.text, "\u{1F980}");

            let yaml_plain = "text: \u{1F980}\n";
            let result_plain: Data = serde_saphyr::from_str(yaml_plain).unwrap();
            assert_eq!(result_plain.text, "\u{1F980}");
        }

        #[test]
        fn borrow_multiple_unicode() {
            let yaml = "text: \u{1F980}\u{1F525}\u{1F680}\n";
            let result: Data = serde_saphyr::from_str(yaml).unwrap();
            assert_eq!(result.text, "\u{1F980}\u{1F525}\u{1F680}");
        }

        #[test]
        fn borrow_unicode_with_spaces() {
            let yaml = "text:  \u{1F980}  \n";
            let result: Data = serde_saphyr::from_str(yaml).unwrap();
            assert_eq!(result.text, "\u{1F980}");
        }
    }
}