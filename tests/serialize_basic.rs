use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// 1. A structure with int, float, boolean, string fields
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Prims {
    int: i32,
    float: f64,
    boolean: bool,
    string: String,
}

#[test]
fn serialize_basic_primitives_struct() {
    let v = Prims {
        int: 7,
        float: 3.5,
        boolean: true,
        string: "hello".to_string(),
    };
    let yaml = serde_saphyr::to_string(&v).expect("serialize primitives struct");
    // Round-trip to validate
    let back: Prims = serde_saphyr::from_str(&yaml).expect("roundtrip primitives struct");
    assert_eq!(back, v);
}

// 2. A structure holding array of structures
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Item {
    id: u32,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Container {
    items: Vec<Item>,
}

#[test]
fn serialize_struct_holding_array_of_structs() {
    let v = Container {
        items: vec![
            Item {
                id: 1,
                name: "first".into(),
            },
            Item {
                id: 2,
                name: "second".into(),
            },
        ],
    };
    let yaml = serde_saphyr::to_string(&v).expect("serialize container");
    // Basic shape checks instead of strict round-trip (serializer formatting may vary)
    assert!(yaml.contains("items:"), "yaml: {}", yaml);
    assert!(
        yaml.contains("\n  -") || yaml.contains("\n- "),
        "yaml: {}",
        yaml
    );
    assert!(yaml.contains("id:"), "yaml: {}", yaml);
    assert!(yaml.contains("name:"), "yaml: {}", yaml);
}

// 3. A structure holding BTreeMap of ints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct MapWrap {
    map: BTreeMap<String, i32>,
}

#[test]
fn serialize_struct_holding_btreemap_of_ints() {
    let mut map = BTreeMap::new();
    map.insert("a".into(), 1);
    map.insert("b".into(), 2);
    map.insert("c".into(), 3);
    let v = MapWrap { map };
    let yaml = serde_saphyr::to_string(&v).expect("serialize mapwrap");
    assert!(yaml.contains("map:"), "yaml: {}", yaml);
    assert!(yaml.contains("a:"), "yaml: {}", yaml);
    assert!(yaml.contains("b:"), "yaml: {}", yaml);
    assert!(yaml.contains("c:"), "yaml: {}", yaml);
}

// 4. Variant enum, with one of the fields being other variant enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Inner {
    Unit,
    Newtype(i32),
    Struct { flag: bool },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Outer {
    Alpha(String),
    Beta { inner: Inner, note: String },
    Gamma(Inner, i64),
}

#[test]
fn serialize_nested_variant_enums() {
    let cases = vec![
        Outer::Alpha("hello".into()),
        Outer::Beta {
            inner: Inner::Struct { flag: true },
            note: "x".into(),
        },
        Outer::Gamma(Inner::Newtype(42), -10),
    ];
    for v in cases {
        let yaml = serde_saphyr::to_string(&v).expect("serialize enum");
        assert!(!yaml.trim().is_empty());
        // Ensure variant names are present in the YAML output
        match v {
            Outer::Alpha(_) => assert!(yaml.contains("Alpha"), "yaml: {}", yaml),
            Outer::Beta { .. } => assert!(yaml.contains("Beta"), "yaml: {}", yaml),
            Outer::Gamma(_, _) => assert!(yaml.contains("Gamma"), "yaml: {}", yaml),
        }
    }

    // Also exercise to_writer and to_writer_with_indent
    let v = Outer::Beta {
        inner: Inner::Unit,
        note: "ok".into(),
    };
    let mut buf = String::new();
    serde_saphyr::to_fmt_writer(&mut buf, &v).expect("to_writer works");
    assert!(!buf.is_empty());
    let mut buf2 = String::new();
    let opts = serde_saphyr::SerializerOptions {
        indent_step: 4,
        anchor_generator: None,
        ..Default::default()
    };
    serde_saphyr::to_fmt_writer_with_options(&mut buf2, &v, opts)
        .expect("to_writer_with_options works");
    assert!(!buf2.is_empty());
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct VecOfMaps {
    vec: Vec<BTreeMap<String, String>>,
}

#[test]
fn serialize_array_of_empty_maps() {
    let v = VecOfMaps {
        vec: vec![Default::default(), Default::default(), Default::default()],
    };
    let mut buf = String::new();
    serde_saphyr::to_fmt_writer(&mut buf, &v).expect("to_writer works");
    let v2: VecOfMaps = serde_saphyr::from_str(&buf).expect("deserialize just serialized data");
    assert_eq!(v, v2);
}

#[test]
fn serialize_array_of_empty_maps_to_io() {
    let v = VecOfMaps {
        vec: vec![Default::default(), Default::default(), Default::default()],
    };
    let mut buf: Vec<u8> = Vec::new();
    serde_saphyr::to_io_writer(&mut buf, &v).expect("to_writer works");
    let s = String::from_utf8(buf).expect("valid utf-8");
    let v2: VecOfMaps = serde_saphyr::from_str(&s).expect("deserialize just serialized data");
    assert_eq!(v, v2);
}
