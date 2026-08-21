#![cfg(all(feature = "serialize", feature = "deserialize"))]

use serde::{Deserialize, Serialize};

// Regression coverage adapted (assuming MIT and Apache licenses) from:
// - Pull request: https://github.com/yaml/yaml-serde/pull/9
// - Author: cho-minsung (https://github.com/cho-minsung)
fn assert_round_trip<T>(value: &T)
where
    T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    let yaml = serde_saphyr::to_string(value).expect("nested enum should serialize");
    let deserialized =
        serde_saphyr::from_str(&yaml).expect("serialized nested enum should deserialize");

    assert_eq!(*value, deserialized, "failed to round-trip:\n{yaml}");
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
enum Outer {
    Variant(Inner),
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
enum Inner {
    A,
    Newtype(i32),
}

#[test]
fn nested_newtype_variant_round_trips() {
    assert_round_trip(&Outer::Variant(Inner::Newtype(1)));
}

#[test]
fn nested_enum_payload_shapes_round_trip() {
    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    enum Outer {
        Newtype(Inner),
        Tuple(Inner, i32),
        Struct { inner: Inner, other: i32 },
    }

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    enum Inner {
        Newtype(i32),
        Tuple(i32, i32),
        Struct { x: i32, y: i32 },
    }

    for value in [
        Outer::Newtype(Inner::Newtype(1)),
        Outer::Newtype(Inner::Tuple(1, 2)),
        Outer::Tuple(Inner::Struct { x: 1, y: 2 }, 42),
        Outer::Struct {
            inner: Inner::Newtype(1),
            other: 42,
        },
    ] {
        assert_round_trip(&value);
    }
}

#[test]
fn nested_unit_variant_round_trips() {
    assert_round_trip(&Outer::Variant(Inner::A));
}
