//! Tests that custom deserializer errors include YAML location information.

use serde::{de::Error, Deserialize};

/// A type whose deserialization always fails with a custom error.
#[derive(Debug)]
struct AlwaysFails;

impl<'de> Deserialize<'de> for AlwaysFails {
    fn deserialize<D>(_: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Err(Error::custom("oh noes"))
    }
}

#[derive(Debug, Deserialize)]
struct Outer {
    #[allow(dead_code)]
    value: AlwaysFails,
}

#[derive(Debug, Deserialize)]
struct OuterSeq {
    #[allow(dead_code)]
    items: Vec<AlwaysFails>,
}

/// Custom deserializer errors in struct fields should include YAML location.
#[test]
fn custom_deserializer_error_in_struct_field_has_location() {
    let yaml = r#"
value: "doesn't matter"
"#;
    let err = serde_saphyr::from_str::<Outer>(yaml).unwrap_err();

    let location = err.location().expect("error should have location");
    assert_eq!(location.line(), 2, "error should point to line 2");
    // Column points to the start of the value (column 1 is the key start)
    assert!(location.column() >= 1, "error should have a valid column");

    // Verify the error message contains the custom message
    let msg = err.to_string();
    assert!(msg.contains("oh noes"), "error message should contain 'oh noes'");
}

/// Custom deserializer errors in sequence elements should include YAML location.
#[test]
fn custom_deserializer_error_in_seq_element_has_location() {
    let yaml = r#"
items:
  - first
  - second
"#;
    let err = serde_saphyr::from_str::<OuterSeq>(yaml).unwrap_err();

    let location = err.location().expect("error should have location");
    // The error should point to the first sequence element
    assert_eq!(location.line(), 3, "error should point to line 3 (first element)");

    let msg = err.to_string();
    assert!(msg.contains("oh noes"), "error message should contain 'oh noes'");
}

/// Custom deserializer errors on aliased values should report both locations.
#[test]
fn custom_deserializer_error_on_alias_has_both_locations() {
    let yaml = r#"
anchor: &a "will fail"
value: *a
"#;
    let err = serde_saphyr::from_str::<Outer>(yaml).unwrap_err();

    // The error should have location information
    let location = err.location().expect("error should have location");
    // Primary location should be the alias use-site (line 3)
    assert_eq!(location.line(), 3, "primary location should point to alias use-site (line 3)");

    // Check that we have both locations via the locations() method
    let locations = err.locations().expect("error should have locations");
    
    // Reference location is where the alias is used (*a on line 3)
    assert_eq!(locations.reference_location.line(), 3, "reference location should be line 3");
    
    // Defined location is where the anchor is defined (&a on line 2)
    assert_eq!(locations.defined_location.line(), 2, "defined location should be line 2");

    // The error message should contain the custom message
    let msg = err.to_string();
    assert!(msg.contains("oh noes"), "error message should contain 'oh noes'");
    
    // The display should mention both locations
    assert!(msg.contains("line 3"), "error should mention use-site line");
    assert!(msg.contains("line 2"), "error should mention definition-site line");
}

/// Custom deserializer errors in newtype enum variants should include YAML location.
#[test]
fn custom_deserializer_error_in_newtype_variant_has_location() {
    #[derive(Debug, Deserialize)]
    enum MyEnum {
        Wrapper(AlwaysFails),
    }

    let yaml = r#"
Wrapper: "will fail"
"#;
    let err = serde_saphyr::from_str::<MyEnum>(yaml).unwrap_err();

    let location = err.location().expect("error should have location");
    // The error should point to the value (line 2)
    assert_eq!(location.line(), 2, "error should point to line 2");

    let msg = err.to_string();
    assert!(msg.contains("oh noes"), "error message should contain 'oh noes'");
}

/// A key type whose deserialization always fails with a custom error.
#[derive(Debug, PartialEq, Eq, Hash)]
struct AlwaysFailsKey;

impl<'de> Deserialize<'de> for AlwaysFailsKey {
    fn deserialize<D>(_: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Err(Error::custom("key error"))
    }
}

/// Custom deserializer errors in map keys should include YAML location.
#[test]
fn custom_deserializer_error_in_map_key_has_location() {
    use std::collections::HashMap;

    let yaml = r#"
"key1": value1
"key2": value2
"#;
    // Try to deserialize into a map with AlwaysFailsKey as key type
    let err = serde_saphyr::from_str::<HashMap<AlwaysFailsKey, String>>(yaml).unwrap_err();

    let location = err.location().expect("error should have location");
    // The error should point to the first key (line 2)
    assert_eq!(location.line(), 2, "error should point to line 2");

    let msg = err.to_string();
    assert!(msg.contains("key error"), "error message should contain 'key error'");
}