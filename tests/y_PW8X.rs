use serde::Deserialize;
use std::collections::BTreeMap;

// PW8X: Anchors on Empty Scalars
// Define an exact structure to deserialize into without using serde_json.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Item {
    // A scalar which may be null or a string (e.g., null anchored as &a, or "a")
    Scalar(Option<String>),
    // A mapping with string keys and optional string values (some values are null)
    Map(BTreeMap<String, Option<String>>),
}

#[test]
#[ignore]
fn yaml_pw8x_anchors_on_empty_scalars() {
    // Unescaped YAML sequence directly embedded
    let y = r#"- &a
- a
-
  &a : a
  b: &b
-
  &c : &a
-
  ? &d
-
  ? &e
  : &a
"#;

    let v: Vec<Item> = serde_saphyr::from_str(y)
        .unwrap_or_else(|e| panic!("failed to parse PW8X: {e}"));

    assert_eq!(v.len(), 6, "expected 6 elements");

    // 1) first is null (anchored as &a)
    match &v[0] {
        Item::Scalar(None) => {}
        other => panic!("first element should be null scalar, got: {:?}", other),
    }

    // 2) second is string "a"
    match &v[1] {
        Item::Scalar(Some(s)) => assert_eq!(s, "a"),
        other => panic!("second element should be string 'a', got: {:?}", other),
    }

    // 3) third is map with empty key "" -> "a", and key "b" -> null
    match &v[2] {
        Item::Map(m) => {
            assert_eq!(m.get("").and_then(|o| o.as_ref().map(String::as_str)), Some("a"));
            assert!(m.get("b").is_some() && m.get("b").unwrap().is_none());
        }
        other => panic!("third element should be a map, got: {:?}", other),
    }

    // 4) fourth: map with empty key -> null (alias to first null)
    match &v[3] {
        Item::Map(m) => {
            assert!(m.get("").is_some() && m.get("").unwrap().is_none());
        }
        other => panic!("fourth element should be a map, got: {:?}", other),
    }

    // 5) fifth: map with empty key -> null
    match &v[4] {
        Item::Map(m) => {
            assert!(m.get("").is_some() && m.get("").unwrap().is_none());
        }
        other => panic!("fifth element should be a map, got: {:?}", other),
    }

    // 6) sixth: map with empty key -> null (alias to first null)
    match &v[5] {
        Item::Map(m) => {
            assert!(m.get("").is_some() && m.get("").unwrap().is_none());
        }
        other => panic!("sixth element should be a map, got: {:?}", other),
    }
}
