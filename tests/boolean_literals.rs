#![cfg(all(feature = "serialize", feature = "deserialize"))]
use rstest::rstest;

#[rstest]
#[case("true")]
#[case("True")]
#[case("TRUE")]
#[case("yes")]
#[case("Yes")]
#[case("Y")]
#[case("on")]
#[case("ON")]
fn yaml11_truthy_boolean_literals(#[case] literal: &str) {
    let input = format!("{literal}\n");
    let v: bool = serde_saphyr::from_str(&input).expect("expected boolean to parse");
    assert!(v, "literal `{literal}` should parse as true");
}

#[rstest]
#[case("false")]
#[case("False")]
#[case("FALSE")]
#[case("no")]
#[case("No")]
#[case("N")]
#[case("off")]
#[case("OFF")]
fn yaml11_falsey_boolean_literals(#[case] literal: &str) {
    let input = format!("{literal}\n");
    let v: bool = serde_saphyr::from_str(&input).expect("expected boolean to parse");
    assert!(!v, "literal `{literal}` should parse as false");
}

#[rstest]
#[case("truth")]
#[case("affirmative")]
#[case("1")]
#[case("0")]
#[case("yess")]
fn yaml11_invalid_boolean_literals_error(#[case] literal: &str) {
    let input = format!("{literal}\n");
    let err = serde_saphyr::from_str::<bool>(&input).expect_err("expected parse error");
    let msg = format!("{err}");
    assert!(
        msg.contains("invalid boolean")
            || msg.contains("invalid bool")
            || msg.contains("invalid YAML 1.1 bool")
    );
}
