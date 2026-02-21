//! Tests for `TransformReason` enum and `CannotBorrowTransformedString` error.
//!
//! These tests verify the error infrastructure for zero-copy deserialization support,
//! which provides clear error messages when deserializing to `&str` fails because
//! the string was transformed during parsing.

#[cfg(test)]
mod tests {
    use serde_saphyr::{Error, TransformReason};

    #[test]
    fn transform_reason_display_escape_sequence() {
        let reason = TransformReason::EscapeSequence;
        assert_eq!(format!("{}", reason), "escape sequence processing");
    }

    #[test]
    fn transform_reason_display_line_folding() {
        let reason = TransformReason::LineFolding;
        assert_eq!(format!("{}", reason), "line folding");
    }

    #[test]
    fn transform_reason_display_multi_line_normalization() {
        let reason = TransformReason::MultiLineNormalization;
        assert_eq!(format!("{}", reason), "multi-line whitespace normalization");
    }

    #[test]
    fn transform_reason_display_block_scalar_processing() {
        let reason = TransformReason::BlockScalarProcessing;
        assert_eq!(format!("{}", reason), "block scalar processing");
    }

    #[test]
    fn transform_reason_display_single_quote_escape() {
        let reason = TransformReason::SingleQuoteEscape;
        assert_eq!(format!("{}", reason), "single-quote escape processing");
    }

    #[test]
    fn cannot_borrow_transformed_error_construction() {
        let err = Error::cannot_borrow_transformed(TransformReason::EscapeSequence);
        
        // The error should have unknown location initially
        assert!(err.location().is_none());
        
        // Check the error message contains the reason
        let msg = format!("{}", err);
        assert!(msg.contains("cannot deserialize into &str"));
        assert!(msg.contains("escape sequence processing"));
        assert!(msg.contains("String or Cow<str>"));
    }

    #[test]
    fn transform_reason_equality() {
        assert_eq!(TransformReason::EscapeSequence, TransformReason::EscapeSequence);
        assert_eq!(TransformReason::LineFolding, TransformReason::LineFolding);
        assert_ne!(TransformReason::EscapeSequence, TransformReason::LineFolding);
    }

    #[test]
    fn transform_reason_copy() {
        let reason = TransformReason::MultiLineNormalization;
        let copied = reason;
        assert_eq!(reason, copied);
    }

    #[test]
    fn transform_reason_debug() {
        let reason = TransformReason::SingleQuoteEscape;
        let debug_str = format!("{:?}", reason);
        assert!(debug_str.contains("SingleQuoteEscape"));
    }

    #[test]
    fn cannot_borrow_error_message_suggests_alternatives() {
        // Verify the error message provides actionable guidance
        let err = Error::cannot_borrow_transformed(TransformReason::EscapeSequence);
        let msg = format!("{}", err);
        
        // Should suggest using String or Cow<str>
        assert!(msg.contains("String") || msg.contains("Cow"));
        assert!(msg.contains("&str"));
    }

    #[test]
    fn all_transform_reasons_have_distinct_messages() {
        let reasons = [
            TransformReason::EscapeSequence,
            TransformReason::LineFolding,
            TransformReason::MultiLineNormalization,
            TransformReason::BlockScalarProcessing,
            TransformReason::SingleQuoteEscape,
            TransformReason::InputNotBorrowable,
        ];
        
        let messages: Vec<String> = reasons.iter().map(|r| format!("{}", r)).collect();
        
        // Verify all messages are unique
        for (i, msg1) in messages.iter().enumerate() {
            for (j, msg2) in messages.iter().enumerate() {
                if i != j {
                    assert_ne!(msg1, msg2, "Transform reasons should have distinct messages");
                }
            }
        }
    }

    #[test]
    fn transform_reason_display_input_not_borrowable() {
        let reason = TransformReason::InputNotBorrowable;
        let msg = format!("{}", reason);
        assert!(msg.contains("not available") || msg.contains("borrowing"));
    }

    #[test]
    fn cannot_borrow_input_not_borrowable_error() {
        // Verify the error for InputNotBorrowable provides helpful guidance
        let err = Error::cannot_borrow_transformed(TransformReason::InputNotBorrowable);
        let msg = format!("{}", err);
        
        assert!(msg.contains("cannot deserialize into &str"));
        assert!(msg.contains("String") || msg.contains("Cow"));
    }

    #[test]
    fn cannot_borrow_transformed_error_copy() {
        // TransformReason should be Copy
        let reason = TransformReason::LineFolding;
        let copied = reason; // Copy
        assert_eq!(reason, copied); // Original still usable
    }

    #[test]
    fn error_variant_matches_cannot_borrow() {
        let err = Error::cannot_borrow_transformed(TransformReason::BlockScalarProcessing);
        
        // Verify we can match on the error variant
        match err {
            Error::CannotBorrowTransformedString { reason, .. } => {
                assert_eq!(reason, TransformReason::BlockScalarProcessing);
            }
            _ => panic!("Expected CannotBorrowTransformedString variant"),
        }
    }
}
