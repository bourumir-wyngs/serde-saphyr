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

    fn from_str<'de, T: DeserializeOwned>(string: &'de str) -> Result<T, Self::Error> {
        crate::from_str(string)
    }

    fn from_path<T: DeserializeOwned>(path: &Path) -> Result<T, Self::Error> {
        let bytes = std::fs::read(path).map_err(|cause| crate::Error::IOError { cause })?;
        crate::from_slice(&bytes)
    }
}
