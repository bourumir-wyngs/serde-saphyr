//! Example demonstrating `Commented<T>` during deserialization and serialization.
//!
//! Run with: cargo run --example commented

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_saphyr::{CommentPosition, Commented, ser_options, to_string_with_options};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct DeploymentConfig {
    name: Commented<String>,
    image: Commented<String>,
    ports: Commented<Vec<Commented<u16>>>,
    labels: Commented<BTreeMap<String, Commented<String>>>,
}

fn main() -> anyhow::Result<()> {
    let yaml = r#"
# deployment manifest
name: checkout
image: registry.example.com/checkout:v1 # container image to deploy
ports: # sequence of exposed ports
  - 80 # public HTTP
  - 443 # public HTTPS
labels: # mapping of Kubernetes labels
  app: checkout # stable app label
  tier: frontend # routing tier
"#;

    let config: Commented<DeploymentConfig> = serde_saphyr::from_str(yaml)?;
    let deployment = &config.0;

    assert_eq!(config.1, "deployment manifest");
    assert_eq!(deployment.name, Commented("checkout".into(), String::new()));
    assert_eq!(
        deployment.image,
        Commented(
            "registry.example.com/checkout:v1".into(),
            "container image to deploy".into()
        )
    );
    assert_eq!(deployment.ports.1, "sequence of exposed ports");
    assert_eq!(deployment.ports.0[0], Commented(80, "public HTTP".into()));
    assert_eq!(deployment.ports.0[1], Commented(443, "public HTTPS".into()));
    assert_eq!(deployment.labels.1, "mapping of Kubernetes labels");
    assert_eq!(
        deployment.labels.0["app"],
        Commented("checkout".into(), "stable app label".into())
    );

    println!("=== Read into Rust ===");
    println!("{config:#?}");

    println!("\n=== Written with Comments Above Values ===");
    let written = to_string_with_options(
        &config,
        ser_options! { comment_position: CommentPosition::Above },
    )?;
    println!("{written}");

    Ok(())
}
