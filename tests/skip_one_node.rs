#[cfg(test)]
mod tests {
    use serde_json::Value;
    use serde_saphyr::Error;
    use serde_saphyr::options::DuplicateKeyPolicy;
    use serde_saphyr::{Options, from_str_with_options};
    use std::collections::BTreeMap;

    /// Parse a YAML mapping into a BTreeMap<String, serde_json::Value>,
    /// configuring the deserializer to use DuplicateKeyPolicy::FirstWins.
    ///
    /// Params:
    /// - `yaml`: the YAML input as &str.
    ///
    /// Returns:
    /// - `Result<BTreeMap<String, Value>, Error>` with parsed map or a deserialization error.
    fn parse_first_wins_map(yaml: &str) -> Result<BTreeMap<String, Value>, Error> {
        let opts = Options {
            duplicate_keys: DuplicateKeyPolicy::FirstWins,
            ..Options::default()
        };
        from_str_with_options::<BTreeMap<String, Value>>(yaml, opts)
    }

    #[test]
    fn first_wins_skips_scalar_value_of_duplicate_key() {
        let yaml = r#"
a: 1
a: 2
b: 3
"#;
        let map = parse_first_wins_map(yaml).expect("parse ok");
        assert_eq!(map.get("a"), Some(&Value::from(1)));
        assert_eq!(map.get("b"), Some(&Value::from(3)));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn first_wins_skips_sequence_value_of_duplicate_key() {
        let yaml = r#"
a: [1, 2]
a: [9, 9, 9]   # should be skipped entirely
b: 3
"#;
        let map = parse_first_wins_map(yaml).expect("parse ok");
        assert_eq!(map.get("a"), Some(&Value::from(vec![1, 2])));
        assert_eq!(map.get("b"), Some(&Value::from(3)));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn first_wins_skips_mapping_value_of_duplicate_key_including_nested() {
        let yaml = r#"
a:
  x: 1
  y:
    z: 2
a:
  X: 9         # whole mapping must be skipped
  Y: { Z: 10 } # including nested mapping
b: 42
"#;
        let map = parse_first_wins_map(yaml).expect("parse ok");
        // The original 'a' must remain unchanged
        let a = map.get("a").expect("key a present");
        assert_eq!(
            a,
            &serde_json::json!({
                "x": 1,
                "y": { "z": 2 }
            })
        );
        // And 'b' must still be read correctly after skipping
        assert_eq!(map.get("b"), Some(&Value::from(42)));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn first_wins_handles_duplicate_in_middle_and_keeps_following_keys() {
        let yaml = r#"
k1: start
dup:
  inner: [1, 2, { q: 9 }]
dup:
  should: be-ignored
  and: "this too"
k2: end
"#;
        let map = parse_first_wins_map(yaml).expect("parse ok");
        assert_eq!(map.get("k1"), Some(&Value::from("start")));
        assert_eq!(
            map.get("dup"),
            Some(&serde_json::json!({
                "inner": [1, 2, { "q": 9 }]
            }))
        );
        // Ensure parser stayed in sync and k2 was parsed after skipping the second 'dup' value.
        assert_eq!(map.get("k2"), Some(&Value::from("end")));
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn first_wins_skips_nullish_scalar_duplicate_as_one_node_only() {
        // Even when the duplicate value is a null-ish scalar, it should skip exactly that node.
        let yaml = r#"
a: { keep: true }
a: null
b: ok
"#;
        let map = parse_first_wins_map(yaml).expect("parse ok");
        assert_eq!(map.get("a"), Some(&serde_json::json!({ "keep": true })));
        assert_eq!(map.get("b"), Some(&Value::from("ok")));
        assert_eq!(map.len(), 2);
    }
}
