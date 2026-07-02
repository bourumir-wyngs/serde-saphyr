#[test]
fn test_quoting() {
    let samples = vec![
        "2413",
        "2_4_13",
        "1000_1000_1000",
        "0x11",
        "0o11",
        "0b11",
        "1.1",
        ".1",
        "1e3",
        ".inf",
        ".nan",
    ];
    for v in samples {
        let s = serde_saphyr::to_string(&v).unwrap();
        let vv = serde_saphyr::from_str::<serde_json::Value>(&s).unwrap();
        assert!(vv.is_string(), "{} is {}", s, vv);
    }
}

#[test]
fn document_marker_strings_are_quoted_at_top_level() {
    for value in ["---", "...", "--- value", "... value"] {
        let serialized = serde_saphyr::to_string(&value).unwrap();

        assert_eq!(serialized, format!("\"{value}\"\n"));

        let decoded: String = serde_saphyr::from_str(&serialized).unwrap();
        assert_eq!(decoded, value);
    }
}

#[test]
fn document_marker_strings_are_quoted_in_multiple_documents() {
    let values = ["first", "---", "...", "--- value", "... value"];
    let serialized = serde_saphyr::to_string_multiple(&values).unwrap();

    assert_eq!(
        serialized,
        "first\n---\n\"---\"\n---\n\"...\"\n---\n\"--- value\"\n---\n\"... value\"\n"
    );

    let decoded: Vec<String> = serde_saphyr::from_multiple(&serialized).unwrap();
    let expected = values.iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(decoded, expected);
}
