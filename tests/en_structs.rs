#![cfg(all(feature = "serialize", feature = "deserialize"))]
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[test]
fn test_enum_struct_1() -> anyhow::Result<()> {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    pub struct Foo {
        foo: Bar,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    pub enum Bar {
        Bar(i32),
    }

    let yaml = serde_saphyr::to_string(&vec![Foo { foo: Bar::Bar(2) }])?;
    let foo: Vec<Foo> = serde_saphyr::from_str(yaml.as_str())?;
    assert_eq!(foo.first().expect("empty vector?").foo, Bar::Bar(2));
    Ok(())
}

#[test]
fn test_enum_struct_2() -> anyhow::Result<()> {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    pub struct Outer {
        inner: Vec<Inner>,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    pub struct Inner {
        foo: Foo,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    pub enum Foo {
        Bar(Bar),
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    pub enum Bar {
        Baz(i32),
    }

    let value = Outer {
        inner: vec![Inner {
            foo: Foo::Bar(Bar::Baz(42)),
        }],
    };
    let yaml = serde_saphyr::to_string(&value)?;

    let r: Outer = serde_saphyr::from_str(&yaml)?;
    assert_eq!(value, r);

    Ok(())
}

#[test]
fn tuple_variant_round_trips_when_nested_under_a_key() -> anyhow::Result<()> {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    enum E {
        Tup(u8, u8, u8),
    }

    let value = BTreeMap::from([(
        "hello".to_string(),
        BTreeMap::from([("world".to_string(), E::Tup(1, 2, 3))]),
    )]);
    let yaml = serde_saphyr::to_string(&value)?;
    let r: BTreeMap<String, BTreeMap<String, E>> = serde_saphyr::from_str(&yaml)?;
    assert_eq!(value, r);

    Ok(())
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum ComplexFieldEnum {
    Tup(Box<ComplexFieldEnum>, Box<ComplexFieldEnum>),
    Strukt { field: Box<ComplexFieldEnum> },
    Seq(Vec<ComplexFieldEnum>),
    Unit,
}

/// A tuple-variant whose first field is itself a complex node (a struct variant,
/// a sequence, another tuple variant). Each field must keep its body indented
/// under its own dash rather than dedenting to the dash column.
#[rstest]
#[case::struct_variant(ComplexFieldEnum::Strukt {
    field: Box::new(ComplexFieldEnum::Unit),
})]
#[case::sequence(ComplexFieldEnum::Seq(vec![ComplexFieldEnum::Unit]))]
#[case::tuple_variant(ComplexFieldEnum::Tup(
    Box::new(ComplexFieldEnum::Unit),
    Box::new(ComplexFieldEnum::Unit),
))]
fn tuple_variant_round_trips_with_complex_fields(
    #[case] first_field: ComplexFieldEnum,
) -> anyhow::Result<()> {
    let value = ComplexFieldEnum::Tup(Box::new(first_field), Box::new(ComplexFieldEnum::Unit));
    let yaml = serde_saphyr::to_string(&value)?;
    let r: ComplexFieldEnum =
        serde_saphyr::from_str(&yaml).map_err(|e| anyhow::anyhow!("{e}\n--- yaml ---\n{yaml}"))?;
    assert_eq!(value, r, "round-trip mismatch for:\n{yaml}");

    Ok(())
}

#[rstest]
#[case::after_block_seq(ComplexFieldEnum::Seq(vec![
    ComplexFieldEnum::Unit,
    ComplexFieldEnum::Unit,
]))]
#[case::after_nested_tuple(ComplexFieldEnum::Tup(
    Box::new(ComplexFieldEnum::Unit),
    Box::new(ComplexFieldEnum::Unit),
))]
fn empty_tuple_field_after_block_sibling_round_trips(
    #[case] block_first_field: ComplexFieldEnum,
) -> anyhow::Result<()> {
    let value = ComplexFieldEnum::Tup(
        Box::new(block_first_field),
        Box::new(ComplexFieldEnum::Seq(vec![])),
    );
    let yaml = serde_saphyr::to_string(&value)?;
    let r: ComplexFieldEnum = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(value, r, "round-trip mismatch for:\n{yaml}");

    Ok(())
}

#[test]
fn struct_variant_round_trips_when_nested_under_a_key() -> anyhow::Result<()> {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    enum E {
        Strukt { a: u8, b: u8 },
    }

    let value = BTreeMap::from([(
        "hello".to_string(),
        BTreeMap::from([("world".to_string(), E::Strukt { a: 1, b: 2 })]),
    )]);
    let yaml = serde_saphyr::to_string(&value)?;
    let r: BTreeMap<String, BTreeMap<String, E>> = serde_saphyr::from_str(&yaml)?;
    assert_eq!(value, r);

    Ok(())
}
