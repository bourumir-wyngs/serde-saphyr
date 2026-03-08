use serde::Deserialize;
#[cfg(feature = "include")]
use serde_saphyr::{IncludeResolveError, InputSource, ResolvedInclude};
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
    
    let options = Options::default().with_include_resolver(|s: std::borrow::Cow<str>| -> Result<ResolvedInclude, IncludeResolveError> {
        if s == "bar.yaml" {
            Ok(ResolvedInclude {
                id: s.clone().into_owned(),
                name: s.into_owned(),
                source: InputSource::Text("bar_value\n".to_string())
            })
        } else {
            Err(IncludeResolveError::Message("File not found".to_string()))
        }
    });

    let config: Config = serde_saphyr::from_reader_with_options(
        cursor,
        options,
    ).unwrap();
    
    assert_eq!(config.foo, "bar_value");
}

#[cfg(feature = "include")]
#[test]
fn test_str_resolver() {
    let yaml = "foo: !include bar.yaml\n";
    
    let options = Options::default().with_include_resolver(|s: std::borrow::Cow<str>| -> Result<ResolvedInclude, IncludeResolveError> {
        if s == "bar.yaml" {
            Ok(ResolvedInclude {
                id: s.clone().into_owned(),
                name: s.into_owned(),
                source: InputSource::Text("bar_value\n".to_string())
            })
        } else {
            Err(IncludeResolveError::Message("File not found".to_string()))
        }
    });

    let config: Config = serde_saphyr::from_str_with_options(
        yaml,
        options,
    ).unwrap();
    
    assert_eq!(config.foo, "bar_value");
}

#[cfg(feature = "include")]
#[test]
fn test_slice_resolver() {
    let yaml = b"foo: !include bar.yaml\n";
    
    let options = Options::default().with_include_resolver(|s: std::borrow::Cow<str>| -> Result<ResolvedInclude, IncludeResolveError> {
        if s == "bar.yaml" {
            Ok(ResolvedInclude {
                id: s.clone().into_owned(),
                name: s.into_owned(),
                source: InputSource::Text("bar_value\n".to_string())
            })
        } else {
            Err(IncludeResolveError::Message("File not found".to_string()))
        }
    });

    let config: Config = serde_saphyr::from_slice_with_options(
        yaml,
        options,
    ).unwrap();
    
    assert_eq!(config.foo, "bar_value");
}

#[cfg(feature = "include")]
#[test]
fn test_nested_reader_budget() {
    let yaml = "foo: !include bar.yaml\n";
    let cursor = std::io::Cursor::new(yaml.as_bytes());

    let mut options = Options::default().with_include_resolver(|s: std::borrow::Cow<str>| -> Result<ResolvedInclude, IncludeResolveError> {
        if s == "bar.yaml" {
            Ok(ResolvedInclude {
                id: s.clone().into_owned(),
                name: s.into_owned(),
                // A reader that exceeds the budget: 15 bytes long
                source: InputSource::from_reader(std::io::Cursor::new(b"long_bar_value\n"))
            })
        } else {
            Err(IncludeResolveError::Message("File not found".to_string()))
        }
    });
    
    // Set a very small reader limit
    if let Some(ref mut b) = options.budget {
        b.max_reader_input_bytes = Some(5);
    } else {
        options.budget = Some(serde_saphyr::budget::Budget {
            max_reader_input_bytes: Some(5),
            ..Default::default()
        });
    }

    let config_res: Result<Config, serde_saphyr::Error> = serde_saphyr::from_reader_with_options(
        cursor,
        options,
    );

    // It should fail due to ExceededReaderInputLimit
    assert!(config_res.is_err());
    let err_msg = config_res.unwrap_err().to_string();
    assert!(err_msg.contains("size limit"), "Expected budget error, got: {}", err_msg);
}


#[test]
fn test_cyclic_include() {
    use serde_saphyr::{InputSource, ResolvedInclude};
    use std::borrow::Cow;
    let input = "
root: !include self.yaml
";
    let resolver = |s: Cow<'_, str>| -> Result<ResolvedInclude, _> {
        Ok(ResolvedInclude {
            id: s.to_string(),
            name: s.to_string(),
            source: InputSource::from_string("root2: !include self.yaml".to_string()),
        })
    };
    let options = serde_saphyr::Options::default().with_include_resolver(resolver);
    let result: Result<serde::de::IgnoredAny, _> = serde_saphyr::from_str_with_options(input, options);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("cyclic include detected"), "{}", err_msg);
}