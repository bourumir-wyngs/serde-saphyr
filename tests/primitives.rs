#![cfg(all(feature = "serialize", feature = "deserialize"))]
use rstest::rstest;
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

    let parsed =
        serde_saphyr::from_str::<PrimitiveStruct>(yaml).expect("failed to deserialize YAML");

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
        f32_val: std::f32::consts::PI,
        f64_val: std::f64::consts::E,
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

#[rstest]
#[case::unquoted_ascii("c: A\n", 'A')]
#[case::quoted_ascii("c: 'B'\n", 'B')]
#[case::greek_lambda("c: λ\n", 'λ')]
#[case::emoji("c: 🙂\n", '🙂')]
fn char_ok_cases(#[case] yaml: &str, #[case] expected: char) {
    let v: HasChar = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(v.c, expected);
}

#[rstest]
#[case::more_than_one_character("c: AB\n")]
#[case::yaml_null_tilde("c: ~\n")]
#[case::yaml_null_word("c: Null\n")]
#[case::empty_scalar("c:\n")]
fn char_error_cases(#[case] yaml: &str) {
    let err = serde_saphyr::from_str::<HasChar>(yaml).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("invalid char"));
}

#[test]
fn deserialize_btreemap_with_tuple_keys() {
    let yaml = r#"
? [1, 2]
: 3
? [4, 5]
: 6
"#;

    let parsed = serde_saphyr::from_str::<BTreeMap<(i32, i32), i32>>(yaml)
        .expect("failed to deserialize map");

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
