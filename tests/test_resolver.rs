#[cfg(feature = "include")]
use serde::Deserialize;
#[cfg(feature = "include")]
use serde_saphyr::{IncludeResolveError, InputSource, ResolvedInclude};
#[cfg(feature = "include")]
use serde_saphyr::Options;

#[cfg(feature = "include")]
#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    foo: String,
}

#[cfg(feature = "include")]
#[test]
fn test_reader_resolver() {
    let yaml = "foo: !include bar.yaml\n";
    let cursor = std::io::Cursor::new(yaml.as_bytes());
    
    let options = Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
        let s = req.spec;
        if s == "bar.yaml" {
            Ok(ResolvedInclude {
                id: s.to_string(),
                name: s.to_string(),
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
    
    let options = Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
        let s = req.spec;
        if s == "bar.yaml" {
            Ok(ResolvedInclude {
                id: s.to_string(),
                name: s.to_string(),
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
    
    let options = Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
        let s = req.spec;
        if s == "bar.yaml" {
            Ok(ResolvedInclude {
                id: s.to_string(),
                name: s.to_string(),
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

    let mut options = Options::default().with_include_resolver(|req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
        let s = req.spec;
        if s == "bar.yaml" {
            Ok(ResolvedInclude {
                id: s.to_string(),
                name: s.to_string(),
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


#[cfg(feature = "include")]
#[test]
fn test_cyclic_include() {
    use serde_saphyr::{InputSource, ResolvedInclude};
    let input = "
root: !include self.yaml
";
    let resolver = |req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, _> {
        let s = req.spec;
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

#[cfg(feature = "include")]
#[test]
fn test_repeated_includes_not_cyclic() {
    use serde_saphyr::{InputSource, ResolvedInclude};
    let input = "
list:
  - !include item.yaml
  - !include item.yaml
";
    let resolver = |req: serde_saphyr::IncludeRequest| -> Result<ResolvedInclude, _> {
        let s = req.spec;
        Ok(ResolvedInclude {
            id: s.to_string(),
            name: s.to_string(),
            source: InputSource::from_string("value".to_string()),
        })
    };
    let options = serde_saphyr::Options::default().with_include_resolver(resolver);
    // Should not fail with cyclic include error
    let result: Result<serde::de::IgnoredAny, _> = serde_saphyr::from_str_with_options(input, options);
    assert!(result.is_ok(), "Expected Ok, got {:?}", result.unwrap_err());
}

#[cfg(feature = "include")]
#[test]
fn test_unsupported_include_form() {
    let input = "
foo: !include { \"path\": \"file_b.yml\", \"extension\": \"txt\" }
";
    // We shouldn't even reach the resolver, so a dummy one is fine.
    let resolver = |_req: serde_saphyr::IncludeRequest| -> Result<serde_saphyr::ResolvedInclude, serde_saphyr::IncludeResolveError> {
        Err(serde_saphyr::IncludeResolveError::Message("Not reached".to_string()))
    };
    let options = serde_saphyr::Options::default().with_include_resolver(resolver);
    let result: Result<Config, _> = serde_saphyr::from_str_with_options(input, options);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("currently only supports the scalar form"),
        "Expected unsupported include form error, got: {}",
        err_msg
    );
}