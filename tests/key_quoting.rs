use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_saphyr::{from_str, to_string};

    /// Round-trips every printable ASCII single-character key (32..=126)
    /// through serde_saphyr using the same pattern as the provided snippet.
    /// This ensures that characters like comma `,` are serialized in a way
    /// that can be parsed back into the same HashMap.
    #[test]
    fn printable_ascii_single_char_keys_roundtrip() {
        for c in 32_u8..=126 {
            let s = String::from_utf8(vec![c]).expect("valid UTF-8 single byte");
            let mut h = HashMap::new();
            h.insert(s.clone(), s.clone());

            // NOTE: using the same pattern as the snippet:
            // serialize then deserialize back into a HashMap<String, String>.
            // If a key (e.g., ",") requires quoting, the serializer must emit it.
            let yaml = to_string(&serde_saphyr::FlowSeq(h.clone()))
                .expect("serialize FlowSeq<HashMap<..>>");
            let parsed: HashMap<String, String> = from_str(&yaml)
                .unwrap_or_else(|_| panic!("deserialize [{}] back into HashMap", yaml));

            assert_eq!(parsed, h, "Round-trip failed for key {:?}", s);
        }
    }

    /// Focused check for a comma key. This ensures that a map with `,` as a key
    /// survives serialization and deserialization exactly.
    #[test]
    fn specific_key_roundtrip() {
        let mut h = HashMap::new();
        h.insert(",".to_string(), ",".to_string());
        h.insert("".to_string(), " ".to_string()); // empty key
        h.insert("null".to_string(), " ".to_string()); // null key

        // Serialize with the same FlowSeq wrapper as in the original snippet.
        let yaml =
            to_string(&serde_saphyr::FlowSeq(h.clone())).expect("serialize FlowSeq<HashMap<..>>");

        // It must deserialize back to the identical map.
        let parsed: HashMap<String, String> =
            from_str(&yaml).unwrap_or_else(|_| panic!("deserialize [{}] back into HashMap", yaml));

        assert_eq!(parsed, h, "Comma key/value did not round-trip as expected");
    }

    /// Ensures that string keys that look like numbers ("1", "2.42") are quoted
    /// during serialization so they round-trip as strings, not numbers.
    #[test]
    fn numeric_string_keys_roundtrip() {
        let mut map = HashMap::new();
        map.insert("1".to_string(), "value1".to_string());
        map.insert("2".to_string(), "value2".to_string());
        map.insert("42".to_string(), "value42".to_string());
        map.insert("-5".to_string(), "negative".to_string());
        map.insert("3.14".to_string(), "pi".to_string());
        // Oversized numeric-looking keys that can exceed common integer/float parsing ranges.
        let huge_int = "9".repeat(200);
        let huge_float_exp = "1e99999999".to_string();
        map.insert(huge_int.clone(), "huge_int".to_string());
        map.insert(huge_float_exp.clone(), "huge_float_exp".to_string());

        let yaml = to_string(&map).expect("serialize HashMap with numeric string keys");

        // The keys should be quoted in the YAML output
        for value in [
            "1",
            "2",
            "42",
            "-5",
            "3.14",
            huge_int.as_str(),
            huge_float_exp.as_str(),
        ] {
            assert!(
                yaml.contains(&format!("\"{value}\"")),
                "Key '{}' should be quoted in YAML output, got:\n{}",
                value,
                yaml
            );
        }
        // Verify they round-trip correctly as strings
        let parsed: HashMap<String, String> =
            from_str(&yaml).unwrap_or_else(|_| panic!("deserialize [{}] back into HashMap", yaml));

        assert_eq!(parsed, map);
    }
}
