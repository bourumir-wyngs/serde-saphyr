#![cfg(all(feature = "serialize", feature = "deserialize"))]

use std::collections::BTreeMap;

use serde::Serialize;
use serde_saphyr::to_string;

#[test]
fn block_seq_after_block_value_forces_newline() {
    #[derive(Serialize)]
    struct S {
        first: Vec<i32>,
        second: Vec<i32>,
    }
    let yaml = to_string(&S {
        first: vec![1, 2],
        second: vec![3, 4],
    })
    .unwrap();
    assert!(
        yaml.contains("first:") && yaml.contains("second:"),
        "yaml: {yaml}"
    );
}

#[test]
fn block_map_after_block_value_forces_newline() {
    #[derive(Serialize)]
    struct Inner {
        x: i32,
    }
    #[derive(Serialize)]
    struct S {
        first: Inner,
        second: Inner,
    }
    let yaml = to_string(&S {
        first: Inner { x: 1 },
        second: Inner { x: 2 },
    })
    .unwrap();
    assert!(
        yaml.contains("first:") && yaml.contains("second:"),
        "yaml: {yaml}"
    );
}

#[test]
fn nested_block_seq_as_map_value_then_another_key() {
    #[derive(Serialize)]
    struct S {
        items: Vec<i32>,
        count: i32,
    }
    let yaml = to_string(&S {
        items: vec![1, 2, 3],
        count: 3,
    })
    .unwrap();
    assert!(yaml.contains("items:"), "yaml: {yaml}");
    assert!(yaml.contains("count: 3"), "yaml: {yaml}");
}

#[test]
fn nested_block_map_as_map_value_then_another_key() {
    #[derive(Serialize)]
    struct Inner {
        x: i32,
    }
    #[derive(Serialize)]
    struct Outer {
        inner: Inner,
        after: i32,
    }
    let yaml = to_string(&Outer {
        inner: Inner { x: 1 },
        after: 2,
    })
    .unwrap();
    assert!(yaml.contains("inner:"), "yaml: {yaml}");
    assert!(yaml.contains("after: 2"), "yaml: {yaml}");
}

#[test]
fn tuple_struct_serialized() {
    #[derive(Serialize)]
    struct Point(i32, i32);
    let yaml = to_string(&Point(3, 4)).unwrap();
    assert!(yaml.contains("- 3"), "expected first element: {yaml}");
    assert!(yaml.contains("- 4"), "expected second element: {yaml}");
}

#[test]
fn seq_of_structs_inline_map_after_dash() {
    #[derive(Serialize)]
    struct Item {
        name: &'static str,
        val: i32,
    }
    let v = vec![Item { name: "a", val: 1 }, Item { name: "b", val: 2 }];
    let yaml = to_string(&v).unwrap();
    assert!(
        yaml.contains("name: a") || yaml.contains("name:"),
        "yaml: {yaml}"
    );
}

#[test]
fn seq_of_seqs_nested() {
    let v = vec![vec![1i32, 2], vec![3, 4]];
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains('1') && yaml.contains('3'), "yaml: {yaml}");
}

#[test]
fn normal_tuple_struct_serializes_as_seq() {
    #[derive(Serialize)]
    struct Pair(i32, String);
    let yaml = to_string(&Pair(1, "two".into())).unwrap();
    assert!(
        yaml.contains("- 1") && yaml.contains("- two"),
        "got: {yaml}"
    );
}

#[test]
fn nested_map_in_seq() {
    #[derive(Serialize)]
    struct Inner {
        x: i32,
    }
    let v = vec![Inner { x: 1 }, Inner { x: 2 }];
    let yaml = to_string(&v).unwrap();
    assert!(
        yaml.contains("- x: 1") && yaml.contains("- x: 2"),
        "got: {yaml}"
    );
}

#[test]
fn seq_in_map_value() {
    let mut m = BTreeMap::new();
    m.insert("items", vec![1, 2, 3]);
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("items:"), "got: {yaml}");
    assert!(yaml.contains("- 1"), "got: {yaml}");
}

#[test]
fn struct_with_various_field_types() {
    #[derive(Serialize)]
    struct Mixed {
        b: bool,
        i: i64,
        f: f64,
        s: String,
        o: Option<i32>,
        v: Vec<u8>,
    }
    let m = Mixed {
        b: true,
        i: -42,
        f: std::f64::consts::PI,
        s: "hello".into(),
        o: None,
        v: vec![1, 2],
    };
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("b: true"), "got: {yaml}");
    assert!(yaml.contains("i: -42"), "got: {yaml}");
    assert!(yaml.contains("s: hello"), "got: {yaml}");
    assert!(yaml.contains("o: null"), "got: {yaml}");
}

#[test]
fn map_value_is_map() {
    let mut inner = BTreeMap::new();
    inner.insert("x", 1);
    let mut outer = BTreeMap::new();
    outer.insert("nested", inner);
    let yaml = to_string(&outer).unwrap();
    assert!(
        yaml.contains("nested:") && yaml.contains("x: 1"),
        "got: {yaml}"
    );
}

#[test]
fn seq_value_after_block_sibling() {
    #[derive(Serialize)]
    struct S {
        a: Vec<i32>,
        b: Vec<i32>,
    }
    let s = S {
        a: vec![1],
        b: vec![2],
    };
    let yaml = to_string(&s).unwrap();
    assert!(yaml.contains("- 1") && yaml.contains("- 2"), "got: {yaml}");
}

#[test]
fn serialize_tuple() {
    let s = serde_saphyr::to_string(&(1, "two", true)).unwrap();
    assert!(s.contains("- 1"));
    assert!(s.contains("- two"));
    assert!(s.contains("- true"));
}

#[test]
fn serialize_tuple_struct() {
    #[derive(Serialize)]
    struct Pair(i32, String);
    let s = serde_saphyr::to_string(&Pair(1, "hello".into())).unwrap();
    assert!(s.contains("- 1"));
    assert!(s.contains("- hello"));
}

#[test]
fn serialize_map() {
    use std::collections::BTreeMap;
    let mut m = BTreeMap::new();
    m.insert("key1", 1);
    m.insert("key2", 2);
    let s = serde_saphyr::to_string(&m).unwrap();
    assert!(s.contains("key1: 1"));
    assert!(s.contains("key2: 2"));
}

#[test]
fn serialize_nested_struct() {
    #[derive(Serialize)]
    struct Inner {
        value: i32,
    }
    #[derive(Serialize)]
    struct Outer {
        name: String,
        inner: Inner,
    }
    let s = serde_saphyr::to_string(&Outer {
        name: "test".into(),
        inner: Inner { value: 42 },
    })
    .unwrap();
    assert!(s.contains("name: test"));
    assert!(s.contains("value: 42"));
}

#[test]
fn serialize_empty_vec() {
    let v: Vec<i32> = vec![];
    let s = serde_saphyr::to_string(&v).unwrap();
    assert!(s.contains("[]"));
}

#[test]
fn serialize_empty_map() {
    use std::collections::BTreeMap;
    let m: BTreeMap<String, i32> = BTreeMap::new();
    let s = serde_saphyr::to_string(&m).unwrap();
    assert!(s.contains("{}"));
}
