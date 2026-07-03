#![cfg(all(feature = "serialize", feature = "deserialize"))]

use std::collections::BTreeMap;

use serde::Serialize;
use serde_saphyr::{to_string, to_string_with_options};

#[test]
fn empty_seq_with_empty_as_braces() {
    let opts = serde_saphyr::ser_options! {
        empty_as_braces: true,
    };
    let v: Vec<i32> = vec![];
    let yaml = to_string_with_options(&v, opts).unwrap();
    assert!(yaml.contains("[]"), "yaml: {yaml}");
}

#[test]
fn empty_map_with_empty_as_braces() {
    let opts = serde_saphyr::ser_options! {
        empty_as_braces: true,
    };
    let m: BTreeMap<String, i32> = BTreeMap::new();
    let yaml = to_string_with_options(&m, opts).unwrap();
    assert!(yaml.contains("{}"), "yaml: {yaml}");
}

#[test]
fn yaml_12_option_bool_key() {
    let opts = serde_saphyr::ser_options! {
        yaml_12: true,
    };
    let mut m = BTreeMap::new();
    m.insert(true, "yes");
    let yaml = to_string_with_options(&m, opts).unwrap();
    assert!(yaml.contains("true:"), "yaml: {yaml}");
}

#[test]
fn empty_block_seq_as_map_value_with_empty_as_braces() {
    let opts = serde_saphyr::ser_options! {
        empty_as_braces: true,
    };
    #[derive(Serialize)]
    struct S {
        items: Vec<i32>,
    }
    let yaml = to_string_with_options(&S { items: vec![] }, opts).unwrap();
    assert!(yaml.contains("[]"), "yaml: {yaml}");
}

#[test]
fn empty_block_map_as_map_value_with_empty_as_braces() {
    let opts = serde_saphyr::ser_options! {
        empty_as_braces: true,
    };
    #[derive(Serialize)]
    struct S {
        m: BTreeMap<String, i32>,
    }
    let yaml = to_string_with_options(&S { m: BTreeMap::new() }, opts).unwrap();
    assert!(yaml.contains("{}"), "yaml: {yaml}");
}

#[test]
fn yaml_12_option_emits_directive() {
    let opts = serde_saphyr::ser_options! {
        yaml_12: true,
    };
    let yaml = to_string_with_options(&42i32, opts).unwrap();
    assert!(
        yaml.contains("%YAML 1.2"),
        "expected YAML 1.2 directive: {yaml}"
    );
}

#[test]
fn empty_map_as_braces() {
    #[derive(Serialize)]
    struct S {
        m: BTreeMap<String, i32>,
    }
    let s = S { m: BTreeMap::new() };
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains("{}"), "expected empty braces: {yaml}");
}

#[test]
fn empty_seq_as_brackets() {
    #[derive(Serialize)]
    struct S {
        v: Vec<i32>,
    }
    let s = S { v: vec![] };
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains("[]"), "expected empty brackets: {yaml}");
}

#[test]
fn empty_map_without_braces() {
    #[derive(Serialize)]
    struct S {
        m: BTreeMap<String, i32>,
    }
    let s = S { m: BTreeMap::new() };
    let opts = serde_saphyr::ser_options! { empty_as_braces: false };
    let yaml = to_string_with_options(&s, opts).unwrap();
    // Without braces, empty map should not contain {}
    assert!(!yaml.contains("{}"), "should not have braces: {yaml}");
}

#[test]
fn yaml_12_emits_directive() {
    let opts = serde_saphyr::ser_options! { yaml_12: true };
    let yaml = to_string_with_options(&42, opts).unwrap();
    assert!(
        yaml.contains("%YAML 1.2"),
        "expected YAML directive: {yaml}"
    );
}
