use indoc::indoc;
use serde_json::Value;

#[test]
fn test_from_str_value_resolves_alias() {
    let yaml = "a: &id 1\nb: *id";
    let value: Value = serde_saphyr::from_str(yaml).unwrap();

    assert_eq!(value["b"], Value::from(1));
}

#[test]
fn test_from_str_value_applies_merge() {
    let yaml = indoc! {
        r#"
        defaults: &defaults
          a: 1
          b: 2

        actual:
          <<: *defaults
          c: 3
        "#
    };

    let value: Value = serde_saphyr::from_str(yaml).unwrap();

    assert_eq!(value["actual"]["a"], Value::from(1));
    assert_eq!(value["actual"]["b"], Value::from(2));
    assert_eq!(value["actual"]["c"], Value::from(3));
}
