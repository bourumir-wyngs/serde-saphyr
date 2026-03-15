/// Verifies that a JSON string containing a UTF-16 surrogate pair is accepted
/// and decoded the same way as the equivalent YAML `\U` escape.
///
/// The JSON input `{"a":"\uD834\uDD1E"}` encodes the Unicode scalar value
/// U+1D11E (MUSICAL SYMBOL G CLEF) using a UTF-16 surrogate pair, as permitted
/// by JSON string escaping rules. For JSON compatibility, the YAML parser is
/// expected to accept this input and deserialize it to the single Unicode
/// character `𝄞`.
///
/// This test also checks semantic equivalence with the YAML-native form
/// `a: "\U0001D11E"`, ensuring that both inputs produce the same deserialized
/// value rather than preserving escape syntax details.
///
/// Related negative tests should reject malformed surrogate usage such as:
/// - an unpaired high surrogate
/// - an unpaired low surrogate
/// - a reversed surrogate pair
#[cfg(test)]
mod tests {
    use serde::Deserialize;

    // Adjust this import if serde-saphyr exposes from_str somewhere else.
    use serde_saphyr::from_str;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Doc {
        a: String,
    }

    #[test]
    fn json_surrogate_pair_parses_as_single_unicode_scalar() {
        let doc: Doc = from_str(r#"{"a":"\uD834\uDD1E"}"#).unwrap();

        assert_eq!(doc.a, "𝄞");
        assert_eq!(doc.a.chars().count(), 1);
        assert_eq!(doc.a.as_bytes(), &[0xF0, 0x9D, 0x84, 0x9E]);
    }

    #[test]
    fn json_surrogate_pair_matches_yaml_u_escape() {
        let from_json: Doc = from_str(r#"{"a":"\uD834\uDD1E"}"#).unwrap();
        let from_yaml: Doc = from_str("a: \"\\U0001D11E\"\n").unwrap();

        assert_eq!(from_json, from_yaml);
        assert_eq!(from_json.a, "𝄞");
    }

    #[test]
    fn rejects_unpaired_high_surrogate() {
        let err = from_str::<Doc>(r#"{"a":"\uD834"}"#).unwrap_err();
        let _ = err; // keep for debugging if needed
    }

    #[test]
    fn rejects_unpaired_low_surrogate() {
        let err = from_str::<Doc>(r#"{"a":"\uDD1E"}"#).unwrap_err();
        let _ = err;
    }

    #[test]
    fn rejects_reversed_surrogate_pair() {
        let err = from_str::<Doc>(r#"{"a":"\uDD1E\uD834"}"#).unwrap_err();
        let _ = err;
    }
}