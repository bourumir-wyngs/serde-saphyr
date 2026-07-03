#![cfg(all(feature = "serialize", feature = "deserialize"))]

use std::collections::BTreeMap;

use serde::Serialize;
use serde_saphyr::{to_string, to_string_with_options};

#[test]
fn tagged_enums_option_serializes_with_tag() {
    #[derive(Serialize)]
    enum MyEnum {
        Variant(i32),
    }
    let opts = serde_saphyr::ser_options! {
        tagged_enums: true,
    };
    let yaml = to_string_with_options(&MyEnum::Variant(5), opts).unwrap();
    assert!(!yaml.is_empty(), "yaml: {yaml}");
}

#[test]
fn tuple_variant_serialized() {
    #[derive(Serialize)]
    enum Shape {
        #[allow(dead_code)]
        Circle(f64),
        Rect(f64, f64),
    }
    let yaml = to_string(&Shape::Rect(2.0, 3.0)).unwrap();
    assert!(yaml.contains("Rect"), "expected variant name: {yaml}");
    assert!(
        yaml.contains("2.0") || yaml.contains("2."),
        "expected first field: {yaml}"
    );
    assert!(
        yaml.contains("3.0") || yaml.contains("3."),
        "expected second field: {yaml}"
    );
}

#[test]
fn struct_variant_serialized() {
    #[derive(Serialize)]
    enum Event {
        Move { x: i32, y: i32 },
    }
    let yaml = to_string(&Event::Move { x: 10, y: 20 }).unwrap();
    assert!(yaml.contains("Move"), "expected variant name: {yaml}");
    assert!(yaml.contains("x: 10"), "expected first field: {yaml}");
    assert!(
        yaml.contains("y: 20") || yaml.contains("\"y\": 20"),
        "expected second field: {yaml}"
    );
}

#[test]
fn newtype_variant_serialized() {
    #[derive(Serialize)]
    enum Wrapper {
        Int(i32),
    }
    let yaml = to_string(&Wrapper::Int(7)).unwrap();
    assert!(yaml.contains("7"), "yaml: {yaml}");
}

#[test]
fn newtype_variant_tagged_enums() {
    #[derive(Serialize)]
    enum Wrapper {
        Int(i32),
    }
    let opts = serde_saphyr::ser_options! {
        tagged_enums: true,
    };
    let yaml = to_string_with_options(&Wrapper::Int(7), opts).unwrap();
    assert!(yaml.contains("7"), "yaml: {yaml}");
}

#[test]
fn tagged_enums_emit_yaml_tags() {
    #[derive(Serialize)]
    enum Color {
        Red,
    }
    let opts = serde_saphyr::ser_options! { tagged_enums: true };
    let yaml = to_string_with_options(&Color::Red, opts).unwrap();
    assert!(yaml.contains("!!Color"), "expected tag: {yaml}");
}

#[test]
fn tuple_variant_serializes_as_mapping_with_seq() {
    #[derive(Serialize)]
    enum E {
        Pair(i32, i32),
    }
    let yaml = to_string(&E::Pair(1, 2)).unwrap();
    assert!(yaml.contains("Pair"), "expected variant name: {yaml}");
    assert!(
        yaml.contains("- 1") && yaml.contains("- 2"),
        "expected seq elements: {yaml}"
    );
}

#[test]
fn struct_variant_serializes_as_mapping() {
    #[derive(Serialize)]
    enum E {
        Point { x: i32, y: i32 },
    }
    let yaml = to_string(&E::Point { x: 1, y: 2 }).unwrap();
    assert!(yaml.contains("Point"), "expected variant name: {yaml}");
    assert!(yaml.contains("x: 1"), "expected x field: {yaml}");
    assert!(
        yaml.contains("y") && yaml.contains(": 2"),
        "expected y field: {yaml}"
    );
}

#[test]
fn struct_variant_as_map_value() {
    #[derive(Serialize)]
    enum E {
        Point { x: i32, y: i32 },
    }
    let mut m = BTreeMap::new();
    m.insert("loc", E::Point { x: 3, y: 4 });
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("loc"), "expected key: {yaml}");
    assert!(yaml.contains("Point"), "expected variant: {yaml}");
}

#[test]
fn newtype_variant_serializes() {
    #[derive(Serialize)]
    enum Wrap {
        Val(i32),
    }
    let yaml = to_string(&Wrap::Val(42)).unwrap();
    assert!(yaml.contains("Val") && yaml.contains("42"), "got: {yaml}");
}

#[test]
fn newtype_variant_as_map_value() {
    #[derive(Serialize)]
    enum Wrap {
        Val(i32),
    }
    let mut m = BTreeMap::new();
    m.insert("w", Wrap::Val(7));
    let yaml = to_string(&m).unwrap();
    assert!(yaml.contains("Val") && yaml.contains("7"), "got: {yaml}");
}

#[test]
fn struct_variant_in_sequence() {
    #[derive(Serialize)]
    enum E {
        Point { x: i32, y: i32 },
    }
    let v = vec![E::Point { x: 1, y: 2 }, E::Point { x: 3, y: 4 }];
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("Point"), "got: {yaml}");
    assert!(
        yaml.contains("x: 1") && yaml.contains("x: 3"),
        "got: {yaml}"
    );
}

#[test]
fn tuple_variant_in_sequence() {
    #[derive(Serialize)]
    enum E {
        Pair(i32, i32),
    }
    let v = vec![E::Pair(1, 2), E::Pair(3, 4)];
    let yaml = to_string(&v).unwrap();
    assert!(yaml.contains("Pair"), "got: {yaml}");
}

#[test]
fn newtype_variant_with_struct_inner_as_map_value() {
    #[derive(Serialize)]
    struct Inner {
        a: i32,
    }
    #[derive(Serialize)]
    enum E {
        Wrap(Inner),
    }
    let mut m = BTreeMap::new();
    m.insert("k", E::Wrap(Inner { a: 5 }));
    let yaml = to_string(&m).unwrap();
    assert!(
        yaml.contains("Wrap") && yaml.contains("a: 5"),
        "got: {yaml}"
    );
}

#[test]
fn serialize_struct_variant() {
    #[derive(Serialize)]
    enum E {
        Variant { x: i32, y: String },
    }
    let s = serde_saphyr::to_string(&E::Variant {
        x: 10,
        y: "hi".into(),
    })
    .unwrap();
    assert!(s.contains("Variant"));
    assert!(s.contains("x: 10"));
}

#[test]
fn serialize_tuple_variant() {
    #[derive(Serialize)]
    enum E {
        Tup(i32, bool),
    }
    let s = serde_saphyr::to_string(&E::Tup(1, true)).unwrap();
    assert!(s.contains("Tup"));
}

#[test]
fn serialize_unit_variant() {
    #[derive(Serialize)]
    #[allow(dead_code)]
    enum Color {
        Red,
        Blue,
    }
    let s = serde_saphyr::to_string(&Color::Red).unwrap();
    assert!(s.contains("Red"));
}

#[test]
fn serialize_newtype_variant() {
    #[derive(Serialize)]
    enum Wrapper {
        Int(i32),
        Str(String),
    }
    let s = serde_saphyr::to_string(&Wrapper::Int(5)).unwrap();
    assert!(s.contains("Int: 5") || s.contains("Int"));
    let s2 = serde_saphyr::to_string(&Wrapper::Str("hi".into())).unwrap();
    assert!(s2.contains("Str"));
}
