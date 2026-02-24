#[test]
fn saphyr_serialization_enum() {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    pub struct Foo {
        bars: Vec<Bar>,
    }

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    pub enum Bar {
        Zip(Zip),
    }

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    pub struct Zip {
        a: i32,
        b: i32,
    }

    let foo = Foo {
        bars: vec![Bar::Zip(Zip { a: 1, b: 2 }), Bar::Zip(Zip { a: 3, b: 4 })],
    };
    let serialized = serde_saphyr::to_string(&foo).unwrap();
    assert_eq!(
        serialized,
        "bars:\n  - Zip:\n      a: 1\n      b: 2\n  - Zip:\n      a: 3\n      b: 4\n"
    );

    let opts = serde_saphyr::ser_options! {
        compact_list_indent: true,
    };
    let compact_serialized = serde_saphyr::to_string_with_options(&foo, opts).unwrap();
    assert_eq!(
        compact_serialized,
        "bars:\n- Zip:\n    a: 1\n    b: 2\n- Zip:\n    a: 3\n    b: 4\n"
    );
    let parsed: Foo = serde_saphyr::from_str(&compact_serialized).unwrap();
    assert_eq!(parsed, foo);
}
