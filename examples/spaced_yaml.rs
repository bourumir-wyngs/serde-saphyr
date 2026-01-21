//! Example demonstrating SpaceAfter wrapper for adding empty lines in YAML output.
//!
//! This wrapper allows you to visually separate sections in your YAML
//! configuration files by adding blank lines after values.
//!
//! Run with: cargo run --example spaced_yaml

use serde::{Deserialize, Serialize};
use serde_saphyr::{Commented, SpaceAfter, to_string};

/// A configuration file with visually separated sections.
#[derive(Debug, Serialize, Deserialize)]
struct Config {
    /// Application metadata
    name: String,
    version: SpaceAfter<String>, // Blank line after version

    /// Database section
    database: SpaceAfter<DatabaseConfig>, // Blank line after database section

    /// Server section
    server: SpaceAfter<ServerConfig>, // Blank line after server section

    /// Logging section (last, no spacing needed after)
    logging: LoggingConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct DatabaseConfig {
    host: String,
    port: u16,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerConfig {
    host: String,
    port: u16,
    workers: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoggingConfig {
    level: String,
    file: String,
}

/// Demonstrates combining SpaceAfter with other wrappers like Commented.
#[derive(Debug, Serialize)]
struct AnnotatedConfig {
    title: SpaceAfter<String>, // Blank line after title

    /// Important setting with a comment and blank line after
    critical_value: SpaceAfter<Commented<i32>>,

    /// Items list
    items: Vec<String>,
}

fn main() {
    // Example 1: Configuration with section separators
    let config = Config {
        name: "my-app".into(),
        version: SpaceAfter("1.0.0".into()),
        database: SpaceAfter(DatabaseConfig {
            host: "localhost".into(),
            port: 5432,
            name: "mydb".into(),
        }),
        server: SpaceAfter(ServerConfig {
            host: "0.0.0.0".into(),
            port: 8080,
            workers: 4,
        }),
        logging: LoggingConfig {
            level: "info".into(),
            file: "/var/log/app.log".into(),
        },
    };

    println!("=== Configuration with Section Separators ===\n");
    let yaml = to_string(&config).unwrap();
    println!("{yaml}");

    // Example 2: Combining with Commented wrapper
    let annotated = AnnotatedConfig {
        title: SpaceAfter("Settings".into()),
        critical_value: SpaceAfter(Commented(42, "do not change!".into())),
        items: vec!["item1".into(), "item2".into(), "item3".into()],
    };

    println!("=== Annotated Configuration ===\n");
    let yaml = to_string(&annotated).unwrap();
    println!("{yaml}");

    // Example 3: Demonstrate that wrappers are transparent during deserialization
    let yaml_input = r#"
name: test-app
version: 2.0.0
database:
  host: db.example.com
  port: 5432
  name: production
server:
  host: 0.0.0.0
  port: 443
  workers: 8
logging:
  level: debug
  file: /tmp/debug.log
"#;

    let parsed: Config = serde_saphyr::from_str(yaml_input).unwrap();
    let formatted = to_string(&parsed).unwrap();
    println!("{formatted}");
}
