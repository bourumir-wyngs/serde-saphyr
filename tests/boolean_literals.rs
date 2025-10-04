use serde_saphyr;

#[test]
fn yaml11_truthy_boolean_literals() {
    let cases = ["true", "True", "TRUE", "yes", "Yes", "Y", "on", "ON"];
    for case in cases {
        let input = format!("{case}\n");
        let v: bool = serde_saphyr::from_str(&input).expect("expected boolean to parse");
        assert!(v, "literal `{case}` should parse as true");
    }
}

#[test]
fn yaml11_falsey_boolean_literals() {
    let cases = ["false", "False", "FALSE", "no", "No", "N", "off", "OFF"];
    for case in cases {
        let input = format!("{case}\n");
        let v: bool = serde_saphyr::from_str(&input).expect("expected boolean to parse");
        assert!(!v, "literal `{case}` should parse as false");
    }
}

#[test]
fn yaml11_invalid_boolean_literals_error() {
    let cases = ["truth", "affirmative", "1", "0", "yess"];
    for case in cases {
        let input = format!("{case}\n");
        let err = serde_saphyr::from_str::<bool>(&input).expect_err("expected parse error");
        let msg = format!("{err}");
        assert!(
            msg.contains("invalid boolean")
                || msg.contains("invalid bool")
                || msg.contains("invalid YAML 1.1 bool")
        );
    }
}