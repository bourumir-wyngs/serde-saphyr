use serde::de::DeserializeOwned;
pub use de::{
    Budget, Options, Error, Location, DuplicateKeyPolicy
};

pub use ser::{ to_string, to_writer, to_writer_with_indent };
pub use ser::{ RcAnchor, ArcAnchor, RcWeakAnchor, ArcWeakAnchor };

use crate::live_events::LiveEvents;
use crate::parse_scalars::scalar_is_nullish;
use crate::de::{Ev, Events};

mod base64;
pub mod budget;
pub mod options;
mod parse_scalars;
mod de;
mod error;
mod live_events;
mod tags;
mod ser;

/// Deserialize any `T: serde::de::DeserializeOwned` directly from a YAML string.
///
/// This is the simplest entry point; it parses a single YAML document. If the
/// input contains multiple documents, this returns an error advising to use
/// [`from_multiple`] or [`from_multiple_with_options`].
///
/// Example: read a small `Config` structure from a YAML string.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
///     name: My Application
///     enabled: true
///     retries: 5
/// "#;
///
/// let cfg: Config = serde_saphyr::from_str(yaml).unwrap();
/// assert!(cfg.enabled);
/// ```
pub fn from_str<T: DeserializeOwned>(input: &str) -> Result<T, Error> {
    from_str_with_options(input, Options::default())
}

/// Deserialize a single YAML document with configurable [`Options`].
///
/// Example: read a small `Config` with a custom budget and default duplicate-key policy.
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::DuplicateKeyPolicy;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
///      name: My Application
///      enabled: true
///      retries: 5
/// "#;
///
/// let options = serde_saphyr::Options {
///      budget: Some(serde_saphyr::Budget {
///            max_anchors: 200,
///            .. serde_saphyr::Budget::default()
///      }),
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
///     .. serde_saphyr::Options::default()
/// };
/// let cfg: Config = serde_saphyr::from_str_with_options(yaml, options).unwrap();
/// assert_eq!(cfg.retries, 5);
/// ```
pub fn from_str_with_options<T: DeserializeOwned>(
    input: &str,
    options: Options,
) -> Result<T, Error> {
    let cfg = crate::de::Cfg {
        dup_policy: options.duplicate_keys,
        legacy_octal_numbers: options.legacy_octal_numbers,
        strict_booleans: options.strict_booleans,
    };
    let mut src = LiveEvents::new(input, options.budget, options.alias_limits);
    let value = T::deserialize(crate::de::Deser::new(&mut src, cfg))?;
    if let Some(ev) = src.peek()? {
        return Err(Error::msg(
            "multiple YAML documents detected; use from_multiple or from_multiple_with_options",
        )
            .with_location(ev.location()));
    }
    src.finish()?;
    Ok(value)
}

/// Deserialize multiple YAML documents from a single string into a vector of `T`.
/// Completely empty documents are ignored and not included into returned vector.
///
/// Example: read two `Config` documents separated by `---`.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: First
/// enabled: true
/// retries: 1
/// ---
/// name: Second
/// enabled: false
/// retries: 2
/// "#;
///
/// let cfgs: Vec<Config> = serde_saphyr::from_multiple(yaml).unwrap();
/// assert_eq!(cfgs.len(), 2);
/// assert_eq!(cfgs[0].name, "First");
/// ```
pub fn from_multiple<T: DeserializeOwned>(input: &str) -> Result<Vec<T>, Error> {
    from_multiple_with_options(input, Options::default())
}

/// Deserialize multiple YAML documents into a vector with configurable [`Options`].
///
/// Example: two `Config` documents with a custom budget.
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::DuplicateKeyPolicy;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: First
/// enabled: true
/// retries: 1
/// ---
/// name: Second
/// enabled: false
/// retries: 2
/// "#;
///
/// let options = serde_saphyr::Options {
///      budget: Some(serde_saphyr::Budget {
///            max_anchors: 200,
///            .. serde_saphyr::Budget::default()
///      }),
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
///     .. serde_saphyr::Options::default()
/// };
/// let cfgs: Vec<Config> = serde_saphyr::from_multiple_with_options(yaml, options).unwrap();
/// assert_eq!(cfgs.len(), 2);
/// assert!(!cfgs[1].enabled);
/// ```
pub fn from_multiple_with_options<T: DeserializeOwned>(
    input: &str,
    options: Options,
) -> Result<Vec<T>, Error> {
    let cfg = crate::de::Cfg {
        dup_policy: options.duplicate_keys,
        legacy_octal_numbers: options.legacy_octal_numbers,
        strict_booleans: options.strict_booleans,
    };
    let mut src = LiveEvents::new(input, options.budget, options.alias_limits);
    let mut values = Vec::new();

    loop {
        match src.peek()? {
            // Skip documents that are explicit null-like scalars ("", "~", or "null").
            Some(Ev::Scalar {
                     value: s,
                     style,
                     ..
                 }) if scalar_is_nullish(s, style) => {
                let _ = src.next()?; // consume the null scalar document
                // Do not push anything for this document; move to the next one.
                continue;
            }
            Some(_) => {
                let value = T::deserialize(crate::de::Deser::new(&mut src, cfg))?;
                values.push(value);
            }
            None => break,
        }
    }

    src.finish()?;
    Ok(values)
}

