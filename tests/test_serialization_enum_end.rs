#[test]
fn saphyr_serialization_enum() {
    use serde::Serialize;

    #[derive(Serialize)]
    pub struct Foo {
        bars: Vec<Bar>,
    }

    #[derive(Serialize)]
    pub enum Bar {
        Zip(Zip),
    }

    #[derive(Serialize)]
    pub struct Zip {
        a: i32,
        b: i32,
    }

    let foo = Foo {
        bars: vec![Bar::Zip(Zip { a: 1, b: 2 }), Bar::Zip(Zip { a: 3, b: 4 })],
    };
    let serialized = serde_saphyr::to_string(&foo).unwrap();
    println!("{}", serialized);
    assert_eq!(
        serialized,
        "bars:\n  - Zip:\n      a: 1\n      b: 2\n  - Zip:\n      a: 3\n      b: 4\n"
    );
}