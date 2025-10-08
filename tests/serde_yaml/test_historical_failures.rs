use serde_saphyr;
use std::collections::HashMap;
use serde::Deserialize;
use serde_json::Value;

#[test]
fn test_recursive_yaml_references_fail() {
    let yaml = "a: &anchor\n  b: *anchor";
    let res: Result<Value, _> = serde_saphyr::from_str(yaml);
    assert!(res.is_err(), "Recursive references should fail");
}

#[test]
fn test_non_string_keys_fail() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Data {
        map: HashMap<String, String>,
    }

    let yaml = "map:\n  ? [1, 2, 3]\n  : \"value\"";
    let res: Result<Data, _> = serde_saphyr::from_str(yaml);
    assert!(res.is_err(), "Non-string keys should fail");
}


#[test]
fn test_large_integer_overflow_fail() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Data {
        big: u64,
    }

    let yaml = "big: 123456789012345678901234567890";
    let res: Result<Data, _> = serde_saphyr::from_str(yaml);
    assert!(res.is_err(), "Large integer overflow should fail");
}

#[test]
fn test_circular_references_fail() {
    let yaml = "a: &anchor\n  b: &anchor2\n    c: *anchor";
    let res: Result<Value, _> = serde_saphyr::from_str(yaml);
    assert!(res.is_err(), "Circular references should fail");
}

#[test]
fn test_unexpected_type_fail() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Config {
        name: String,
        age: u32,
    }

    let yaml = "config: John";
    let res: Result<HashMap<String, Config>, _> = serde_saphyr::from_str(yaml);
    assert!(res.is_err(), "Unexpected scalar instead of struct should fail");
}

#[test]
fn test_invalid_base64_fail() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Data {
        data: Vec<u8>,
    }

    let yaml = "data: !!binary invalid-base64-data";
    let res: Result<Data, _> = serde_saphyr::from_str(yaml);
    assert!(res.is_err(), "Invalid base64 should fail");
}

