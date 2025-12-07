use serde::Deserialize;
use std::collections::BTreeMap;

// PW8X: Anchors on Empty Scalars
// Define an exact structure to deserialize into without using serde_json.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Item {
    // A scalar which may be null or a string (e.g., null anchored as &a, or "a")
    Scalar(Option<String>),
    // A mapping whose keys and values may be null (explicit-key with empty/null scalars)
    Map(BTreeMap<Option<String>, Option<String>>),
}

#[derive(Debug, Deserialize)]
struct CaseEnvelope {
    // Only the field we care about; extra fields in the wrapper are ignored by default.
    yaml: String,
}

#[test]
fn yaml_pw8x_anchors_on_empty_scalars() {
    // This is the yaml-test-suite wrapper document. We must first deserialize the
    // outer envelope to extract the inner YAML under the `yaml:` literal block.
    let yaml = r#"---
- name: Anchors on Empty Scalars
  from: NimYAML tests
  tags: anchor explicit-key
  yaml: |
    - &a
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
  dump: |
    - &a
    - a
    - &a : a
      b: &b
    - &c : &a
    - &d :
    - &e : &a
"#;

    // First, parse the outer test-suite wrapper.
    let cases: Vec<CaseEnvelope> = serde_saphyr::from_str(&yaml)
        .unwrap_or_else(|e| panic!("failed to parse PW8X wrapper: {e}"));
    assert_eq!(cases.len(), 1, "expected exactly one case in the wrapper");

    // Now parse the inner YAML content into the intended structure.
    let v: Vec<Item> = serde_saphyr::from_str(&cases[0].yaml)
        .unwrap_or_else(|e| panic!("failed to parse PW8X inner YAML: {e}"));

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

    // 3) third is map with null key -> "a", and key "b" -> null
    match &v[2] {
        Item::Map(m) => {
            assert_eq!(m.get(&None).cloned().flatten().as_deref(), Some("a"));
            assert!(m.get(&Some("b".to_string())).is_some() && m.get(&Some("b".to_string())).unwrap().is_none());
        }
        other => panic!("third element should be a map, got: {:?}", other),
    }

    // 4) fourth: map with null key -> null (alias to first null)
    match &v[3] {
        Item::Map(m) => {
            assert!(m.get(&None).is_some() && m.get(&None).unwrap().is_none());
        }
        other => panic!("fourth element should be a map, got: {:?}", other),
    }

    // 5) fifth: map with null key -> null
    match &v[4] {
        Item::Map(m) => {
            assert!(m.get(&None).is_some() && m.get(&None).unwrap().is_none());
        }
        other => panic!("fifth element should be a map, got: {:?}", other),
    }

    // 6) sixth: map with null key -> null (alias to first null)
    match &v[5] {
        Item::Map(m) => {
            assert!(m.get(&None).is_some() && m.get(&None).unwrap().is_none());
        }
        other => panic!("sixth element should be a map, got: {:?}", other),
    }
}
