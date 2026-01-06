use serde::{Deserialize, Serialize};
use serde_saphyr as yaml;
use serde_saphyr::LitString;

#[derive(Serialize, Deserialize)]
struct Foo {
    a: i32,
    b: bool,
    short: String,
    long: String,
}

#[test]
fn yaml_long_strings() {
    let reference = Foo {
        a: 32,
        b: true,
        short: "A".repeat(20),
        long: "A".repeat(200),
    };

    let mut serialized = String::new();
    yaml::to_fmt_writer_with_options(
        &mut serialized,
        &reference,
        yaml::SerializerOptions {
            prefer_block_scalars: false,
            ..Default::default()
        },
    )
    .expect("Unable to serialize my struct!");
    println!("Serialized YAML: r#\"\n{serialized}\"#");

    let test: Foo = yaml::from_str(serialized.as_str()).expect("Unable to serialize my struct!");
    assert_eq!(reference.long, test.long);
}

#[derive(Serialize, Deserialize)]
struct FooLs {
    a: i32,
    b: bool,
    short: String,
    long: LitString,
}

#[test]
fn yaml_long_strings_ls() {
    let reference = FooLs {
        a: 32,
        b: true,
        short: "A".repeat(20),
        long: LitString("A".repeat(200)),
    };

    let serialized = yaml::to_string(&reference).expect("Unable to serialize my struct!");
    println!("Serialized YAML: r#\"\n{serialized}\"#");

    let test: Foo = yaml::from_str(serialized.as_str()).expect("Unable to serialize my struct!");
    assert_eq!(reference.long.0, test.long);
}

#[ignore]
#[test]
fn yaml_long_strings_2() {
    let reference = Foo {
        a: 32,
        b: true,
        short: "A".repeat(20),
        long: "A".repeat(200)
    };
    let serialized = yaml::to_string(&reference).expect("Unable to serialize my struct!");
    println!("Serialized YAML: r#\"\n{serialized}\"#");

    let test: Foo = yaml::from_str(serialized.as_str()).expect("Unable to serialize my struct!");
    assert_eq!(reference.long,test.long);
}
