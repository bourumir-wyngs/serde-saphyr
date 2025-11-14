use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

// Common serializer used by all scenarios
fn serialize_regex<S>(regex: &Regex, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(regex.as_str())
}

// -----------------------------------------------------------------------------
// 1) Borrowed &str requested -> should FAIL with our friendly error message
// -----------------------------------------------------------------------------
mod borrowed_str_fails {
    use super::*;

    #[derive(Serialize, Deserialize, Debug)]
    struct RegexConf {
        #[serde(
            serialize_with = "super::serialize_regex",
            deserialize_with = "deserialize_regex_borrowed"
        )]
        expr: Regex,
        get: usize,
    }

    fn deserialize_regex_borrowed<'de, D>(deserializer: D) -> Result<Regex, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::Deserialize as _;
        Regex::new(<&str>::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Root {
        global: GlobalConf,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct GlobalConf {
        regex: RegexConf,
    }

    #[test]
    fn test_str_deserializer_fails_for_borrowed() -> Result<()> {
        let yaml = r#"
global:
  # The default regex to find .exe download links.
  regex:
    expr: '((?:https?:)?//[^\s"''<>]+?\.exe(?:[^\s"''<>]*)?)'
    get: 0
"#;

        let res: anyhow::Result<Root> = serde_saphyr::from_str(yaml).map_err(Into::into);
        match res {
            Ok(_config) => anyhow::bail!(
                "expected error when deserializing &str, but got Ok"
            ),
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains("String or Cow<'de, str>"),
                    "hint missing: {}",
                    msg
                );
            }
        }
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// 2) Owned String requested -> should SUCCEED
// -----------------------------------------------------------------------------
mod string_works {
    use super::*;

    #[derive(Serialize, Deserialize, Debug)]
    struct RegexConf {
        #[serde(
            serialize_with = "super::serialize_regex",
            deserialize_with = "deserialize_regex_string"
        )]
        expr: Regex,
        get: usize,
    }

    fn deserialize_regex_string<'de, D>(deserializer: D) -> Result<Regex, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::Deserialize as _;
        let s = String::deserialize(deserializer)?;
        Regex::new(&s).map_err(serde::de::Error::custom)
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Root {
        global: GlobalConf,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct GlobalConf {
        regex: RegexConf,
    }

    #[test]
    fn test_str_deserializer_string_ok() -> Result<()> {
        let yaml = r#"
global:
  regex:
    expr: '((?:https?:)?//[^\s"''<>]+?\.exe(?:[^\s"''<>]*)?)'
    get: 0
"#;

        let cfg: Root = serde_saphyr::from_str(yaml)?;
        // Quick sanity: regex compiled and round-trips via serializer
        assert_eq!(cfg.global.regex.get, 0);
        assert!(cfg.global.regex.expr.is_match("https://example.com/a.exe"));
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// 3) Cow<'de, str> requested -> should SUCCEED (borrow when possible)
// -----------------------------------------------------------------------------
mod cow_works {
    use super::*;

    #[derive(Serialize, Deserialize, Debug)]
    struct RegexConf {
        #[serde(
            serialize_with = "super::serialize_regex",
            deserialize_with = "deserialize_regex_cow"
        )]
        expr: Regex,
        get: usize,
    }

    fn deserialize_regex_cow<'de, D>(deserializer: D) -> Result<Regex, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::Deserialize as _;
        let s: Cow<'de, str> = Cow::deserialize(deserializer)?;
        Regex::new(&s).map_err(serde::de::Error::custom)
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Root {
        global: GlobalConf,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct GlobalConf {
        regex: RegexConf,
    }

    #[test]
    fn test_str_deserializer_cow_ok() -> Result<()> {
        let yaml = r#"
global:
  regex:
    expr: '((?:https?:)?//[^\s"''<>]+?\.exe(?:[^\s"''<>]*)?)'
    get: 0
"#;

        let cfg: Root = serde_saphyr::from_str(yaml)?;
        assert_eq!(cfg.global.regex.get, 0);
        assert!(cfg.global.regex.expr.is_match("http://dl.site/x.exe"));
        Ok(())
    }
}