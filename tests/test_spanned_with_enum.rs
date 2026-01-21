//! Tests for `Spanned<T>` with enums, including workarounds for untagged enums.

use serde::Deserialize;
use serde_saphyr::Spanned;

// ============================================================================
// Untagged enum - demonstrates the limitation
// ============================================================================

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum PayloadPlain {
    StringVariant { message: String },
    IntVariant { count: u32 },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum PayloadWithSpanned {
    StringVariant { message: Spanned<String> },
    IntVariant { count: Spanned<u32> },
}

/// This test demonstrates that `Spanned<T>` inside untagged enum variants fails.
/// This is a known limitation due to how serde handles untagged enums.
#[test]
fn test_spanned_inside_untagged_enum_fails() {
    let yaml = "message: hello";

    // Plain version works
    let plain_result = serde_saphyr::from_str::<PayloadPlain>(yaml).unwrap();
    assert_eq!(
        plain_result,
        PayloadPlain::StringVariant {
            message: "hello".to_string()
        }
    );

    // Spanned inside untagged enum fails - this is expected
    let spanned_result = serde_saphyr::from_str::<PayloadWithSpanned>(yaml);
    assert!(
        spanned_result.is_err(),
        "Expected error when using Spanned<T> inside untagged enum"
    );

    // Verify the error message contains the helpful hint
    let err_msg = spanned_result.unwrap_err().to_string();
    assert!(
        err_msg.contains("untagged"),
        "Error message should mention untagged enums: {err_msg}"
    );
}

// ============================================================================
// Workaround 1: Wrap the entire enum in Spanned
// ============================================================================

/// This test demonstrates the workaround: wrap the entire enum with `Spanned<Payload>`
/// instead of putting `Spanned<T>` inside each variant.
#[test]
fn test_workaround_spanned_wrapping_entire_enum() {
    let yaml = "message: hello";

    // Wrap the entire enum in Spanned - this works!
    let result: Spanned<PayloadPlain> = serde_saphyr::from_str(yaml).unwrap();

    // We get span information for the entire enum
    assert_eq!(result.referenced.line(), 1);
    assert_eq!(result.referenced.column(), 1);

    // And we can access the inner value
    assert_eq!(
        result.value,
        PayloadPlain::StringVariant {
            message: "hello".to_string()
        }
    );
}

#[test]
fn test_workaround_spanned_wrapping_entire_enum_int_variant() {
    let yaml = "count: 42";

    let result: Spanned<PayloadPlain> = serde_saphyr::from_str(yaml).unwrap();

    assert_eq!(result.referenced.line(), 1);
    assert_eq!(result.value, PayloadPlain::IntVariant { count: 42 });
}

// ============================================================================
// Limitation: Internally tagged enums also don't work
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum InternallyTaggedPayload {
    StringVariant { message: Spanned<String> },
    IntVariant { count: Spanned<u32> },
}

/// This test demonstrates that internally tagged enums (`#[serde(tag = "...")]`)
/// also have the same limitation as untagged enums.
#[test]
fn test_spanned_inside_internally_tagged_enum_fails() {
    let yaml = "type: StringVariant\nmessage: hello";

    // Internally tagged enums also fail - serde buffers content for these too
    let result = serde_saphyr::from_str::<InternallyTaggedPayload>(yaml);
    assert!(
        result.is_err(),
        "Expected error when using Spanned<T> inside internally tagged enum"
    );
}

// ============================================================================
// Workaround 3: Adjacently tagged enums
// ============================================================================

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum AdjacentlyTaggedPayload {
    StringVariant { message: Spanned<String> },
    IntVariant { count: Spanned<u32> },
}

#[test]
fn test_workaround_adjacently_tagged_enum() {
    let yaml = "type: StringVariant\ndata:\n  message: hello";

    let result: AdjacentlyTaggedPayload = serde_saphyr::from_str(yaml).unwrap();

    match result {
        AdjacentlyTaggedPayload::StringVariant { message } => {
            assert_eq!(&message.value, "hello");
            assert_eq!(message.referenced.line(), 3);
        }
        _ => panic!("Expected StringVariant"),
    }
}

// ============================================================================
// Workaround 4: Externally tagged enums (serde default)
// ============================================================================

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub enum ExternallyTaggedPayload {
    StringVariant { message: Spanned<String> },
    IntVariant { count: Spanned<u32> },
}

#[test]
fn test_workaround_externally_tagged_enum() {
    let yaml = "StringVariant:\n  message: hello";

    let result: ExternallyTaggedPayload = serde_saphyr::from_str(yaml).unwrap();

    match result {
        ExternallyTaggedPayload::StringVariant { message } => {
            assert_eq!(&message.value, "hello");
            assert_eq!(message.referenced.line(), 2);
        }
        _ => panic!("Expected StringVariant"),
    }
}
