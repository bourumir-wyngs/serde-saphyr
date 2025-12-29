use serde::{Deserialize, Serialize};

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
