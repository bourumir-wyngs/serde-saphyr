use serde::Deserialize;
#[cfg(feature = "include")]
use serde_saphyr::{IncludeResolveError, InputSource};
use serde_saphyr::Options;

#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    foo: String,
}

#[cfg(feature = "include")]
#[test]
fn test_reader_resolver() {
    let yaml = "foo: !include bar.yaml\n";
    let cursor = std::io::Cursor::new(yaml.as_bytes());
    
    let resolver = Box::new(|s: &str| -> Result<InputSource, IncludeResolveError> {
        if s == "bar.yaml" {
            Ok(InputSource::Text("bar_value\n".to_string()))
        } else {
            Err(IncludeResolveError::Message("File not found".to_string()))
        }
    });

    let config: Config = serde_saphyr::from_reader_with_options_with_resolver(
        cursor,
        Options::default(),
        Some(resolver),
    ).unwrap();
    
    assert_eq!(config.foo, "bar_value");
}

#[cfg(feature = "include")]
#[test]
fn test_str_resolver() {
    let yaml = "foo: !include bar.yaml\n";
    
    let resolver = Box::new(|s: &str| -> Result<InputSource, IncludeResolveError> {
        if s == "bar.yaml" {
            Ok(InputSource::Text("bar_value\n".to_string()))
        } else {
            Err(IncludeResolveError::Message("File not found".to_string()))
        }
    });

    let config: Config = serde_saphyr::from_str_with_options_with_resolver(
        yaml,
        Options::default(),
        Some(resolver),
    ).unwrap();
    
    assert_eq!(config.foo, "bar_value");
}

#[cfg(feature = "include")]
#[test]
fn test_slice_resolver() {
    let yaml = b"foo: !include bar.yaml\n";
    
    let resolver = Box::new(|s: &str| -> Result<InputSource, IncludeResolveError> {
        if s == "bar.yaml" {
            Ok(InputSource::Text("bar_value\n".to_string()))
        } else {
            Err(IncludeResolveError::Message("File not found".to_string()))
        }
    });

    let config: Config = serde_saphyr::from_slice_with_options_with_resolver(
        yaml,
        Options::default(),
        Some(resolver),
    ).unwrap();
    
    assert_eq!(config.foo, "bar_value");
}
