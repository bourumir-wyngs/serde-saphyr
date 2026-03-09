#![cfg(feature = "include")]

use serde::Deserialize;
use serde_saphyr::{from_str_with_options, IncludeRequest, IncludeResolveError, Options, ResolvedInclude};

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct Config {
    a: String,
    b: usize,
    c: String,
}

#[test]
fn test_include_error_snippet() {
    let main_yaml = r#"
      a: one
      b: !include included.yaml
      c: three
"#;
    let included_yaml = "\nstring\n";

    let options = Options::default().with_include_resolver(|req: IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
        if req.spec == "included.yaml" {
            Ok(ResolvedInclude {
                id: "included.yaml".to_string(),
                name: "included.yaml".to_string(),
                source: serde_saphyr::InputSource::from_string(included_yaml.to_string()),
            })
        } else {
            Err(IncludeResolveError::Message(format!("file not found: {}", req.spec)))
        }
    });

    let result: Result<Config, _> = from_str_with_options(main_yaml, options);
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    
    assert!(err_str.contains("included from here:"));
    assert!(err_str.contains("b: !include included.yaml"));
    assert!(err_str.contains("string"));
}
