use serde_json::json;
use serde_json::Value;

#[test]
fn test_non_ascii_comment_middle() {
    let yaml_str = "
a1:
  b: 1
# A \u{AC00}
a2:
  b: 2
";
    let yaml_value: serde_json::Value =
        serde_saphyr::from_str(yaml_str).unwrap_or_else(|e| panic!("{}", e));

    let expected = json!({
        "a1": { "b": 1 },
        "a2": { "b": 2 }
    });

    assert_eq!(yaml_value, expected);
}

#[test]
fn test_non_ascii_comment_start() {
    // Non-ASCII character '가' as is, before the map
    let yaml_str = "
# A \u{AC00}
    a1:
        b: 1
    a2:
        b: 2
    ";

    let yaml_value: serde_json::Value =
        serde_saphyr::from_str(yaml_str).unwrap_or_else(|e| panic!("{}", e));

    let expected = json!({
        "a1": { "b": 1 },
        "a2": { "b": 2 }
    });

    assert_eq!(yaml_value, expected);
}

#[test]
fn test_non_ascii_data() {
    let yaml = "
# A \u{AC00}
\u{AC00}: \u{AC00}
\u{AC00}a: \u{AC00}a
a\u{AC00}a: a\u{AC00} # \u{AC00}a: \u{AC00}a
a1: # A \u{AC00}
  b: 1 # A \u{AC00}
a2: # A \u{AC00} # A \u{AC00}
  b: 2 # A \u{AC00}
  c: [ 1, 2, 3 ] # \u{AC00}
  d: # \u{AC00}
    - 1 \u{AC00}
    - 2 \u{AC00}
    - 3 \u{AC00}
# A \u{AC00}
";

    let obj: Value = serde_saphyr::from_str(yaml).unwrap();
    // Top-level scalar keys
    assert_eq!(obj.get("가").unwrap(), &Value::String("가".to_string()));
    assert_eq!(obj.get("가a").unwrap(), &Value::String("가a".to_string()));
    assert_eq!(obj.get("a가a").unwrap(), &Value::String("a가".to_string()));

    // a1: { b: 1 }
    let a1 = obj.get("a1").unwrap();
    assert!(a1.is_object());
    let a1o = a1.as_object().unwrap();
    assert_eq!(a1o.get("b").unwrap(), &Value::Number(1.into()));

    // a2: { b: 2, c: [1,2,3], d: [ "1 가", "2 가", "3 가" ] }
    let a2 = obj.get("a2").unwrap();
    assert!(a2.is_object());
    let a2o = a2.as_object().unwrap();

    assert_eq!(a2o.get("b").unwrap(), &Value::Number(2.into()));

    let c = a2o.get("c").unwrap();
    assert!(c.is_array());
    let ca = c.as_array().unwrap();
    assert_eq!(ca.len(), 3);
    assert_eq!(ca[0], Value::Number(1.into()));
    assert_eq!(ca[1], Value::Number(2.into()));
    assert_eq!(ca[2], Value::Number(3.into()));

    let d = a2o.get("d").unwrap();
    assert!(d.is_array());
    let da = d.as_array().unwrap();
    assert_eq!(da.len(), 3);
    assert_eq!(da[0], Value::String("1 가".to_string()));
    assert_eq!(da[1], Value::String("2 가".to_string()));
    assert_eq!(da[2], Value::String("3 가".to_string()));
}
