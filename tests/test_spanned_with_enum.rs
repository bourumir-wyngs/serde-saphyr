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

/// This test demonstrates that `Spanned<T>` inside untagged enum variants succeeds
/// but loses location information (Location::UNKNOWN). Previously this would fail;
/// now the fallback plain-value path in `Spanned<T>::deserialize` allows it to succeed.
#[test]
fn test_spanned_inside_untagged_enum_succeeds_with_unknown_location() {
    let yaml = "message: hello";

    // Plain version works
    let plain_result = serde_saphyr::from_str::<PayloadPlain>(yaml).unwrap();
    assert_eq!(
        plain_result,
        PayloadPlain::StringVariant {
            message: "hello".to_string()
        }
    );

    // Spanned inside untagged enum now succeeds, but location info is unavailable
    let spanned_result = serde_saphyr::from_str::<PayloadWithSpanned>(yaml)
        .expect("Spanned inside untagged enum should now succeed");
    match spanned_result {
        PayloadWithSpanned::StringVariant { message } => {
            assert_eq!(message.value, "hello");
            // Location is unavailable through untagged enum buffering.
            assert_eq!(message.referenced.line(), 0);
        }
        other => panic!("Expected StringVariant, got {other:?}"),
    }
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
// Internally tagged enums: work but lose location info (Location::UNKNOWN)
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum InternallyTaggedPayload {
    StringVariant { message: Spanned<String> },
    IntVariant { count: Spanned<u32> },
}

/// This test demonstrates that internally tagged enums (`#[serde(tag = "...")]`)
/// now succeed but lose location information (Location::UNKNOWN), same as untagged enums.
#[test]
fn test_spanned_inside_internally_tagged_enum_succeeds_with_unknown_location() {
    let yaml = "type: StringVariant\nmessage: hello";

    // Internally tagged enums now succeed, but location info is unavailable
    let result = serde_saphyr::from_str::<InternallyTaggedPayload>(yaml)
        .expect("Spanned inside internally tagged enum should now succeed");
    match result {
        InternallyTaggedPayload::StringVariant { message } => {
            assert_eq!(message.value, "hello");
            // Location is unavailable through internally tagged enum buffering.
            assert_eq!(message.referenced.line(), 0);
        }
        other => panic!("Expected StringVariant, got {other:?}"),
    }
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
