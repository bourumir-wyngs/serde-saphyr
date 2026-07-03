#![cfg(all(feature = "serialize", feature = "deserialize"))]

use serde::Deserialize;

#[test]
fn deserialize_bool() {
    let v: bool = serde_saphyr::from_str("true").unwrap();
    assert!(v);
}

#[test]
fn deserialize_i128() {
    let v: i128 = serde_saphyr::from_str("170141183460469231731687303715884105727").unwrap();
    assert_eq!(v, 170141183460469231731687303715884105727i128);
}

#[test]
fn deserialize_u128() {
    let v: u128 = serde_saphyr::from_str("340282366920938463463374607431768211455").unwrap();
    assert_eq!(v, 340282366920938463463374607431768211455u128);
}

#[test]
fn deserialize_char() {
    let v: char = serde_saphyr::from_str("Z").unwrap();
    assert_eq!(v, 'Z');
}

#[test]
fn deserialize_unit() {
    let _: () = serde_saphyr::from_str("null").unwrap();
}

#[test]
fn deserialize_unit_struct() {
    #[derive(Deserialize, Debug)]
    struct Unit;
    let _: Unit = serde_saphyr::from_str("null").unwrap();
}

#[test]
fn deserialize_newtype_struct() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Wrapper(i32);
    let v: Wrapper = serde_saphyr::from_str("42").unwrap();
    assert_eq!(v, Wrapper(42));
}

#[test]
fn deserialize_tuple() {
    let v: (i32, String, bool) = serde_saphyr::from_str("- 1\n- two\n- true").unwrap();
    assert_eq!(v, (1, "two".to_string(), true));
}

#[test]
fn deserialize_tuple_struct() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Pair(i32, String);
    let v: Pair = serde_saphyr::from_str("- 1\n- hello").unwrap();
    assert_eq!(v, Pair(1, "hello".into()));
}

#[test]
fn deserialize_enum_variants() {
    #[derive(Deserialize, Debug, PartialEq)]
    enum E {
        Unit,
        Newtype(i32),
        Tuple(i32, bool),
        Struct { x: i32 },
    }
    let v: E = serde_saphyr::from_str("Unit").unwrap();
    assert_eq!(v, E::Unit);

    let v: E = serde_saphyr::from_str("Newtype: 5").unwrap();
    assert_eq!(v, E::Newtype(5));

    let v: E = serde_saphyr::from_str("Tuple:\n  - 1\n  - true").unwrap();
    assert_eq!(v, E::Tuple(1, true));

    let v: E = serde_saphyr::from_str("Struct:\n  x: 10").unwrap();
    assert_eq!(v, E::Struct { x: 10 });
}