/// Deserialize a single YAML document from a UTF-8 byte slice.
///
/// This is equivalent to [`from_str`], but accepts `&[u8]` and validates it is
/// valid UTF-8 before parsing.
///
/// Example: read a small `Config` structure from bytes.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: My Application
/// enabled: true
/// retries: 5
/// "#;
/// let bytes = yaml.as_bytes();
/// let cfg: Config = serde_saphyr::from_slice(bytes).unwrap();
/// assert!(cfg.enabled);
/// ```
///
pub fn from_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    from_slice_with_options(bytes, Options::default())
}

/// Deserialize a single YAML document from a UTF-8 byte slice with configurable [`Options`].
///
/// Example: read a small `Config` with a custom budget from bytes.
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::DuplicateKeyPolicy;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
///      name: My Application
///      enabled: true
///      retries: 5
/// "#;
/// let bytes = yaml.as_bytes();
/// let options = serde_saphyr::Options {
///      budget: Some(serde_saphyr::Budget {
///            max_anchors: 200,
///            .. serde_saphyr::Budget::default()
///      }),
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
///     .. serde_saphyr::Options::default()
/// };
/// let cfg: Config = serde_saphyr::from_slice_with_options(bytes, options).unwrap();
/// assert_eq!(cfg.retries, 5);
/// ```
pub fn from_slice_with_options<T: DeserializeOwned>(
    bytes: &[u8],
    options: Options,
) -> Result<T, Error> {
    let s = std::str::from_utf8(bytes).map_err(|_| Error::msg("input is not valid UTF-8"))?;
    from_str_with_options(s, options)
}

/// Deserialize multiple YAML documents from a UTF-8 byte slice into a vector of `T`.
///
/// Example: read two `Config` documents separated by `---` from bytes.
///
/// ```rust
/// use serde::Deserialize;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: First
/// enabled: true
/// retries: 1
/// ---
/// name: Second
/// enabled: false
/// retries: 2
/// "#;
/// let bytes = yaml.as_bytes();
/// let cfgs: Vec<Config> = serde_saphyr::from_slice_multiple(bytes).unwrap();
/// assert_eq!(cfgs.len(), 2);
/// assert_eq!(cfgs[0].name, "First");
/// ```
pub fn from_slice_multiple<T: DeserializeOwned>(bytes: &[u8]) -> Result<Vec<T>, Error> {
    from_slice_multiple_with_options(bytes, Options::default())
}

/// Deserialize multiple YAML documents from bytes with configurable [`Options`].
/// Completely empty documents are ignored and not included into returned vector.
///
/// Example: two `Config` documents with a custom budget from bytes.
///
/// ```rust
/// use serde::Deserialize;
/// use serde_saphyr::DuplicateKeyPolicy;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Config {
///     name: String,
///     enabled: bool,
///     retries: i32,
/// }
///
/// let yaml = r#"
/// name: First
/// enabled: true
/// retries: 1
/// ---
/// name: Second
/// enabled: false
/// retries: 2
/// "#;
/// let bytes = yaml.as_bytes();
/// let options = serde_saphyr::Options {
///      budget: Some(serde_saphyr::Budget {
///            max_anchors: 200,
///            .. serde_saphyr::Budget::default()
///      }),
///     duplicate_keys: DuplicateKeyPolicy::FirstWins,
///     .. serde_saphyr::Options::default()
/// };
/// let cfgs: Vec<Config> = serde_saphyr::from_slice_multiple_with_options(bytes, options).unwrap();
/// assert_eq!(cfgs.len(), 2);
/// assert!(!cfgs[1].enabled);
/// ```
pub fn from_slice_multiple_with_options<T: DeserializeOwned>(
    bytes: &[u8],
    options: Options,
) -> Result<Vec<T>, Error> {
    let s = std::str::from_utf8(bytes).map_err(|_| Error::msg("input is not valid UTF-8"))?;
    from_multiple_with_options(s, options)
}
