use serde::Deserialize;
use serde_saphyr::{
    from_str_with_options, IncludeRequest, IncludeResolveError, InputSource, Options,
    ResolvedInclude,
};

#[derive(Debug, Deserialize, PartialEq)]
struct ServerConfig {
    host: String,
    port: u16,
    log_level: String,
    max_connections: u32,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Environments {
    production: ServerConfig,
    staging: ServerConfig,
    default_server: ServerConfig,
    supported_versions: Vec<u32>,
}

fn main() {
    // The main configuration file
    // It defines two environments that merge a common base configuration,
    // but override specific fields.
    let main_yaml = r#"
production:
  <<: !include base_config.yaml
  host: api.production.internal
  log_level: error

staging:
  <<: !include base_config.yaml
  host: api.staging.internal
  log_level: debug
  max_connections: 50

# including entire mapping
default_server: !include base_config.yaml

# including the list
supported_versions: !include versions.yaml
"#;

    // The included base configuration file (simulated by the resolver)
    let base_yaml = r#"
host: "localhost"
port: 8080
log_level: info
max_connections: 1000
"#;

    // Configure the resolver to serve `base_config.yaml`
    let resolver = move |req: IncludeRequest| -> Result<ResolvedInclude, IncludeResolveError> {
        if req.spec == "base_config.yaml" {
            Ok(ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: InputSource::Text(base_yaml.to_string()),
            })
        } else if req.spec == "versions.yaml" {
            Ok(ResolvedInclude {
                id: req.spec.to_string(),
                name: req.spec.to_string(),
                source: InputSource::Text("[1, 2, 3]".to_string()),
            })
        } else {
            Err(IncludeResolveError::Message(format!(
                "File not found: {}",
                req.spec
            )))
        }
    };

    let options = Options::default().with_include_resolver(resolver);

    let config: Environments =
        from_str_with_options(main_yaml, options).expect("Failed to parse YAML");

    println!("Parsed Configuration:\n{:#?}", config);

    assert_eq!(config.production.host, "api.production.internal");
    assert_eq!(config.production.port, 8080); // Inherited
    assert_eq!(config.production.log_level, "error"); // Overridden
    assert_eq!(config.production.max_connections, 1000); // Inherited

    assert_eq!(config.staging.host, "api.staging.internal");
    assert_eq!(config.staging.port, 8080); // Inherited
    assert_eq!(config.staging.log_level, "debug"); // Overridden
    assert_eq!(config.staging.max_connections, 50); // Overridden

    assert_eq!(config.default_server.host, "localhost");
    assert_eq!(config.default_server.port, 8080);
    assert_eq!(config.default_server.log_level, "info");
    assert_eq!(config.default_server.max_connections, 1000);

    assert_eq!(config.supported_versions, vec![1, 2, 3]);
}
