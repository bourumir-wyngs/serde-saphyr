#![allow(clippy::derive_partial_eq_without_eq)]

use indoc::indoc;
use serde::{Deserialize, Serialize};
use serde_saphyr;
use std::fmt::Debug;
use serde_saphyr::from_str;

fn test_serde<T>(thing: &T, yaml: &str)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + Debug,
{
    let serialized = serde_saphyr::to_string(thing).unwrap();
    assert_eq!(yaml, serialized);
    let round_trip: T = serde_saphyr::from_str(&serialized).unwrap();
    assert_eq!(*thing, round_trip);
}

#[test]
fn test_simple_enum() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    enum Simple {
        A,
        B,
    }
    let thing = Simple::A;
    let yaml = "A\n";
    test_serde(&thing, yaml);
}

#[test]
fn test_enum_with_fields() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    enum Variant {
        Color { r: u8, g: u8, b: u8 },
    }
    let thing = Variant::Color { r: 32, g: 64, b: 96 };
    let yaml = indoc! {r#"
        Color:
          r: 32
          g: 64
          b: 96
    "#};
    test_serde(&thing, yaml);
}

#[test]
fn test_nested_enum() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    enum Outer {
        Inner(Inner),
    }
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    enum Inner {
        Newtype(u8),
    }
    let thing = Outer::Inner(Inner::Newtype(0));
    let yaml = indoc! {r#"
        Inner:
          Newtype: 0
    "#};
    test_serde(&thing, yaml);
}


#[test]
fn parse_mixed_item_list_yaml() {

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    enum Debut {
        Shown {
            cinema: String,
        },
        Bookstore {
            address: String,
        }
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    enum Publication {
        Book {
            title: String,
            publisher: String,
            published_at: String,
            debut: Debut
        },
        Movie {
            title: String,
            director: String,
            debut: Debut
        },
    }

    let yaml = r#"
    - Book:
          title: Life
          publisher: someone
          published_at: 2023-02-24T09:31:00Z+09:00
          debut:
            Bookstore:
                address: Good Books
    - Movie:
          title: Life
          director: someone else
          debut:
            Shown:
                cinema: Europa Center Movie
    - Movie:
          title: Afterlife
          director: someone else
          debut:
            Bookstore:
                address: My DVD shop
"#;

    let parsed: Vec<Publication> = from_str(yaml).expect("Failed to parse items YAML");

    let expected = vec![
        Publication::Book {
            title: "Life".into(),
            publisher: "someone".into(),
            published_at: "2023-02-24T09:31:00Z+09:00".into(),
            debut: Debut::Bookstore {
                address: "Good Books".into(),
            },
        },
        Publication::Movie {
            title: "Life".into(),
            director: "someone else".into(),
            debut: Debut::Shown {
                cinema: "Europa Center Movie".into(),
            },
        },
        Publication::Movie {
            title: "Afterlife".into(),
            director: "someone else".into(),
            debut: Debut::Bookstore {
                address: "My DVD shop".into(),
            },
        },
    ];

    assert_eq!(parsed, expected);
}

#[test]
fn test_nested_enum_yaml_bw() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Foo {
        common: String,
        bar: Bar,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    #[serde(rename_all = "snake_case")]
    enum Bar {
        BarA { a: String },
        BarB { b: String },
    }

    let input = r#"
          common: Hello
          bar:
            bar_a:
                a: World
    "#;

    let foo: Foo = from_str(input).expect("Failed");

    assert_eq!(
        foo,
        Foo {
            common: "Hello".into(),
            bar: Bar::BarA { a: "World".into() },
        }
    );
}