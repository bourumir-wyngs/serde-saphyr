#![cfg(all(feature = "serialize", feature = "deserialize"))]
#[test]
fn test_enum_deserialization_yaml_to_json_value() {
    let inputs: Vec<(&str, &str, &str)> = vec![
        ("AnonymousValues::Unit", "Unit", r#""Unit""#),
        (
            "AnonymousValues::Newtype",
            "Newtype: 42",
            r#"{"Newtype":42}"#,
        ),
        (
            "AnonymousValues::Tuple",
            "Tuple:\n  - 1\n  - hello",
            r#"{"Tuple":[1,"hello"]}"#,
        ),
        (
            "NamedValues::Struct",
            "Struct:\n  x: 10\n  y: world",
            r#"{"Struct":{"x":10,"y":"world"}}"#,
        ),
        ("NamedValues::Empty", "Empty: {}", r#"{"Empty":{}}"#),
        // Tagged with !
        ("!Unit", "!Unit", r#"null"#),
        ("!Newtype", "!Newtype 42", r#"42"#),
        (
            "!Tuple sequence",
            "!Tuple\n  - 1\n  - hello",
            r#"[1,"hello"]"#,
        ),
        (
            "!Struct mapping",
            "!Struct\n  x: 10\n  y: world",
            r#"{"x":10,"y":"world"}"#,
        ),
        ("!Empty mapping", "!Empty {}", r#"{}"#),
    ];

    for (_label, yaml, expected_json) in inputs {
        let result: serde_json::Value = serde_saphyr::from_str(yaml).unwrap();
        let expected: serde_json::Value = serde_json::from_str(expected_json).unwrap();
        assert_eq!(result, expected, "failed for yaml: {:?}", yaml);
    }
}
