use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub endpoint: String,
    pub limit: usize,
    #[serde(rename = "site-language")]
    pub site_language: String,
    pub restrict: String,
    #[serde(rename = "selected-languages")]
    pub selected_languages: Vec<String>,
    #[serde(rename = "match-partial")]
    pub match_partial: bool,

    #[serde(rename = "1000")]
    pub the_thousand: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Root {
    pub settings: Settings,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_assert_settings() -> anyhow::Result<()> {
        let yaml = r#"
settings:
    endpoint: "http://localhost/api/search/instant"
    limit: 50
    site-language: "en"
    restrict: "all"
    selected-languages: [ "lzh", "en", "pgd", "kho", "pli", "pra", "san", "xct", "xto", "uig" ]
    match-partial: false
    1000: 1001
"#;

        let root: Root =
            serde_saphyr::from_str(yaml).with_context(|| "Failed to deserialize YAML into Root")?;
        let settings = root.settings;

        // Exact assertions
        assert_eq!(settings.endpoint, "http://localhost/api/search/instant");
        assert_eq!(settings.limit, 50);
        assert_eq!(settings.site_language, "en");
        assert_eq!(settings.restrict, "all");
        assert!(!settings.match_partial);
        assert_eq!(settings.the_thousand, 1001);

        // Languages list equality (order is preserved from YAML)
        let expected = vec![
            "lzh", "en", "pgd", "kho", "pli", "pra", "san", "xct", "xto", "uig",
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
        assert_eq!(settings.selected_languages, expected);

        Ok(())
    }
}
