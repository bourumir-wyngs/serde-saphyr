#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::{self, Value as JsonValue};
    use std::collections::BTreeMap;

    use serde_saphyr as yaml;

    /// Outer test case envelope.
    #[derive(Debug, Deserialize)]
    struct Case {
        name: String,
        from: String,
        tags: String,
        yaml: String,
        tree: String,
        json: String,
        dump: String,
    }

    /// Minimal data model matching the snippet (seq/map/str).
    #[derive(Debug, Clone, PartialEq, Deserialize)]
    #[serde(untagged)]
    enum Node {
        Seq(Vec<Node>),
        Map(BTreeMap<String, Node>),
        Str(String),
    }

    impl Node {
        fn to_json(&self) -> JsonValue {
            match self {
                Node::Seq(v) => JsonValue::Array(v.iter().map(|n| n.to_json()).collect()),
                Node::Map(m) => {
                    let mut obj = serde_json::Map::new();
                    for (k, v) in m {
                        obj.insert(k.clone(), v.to_json());
                    }
                    JsonValue::Object(obj)
                }
                Node::Str(s) => JsonValue::String(s.clone()),
            }
        }
    }

    // Fixture (embedded).
    const OUTER_YAML: &str = r#"---
- name: Spec Example 7.23. Flow Content
  from: http://www.yaml.org/spec/1.2/spec.html#id2793163
  tags: spec flow sequence mapping
  yaml: |
    - [ a, b ]
    - { a: b }
    - "a"
    - 'b'
    - c
  tree: |
    +STR
     +DOC
      +SEQ
       +SEQ []
        =VAL :a
        =VAL :b
       -SEQ
       +MAP {}
        =VAL :a
        =VAL :b
       -MAP
       =VAL "a
       =VAL 'b
       =VAL :c
      -SEQ
     -DOC
    -STR
  json: |
    [
      [
        "a",
        "b"
      ],
      {
        "a": "b"
      },
      "a",
      "b",
      "c"
    ]
  dump: |
    - - a
      - b
    - a: b
    - "a"
    - 'b'
    - c
"#;

    // Exact expectations for "from" and "tree" (including the trailing newline from the block scalar).
    const FROM_EXPECTED: &str = "http://www.yaml.org/spec/1.2/spec.html#id2793163";
    const TREE_EXPECTED: &str = "\
+STR
 +DOC
  +SEQ
   +SEQ []
    =VAL :a
    =VAL :b
   -SEQ
   +MAP {}
    =VAL :a
    =VAL :b
   -MAP
   =VAL \"a
   =VAL 'b
   =VAL :c
  -SEQ
 -DOC
-STR
";

    #[test]
    fn spec_example_7_23_flow_content_without_value_type() {
        // 1) Parse the outer YAML into our envelope.
        let cases: Vec<Case> = yaml::from_str(OUTER_YAML).expect("outer YAML parses");
        assert_eq!(cases.len(), 1, "one test case expected");
        let case = &cases[0];

        // 2) Assert exact 'from' and 'tree' contents.
        assert_eq!(case.from, FROM_EXPECTED, "'from' must match exactly");
        assert_eq!(
            case.tree, TREE_EXPECTED,
            "'tree' block scalar must match exactly"
        );

        // 3) Basic metadata checks.
        assert_eq!(case.name, "Spec Example 7.23. Flow Content");
        assert!(
            case.tags.split_whitespace().any(|t| t == "flow"),
            "tags should include 'flow'"
        );

        // 4) Parse the YAML-under-test into our struct model.
        let parsed: Vec<Node> = yaml::from_str(&case.yaml).expect("embedded YAML parses");

        // 5) Structure sanity and fine-grained assertions.
        assert_eq!(parsed.len(), 5, "top-level should contain 5 elements");

        // [ a, b ]
        match &parsed[0] {
            Node::Seq(v) => {
                assert_eq!(v.len(), 2);
                assert_eq!(v[0], Node::Str("a".into()));
                assert_eq!(v[1], Node::Str("b".into()));
            }
            other => panic!("element 0 should be a sequence, got {:?}", other),
        }

        // { a: b }
        match &parsed[1] {
            Node::Map(m) => {
                assert_eq!(m.len(), 1);
                assert_eq!(m.get("a"), Some(&Node::Str("b".into())));
            }
            other => panic!("element 1 should be a mapping, got {:?}", other),
        }

        // "a"
        assert_eq!(parsed[2], Node::Str("a".into()));
        // 'b'
        assert_eq!(parsed[3], Node::Str("b".into()));
        // c
        assert_eq!(parsed[4], Node::Str("c".into()));

        // 6) Compare against provided JSON (semantic structure).
        let expected_json: JsonValue =
            serde_json::from_str(&case.json).expect("embedded JSON parses");
        let actual_json = Node::Seq(parsed.clone()).to_json();
        assert_eq!(actual_json, expected_json, "semantic structure must match");

        // 7) Verify 'dump' YAML represents the same structure.
        let dump_parsed: Vec<Node> = yaml::from_str(&case.dump).expect("'dump' YAML parses");
        let dump_json = Node::Seq(dump_parsed).to_json();
        assert_eq!(dump_json, expected_json, "'dump' must match semantics");
    }
}
