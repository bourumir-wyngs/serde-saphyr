#[test]
fn string_from_plain_null_errors() {
    // Plain null keyword should not deserialize into String
    let yaml = "null";
    let err =
        serde_saphyr::from_str::<String>(yaml).expect_err("expected error for null -> String");
    let msg = format!("{}", err);
    assert!(
        msg.contains("cannot deserialize null into string"),
        "unexpected error message: {msg}"
    );
}

#[test]
fn string_from_tilde_errors() {
    let yaml = "~";
    let err = serde_saphyr::from_str::<String>(yaml).expect_err("expected error for ~ -> String");
    let msg = format!("{}", err);
    assert!(
        msg.contains("cannot deserialize null into string"),
        "unexpected error message: {msg}"
    );
}

#[test]
fn option_string_from_plain_null_is_none() {
    let yaml = "null";
    let v =
        serde_saphyr::from_str::<Option<String>>(yaml).expect("Option<String> should accept null");
    assert!(v.is_none());
}

#[test]
fn option_string_from_tilde_is_none() {
    let yaml = "~";
    let v = serde_saphyr::from_str::<Option<String>>(yaml).expect("Option<String> should accept ~");
    assert!(v.is_none());
}

#[test]
fn string_from_quoted_null_ok() {
    let yaml = "\"null\""; // quoted "null"
    let s = serde_saphyr::from_str::<String>(yaml).expect("quoted null should be a string");
    assert_eq!(s, "null");
}

#[test]
fn string_from_single_quoted_null_ok() {
    let yaml = "'null'"; // single-quoted 'null'
    let s = serde_saphyr::from_str::<String>(yaml).expect("single-quoted null should be a string");
    assert_eq!(s, "null");
}

#[test]
fn string_from_tagged_null_errors() {
    let yaml = "!!null";
    let err =
        serde_saphyr::from_str::<String>(yaml).expect_err("expected error for !!null -> String");
    let msg = format!("{}", err);
    assert!(
        msg.contains("cannot deserialize null into string"),
        "unexpected error message: {msg}"
    );
}

#[test]
fn rv_second() {
    #[derive(Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
    pub struct TestStruct {
        a: String,
        b: Option<String>,
        c: String,
    }

    let value = r#"---
a: abc
b: null
c: ghi
"#;
    let deserialized = serde_saphyr::from_str::<TestStruct>(value).map_err(|inp| inp.to_string());
    assert_eq!(
        deserialized,
        Ok(TestStruct {
            a: "abc".to_owned(),
            b: None,
            c: "ghi".to_owned()
        })
    );
}
