use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
enum Color {
    RED,
    GREEN,
}

#[test]
fn tagged_enum_parses_unit_variant() {
    let yaml = "!!Color RED";
    let parsed: Color = serde_saphyr::from_str(yaml).expect("failed to parse tagged enum");
    assert_eq!(parsed, Color::RED);
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
enum Shape {
    CIRCLE,
    SQUARE,
}

#[test]
fn tagged_enum_with_wrong_type_errors() {
    let yaml = "!!Color GREEN";
    let err = serde_saphyr::from_str::<Shape>(yaml).expect_err("expected a type mismatch");
    assert!(
        err.to_string()
            .contains("tagged enum `Color` does not match target enum `Shape`")
    );
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum Simple {
    VALUE,
}

#[test]
fn unit_variant_serializes_plain_by_default() {
    let mut out = String::new();
    serde_saphyr::to_fmt_writer(&mut out, &Simple::VALUE)
        .expect("failed to serialize plain enum variant");
    assert_eq!(out, "VALUE\n");
}

#[test]
fn unit_variant_serializes_tagged_when_enabled() {
    let mut out = String::new();
    let opts = serde_saphyr::SerializerOptions {
        tagged_enums: true,
        ..serde_saphyr::SerializerOptions::default()
    };
    serde_saphyr::to_fmt_writer_with_options(&mut out, &Simple::VALUE, opts)
        .expect("failed to serialize tagged enum variant");
    assert_eq!(out, "!!Simple VALUE\n");
}

#[test]
fn struct_with_enum() -> anyhow::Result<()> {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct MyStruct {
        shape: Shape,
        color: Color,
        area: usize,
    }

    let mut opts = serde_saphyr::SerializerOptions {
        tagged_enums: true,
        ..serde_saphyr::SerializerOptions::default()
    };

    let mut yaml = String::new();
    let s = MyStruct {
        shape: Shape::SQUARE,
        color: Color::GREEN,
        area: 51,
    };
    serde_saphyr::to_fmt_writer_with_options(&mut yaml, &s, opts)?;
    assert_eq!("shape: !!Shape SQUARE\ncolor: !!Color GREEN\narea: 51\n", yaml);
    println!("yaml: {:#?}", yaml);
    let d: MyStruct = serde_saphyr::from_str(&yaml)?;
    assert_eq!(d, s);
    yaml.clear();

    opts.tagged_enums = false;
    serde_saphyr::to_fmt_writer_with_options(&mut yaml, &s, opts)?;
    assert_eq!("shape: SQUARE\ncolor: GREEN\narea: 51\n", yaml);
    let d: MyStruct = serde_saphyr::from_str(&yaml)?;
    assert_eq!(d, s);

    Ok(())
}
