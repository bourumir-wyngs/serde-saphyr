use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Foo {
    foo: Bar,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum Bar {
    Bar(i32),
}

#[test]
fn test_enum_struct() -> anyhow::Result<()> {
    let yaml = serde_saphyr::to_string(&vec![Foo { foo: Bar::Bar(2) }])?;
    let foo: Vec<Foo> = serde_saphyr::from_str(yaml.as_str())?;
    assert_eq!(foo.first().expect("empty vector?").foo, Bar::Bar(2));
    Ok(())
}


