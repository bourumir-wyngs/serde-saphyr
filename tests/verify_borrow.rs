//! Tests for `Cow<'a, str>` borrowing behavior.
//!
//! This file contains assertions that verify whether `std::borrow::Cow<'a, str>`
//! correctly utilizes zero-copy deserialization when used with `serde-saphyr`.
//!
//! # Context
//!
//! `serde-saphyr` implements `deserialize_string` (used by `Cow<str>`) to attempt
//! borrowing from the input whenever possible. If the string exists verbatim in the
//! input (e.g., plain scalars, quoted strings without escapes), it calls
//! `visitor.visit_borrowed_str`.
//!
//! Ideally, `Cow::deserialize` should accept this borrowed string and return
//! `Cow::Borrowed`. However, due to complex interactions with Serde's default
//! `Cow` visitor or lifetime bounds, `Cow` often defaults to `Cow::Owned` even
//! when `visit_borrowed_str` is offered.
//!
//! # Status
//!
//! These tests are currently marked `#[ignore]` because `Cow` integration is not
//! yet fully working as expected, even though the underlying deserializer logic
//! is correct (proven by `tests/borrowing_improvement.rs`).
//!
//! These tests serve as a target: they *should* pass when the `Cow` integration
//! issues are resolved.

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_saphyr::from_str;
    use std::borrow::Cow;

    /// A struct wrapper around `Cow<'a, str>` to test field deserialization.
    #[derive(Debug, Deserialize)]
    struct CowStruct<'a> {
        /// This field should be `Cow::Borrowed` when the input allows it.
        ///
        /// Note: The lifetime `'a` is tied to the input string of `from_str`.
        #[serde(borrow)]
        s: Cow<'a, str>,
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
    #[ignore = "FIXME: Standard Cow currently defaults to Owned despite visit_borrowed_str being called"]
    fn test_cow_borrowing_simple() {
        let input = "s: hello\n";
        let cow: CowStruct = from_str(input).unwrap();

        match &cow.s {
            Cow::Borrowed(b) => {
                // Ensure it really points to the input string
                assert_eq!(*b, "hello");
            }
            Cow::Owned(s) => {
                panic!("Expected Cow::Borrowed for simple scalar 'hello', but got Cow::Owned('{}')", s);
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
    #[ignore = "FIXME: Standard Cow currently defaults to Owned despite visit_borrowed_str being called"]
    fn test_cow_borrowing_quoted_no_escape() {
        let input = "s: \"hello world\"\n";
        let cow: CowStruct = from_str(input).unwrap();

        match &cow.s {
            Cow::Borrowed(b) => {
                assert_eq!(*b, "hello world");
            }
            Cow::Owned(s) => {
                panic!("Expected Cow::Borrowed for quoted string \"hello world\", but got Cow::Owned('{}')", s);
            }
        }
    }

    /// Verifies that a top-level `Cow<'a, str>` deserializes into `Cow::Borrowed`.
    ///
    /// # Input
    /// `hello`
    ///
    /// # Expected Behavior
    /// Similar to struct fields, a top-level `Cow` should borrow when possible.
    #[test]
    #[ignore = "FIXME: Standard Cow currently defaults to Owned despite visit_borrowed_str being called"]
    fn test_cow_borrowing_direct() {
        let input = "hello";
        let cow: Cow<str> = from_str(input).unwrap();
        
        if let Cow::Owned(s) = cow {
             panic!("Expected Cow::Borrowed for direct scalar, but got Cow::Owned('{}')", s);
        }
    }
}
