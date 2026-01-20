#[test]
fn main() {
    use serde::Deserialize;

    #[derive(Clone, Deserialize, Debug)]
    pub enum LogLevel {
        #[serde(alias = "error", alias = "ERROR")]
        Error,
        #[serde(alias = "warn", alias = "WARN")]
        Warn,
        #[serde(alias = "info", alias = "INFO")]
        Info,
        #[serde(alias = "debug", alias = "DEBUG")]
        Debug,
        #[serde(alias = "trace", alias = "TRACE")]
        Trace,
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Cfg {
        pub level: LogLevel,
    }

    let yaml = r#"
level: warnn
"#;

    let validation_result = serde_saphyr::from_str::<Cfg>(yaml);
    let error = validation_result.expect_err("expected enum error");
    let error_text = error.to_string();
    assert!(
        error_text.contains("^ unknown variant `warnn`"),
        "expected error snippet to include invalid value: {error_text}"
    );
}
