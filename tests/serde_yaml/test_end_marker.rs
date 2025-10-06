use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Person {
    name: String,
    age: u8,
    note: Option<String>,
}

#[test]
fn test_yaml_end_marker_ignores_following_content() {
    let yaml_input = "\
---
name: John Smith
age: 30
...
!!! this is invalid YAML that should be ignored
";

    let expected = Person {
        name: "John Smith".into(),
        age: 30,
        note: None,
    };

    let result: Result<Person, _> = serde_saphyr::from_str(yaml_input);

    assert!(
        result.is_ok(),
        "Parser incorrectly considered content after end-of-document marker."
    );

    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_legitimate_triple_dots() {
    let yaml_input = "\
---
# Three dots ... can be legitimate
name: John ... Smith ...
age: 30
note: ...
...
!!! this is invalid YAML that should be ignored
";

    let expected = Person {
        name: "John ... Smith ...".into(),
        age: 30,
        note: Some("...".into()),
    };

    let result: Result<Person, _> = serde_saphyr::from_str(yaml_input);

    assert!(
        result.is_ok(),
        "Parser incorrectly considered content after end-of-document marker."
    );

    assert_eq!(result.unwrap(), expected);
}
