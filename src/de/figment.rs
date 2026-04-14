use serde::de::DeserializeOwned;
use std::path::Path;

/// A [`figment::providers::Format`] implementation for YAML backed by `serde-saphyr`.
///
/// This enables Figment usage like:
///
/// ```rust
/// # #[cfg(feature = "figment")]
/// # {
/// use figment::{Figment, providers::Format};
/// use serde::Deserialize;
/// use serde_saphyr::figment::Yaml;
///
/// #[derive(Deserialize)]
/// struct Config { answer: i32 }
///
/// let cfg: Config = Figment::from(Yaml::string("answer: 42")).extract().unwrap();
/// assert_eq!(cfg.answer, 42);
/// # }
/// ```
pub struct Yaml;

impl ::figment::providers::Format for Yaml {
    type Error = crate::Error;

    const NAME: &'static str = "YAML";

    fn from_str<T: DeserializeOwned>(string: &str) -> Result<T, Self::Error> {
        // figment does not render out snippet anyway
        crate::from_str_with_options(
            string,
            crate::options! {
                with_snippet: false,
            },
        )
    }

    fn from_path<T: DeserializeOwned>(path: &Path) -> Result<T, Self::Error> {
        let file = std::fs::File::open(path).map_err(|cause| crate::Error::IOError { cause })?;
        // figment does not render out snippet anyway
        crate::from_reader_with_options(
            file,
            crate::options! {
                with_snippet: false,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Yaml;
    use figment::providers::Format;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Config {
        answer: i32,
    }

    #[test]
    fn from_str_deserializes_yaml() {
        let cfg: Config = <Yaml as Format>::from_str("answer: 42\n").unwrap();
        assert_eq!(cfg, Config { answer: 42 });
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn from_path_reads_yaml_file() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), "answer: 7\n").unwrap();

        let cfg: Config = <Yaml as Format>::from_path(file.path()).unwrap();
        assert_eq!(cfg, Config { answer: 7 });
    }
}
