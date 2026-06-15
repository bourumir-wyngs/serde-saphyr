#[cfg(all(test, feature = "serialize"))]
mod serialize_tests {
    use serde::Serialize;
    use serde_saphyr::Quoted;

    #[derive(Serialize)]
    struct QuotedDoc {
        text: Quoted<&'static str>,
        escaped: Quoted<&'static str>,
        owned: Quoted<String>,
    }

    #[test]
    fn quoted_forces_double_quoted_scalar_output() {
        let value = QuotedDoc {
            text: Quoted("plain"),
            escaped: Quoted("line\nbreak"),
            owned: Quoted("owned".to_string()),
        };

        let yaml = serde_saphyr::to_string(&value).unwrap();

        assert_eq!(
            yaml,
            "text: \"plain\"\nescaped: \"line\\nbreak\"\nowned: \"owned\"\n"
        );
    }
}
