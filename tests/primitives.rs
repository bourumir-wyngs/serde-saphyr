use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, PartialEq)]
struct PrimitiveStruct {
    text: String,
    flag: bool,
    i8_val: i8,
    i16_val: i16,
    i32_val: i32,
    i64_val: i64,
    i128_val: i128,
    isize_val: isize,
    u8_val: u8,
    u16_val: u16,
    u32_val: u32,
    u64_val: u64,
    u128_val: u128,
    usize_val: usize,
    f32_val: f32,
    f64_val: f64,
}

#[derive(Debug, Deserialize, PartialEq)]
struct InnerStruct {
    name: String,
    value: i32,
}

#[derive(Debug, Deserialize, PartialEq)]
struct NestedStruct {
    title: String,
    inner: InnerStruct,
}

#[derive(Debug, Deserialize, PartialEq)]
struct SequenceHolder {
    label: String,
    items: Vec<InnerStruct>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct IntSequenceStruct {
    description: String,
    values: Vec<i32>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Debug, Deserialize, PartialEq)]
struct StructKeyMap {
    mapping: BTreeMap<Point, i32>,
}

#[test]
fn deserialize_primitives_structure() {
    let yaml = r#"
text: "Hello Serde"
flag: true
i8_val: -8
i16_val: -1600
i32_val: -3200000
i64_val: -6400000000
i128_val: -128000000000000000000
isize_val: -123456
u8_val: 8
u16_val: 1600
u32_val: 3200000
u64_val: 6400000000
u128_val: 128000000000000000000
usize_val: 123456
f32_val: 3.1415927
f64_val: 2.718281828459045
"#;

    let parsed = serde_saphyr::from_str::<PrimitiveStruct>(yaml).expect("failed to deserialize YAML");

    let expected = PrimitiveStruct {
        text: "Hello Serde".to_string(),
        flag: true,
        i8_val: -8,
        i16_val: -1600,
        i32_val: -3200000,
        i64_val: -6400000000,
        i128_val: -128000000000000000000,
        isize_val: -123456,
        u8_val: 8,
        u16_val: 1600,
        u32_val: 3200000,
        u64_val: 6400000000,
        u128_val: 128000000000000000000,
        usize_val: 123456,
        f32_val: 3.1415927,
        f64_val: 2.718281828459045,
    };

    assert_eq!(parsed, expected);
}

#[test]
fn deserialize_nested_structure() {
    let yaml = r#"
title: "Nested Example"
inner:
  name: "Inner"
  value: 42
"#;

    let parsed =
        serde_saphyr::from_str::<NestedStruct>(yaml).expect("failed to deserialize nested struct");

    let expected = NestedStruct {
        title: "Nested Example".to_string(),
        inner: InnerStruct {
            name: "Inner".to_string(),
            value: 42,
        },
    };

    assert_eq!(parsed, expected);
}

#[test]
fn deserialize_sequence_of_structs() {
    let yaml = r#"
label: "Collection"
items:
  - name: "First"
    value: 1
  - name: "Second"
    value: 2
  - name: "Third"
    value: 3
"#;

    let parsed = serde_saphyr::from_str::<SequenceHolder>(yaml)
        .expect("failed to deserialize struct containing sequence of structs");

    let expected = SequenceHolder {
        label: "Collection".to_string(),
        items: vec![
            InnerStruct {
                name: "First".to_string(),
                value: 1,
            },
            InnerStruct {
                name: "Second".to_string(),
                value: 2,
            },
            InnerStruct {
                name: "Third".to_string(),
                value: 3,
            },
        ],
    };

    assert_eq!(parsed, expected);
}

#[test]
fn deserialize_struct_with_int_sequence() {
    let yaml = r#"
description: "Sequence of integers"
values:
  - 10
  - 20
  - 30
  - 40
"#;

    let parsed = serde_saphyr::from_str::<IntSequenceStruct>(yaml)
        .expect("failed to deserialize struct with int sequence");

    let expected = IntSequenceStruct {
        description: "Sequence of integers".to_string(),
        values: vec![10, 20, 30, 40],
    };

    assert_eq!(parsed, expected);
}

#[derive(Debug, Deserialize, PartialEq)]
struct HasChar {
    c: char,
}

#[test]
fn char_ok_cases() {
    // unquoted ASCII
    let y1 = "c: A\n";
    let v1: HasChar = serde_saphyr::from_str(y1).unwrap();
    assert_eq!(v1.c, 'A');

    // quoted ASCII
    let y2 = "c: 'B'\n";
    let v2: HasChar = serde_saphyr::from_str(y2).unwrap();
    assert_eq!(v2.c, 'B');

    // single Unicode scalar (Greek lambda)
    let y3 = "c: λ\n";
    let v3: HasChar = serde_saphyr::from_str(y3).unwrap();
    assert_eq!(v3.c, 'λ');

    // single Unicode scalar (emoji)
    let y4 = "c: 🙂\n";
    let v4: HasChar = serde_saphyr::from_str(y4).unwrap();
    assert_eq!(v4.c, '🙂');
}

#[test]
fn char_error_cases() {
    // more than one character
    let y1 = "c: AB\n";
    let err1 = serde_saphyr::from_str::<HasChar>(y1).unwrap_err();
    let msg1 = format!("{err1}");
    assert!(msg1.contains("invalid char"));

    // YAML null tilde
    let y2 = "c: ~\n";
    let err2 = serde_saphyr::from_str::<HasChar>(y2).unwrap_err();
    let msg2 = format!("{err2}");
    assert!(msg2.contains("invalid char"));

    // YAML 'null' (any case)
    let y3 = "c: Null\n";
    let err3 = serde_saphyr::from_str::<HasChar>(y3).unwrap_err();
    let msg3 = format!("{err3}");
    assert!(msg3.contains("invalid char"));

    // empty scalar (missing value)
    let y4 = "c:\n";
    let err4 = serde_saphyr::from_str::<HasChar>(y4).unwrap_err();
    let msg4 = format!("{err4}");
    assert!(msg4.contains("invalid char"));
}

#[test]
fn deserialize_btreemap_with_tuple_keys() {
    let yaml = r#"
? [1, 2]
: 3
? [4, 5]
: 6
"#;

    let parsed =
        serde_saphyr::from_str::<BTreeMap<(i32, i32), i32>>(yaml).expect("failed to deserialize map");

    let mut expected = BTreeMap::new();
    expected.insert((1, 2), 3);
    expected.insert((4, 5), 6);

    assert_eq!(parsed, expected);
}

#[test]
fn deserialize_map_with_struct_keys() {
    let yaml = r#"
mapping:
  ?
    x: 1
    y: 2
  : 10
  ?
    x: 3
    y: 4
  : 20
"#;

    let parsed = serde_saphyr::from_str::<StructKeyMap>(yaml)
        .expect("failed to deserialize struct containing map with struct keys");

    let mut expected_map = BTreeMap::new();
    expected_map.insert(Point { x: 1, y: 2 }, 10);
    expected_map.insert(Point { x: 3, y: 4 }, 20);

    let expected = StructKeyMap {
        mapping: expected_map,
    };

    assert_eq!(parsed, expected);
}
