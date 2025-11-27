use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
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

#[derive(Debug, Deserialize, PartialEq)]
enum Shape {
    CIRCLE,
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
