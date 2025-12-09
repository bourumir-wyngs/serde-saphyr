use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_saphyr::SerializerOptions;

#[test]
fn empty_top_level_map_braces_by_default() {
    let map: HashMap<String, String> = HashMap::new();
    let s = serde_saphyr::to_string(&map).unwrap();
    assert_eq!(s, "{}\n");
}

#[derive(Serialize)]
struct WrapMap {
    m: BTreeMap<String, String>,
}

#[test]
fn empty_map_value_braces_by_default() {
    let v = WrapMap { m: BTreeMap::new() };
    let s = serde_saphyr::to_string(&v).unwrap();
    assert!(s.contains("m: {}\n") || s.ends_with("m: {}\n"), "yaml: {}", s);
}

#[test]
fn struct_with_empty_map_renders_exact() {
    let v = WrapMap { m: BTreeMap::new() };
    let s = serde_saphyr::to_string(&v).unwrap();
    assert_eq!(s, "m: {}\n");

    let options = SerializerOptions {
        empty_as_braces: false,
        ..Default::default()
    };

    let mut s: String = String::new();
    serde_saphyr::to_fmt_writer_with_options(& mut s, &v, options).unwrap();
    assert_eq!(s, "m:\n\n");
}

#[test]
fn test_struct_empty_and_not() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct OneOne {
        empty: HashMap<String, usize>,
        non_empty: HashMap<String, usize>,
    }

    let o = OneOne {
        empty: HashMap::new(),
        non_empty: HashMap::from([("is".to_string(), 7)])
    };

    let yaml = serde_saphyr::to_string(&o).unwrap();
    let expected = r#"empty: {}
non_empty:
  is: 7
"#;
    assert_eq!(yaml, expected);

    let mut opts = SerializerOptions::default();
    opts.empty_as_braces = false;
    let yaml_legacy = {
        let mut out = String::new();
        serde_saphyr::to_fmt_writer_with_options(&mut out, &o, opts).unwrap();
        out
    };

    let legacy_expected = "empty:\n\nnon_empty:\n  is: 7\n";
    assert_eq!(legacy_expected, yaml_legacy);

    // Round trip test
    let or1: OneOne = serde_saphyr::from_str(&yaml).unwrap();
    let or2: OneOne = serde_saphyr::from_str(yaml_legacy.as_str()).unwrap();

    assert_eq!(or1, o);
    assert_eq!(or2, o);

    opts.indent_step = 3;
    opts.empty_as_braces = true;
    let yaml = {
        let mut out = String::new();
        serde_saphyr::to_fmt_writer_with_options(&mut out, &o, opts).unwrap();
        out
    };

    // Just one space more
    let expected = r#"empty: {}
non_empty:
   is: 7
"#;
    assert_eq!(yaml, expected);

    opts.empty_as_braces = false;
    let yaml_legacy = {
        let mut out = String::new();
        serde_saphyr::to_fmt_writer_with_options(&mut out, &o, opts).unwrap();
        out
    };

    // Just one space more
    let legacy_expected = "empty:\n\nnon_empty:\n   is: 7\n";
    assert_eq!(legacy_expected, yaml_legacy);

}

#[test]
fn struct_with_non_empty_map_renders_block_mapping() {
    let mut m = BTreeMap::new();
    m.insert("x".to_string(), "foo".to_string());
    m.insert("y".to_string(), "bar".to_string());
    let v = WrapMap { m };
    let s = serde_saphyr::to_string(&v).unwrap();
    let expected = "m:\n  x: foo\n  y: bar\n";
    assert_eq!(s, expected, "yaml: {}", s);
}

#[derive(Serialize)]
struct WrapSeq {
    v: Vec<BTreeMap<String, String>>,
}

#[test]
fn empty_map_in_sequence_braces_by_default() {
    let v = WrapSeq { v: vec![BTreeMap::new()] };
    let s = serde_saphyr::to_string(&v).unwrap();
    // Allow either "- {}" or inline after key with potential spaces/newline nuances.
    assert!(s.contains("- {}\n") || s.ends_with("- {}\n"), "yaml: {}", s);
}

#[test]
fn disable_empty_map_as_braces_restores_legacy() {
    // When disabled, an empty map in block position should not render braces. We test a map value
    // position where previously an empty body produced just a newline after the key.
    let v = WrapMap { m: BTreeMap::new() };
    let mut opts = SerializerOptions::default();
    opts.empty_as_braces = false;
    let s = {
        let mut out = String::new();
        serde_saphyr::to_fmt_writer_with_options(&mut out, &v, opts).unwrap();
        out
    };
    // We expect there is no "{}" in output when disabled (legacy behavior).
    assert!(!s.contains("{}"), "yaml: {}", s);
}
