use serde_core::de::DeserializeOwned;
use std::path::Path;

/// A [`figment2::providers::Format`] implementation for YAML backed by `serde-saphyr`.
///
/// This enables Figment2 usage like:
///
/// ```rust
/// # #[cfg(feature = "figment2")]
/// # {
/// use figment2::{Figment, providers::Format};
/// use serde::Deserialize;
/// use serde_saphyr::figment2::Yaml;
///
/// #[derive(Deserialize)]
/// struct Config { answer: i32 }
///
/// let cfg: Config = Figment::from(Yaml::string("answer: 42")).extract().unwrap();
/// assert_eq!(cfg.answer, 42);
/// # }
/// ```
///
/// I/O failures include their cause in a standalone message because the provider
/// retains only the error's display text.
pub struct Yaml;

impl ::figment2::providers::Format for Yaml {
    type Error = crate::Error;

    const NAME: &'static str = "YAML";

    fn from_str<T: DeserializeOwned>(string: &str) -> Result<T, Self::Error> {
        // figment2 does not render out snippet anyway
        crate::from_str_with_options(
            string,
            crate::options! {
                with_snippet: false,
            },
        )
    }

    fn from_path<T: DeserializeOwned>(path: &Path) -> Result<T, Self::Error> {
        std::fs::File::open(path)
            .map_err(|cause| crate::Error::IOError { cause })
            .and_then(|file| {
                crate::from_reader_with_options(
                    file,
                    crate::options! {
                        with_snippet: false,
                    },
                )
            })
            .map_err(|error| match error {
                crate::Error::IOError { cause } => crate::Error::msg(format!("IO error: {cause}")),
                other => other,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::Yaml;
    use figment2::providers::Format;
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

    #[test]
    #[cfg_attr(miri, ignore)]
    fn from_path_retains_file_open_error_in_display_without_duplicate_sources() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("missing.yaml");
        let cause = std::fs::File::open(&path).unwrap_err();
        let expected = format!("IO error: {cause}");

        let error = <Yaml as Format>::from_path::<Config>(&path).unwrap_err();
        assert_eq!(error.to_string(), expected);
        assert_eq!(format!("{:#}", anyhow::Error::new(error)), expected);
    }

    #[test]
    #[cfg(unix)]
    #[cfg_attr(miri, ignore)]
    fn from_path_retains_file_read_error_in_display_without_duplicate_sources() {
        let directory = tempfile::tempdir().unwrap();
        let mut file = std::fs::File::open(directory.path()).unwrap();
        let cause = std::io::Read::read(&mut file, &mut [0]).unwrap_err();
        let expected = format!("IO error: {cause}");

        let error = <Yaml as Format>::from_path::<Config>(directory.path()).unwrap_err();
        assert_eq!(error.to_string(), expected);
        assert_eq!(format!("{:#}", anyhow::Error::new(error)), expected);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn provider_preserves_parser_error_message() {
        let yaml = "answer: 'unterminated\n";
        let expected = <Yaml as Format>::from_str::<Config>(yaml)
            .unwrap_err()
            .to_string();
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), yaml).unwrap();

        let error = <Yaml as Format>::from_path::<Config>(file.path()).unwrap_err();
        assert_eq!(error.to_string(), expected);
        let error = figment2::Provider::data(&Yaml::file(file.path()).search(false)).unwrap_err();
        assert_eq!(error.to_string(), expected);
    }
}
