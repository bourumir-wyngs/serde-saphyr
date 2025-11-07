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
            let parsed: HashMap<String, String> =
                from_str(&yaml).expect(&format!("deserialize [{}] back into HashMap", yaml));

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
        let yaml = to_string(&serde_saphyr::FlowSeq(h.clone()))
            .expect("serialize FlowSeq<HashMap<..>>");

        // It must deserialize back to the identical map.
        let parsed: HashMap<String, String> =
            from_str(&yaml).expect(&format!("deserialize [{}] back into HashMap", yaml));

        assert_eq!(parsed, h, "Comma key/value did not round-trip as expected");
    }
}
