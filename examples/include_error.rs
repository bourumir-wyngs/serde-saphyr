use serde::Deserialize;
use serde_saphyr::{
    IncludeRequest, IncludeResolveError, Options, ResolvedInclude, from_str_with_options,
};

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct Config {
    a: String,
    b: usize, // Will not take String
    c: String,
}

fn main() {
    let main_yaml = r#"
      a: one
      b: !include included.yaml
      c: three
"#;

    let included_yaml = r#"
string
"#;

    let options = Options::default().with_include_resolver(
        |req: IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
            if req.spec == "included.yaml" {
                Ok(ResolvedInclude {
                    id: "included.yaml".to_string(),
                    name: "included.yaml".to_string(),
                    source: serde_saphyr::InputSource::from_string(included_yaml.to_string()),
                })
            } else {
                Err(IncludeResolveError::Message(format!(
                    "file not found: {}",
                    req.spec
                )))
            }
        },
    );

    let result: Result<Config, _> = from_str_with_options(main_yaml, options);

    match result {
        Ok(c) => println!("Config: {:?}", c),
        Err(e) => {
            // expected error
            println!("Error expected:\n{}", e);
        }
    }
}
